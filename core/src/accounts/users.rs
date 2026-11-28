//! A person's account, as the pages ask for it, and as the server's
//! command line makes one. The account's records, its email rules and
//! its reads and writes are `data::accounts_users`.

use uuid::Uuid;

use crate::accounts::{AccountError, SIGN_UP_COMMUNITY_NAME};
use crate::data::accounts_credentials;
use crate::data::accounts_users::{self, AccountUser, AccountUserError};
use crate::deployment::{Deployment, Role};

/// One account by its id, unless deleted.
pub async fn get(deployment: &Deployment, id: Uuid) -> Result<AccountUser, AccountUserError> {
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        let request = serde_json::json!({ "user_id": id });
        return match backend.call("accounts/users/get", &request).await {
            Ok(account) => Ok(account),
            Err(error) if error.code() == Some("not_found") => {
                Err(AccountUserError::NotFound { id })
            }
            Err(error) => Err(error.into()),
        };
    }
    let mut tx = deployment.begin().await?;
    let account = accounts_users::get(&mut tx, id).await?;
    tx.commit().await?;
    account.ok_or(AccountUserError::NotFound { id })
}

/// Makes an account with its password, on the server, as a finished
/// sign-up would: confirmed, its user named from the email, and its
/// first community. For a server that takes no sign-ups, from its
/// command line; nothing is mailed.
pub async fn create(
    deployment: &Deployment,
    email: &str,
    password: &str,
) -> Result<AccountUser, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let account = accounts_users::create(&mut tx, Uuid::now_v7(), email, true).await?;
    accounts_credentials::set(&mut tx, account.id, password).await?;
    let name = crate::app::users::name_from_email_in(&mut tx, &account.email, account.id).await?;
    let user = crate::app::users::create_in(&mut tx, account.id, &name).await?;
    crate::app::communities::create_in(&mut tx, Role::Server, &user, SIGN_UP_COMMUNITY_NAME)
        .await?;
    tx.commit().await?;
    Ok(account)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::sign_in_with_password;
    use crate::accounts::tests::PASSWORD;
    use crate::app::communities;
    use crate::data::accounts_credentials::CredentialError;
    use crate::database::with_each_database;

    #[tokio::test]
    async fn accounts_users_create_makes_an_account_that_signs_in_with_its_password() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let account = create(&server, "Dana@Example.com", PASSWORD)
                .await
                .expect("created");
            assert_eq!(account.email, "dana@example.com");
            assert!(account.confirmed_at.is_some());

            let session = sign_in_with_password(&server, "dana@example.com", PASSWORD)
                .await
                .expect("signed in");
            assert_eq!(session.user.id, account.id);
            assert_eq!(session.user.name, "dana");
            let listed = communities::list(&server, &session.user)
                .await
                .expect("listed");
            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].community.name, SIGN_UP_COMMUNITY_NAME);
            assert!(listed[0].membership.is_some(), "a member of it");
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_users_create_refuses_a_taken_email_and_a_short_password() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            create(&server, "dana@example.com", PASSWORD)
                .await
                .expect("created");
            assert!(matches!(
                create(&server, "DANA@example.com", PASSWORD).await,
                Err(AccountError::AccountUser(AccountUserError::EmailTaken))
            ));
            assert!(matches!(
                create(&server, "eli@example.com", "short").await,
                Err(AccountError::Credential(CredentialError::PasswordTooShort))
            ));
            let mut tx = server.begin().await.expect("a transaction");
            assert!(
                accounts_users::get_by_email(&mut tx, "eli@example.com")
                    .await
                    .expect("looked")
                    .is_none(),
                "nothing kept of a refused one"
            );
            tx.commit().await.expect("committed");
        })
        .await;
    }
}
