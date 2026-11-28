//! The server's feed: one row of `app_sync_versions`, with the feed's id,
//! the last global sync version given out and the lowest cursor a pull
//! still answers. Callers own and commit the transaction.

use uuid::Uuid;

use crate::database::Transaction;
use crate::deployment::Role;

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

/// The feed: its id, its head and its floor.
pub(crate) struct Feed {
    pub id: Uuid,
    /// The last global sync version given out and committed.
    pub head: i64,
    /// The lowest cursor a pull still answers; below it, a resync.
    pub floor: i64,
}

/// Gets the feed.
pub(crate) async fn get(tx: &mut Transaction) -> Result<Feed, sqlx::Error> {
    query!(tx, get())
}

/// Gives out the next global sync version. The feed's row stays locked
/// until the transaction ends, so versions commit in the order given.
pub(crate) async fn next(tx: &mut Transaction) -> Result<i64, sqlx::Error> {
    query!(tx, next())
}

/// Locks the feed before a transaction decides a change to replicated
/// rows. On PostgreSQL this takes the row lock `next` would; SQLite has
/// one writer already.
pub(crate) async fn lock(tx: &mut Transaction) -> Result<(), sqlx::Error> {
    query!(tx, lock())
}

/// Locks the feed on the server; nothing on a device.
pub(crate) async fn lock_for_role(tx: &mut Transaction, role: Role) -> Result<(), sqlx::Error> {
    if role == Role::Server {
        lock(tx).await?;
    }
    Ok(())
}

/// Gives the feed a new id: every cursor of the old one resyncs.
pub(crate) async fn update_feed_id(tx: &mut Transaction, feed_id: Uuid) -> Result<(), sqlx::Error> {
    query!(tx, update_feed_id(feed_id))
}

/// Sets the lowest cursor a pull still answers.
pub(crate) async fn update_floor(tx: &mut Transaction, floor: i64) -> Result<(), sqlx::Error> {
    query!(tx, update_floor(floor))
}
