//! The signed-in user's password: their first, or a new one in place
//! of the one they have, see `docs/design/accounts-sign-in.md`.

use serde::{Deserialize, Serialize};

use crate::accounts::AccountError;
use crate::data::accounts_credentials;
use crate::data::app_users::User;
use crate::deployment::{Deployment, Role};

/// A password for the signed-in user, with the one they have now when
/// they have one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordUpdate {
    pub current: Option<String>,
    pub password: String,
}

/// Keeps the password as the signed-in user's, their first or in place
/// of the one they had, which `current` must then be.
pub async fn update(
    deployment: &Deployment,
    user: &User,
    update: &PasswordUpdate,
) -> Result<(), AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let current = update.current.as_deref().unwrap_or_default();
    if accounts_credentials::is_set(&mut tx, user.id).await?
        && !accounts_credentials::verify(&mut tx, Some(user.id), current).await?
    {
        return Err(AccountError::PasswordDoesNotMatch);
    }
    accounts_credentials::set(&mut tx, user.id, &update.password).await?;
    tx.commit().await?;
    Ok(())
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
