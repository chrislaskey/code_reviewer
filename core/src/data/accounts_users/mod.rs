//! Transaction-scoped account records. Workflows own commits.

use crate::accounts::users::{AcceptedAccountUser, AccountUser};
use crate::database::Transaction;
use chrono::{DateTime, Utc};
use uuid::Uuid;

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

/// The limited account changes accepted by the lifecycle workflow.
pub(crate) enum Update<'a> {
    Confirm(DateTime<Utc>),
    SignUp { email: &'a str, at: DateTime<Utc> },
}

/// Gets a live account by ID.
pub(crate) async fn get(
    tx: &mut Transaction,
    id: Uuid,
) -> Result<Option<AccountUser>, sqlx::Error> {
    Ok(query!(tx, get(id))?.filter(|account| account.deleted_at.is_none()))
}

/// Applies a lifecycle change accepted by the account workflow.
pub(crate) async fn update(
    tx: &mut Transaction,
    id: Uuid,
    change: crate::accounts::users::AcceptedUpdate<'_>,
) -> Result<(), sqlx::Error> {
    match change.value() {
        Update::Confirm(at) => query!(tx, set_confirmed_at(id, *at)),
        Update::SignUp { email, at } => query!(tx, set_signed_up(id, email, *at)),
    }
}

/// Soft-deletes an account at the workflow's timestamp.
pub(crate) async fn delete(
    tx: &mut Transaction,
    id: Uuid,
    at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    query!(tx, set_deleted_at(id, at))
}

/// Gets a live account by its workflow-normalized email.
pub(crate) async fn get_by_email(
    tx: &mut Transaction,
    email: &str,
) -> Result<Option<AccountUser>, sqlx::Error> {
    query!(tx, get_by_email(email))
}

/// Persists an account after email validation.
pub(crate) async fn create(
    tx: &mut Transaction,
    account: AcceptedAccountUser<'_>,
) -> Result<(), sqlx::Error> {
    query!(tx, insert(account.row()))
}

/// Explicit maintenance access for internal workflows.
pub(crate) mod maintenance {
    use super::*;

    /// Physically removes accounts and their cascading credentials and tokens.
    pub(crate) async fn delete_unconfirmed_before(
        tx: &mut Transaction,
        time: DateTime<Utc>,
    ) -> Result<u64, sqlx::Error> {
        query!(tx, delete_unconfirmed_before(time))
    }

    /// Physically removes accounts and their cascading credentials and tokens.
    pub(crate) async fn delete_without_user(tx: &mut Transaction) -> Result<u64, sqlx::Error> {
        query!(tx, delete_without_user())
    }

    /// Physically removes accounts and their cascading credentials and tokens.
    pub(crate) async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
        query!(tx, delete_row(id))
    }
}

/// Explicit lifecycle access for internal workflows.
pub(crate) mod lifecycle {
    use super::*;

    /// Gets an account including its deletion state for trusted lifecycle and billing workflows.
    pub(crate) async fn get(
        tx: &mut Transaction,
        id: Uuid,
    ) -> Result<Option<AccountUser>, sqlx::Error> {
        query!(tx, get(id))
    }
}

/// Explicit billing access for internal workflows.
#[cfg(feature = "stripe")]
pub(crate) mod billing {
    use super::*;

    /// Reads or atomically updates the account customer mapping.
    pub(crate) async fn get_stripe_customer_id(
        tx: &mut Transaction,
        id: Uuid,
    ) -> Result<Option<String>, sqlx::Error> {
        query!(tx, get_stripe_customer_id(id))
    }

    /// Reads or atomically updates the account customer mapping.
    pub(crate) async fn set_stripe_customer_id_if_missing(
        tx: &mut Transaction,
        id: Uuid,
        customer_id: &str,
    ) -> Result<(), sqlx::Error> {
        query!(tx, set_stripe_customer_id_if_missing(id, customer_id))
    }

    /// Reads or atomically updates the account customer mapping.
    pub(crate) async fn get_id_by_stripe_customer_id(
        tx: &mut Transaction,
        customer_id: &str,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        query!(tx, get_id_by_stripe_customer_id(customer_id))
    }
}
