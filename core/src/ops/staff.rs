//! Staff: the accounts that may use the ops app, one row of `ops_staff`
//! each. Server only. Someone becomes staff from the server's command
//! line, by their email, never from a page.

use crate::data::ops_staff::{self, Staff};
use thiserror::Error;

use crate::data::accounts_users::{self, AccountUserError};
use crate::data::app_users::User;
use crate::deployment::{Deployment, Role, now};

#[derive(Debug, Error)]
pub enum StaffError {
    #[error("only the server has staff")]
    ServerOnly,
    #[error("Only staff can use this")]
    NotStaff,
    #[error("no account has the email {email}")]
    NoAccount { email: String },
    #[error(transparent)]
    AccountUser(#[from] AccountUserError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The user's staff row; `NotStaff` when they have none.
pub async fn get(deployment: &Deployment, user: &User) -> Result<Staff, StaffError> {
    if deployment.role != Role::Server {
        return Err(StaffError::ServerOnly);
    }
    // A remote backend keeps no staff yet: ops is not in the sample
    if deployment.has_remote_backend() {
        return Err(StaffError::NotStaff);
    }
    let mut tx = deployment.begin().await?;
    let staff = ops_staff::get(&mut tx, user.id).await?;
    tx.commit().await?;
    staff.ok_or(StaffError::NotStaff)
}

/// Makes the account with the email staff; already staff stays so.
pub async fn create(deployment: &Deployment, email: &str) -> Result<Staff, StaffError> {
    if deployment.role != Role::Server {
        return Err(StaffError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let account = account_with_email(&mut tx, email).await?;
    ops_staff::create(
        &mut tx,
        &Staff {
            user_id: account.id,
            inserted_at: now(),
        },
    )
    .await?;
    let staff = ops_staff::get(&mut tx, account.id).await?;
    tx.commit().await?;
    staff.ok_or(StaffError::NotStaff)
}

/// The account with the email is staff no longer; `NotStaff` when it
/// was not.
pub async fn delete(deployment: &Deployment, email: &str) -> Result<(), StaffError> {
    if deployment.role != Role::Server {
        return Err(StaffError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let account = account_with_email(&mut tx, email).await?;
    let deleted = ops_staff::delete(&mut tx, account.id).await?;
    tx.commit().await?;
    if deleted == 0 {
        return Err(StaffError::NotStaff);
    }
    Ok(())
}

async fn account_with_email(
    tx: &mut crate::database::Transaction,
    email: &str,
) -> Result<accounts_users::AccountUser, StaffError> {
    accounts_users::get_by_email(tx, email)
        .await?
        .ok_or_else(|| StaffError::NoAccount {
            email: email.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::users;
    use crate::database::with_each_database;

    #[tokio::test]
    async fn staff_create_then_get_then_delete() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let error = get(&deployment, &alice).await.expect_err("not staff yet");
            assert!(matches!(error, StaffError::NotStaff));

            let staff = create(&deployment, " Alice@Example.com ")
                .await
                .expect("staff");
            assert_eq!(staff.user_id, alice.id);
            let again = create(&deployment, "alice@example.com")
                .await
                .expect("still staff");
            assert_eq!(again, staff);
            assert_eq!(get(&deployment, &alice).await.expect("staff"), staff);

            delete(&deployment, "alice@example.com")
                .await
                .expect("deleted");
            let error = get(&deployment, &alice).await.expect_err("staff no longer");
            assert!(matches!(error, StaffError::NotStaff));
            let error = delete(&deployment, "alice@example.com")
                .await
                .expect_err("was not staff");
            assert!(matches!(error, StaffError::NotStaff));
        })
        .await;
    }

    #[tokio::test]
    async fn staff_create_needs_an_account_with_the_email() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let error = create(&deployment, "nobody@example.com")
                .await
                .expect_err("no account");
            assert!(matches!(error, StaffError::NoAccount { .. }));
        })
        .await;
    }
}
