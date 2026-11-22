//! The code that confirms deleting an account, mailed as a sign-in's
//! is, see `docs/design/accounts-delete.md`.

use crate::accounts::deletions::DeletionError;
use crate::data::accounts_tokens;
use crate::data::app_users::User;
use crate::deployment::{Deployment, Role};
use crate::services::mailer::Mailer;
use crate::services::mailer::mails;

/// Mails the signed-in full user a code to confirm deleting their
/// account, replacing any earlier one; ten minutes.
pub async fn create(
    deployment: &Deployment,
    mailer: &Mailer,
    user: &User,
) -> Result<(), DeletionError> {
    if deployment.role != Role::Server {
        return Err(DeletionError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let account = super::live_account_in(&mut tx, user.id).await?;
    if account.trial_expires_at.is_some() {
        return Err(DeletionError::TrialHasNoCode);
    }
    let code = accounts_tokens::create_delete_code(&mut tx, account.id).await?;
    tx.commit().await?;
    mailer
        .send(mails::delete_code(&account.email, &code))
        .await?;
    Ok(())
}
