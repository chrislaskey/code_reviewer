//! The device's place in the server's rows: the `Cursor` it pulled to,
//! one row of `app_sync_state` per user, and the reads and writes of it.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::Transaction;

/// A place in the pull order: the server's write time and the id of a
/// row, the last one pulled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cursor {
    pub last_synced_at: DateTime<Utc>,
    pub id: Uuid,
}

impl Cursor {
    /// Before every row.
    pub fn start() -> Cursor {
        Cursor {
            last_synced_at: DateTime::<Utc>::UNIX_EPOCH,
            id: Uuid::nil(),
        }
    }
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

/// Gets the stored pull cursor, or none before the first pull.
pub async fn get(tx: &mut Transaction) -> Result<Option<Cursor>, sqlx::Error> {
    query!(tx, cursor())
}

/// Upserts the pull cursor in the same transaction as its applied page.
pub async fn upsert(tx: &mut Transaction, cursor: &Cursor) -> Result<(), sqlx::Error> {
    query!(tx, save_cursor(cursor))
}

/// Explicit maintenance operations; bulk SQL and cascades do not run per-row hooks.
pub mod maintenance {
    use super::*;

    /// Resets device sync rows while preserving the existing pending/conflicted-row rules.
    pub async fn reset(tx: &mut Transaction, user_id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, reset(user_id))
    }
}
