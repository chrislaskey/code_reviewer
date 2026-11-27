//! The singleton identifying the server's feed and its committed range.

use uuid::Uuid;

use crate::database::Transaction;

pub(crate) struct Feed {
    pub id: Uuid,
    pub head: i64,
    pub floor: i64,
}

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

pub(crate) async fn get(tx: &mut Transaction) -> Result<Feed, sqlx::Error> {
    let (id, head, floor): (String, i64, i64) = query!(
        tx,
        sqlx::query_as(
            "SELECT CAST(feed_id AS TEXT), global_sync_version, minimum_global_sync_version
             FROM app_sync_versions WHERE id = 1"
        ),
        fetch_one
    )?;
    let id = Uuid::parse_str(&id).map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
    Ok(Feed { id, head, floor })
}

/// Advances the counter under the caller's transaction lock.
pub(crate) async fn next(tx: &mut Transaction) -> Result<i64, sqlx::Error> {
    query!(
        tx,
        sqlx::query_scalar(
            "UPDATE app_sync_versions SET global_sync_version = global_sync_version + 1
             WHERE id = 1 AND global_sync_version < 9223372036854775807
             RETURNING global_sync_version"
        ),
        fetch_one
    )
}

/// Locks the feed before a transaction changes replicated rows.
pub(crate) async fn lock(tx: &mut Transaction) -> Result<(), sqlx::Error> {
    #[cfg(feature = "postgres")]
    if let Transaction::Postgres(tx) = tx {
        sqlx::query("SELECT id FROM app_sync_versions WHERE id = 1 FOR UPDATE")
            .fetch_one(&mut **tx)
            .await?;
    }
    #[cfg(not(feature = "postgres"))]
    let _ = tx;
    Ok(())
}

/// Acquires the server's write lock before evaluating a replicated operation.
pub(crate) async fn lock_for_role(
    tx: &mut Transaction,
    role: crate::deployment::Role,
) -> Result<(), sqlx::Error> {
    if role == crate::deployment::Role::Server {
        lock(tx).await?;
    }
    Ok(())
}

pub(crate) async fn set_floor(tx: &mut Transaction, floor: i64) -> Result<(), sqlx::Error> {
    query!(
        tx,
        sqlx::query("UPDATE app_sync_versions SET minimum_global_sync_version = $1 WHERE id = 1")
            .bind(floor),
        execute
    )?;
    Ok(())
}
