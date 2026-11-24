//! The code that confirms deleting an account, mailed as a sign-in's
//! is, see `docs/design/accounts-delete.md`.

use crate::accounts::deletions::DeletionError;
use crate::accounts::preferences;
use crate::data::accounts_tokens;
use crate::data::app_users::User;
use crate::deployment::{Deployment, Role};
use crate::services::mailer::Mailer;
use crate::services::mailer::mails;

/// Mails the signed-in full user a code to confirm deleting their
/// account, replacing any earlier one; ten minutes. In their language,
/// else `requested`, the request's.
pub async fn create(
    deployment: &Deployment,
    mailer: &Mailer,
    user: &User,
    requested: &str,
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
    let locale = preferences::mail_locale_in(&mut tx, Some(account.id), requested).await?;
    tx.commit().await?;
    mailer
        .send(mails::delete_code(locale, &account.email, &code))
        .await?;
    Ok(())
}
