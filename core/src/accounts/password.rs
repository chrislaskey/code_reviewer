//! The signed-in user's password, a way to sign in beside the emailed
//! code: whether they have one, and their first or a new one, see
//! `docs/design/accounts-sign-in.md`. A new one needs no current one, so
//! a forgotten password is changed by signing in with a code; the
//! account's email is told each time.

use serde::{Deserialize, Serialize};

use crate::accounts::AccountError;
use crate::data::app_users::User;
use crate::data::{accounts_credentials, accounts_users};
use crate::deployment::{Deployment, Role};
use crate::services::mailer::{Mailer, mails};

/// Whether the signed-in user has a password.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordStatus {
    pub is_set: bool,
}

/// A password for the signed-in user.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordUpdate {
    pub password: String,
}

/// Whether the signed-in user has a password.
pub async fn get(deployment: &Deployment, user: &User) -> Result<PasswordStatus, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let is_set = accounts_credentials::is_set(&mut tx, user.id).await?;
    tx.commit().await?;
    Ok(PasswordStatus { is_set })
}

/// Keeps the password as the signed-in user's, their first or in place
/// of the one they had, and mails their account's email that it changed.
pub async fn update(
    deployment: &Deployment,
    mailer: &Mailer,
    user: &User,
    update: &PasswordUpdate,
) -> Result<(), AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let account = accounts_users::get(&mut tx, user.id)
        .await?
        .ok_or(AccountError::SignInExpired)?;
    accounts_credentials::set(&mut tx, user.id, &update.password).await?;
    tx.commit().await?;
    mailer.send(mails::password_changed(&account.email)).await?;
    Ok(())
}

/// `get` asked of the server by the user signed in on this device.
#[cfg(feature = "sync-http")]
pub async fn get_on_device(
    deployment: &Deployment,
    server: &crate::services::web_server::Http,
) -> Result<PasswordStatus, AccountError> {
    let (_user, token) = crate::app::users::signed_in_with_token(deployment).await?;
    Ok(server.accounts_password_get(&token).await?)
}

/// `update` asked of the server by the user signed in on this
/// device.
#[cfg(feature = "sync-http")]
pub async fn update_on_device(
    deployment: &Deployment,
    server: &crate::services::web_server::Http,
    update: &PasswordUpdate,
) -> Result<(), AccountError> {
    let (_user, token) = crate::app::users::signed_in_with_token(deployment).await?;
    Ok(server.accounts_password_update(&token, update).await?)
}
