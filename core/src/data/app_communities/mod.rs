//! Community records: the `Community` schema, its baseline validation,
//! and the reads and writes of `app_communities`. Callers own and commit
//! the transaction. Writes are crate-private: a community syncs, so each
//! write is an event applied by `app::communities`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data::app_sync_changes;
use crate::database::Transaction;
use crate::deployment::Role;
use crate::helpers::field_errors::FieldErrors;
use crate::sync::VersionRange;

pub const TABLE: &str = "app_communities";
/// The longest name, in characters.
pub const MAX_NAME_LENGTH: usize = 80;

/// A community as the pages show it, with the sync columns every synced
/// row carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Community {
    pub id: Uuid,
    pub name: String,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i64,
    #[serde(with = "crate::sync::version::optional")]
    pub global_sync_version: Option<i64>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_id: Option<Uuid>,
}

/// A community that passed validation, which `create` and `update` require.
pub struct AcceptedCommunity(Community);

impl AcceptedCommunity {
    /// Checks the community, with its name as it is kept.
    pub fn new(community: Community) -> Result<AcceptedCommunity, FieldErrors> {
        let name = checked_name(&community.name)?;
        Ok(AcceptedCommunity(Community { name, ..community }))
    }

    /// The accepted community.
    pub fn row(&self) -> &Community {
        &self.0
    }

    /// The accepted community, owned.
    pub fn into_row(self) -> Community {
        self.0
    }
}

/// The name as it is kept: trimmed, not blank and not too long.
pub fn checked_name(name: &str) -> Result<String, FieldErrors> {
    let name = name.trim();
    if name.is_empty() {
        return Err(FieldErrors::one("name", "is blank"));
    }
    if name.chars().count() > MAX_NAME_LENGTH {
        return Err(FieldErrors::one(
            "name",
            format!("has {MAX_NAME_LENGTH} characters at most"),
        ));
    }
    Ok(name.to_string())
}

#[cfg(feature = "postgres")]
mod postgres;
mod sqlite;

macro_rules! query {
    ($tx:expr, $function:ident($($argument:expr),* $(,)?)) => {
        match $tx {
            Transaction::Sqlite(tx) => sqlite::$function(&mut **tx, $($argument),*).await,
            #[cfg(feature = "postgres")]
            Transaction::Postgres(tx) => postgres::$function(&mut **tx, $($argument),*).await,
        }
    };
}

/// Lists live communities visible to this user, ordered by name and ID.
pub async fn list(tx: &mut Transaction, user_id: Uuid) -> Result<Vec<Community>, sqlx::Error> {
    query!(tx, list_live(user_id))
}

/// Gets a live community visible to this user, including trial isolation.
pub async fn get(
    tx: &mut Transaction,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<Community>, sqlx::Error> {
    let row = query!(&mut *tx, get(id))?.filter(|row| row.deleted_at.is_none());
    if row.is_some() && crate::data::trial_users::community_is_hidden_from(tx, id, user_id).await? {
        return Ok(None);
    }
    Ok(row)
}

/// Inserts a new row. On the server this publishes it: the row gets the
/// next global sync version, recorded in `app_sync_changes` in this
/// transaction. On a device the row keeps the version it has.
pub(crate) async fn create(
    tx: &mut Transaction,
    role: Role,
    community: AcceptedCommunity,
) -> Result<Community, sqlx::Error> {
    let mut community = community.into_row();
    community.global_sync_version =
        app_sync_changes::stamp(tx, role, TABLE, community.id, community.global_sync_version)
            .await?;
    query!(tx, insert(&community))?;
    Ok(community)
}

/// Writes the row over its stored one, a soft delete included, and
/// publishes it on the server as `create` does.
pub(crate) async fn update(
    tx: &mut Transaction,
    role: Role,
    community: AcceptedCommunity,
) -> Result<Community, sqlx::Error> {
    let mut community = community.into_row();
    community.global_sync_version =
        app_sync_changes::stamp(tx, role, TABLE, community.id, community.global_sync_version)
            .await?;
    query!(tx, update(&community))?;
    Ok(community)
}

/// Explicit replication access for internal resource workflows.
pub mod replication {
    use super::*;

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn list_by_ids(
        tx: &mut Transaction,
        ids: &[Uuid],
    ) -> Result<Vec<Community>, sqlx::Error> {
        query!(tx, list_by_ids(ids))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<Community>, sqlx::Error> {
        query!(tx, get(id))
    }

    /// Reads protocol state, including tombstones, in the caller transaction.
    pub async fn list_synced_after(
        tx: &mut Transaction,
        user_id: Uuid,
        range: &VersionRange,
        limit: usize,
    ) -> Result<Vec<Community>, sqlx::Error> {
        query!(tx, list_synced_after(user_id, range, limit))
    }

    /// Persists a row accepted by the resource workflow.
    pub(crate) async fn upsert(
        tx: &mut Transaction,
        community: &Community,
    ) -> Result<(), sqlx::Error> {
        query!(tx, upsert(community))
    }
}

/// Explicit maintenance access for internal resource workflows.
pub(crate) mod maintenance {
    use super::*;

    /// Physically removes a refused local row and cascading children.
    pub(crate) async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete_row(id))
    }
}
