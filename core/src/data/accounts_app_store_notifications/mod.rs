//! Transaction-scoped app store notifications records. Workflows own policy and commits.

use crate::accounts::app_store_notifications::{
    AcceptedAppStoreNotification, AppStoreNotification,
};
use crate::database::Transaction;

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

/// Gets one stored record by ID.
pub(crate) async fn get(
    tx: &mut Transaction,
    id: &str,
) -> Result<Option<AppStoreNotification>, sqlx::Error> {
    query!(tx, get(id))
}

/// Records a store payment or delivery only when its unique key is new.
pub(crate) async fn create(
    tx: &mut Transaction,
    receipt: AcceptedAppStoreNotification<'_>,
) -> Result<u64, sqlx::Error> {
    query!(
        tx,
        insert_if_missing(
            &receipt.row().id,
            &receipt.row().notification_type,
            receipt.row().handled_at
        )
    )
}
