//! Committed sync changes identify rows. Resource contexts own their visibility.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::data::app_sync_versions;
use crate::database::Transaction;
use crate::deployment::{Role, now};
use crate::sync::VersionRange;

macro_rules! query {
    ($tx:expr, $query:expr, execute) => {
        match $tx {
            Transaction::Sqlite(tx) => $query.execute(&mut **tx).await.map(|_| ()),
            #[cfg(feature = "postgres")]
            Transaction::Postgres(tx) => $query.execute(&mut **tx).await.map(|_| ()),
        }
    };
    ($tx:expr, $query:expr, $method:ident) => {
        match $tx {
            Transaction::Sqlite(tx) => $query.$method(&mut **tx).await,
            #[cfg(feature = "postgres")]
            Transaction::Postgres(tx) => $query.$method(&mut **tx).await,
        }
    };
}

/// Records a server change and returns the version to store on its row.
pub(crate) async fn record(
    tx: &mut Transaction,
    role: Role,
    table: &str,
    row_id: Uuid,
) -> Result<Option<i64>, sqlx::Error> {
    if role == Role::Device {
        return Ok(None);
    }
    let version = app_sync_versions::next(tx).await?;
    let written_at = now();
    let statement =
        "INSERT INTO app_sync_changes (global_sync_version, table_name, row_id, written_at)
                     VALUES ($1, $2, $3, $4)";
    match tx {
        Transaction::Sqlite(tx) => sqlx::query(statement)
            .bind(version)
            .bind(table)
            .bind(row_id.hyphenated())
            .bind(written_at)
            .execute(&mut **tx)
            .await
            .map(|_| ())?,
        #[cfg(feature = "postgres")]
        Transaction::Postgres(tx) => sqlx::query(statement)
            .bind(version)
            .bind(table)
            .bind(row_id)
            .bind(written_at)
            .execute(&mut **tx)
            .await
            .map(|_| ())?,
    }
    Ok(Some(version))
}

pub(crate) async fn list_versions(
    tx: &mut Transaction,
    range: &VersionRange,
    limit: usize,
) -> Result<Vec<i64>, sqlx::Error> {
    query!(
        tx,
        sqlx::query_scalar(
            "SELECT global_sync_version FROM app_sync_changes
             WHERE global_sync_version > $1 AND global_sync_version <= $2
             ORDER BY global_sync_version LIMIT $3"
        )
        .bind(range.after)
        .bind(range.through)
        .bind(i64::try_from(limit).unwrap_or(i64::MAX)),
        fetch_all
    )
}

/// Prunes a committed prefix and advances the minimum supported cursor.
pub(crate) async fn purge(
    tx: &mut Transaction,
    horizon: DateTime<Utc>,
) -> Result<i64, sqlx::Error> {
    let feed = app_sync_versions::get(tx).await?;
    let first_retained: Option<i64> = query!(
        tx,
        sqlx::query_scalar(
            "SELECT MIN(global_sync_version) FROM app_sync_changes WHERE written_at >= $1"
        )
        .bind(horizon),
        fetch_one
    )?;
    let floor = first_retained.map_or(feed.head, |version| version - 1);
    query!(
        tx,
        sqlx::query("DELETE FROM app_sync_changes WHERE global_sync_version <= $1").bind(floor),
        execute
    )?;
    app_sync_versions::set_floor(tx, floor).await?;
    Ok(floor)
}
