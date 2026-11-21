//! Accounts: signing up, signing in and signing out. The server does
//! the work, and the emailed code is what proves who someone is, see
//! `docs/design/accounts-sign-in.md`. A sign-up writes the person's
//! account, `users`, unconfirmed, and their user's row with their name,
//! `crate::app::users`; a sign-in finds the account by its email; both then
//! make a pending sign-in, `tokens`, and mail its six-digit code,
//! `mails`, answering the pending token the page holds. The code,
//! `sign_in_with_code`, confirms the email the first time, then asks
//! for whatever the account's `second_factors` are, a password being
//! the only one: an account with a password is asked for it,
//! `sign_in_with_password`, and a new account is offered to make one or
//! skip, `sign_in_with_new_password`. Every way ends in `finish_in`: a
//! session token for the browser's cookie, or, for a sign-in a device
//! started in the browser, a device code the device exchanges for its
//! API token, `sign_in_with_device_code`, with the verifier only it
//! holds. A sign-out deletes the token. A trial user, `trial_users`,
//! signs up with `sign_up_from_trial`, which keeps their user and gives
//! their account the address once its code matches.
//!
//! A device never sees a password: it keeps the session the exchange
//! answered, `keep_signed_in`, as the `signed_in` device setting, and a
//! sign-out clears the setting and keeps the data, so signing in again
//! resumes from the cursor; which file a device signs into is the app's
//! business, see `docs/design/device-accounts.md`.

#[cfg(feature = "app-store")]
pub mod app_store_notifications;
#[cfg(feature = "app-store")]
pub mod app_store_reconcile;
#[cfg(feature = "stripe")]
pub mod checkouts;
pub mod deletions;
pub mod online;
#[cfg(feature = "stripe")]
pub mod plans;
#[cfg(feature = "stripe")]
pub mod prices;
#[cfg(feature = "stripe")]
pub mod products;
#[cfg(feature = "stripe")]
pub mod purchases;
#[cfg(feature = "stripe")]
pub mod store_transactions;
#[cfg(feature = "stripe")]
pub mod stripe_events;
#[cfg(feature = "stripe")]
pub mod stripe_reconcile;
#[cfg(feature = "stripe")]
pub mod subscriptions;
pub mod trial_users;
pub mod users;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::app::communities::{self, CommunityError};
use crate::app::sync::{SyncError, Transport};
use crate::app::users::{SignedIn, UserError};
use crate::credentials::{self, CredentialError};
use crate::data::app_users::User;
use crate::data::device_settings::{self, DeviceSettingsError, SIGNED_IN};
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::helpers::random;
use crate::services::mailer::mails;
use crate::services::mailer::{MailError, Mailer};
use crate::tokens::{self, Context};

use self::users::{AccountUser, AccountUserError};

/// Wrong codes a pending sign-in takes before it is deleted, and wrong
/// passwords after the code.
pub const MAX_CODE_ATTEMPTS: i64 = 5;

/// What an account with a password does about one its owner forgot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForgottenPassword {
    /// After the emailed code they make a new one, so the password
    /// guards only against someone without the inbox.
    Reset,
    /// Nothing: the account stays closed without its password.
    LockedOut,
}

/// The app's choice, to change in a fork.
pub const FORGOTTEN_PASSWORD: ForgottenPassword = ForgottenPassword::Reset;

/// The name of the community a person who signs up starts with: every
/// new user has one of their own, so an app may ask for a current
/// community everywhere. An ordinary community, theirs to rename or
/// delete.
pub const SIGN_UP_COMMUNITY_NAME: &str = "Personal";

/// A second proof of who someone is, asked for after the emailed code
/// when their account has it. A password is the only one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecondFactor {
    Password,
}

/// What the page asks for after the code matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextStep {
    /// The account's password, `sign_in_with_password`.
    Password,
    /// A password to make, or to skip, at the first sign-in,
    /// `sign_in_with_new_password`.
    NewPassword,
}

/// What the emailed code answers: the finished sign-in, or the step
/// still to take with the pending token to take it with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum SignInStep<Finished> {
    Finished(Finished),
    Next {
        next: NextStep,
        pending: String,
        /// Whether a forgotten password may be reset at this step,
        /// `sign_in_with_reset_password`.
        can_reset_password: bool,
    },
}

/// What a finished sign-in answers: the user, their account's email,
/// which a device shows its accounts by, and the token that stands for
/// them from now on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub user: User,
    pub email: String,
    pub token: String,
}

/// A password for the signed-in user, with the one they have now when
/// they have one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordUpdate {
    pub current: Option<String>,
    pub password: String,
}

/// What a sign-up or a sign-in answers: the pending token the page
/// sends back with the code that was emailed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeSent {
    pub pending: String,
}

/// What a sign-in a device started answers once the code matched: the
/// code the device exchanges for its session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCode {
    pub code: String,
}

#[derive(Debug, Error)]
pub enum AccountError {
    #[error("only the server signs people up and in")]
    ServerOnly,
    #[error("only a device keeps someone signed in")]
    DeviceOnly,
    #[error("That password is wrong")]
    PasswordDoesNotMatch,
    #[error("That sign-in has expired; sign in again")]
    SignInExpired,
    #[error("A forgotten password cannot be reset")]
    PasswordResetNotAllowed,
    #[error("That code is wrong or has expired; sign in again to get a new one")]
    CodeDoesNotMatch,
    #[error("That code is wrong or has expired; sign in again")]
    UnknownDeviceCode,
    #[error("Only a trial signs up this way")]
    NotATrial,
    #[error("Trials are full right now; sign up instead")]
    TrialsFull,
    #[error(transparent)]
    User(#[from] UserError),
    #[error(transparent)]
    AccountUser(#[from] AccountUserError),
    #[error(transparent)]
    Credential(#[from] CredentialError),
    #[error(transparent)]
    Community(#[from] CommunityError),
    #[error("Could not send the email; try again")]
    Mail(#[from] MailError),
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error(transparent)]
    DeviceSettings(#[from] DeviceSettingsError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// What a code that matched ends in.
enum Finish<'a> {
    /// A token for the browser's cookie.
    Session,
    /// A code for the device whose challenge this is.
    Device { challenge: &'a str },
}

/// Registers the person on the server, unconfirmed, named from their
/// email, `users::name_from_email_in`, and mails them a code: their
/// account, their user's row and a pending sign-in, in one transaction,
/// then the email. An email that already has a confirmed account is
/// answered the same way, and its owner is emailed that they have one;
/// an unconfirmed one gets a new code, so whoever reads the inbox gets
/// the account.
pub async fn sign_up(
    deployment: &Deployment,
    mailer: &Mailer,
    email: &str,
) -> Result<CodeSent, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let email = users::checked_email(email)?;
    let mut tx = deployment.begin().await?;
    let existing = users::get_by_email_in(&mut tx, &email).await?;
    let (mail, pending) = match existing {
        Some(account) if account.confirmed_at.is_some() => {
            let mail = mails::already_signed_up(&account.email);
            (mail, random::hex(32))
        }
        Some(account) => {
            let (pending, code) = tokens::create_sign_in_code(&mut tx, account.id, None).await?;
            (mails::sign_in_code(&account.email, &code), pending)
        }
        None => {
            let account = users::create_in(&mut tx, Uuid::now_v7(), &email, false).await?;
            let name = crate::app::users::name_from_email_in(&mut tx, &email, account.id).await?;
            crate::app::users::create_in(&mut tx, account.id, &name).await?;
            let (pending, code) = tokens::create_sign_in_code(&mut tx, account.id, None).await?;
            (mails::sign_in_code(&account.email, &code), pending)
        }
    };
    tx.commit().await?;
    mailer.send(mail).await?;
    Ok(CodeSent { pending })
}

/// Mails the account of the email a code for a pending sign-in, on the
/// server. An email with no account is answered the same way, and
/// emailed that it has none.
pub async fn sign_in(
    deployment: &Deployment,
    mailer: &Mailer,
    email: &str,
) -> Result<CodeSent, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let email = users::checked_email(email)?;
    let mut tx = deployment.begin().await?;
    let (mail, pending) = match users::get_by_email_in(&mut tx, &email).await? {
        // A trial user's address is made up: nothing to send, nobody to tell
        Some(account) if account.trial_expires_at.is_some() => (None, random::hex(32)),
        Some(account) => {
            let (pending, code) = tokens::create_sign_in_code(&mut tx, account.id, None).await?;
            (Some(mails::sign_in_code(&account.email, &code)), pending)
        }
        None => (Some(mails::no_account(&email)), random::hex(32)),
    };
    tx.commit().await?;
    if let Some(mail) = mail {
        mailer.send(mail).await?;
    }
    Ok(CodeSent { pending })
}

/// A trial user signing up, on the server: a code is mailed to the
/// address they gave, and when it matches, `sign_in_with_code`, their
/// account takes that address and is a trial no longer; the user and
/// everything they made stay as they are. Their made-up name gives way
/// at once to one from the address, `users::name_from_email_in`. An
/// address that already has a confirmed account is answered the
/// same way, and its owner is emailed that they have one; the trial is
/// unchanged.
pub async fn sign_up_from_trial(
    deployment: &Deployment,
    mailer: &Mailer,
    trial: &User,
    email: &str,
) -> Result<CodeSent, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let email = users::checked_email(email)?;
    let mut tx = deployment.begin().await?;
    let account = live_account_in(&mut tx, trial.id).await?;
    if account.trial_expires_at.is_none() {
        return Err(AccountError::NotATrial);
    }
    let (mail, pending) = match users::get_by_email_in(&mut tx, &email).await? {
        Some(taken) if taken.confirmed_at.is_some() => {
            let mail = mails::already_signed_up(&taken.email);
            (mail, random::hex(32))
        }
        _ => {
            let name = crate::app::users::name_from_email_in(&mut tx, &email, trial.id).await?;
            crate::app::users::update_name_in(&mut tx, trial.id, &name).await?;
            let (pending, code) =
                tokens::create_sign_in_code(&mut tx, trial.id, Some(&email)).await?;
            (mails::sign_in_code(&email, &code), pending)
        }
    };
    tx.commit().await?;
    mailer.send(mail).await?;
    Ok(CodeSent { pending })
}

/// Mails the pending sign-in a new code, on the server, its ten minutes
/// starting over and its wrong attempts forgotten; the old code is no
/// longer good. For the "Send a new code" button; how often is the web
/// server's limit, see `docs/design/accounts-sign-in.md`.
pub async fn resend_code(
    deployment: &Deployment,
    mailer: &Mailer,
    pending: &str,
) -> Result<(), AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let row = tokens::get_valid(&mut tx, &tokens::hash(pending), Context::SignInCode, now())
        .await?
        .ok_or(AccountError::CodeDoesNotMatch)?;
    let account = live_account_in(&mut tx, row.user_id).await?;
    let code = tokens::reset_code(&mut tx, row.id).await?;
    tx.commit().await?;
    let sent_to = row.sent_to.unwrap_or(account.email);
    mailer.send(mails::sign_in_code(&sent_to, &code)).await?;
    Ok(())
}

/// The address `resend_code` would mail, for the server to count the
/// email against before sending; the same refusal as `resend_code`.
pub async fn resend_code_to(
    deployment: &Deployment,
    pending: &str,
) -> Result<String, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let row = tokens::get_valid(&mut tx, &tokens::hash(pending), Context::SignInCode, now())
        .await?
        .ok_or(AccountError::CodeDoesNotMatch)?;
    let account = live_account_in(&mut tx, row.user_id).await?;
    tx.commit().await?;
    Ok(row.sent_to.unwrap_or(account.email))
}

/// Takes the code that was emailed for the pending sign-in, on the
/// server: a token for the browser's cookie, or the step still to take.
/// A wrong code counts; after `MAX_CODE_ATTEMPTS` the pending sign-in
/// is gone.
pub async fn sign_in_with_code(
    deployment: &Deployment,
    pending: &str,
    code: &str,
) -> Result<SignInStep<Session>, AccountError> {
    let proof = Proof::Code { pending, code };
    let (step, _account) = prove(deployment, proof, Finish::Session).await?;
    Ok(step)
}

/// Takes the code, for a sign-in a device started with the challenge:
/// the device code it collects, and no token for the browser, or the
/// step still to take.
pub async fn sign_in_with_code_for_device(
    deployment: &Deployment,
    pending: &str,
    code: &str,
    challenge: &str,
) -> Result<SignInStep<DeviceCode>, AccountError> {
    let proof = Proof::Code { pending, code };
    let (step, _account) = prove(deployment, proof, Finish::Device { challenge }).await?;
    Ok(step)
}

/// Finishes a sign-in whose code matched with the account's password,
/// on the server. A wrong one counts as a wrong code does.
pub async fn sign_in_with_password(
    deployment: &Deployment,
    pending: &str,
    password: &str,
) -> Result<Session, AccountError> {
    let proof = Proof::Password { pending, password };
    let (step, _account) = prove(deployment, proof, Finish::Session).await?;
    finished(step)
}

/// Finishes it with the password for the device whose challenge it is.
pub async fn sign_in_with_password_for_device(
    deployment: &Deployment,
    pending: &str,
    password: &str,
    challenge: &str,
) -> Result<DeviceCode, AccountError> {
    let proof = Proof::Password { pending, password };
    let (step, _account) = prove(deployment, proof, Finish::Device { challenge }).await?;
    finished(step)
}

/// Finishes a first sign-in whose code matched, keeping the password
/// the person made, or none when they skipped.
pub async fn sign_in_with_new_password(
    deployment: &Deployment,
    pending: &str,
    password: Option<&str>,
) -> Result<Session, AccountError> {
    let proof = Proof::NewPassword { pending, password };
    let (step, _account) = prove(deployment, proof, Finish::Session).await?;
    finished(step)
}

/// Finishes it for the device whose challenge it is.
pub async fn sign_in_with_new_password_for_device(
    deployment: &Deployment,
    pending: &str,
    password: Option<&str>,
    challenge: &str,
) -> Result<DeviceCode, AccountError> {
    let proof = Proof::NewPassword { pending, password };
    let (step, _account) = prove(deployment, proof, Finish::Device { challenge }).await?;
    finished(step)
}

/// Finishes a sign-in whose code matched with a new password in place
/// of the one the person forgot, when `FORGOTTEN_PASSWORD` lets them,
/// and mails them that it changed.
pub async fn sign_in_with_reset_password(
    deployment: &Deployment,
    mailer: &Mailer,
    pending: &str,
    password: &str,
) -> Result<Session, AccountError> {
    let proof = Proof::ResetPassword { pending, password };
    let (step, account) = prove(deployment, proof, Finish::Session).await?;
    mailer.send(mails::password_reset(&account.email)).await?;
    finished(step)
}

/// Finishes it for the device whose challenge it is.
pub async fn sign_in_with_reset_password_for_device(
    deployment: &Deployment,
    mailer: &Mailer,
    pending: &str,
    password: &str,
    challenge: &str,
) -> Result<DeviceCode, AccountError> {
    let proof = Proof::ResetPassword { pending, password };
    let (step, account) = prove(deployment, proof, Finish::Device { challenge }).await?;
    mailer.send(mails::password_reset(&account.email)).await?;
    finished(step)
}

/// What the signed-in user's account asks for after the emailed code.
pub async fn second_factors(
    deployment: &Deployment,
    user: &User,
) -> Result<Vec<SecondFactor>, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let factors = second_factors_in(&mut tx, user.id).await?;
    tx.commit().await?;
    Ok(factors)
}

/// Keeps the password as the signed-in user's, their first or in place
/// of the one they had, which `current` must then be.
pub async fn password_update(
    deployment: &Deployment,
    user: &User,
    update: &PasswordUpdate,
) -> Result<(), AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let current = update.current.as_deref().unwrap_or_default();
    if credentials::is_set(&mut tx, user.id).await?
        && !credentials::verify(&mut tx, Some(user.id), current).await?
    {
        return Err(AccountError::PasswordDoesNotMatch);
    }
    credentials::set(&mut tx, user.id, &update.password).await?;
    tx.commit().await?;
    Ok(())
}

/// `second_factors` asked of the server by the user signed in on this
/// device.
#[cfg(feature = "sync-http")]
pub async fn second_factors_on_device(
    deployment: &Deployment,
    server: &crate::app::sync::Http,
) -> Result<Vec<SecondFactor>, AccountError> {
    let (_user, token) = crate::app::users::signed_in_with_token(deployment).await?;
    Ok(server.accounts_second_factors(&token).await?)
}

/// `password_update` asked of the server by the user signed in on this
/// device.
#[cfg(feature = "sync-http")]
pub async fn password_update_on_device(
    deployment: &Deployment,
    server: &crate::app::sync::Http,
    update: &PasswordUpdate,
) -> Result<(), AccountError> {
    let (_user, token) = crate::app::users::signed_in_with_token(deployment).await?;
    Ok(server.accounts_password_update(&token, update).await?)
}

/// Exchanges the device code, with the verifier the device kept, for
/// the device's session, once: the row is found by the code and the
/// verifier's hash together, see `tokens::create_device_code`, and
/// deleted.
pub async fn sign_in_with_device_code(
    deployment: &Deployment,
    code: &str,
    verifier: &str,
) -> Result<Session, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let token_hash = tokens::device_code_hash(code, &challenge_of(verifier));
    let row = tokens::get_valid(&mut tx, &token_hash, Context::DeviceCode, now())
        .await?
        .ok_or(AccountError::UnknownDeviceCode)?;
    tokens::delete_by_id(&mut tx, row.id).await?;
    let account = users::get_in(&mut tx, row.user_id)
        .await?
        .ok_or(AccountError::UnknownDeviceCode)?;
    let user = live_user_in(&mut tx, row.user_id)
        .await?
        .ok_or(AccountError::UnknownDeviceCode)?;
    let token = tokens::create(&mut tx, user.id, Context::Api).await?;
    tx.commit().await?;
    Ok(Session {
        user,
        email: account.email,
        token,
    })
}

/// Deletes the token, on the server, so it stands for nobody from now
/// on; one the server does not know is already gone.
pub async fn sign_out(deployment: &Deployment, token: &str) -> Result<(), AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    tokens::delete(&mut tx, token).await?;
    tx.commit().await?;
    Ok(())
}

/// What a device makes before it opens the browser: the verifier it
/// keeps to itself, and the challenge it puts in the sign-in URL, the
/// verifier's hash.
pub fn verifier_and_challenge() -> (String, String) {
    let verifier = random::hex(32);
    let challenge = challenge_of(&verifier);
    (verifier, challenge)
}

/// The challenge a verifier proves: its SHA-256 as hex.
pub fn challenge_of(verifier: &str) -> String {
    tokens::hash(verifier)
}

/// Exchanges the device code through the transport, `sign_in_with_device_code`
/// on the server: the session, kept nowhere yet.
pub async fn sign_in_with_device_code_through<T: Transport>(
    transport: &T,
    code: &str,
    verifier: &str,
) -> Result<Session, AccountError> {
    match transport.sign_in_with_device_code(code, verifier).await {
        Err(SyncError::Unauthorized) => Err(AccountError::UnknownDeviceCode),
        answer => Ok(answer?),
    }
}

/// Keeps a sign-in on this device: the user's row, and the setting
/// naming them with their token. Whatever the device holds stays, so a
/// sync resumes from its cursor; the app hands a user their own
/// database, never another's.
pub async fn keep_signed_in(
    deployment: &Deployment,
    session: Session,
) -> Result<User, AccountError> {
    if deployment.role != Role::Device {
        return Err(AccountError::DeviceOnly);
    }
    let mut tx = deployment.begin().await?;
    crate::app::users::upsert_in(&mut tx, &session.user).await?;
    let signed_in = SignedIn {
        user_id: session.user.id,
        token: Some(session.token),
    };
    device_settings::set(&mut tx, SIGNED_IN, &Some(signed_in)).await?;
    tx.commit().await?;
    Ok(session.user)
}

/// Tells the server to forget the device's token and clears the setting,
/// so nobody is signed in. The data stays, pending and conflicted
/// changes included, for the next sign-in to resume from; a token the
/// server no longer knows is already forgotten.
pub async fn sign_out_on_device<T: Transport>(
    deployment: &Deployment,
    transport: &T,
) -> Result<(), AccountError> {
    if deployment.role != Role::Device {
        return Err(AccountError::DeviceOnly);
    }
    let (_user, token) = crate::app::users::signed_in_with_token(deployment).await?;
    match transport.sign_out(&token).await {
        Ok(()) | Err(SyncError::Unauthorized) | Err(SyncError::AccountDeleted) => {}
        Err(error) => return Err(error.into()),
    }
    forget_signed_in(deployment).await
}

/// Clears the setting without telling the server, when the server has
/// already forgotten the token: it answered 401. The data stays.
pub async fn forget_signed_in(deployment: &Deployment) -> Result<(), AccountError> {
    if deployment.role != Role::Device {
        return Err(AccountError::DeviceOnly);
    }
    let mut tx = deployment.begin().await?;
    device_settings::set(&mut tx, SIGNED_IN, &None::<SignedIn>).await?;
    tx.commit().await?;
    Ok(())
}

/// What a step of a sign-in is given.
enum Proof<'a> {
    Code {
        pending: &'a str,
        code: &'a str,
    },
    Password {
        pending: &'a str,
        password: &'a str,
    },
    NewPassword {
        pending: &'a str,
        password: Option<&'a str>,
    },
    ResetPassword {
        pending: &'a str,
        password: &'a str,
    },
}

/// One step of a sign-in: the proof is checked, and the sign-in is
/// finished or its next step answered. A refusal is committed too, so
/// a wrong attempt stays counted.
async fn prove<Finished: FromFinish>(
    deployment: &Deployment,
    proof: Proof<'_>,
    finish: Finish<'_>,
) -> Result<(SignInStep<Finished>, AccountUser), AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let proven = match proof {
        Proof::Code { pending, code } => match_code_in(&mut tx, pending, code).await,
        Proof::Password { pending, password } => {
            match_password_in(&mut tx, pending, password).await
        }
        Proof::NewPassword { pending, password } => {
            keep_new_password_in(&mut tx, pending, password).await
        }
        Proof::ResetPassword { pending, password } => {
            keep_reset_password_in(&mut tx, pending, password, FORGOTTEN_PASSWORD).await
        }
    };
    let (account, next) = match proven {
        Ok(proven) => proven,
        Err(error) => {
            tx.commit().await?;
            return Err(error);
        }
    };
    let step = match next {
        Some(next) => {
            let pending = tokens::create_sign_in_password(&mut tx, account.id).await?;
            SignInStep::Next {
                next,
                pending,
                can_reset_password: next == NextStep::Password
                    && FORGOTTEN_PASSWORD == ForgottenPassword::Reset,
            }
        }
        None => {
            let user = live_user_in(&mut tx, account.id)
                .await?
                .ok_or(AccountError::SignInExpired)?;
            let secret = finish_in(&mut tx, &user, finish).await?;
            SignInStep::Finished(Finished::from_finish(user, account.email.clone(), secret))
        }
    };
    tx.commit().await?;
    Ok((step, account))
}

/// What `finish_in` answered, as the caller's kind of finished sign-in.
trait FromFinish {
    fn from_finish(user: User, email: String, secret: String) -> Self;
}

impl FromFinish for Session {
    fn from_finish(user: User, email: String, token: String) -> Self {
        Session { user, email, token }
    }
}

impl FromFinish for DeviceCode {
    fn from_finish(_user: User, _email: String, code: String) -> Self {
        DeviceCode { code }
    }
}

/// A password step always finishes.
fn finished<Finished>(step: SignInStep<Finished>) -> Result<Finished, AccountError> {
    match step {
        SignInStep::Finished(finished) => Ok(finished),
        SignInStep::Next { .. } => Err(AccountError::SignInExpired),
    }
}

/// The account of the pending sign-in, once the code matches, and the
/// step still to take: the pending sign-in is deleted and, if it was
/// not yet, the email confirmed and the person's first community made. A wrong code is counted, and the one
/// that reaches `MAX_CODE_ATTEMPTS` deletes the pending sign-in.
async fn match_code_in(
    tx: &mut Transaction,
    pending: &str,
    code: &str,
) -> Result<(AccountUser, Option<NextStep>), AccountError> {
    let row = tokens::get_valid(tx, &tokens::hash(pending), Context::SignInCode, now())
        .await?
        .ok_or(AccountError::CodeDoesNotMatch)?;
    if row.code_hash.as_deref() != Some(tokens::hash(code.trim()).as_str()) {
        count_attempt_in(tx, row.id).await?;
        return Err(AccountError::CodeDoesNotMatch);
    }
    tokens::delete_by_id(tx, row.id).await?;
    let account = live_account_in(tx, row.user_id).await?;
    let first_sign_in = account.confirmed_at.is_none();
    let account = match row.sent_to.as_deref() {
        Some(email) => sign_up_trial_in(tx, account, email).await?,
        None if first_sign_in => {
            users::confirm_in(tx, account.id).await?;
            let user = live_user_in(tx, account.id)
                .await?
                .ok_or(AccountError::CodeDoesNotMatch)?;
            communities::create_in(tx, Role::Server, &user, SIGN_UP_COMMUNITY_NAME).await?;
            account
        }
        None => account,
    };
    let next = match second_factors_in(tx, account.id).await?.first() {
        Some(SecondFactor::Password) => Some(NextStep::Password),
        None if first_sign_in => Some(NextStep::NewPassword),
        None => None,
    };
    Ok((account, next))
}

/// A trial user's account as the person's own, once the code sent to
/// their address matched: the address, confirmed, the trial over, their
/// communities shown to everyone, and the trial's session forgotten
/// for the one this sign-in ends in. A sign-up that never proved the
/// same address gives way; a confirmed account with it does not.
async fn sign_up_trial_in(
    tx: &mut Transaction,
    account: AccountUser,
    email: &str,
) -> Result<AccountUser, AccountError> {
    if account.trial_expires_at.is_none() {
        return Err(AccountError::CodeDoesNotMatch);
    }
    if let Some(other) = users::get_by_email_in(tx, email).await? {
        if other.confirmed_at.is_some() {
            return Err(AccountError::CodeDoesNotMatch);
        }
        users::delete_unconfirmed_in(tx, other.id).await?;
    }
    users::sign_up_trial_in(tx, account.id, email).await?;
    crate::accounts::trial_users::show_communities_in(tx, account.id).await?;
    tokens::delete_of_user(tx, account.id, Context::Session).await?;
    live_account_in(tx, account.id).await
}

/// The account of the sign-in whose code matched, once the password
/// does too. A wrong one is counted as a wrong code is.
async fn match_password_in(
    tx: &mut Transaction,
    pending: &str,
    password: &str,
) -> Result<(AccountUser, Option<NextStep>), AccountError> {
    let row = tokens::get_valid(tx, &tokens::hash(pending), Context::SignInPassword, now())
        .await?
        .ok_or(AccountError::SignInExpired)?;
    if !credentials::verify(tx, Some(row.user_id), password).await? {
        count_attempt_in(tx, row.id).await?;
        return Err(AccountError::PasswordDoesNotMatch);
    }
    tokens::delete_by_id(tx, row.id).await?;
    Ok((live_account_in(tx, row.user_id).await?, None))
}

/// The account of the first sign-in whose code matched, with the
/// password kept if one was made. An account that has a password is
/// not given another this way.
async fn keep_new_password_in(
    tx: &mut Transaction,
    pending: &str,
    password: Option<&str>,
) -> Result<(AccountUser, Option<NextStep>), AccountError> {
    let row = tokens::get_valid(tx, &tokens::hash(pending), Context::SignInPassword, now())
        .await?
        .ok_or(AccountError::SignInExpired)?;
    if credentials::is_set(tx, row.user_id).await? {
        return Err(AccountError::SignInExpired);
    }
    if let Some(password) = password {
        credentials::set(tx, row.user_id, password).await?;
    }
    tokens::delete_by_id(tx, row.id).await?;
    Ok((live_account_in(tx, row.user_id).await?, None))
}

/// The account of the sign-in whose code matched, with a new password
/// in place of the one it had, when the app lets a forgotten one be
/// reset.
async fn keep_reset_password_in(
    tx: &mut Transaction,
    pending: &str,
    password: &str,
    forgotten_password: ForgottenPassword,
) -> Result<(AccountUser, Option<NextStep>), AccountError> {
    if forgotten_password == ForgottenPassword::LockedOut {
        return Err(AccountError::PasswordResetNotAllowed);
    }
    let row = tokens::get_valid(tx, &tokens::hash(pending), Context::SignInPassword, now())
        .await?
        .ok_or(AccountError::SignInExpired)?;
    if !credentials::is_set(tx, row.user_id).await? {
        return Err(AccountError::SignInExpired);
    }
    credentials::set(tx, row.user_id, password).await?;
    tokens::delete_by_id(tx, row.id).await?;
    Ok((live_account_in(tx, row.user_id).await?, None))
}

/// Counts a wrong attempt against the pending token; the one that
/// reaches `MAX_CODE_ATTEMPTS` deletes it.
async fn count_attempt_in(tx: &mut Transaction, id: Uuid) -> Result<(), AccountError> {
    if tokens::count_attempt(tx, id).await? >= MAX_CODE_ATTEMPTS {
        tokens::delete_by_id(tx, id).await?;
    }
    Ok(())
}

async fn second_factors_in(
    tx: &mut Transaction,
    account_id: Uuid,
) -> Result<Vec<SecondFactor>, AccountError> {
    let mut factors = Vec::new();
    if credentials::is_set(tx, account_id).await? {
        factors.push(SecondFactor::Password);
    }
    Ok(factors)
}

/// Where every proven user ends: the browser's token, or the device
/// code for the device whose challenge was given.
async fn finish_in(
    tx: &mut Transaction,
    user: &User,
    finish: Finish<'_>,
) -> Result<String, AccountError> {
    Ok(match finish {
        Finish::Session => tokens::create(tx, user.id, Context::Session).await?,
        Finish::Device { challenge } => tokens::create_device_code(tx, user.id, challenge).await?,
    })
}

async fn live_account_in(tx: &mut Transaction, id: Uuid) -> Result<AccountUser, AccountError> {
    users::get_in(tx, id)
        .await?
        .ok_or(AccountError::CodeDoesNotMatch)
}

async fn live_user_in(tx: &mut Transaction, id: Uuid) -> Result<Option<User>, AccountError> {
    let user = crate::app::users::get_in(tx, id).await?;
    Ok(user.filter(|user| user.deleted_at.is_none()))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::app::sync::{self, InProcess};
    use crate::app::{posts, users};
    use crate::database::with_each_database;

    pub(crate) const PASSWORD: &str = "correct horse battery";

    /// The code the last email to the address carried.
    pub(crate) fn code_sent_to(mailer: &Mailer, email: &str) -> String {
        let mail = mailer.last_sent_to(email).expect("an email");
        mails::code_in(&mail).expect("a code in it")
    }

    /// The pending token of the step the code answered.
    fn next_of<Finished>(step: SignInStep<Finished>, expected: NextStep) -> String {
        match step {
            SignInStep::Next { next, pending, .. } if next == expected => pending,
            _ => panic!("expected the step {expected:?}"),
        }
    }

    /// The pending token of a step still to take, none when finished.
    fn session_of_or_pending(step: SignInStep<Session>) -> Option<String> {
        match step {
            SignInStep::Finished(_) => None,
            SignInStep::Next { pending, .. } => Some(pending),
        }
    }

    fn session_of(step: SignInStep<Session>) -> Session {
        finished(step).expect("a finished sign-in")
    }

    /// A user signed up on the server with no password, their email
    /// confirmed: the browser's session.
    async fn signed_up(server: &Deployment, mailer: &Mailer, email: &str) -> Session {
        let sent = sign_up(server, mailer, email).await.expect("signed up");
        let code = code_sent_to(mailer, email);
        let step = sign_in_with_code(server, &sent.pending, &code)
            .await
            .expect("the code matched");
        let pending = next_of(step, NextStep::NewPassword);
        sign_in_with_new_password(server, &pending, None)
            .await
            .expect("skipped making a password")
    }

    /// A sign-in of an account with no password, as a device: the
    /// device code and the verifier to exchange it with.
    async fn signed_in_for_a_device(server: &Deployment, email: &str) -> (DeviceCode, String) {
        let mailer = Mailer::memory();
        let sent = sign_in(server, &mailer, email).await.expect("signed in");
        let code = code_sent_to(&mailer, email);
        let (verifier, challenge) = verifier_and_challenge();
        let step = sign_in_with_code_for_device(server, &sent.pending, &code, &challenge)
            .await
            .expect("the code matched");
        (finished(step).expect("a finished sign-in"), verifier)
    }

    /// A user signed up on the server with no password, as a device:
    /// the session its exchange answered, with an API token.
    pub(crate) async fn signed_up_for_a_device(server: &Deployment, email: &str) -> Session {
        signed_up(server, &Mailer::memory(), email).await;
        let (device_code, verifier) = signed_in_for_a_device(server, email).await;
        sign_in_with_device_code(server, &device_code.code, &verifier)
            .await
            .expect("exchanged")
    }

    #[tokio::test]
    async fn accounts_sign_up_mails_a_code_that_confirms_and_signs_in_then_sign_in_and_out() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let sent = sign_up(&server, &mailer, " Alice@Example.com ")
                .await
                .expect("signed up");
            assert_eq!(sent.pending.len(), 64);
            let mail = mailer.last_sent_to("alice@example.com").expect("mailed");
            let code = mails::code_in(&mail).expect("a code");
            assert!(mail.subject.contains("code"));

            let error = sign_in_with_code(&server, &sent.pending, "000000")
                .await
                .expect_err("wrong, unless the code is 000000");
            assert!(matches!(error, AccountError::CodeDoesNotMatch) || code == "000000");

            let step = sign_in_with_code(&server, &sent.pending, &format!(" {code} "))
                .await
                .expect("the code matched, trimmed");
            let pending = next_of(step, NextStep::NewPassword);
            assert!(
                matches!(
                    sign_in_with_code(&server, &sent.pending, &code).await,
                    Err(AccountError::CodeDoesNotMatch)
                ),
                "a code is used once"
            );
            let signed_up = sign_in_with_new_password(&server, &pending, None)
                .await
                .expect("skipped making a password");
            assert_eq!(signed_up.email, "alice@example.com");
            assert_eq!(signed_up.user.name, "alice", "named from the email");
            let account = super::users::get(&server, signed_up.user.id)
                .await
                .expect("their account");
            assert!(account.confirmed_at.is_some(), "the email is proved");
            assert_eq!(
                users::get_by_token(&server, &signed_up.token)
                    .await
                    .expect("stands for alice"),
                signed_up.user
            );
            assert!(
                matches!(
                    sign_in_with_new_password(&server, &pending, None).await,
                    Err(AccountError::SignInExpired)
                ),
                "the step is taken once"
            );
            assert!(
                second_factors(&server, &signed_up.user)
                    .await
                    .expect("looked")
                    .is_empty()
            );

            let sent = sign_in(&server, &mailer, "ALICE@example.com")
                .await
                .expect("signed in");
            let code = code_sent_to(&mailer, "alice@example.com");
            let step = sign_in_with_code(&server, &sent.pending, &code)
                .await
                .expect("signed in");
            let signed_in = session_of(step);
            assert_eq!(signed_in.user, signed_up.user);
            assert_ne!(signed_in.token, signed_up.token, "a token of its own");

            sign_out(&server, &signed_in.token)
                .await
                .expect("signed out");
            assert!(matches!(
                users::get_by_token(&server, &signed_in.token).await,
                Err(UserError::UnknownToken)
            ));
            assert!(
                users::get_by_token(&server, &signed_up.token).await.is_ok(),
                "the other session stays"
            );
            sign_out(&server, &signed_in.token)
                .await
                .expect("already gone is fine");
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_password_made_at_the_first_sign_in_is_asked_for_after_the_code() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let sent = sign_up(&server, &mailer, "alice@example.com")
                .await
                .expect("signed up");
            let code = code_sent_to(&mailer, "alice@example.com");
            let step = sign_in_with_code(&server, &sent.pending, &code)
                .await
                .expect("the code matched");
            let pending = next_of(step, NextStep::NewPassword);
            let error = sign_in_with_new_password(&server, &pending, Some("short"))
                .await
                .expect_err("too short");
            assert!(matches!(
                error,
                AccountError::Credential(CredentialError::PasswordTooShort)
            ));
            let alice = sign_in_with_new_password(&server, &pending, Some(PASSWORD))
                .await
                .expect("the step is still there after a refused password");
            assert_eq!(
                second_factors(&server, &alice.user).await.expect("looked"),
                vec![SecondFactor::Password]
            );

            let sent = sign_in(&server, &mailer, "alice@example.com")
                .await
                .expect("signed in");
            let code = code_sent_to(&mailer, "alice@example.com");
            let step = sign_in_with_code(&server, &sent.pending, &code)
                .await
                .expect("the code matched");
            let pending = next_of(step, NextStep::Password);
            assert!(
                matches!(
                    sign_in_with_new_password(&server, &pending, None).await,
                    Err(AccountError::SignInExpired)
                ),
                "a password is not skipped once the account has one"
            );

            let sent = sign_in(&server, &mailer, "alice@example.com")
                .await
                .expect("signed in again");
            let code = code_sent_to(&mailer, "alice@example.com");
            let step = sign_in_with_code(&server, &sent.pending, &code)
                .await
                .expect("the code matched");
            let pending = next_of(step, NextStep::Password);
            let error = sign_in_with_password(&server, &pending, "wrong horse battery")
                .await
                .expect_err("wrong");
            assert!(matches!(error, AccountError::PasswordDoesNotMatch));
            let session = sign_in_with_password(&server, &pending, PASSWORD)
                .await
                .expect("the password matched");
            assert_eq!(session.user, alice.user);
            assert!(matches!(
                sign_in_with_password(&server, &pending, PASSWORD).await,
                Err(AccountError::SignInExpired)
            ));

            let sent = sign_in(&server, &mailer, "alice@example.com")
                .await
                .expect("signed in");
            let code = code_sent_to(&mailer, "alice@example.com");
            let (verifier, challenge) = verifier_and_challenge();
            let step = sign_in_with_code_for_device(&server, &sent.pending, &code, &challenge)
                .await
                .expect("the code matched");
            let pending = next_of(step, NextStep::Password);
            for _ in 0..MAX_CODE_ATTEMPTS {
                assert!(matches!(
                    sign_in_with_password_for_device(&server, &pending, "wrong", &challenge).await,
                    Err(AccountError::PasswordDoesNotMatch)
                ));
            }
            assert!(
                matches!(
                    sign_in_with_password_for_device(&server, &pending, PASSWORD, &challenge).await,
                    Err(AccountError::SignInExpired)
                ),
                "the right password is too late"
            );

            let sent = sign_in(&server, &mailer, "alice@example.com")
                .await
                .expect("signed in");
            let code = code_sent_to(&mailer, "alice@example.com");
            let step = sign_in_with_code_for_device(&server, &sent.pending, &code, &challenge)
                .await
                .expect("the code matched");
            let pending = next_of(step, NextStep::Password);
            let device_code =
                sign_in_with_password_for_device(&server, &pending, PASSWORD, &challenge)
                    .await
                    .expect("the password matched");
            let session = sign_in_with_device_code(&server, &device_code.code, &verifier)
                .await
                .expect("exchanged");
            assert_eq!(session.user, alice.user);
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_forgotten_password_is_reset_after_the_code_unless_the_app_locks_out() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let alice = signed_up(&server, &mailer, "alice@example.com").await.user;
            let sent = sign_in(&server, &mailer, "alice@example.com")
                .await
                .expect("signed in");
            let code = code_sent_to(&mailer, "alice@example.com");
            let pending = session_of_or_pending(
                sign_in_with_code(&server, &sent.pending, &code)
                    .await
                    .expect("the code matched"),
            );
            assert!(pending.is_none(), "no password, so nothing to reset");

            let first = PasswordUpdate {
                current: None,
                password: PASSWORD.to_string(),
            };
            password_update(&server, &alice, &first)
                .await
                .expect("a password");
            let sent = sign_in(&server, &mailer, "alice@example.com")
                .await
                .expect("signed in");
            let code = code_sent_to(&mailer, "alice@example.com");
            let step = sign_in_with_code(&server, &sent.pending, &code)
                .await
                .expect("the code matched");
            assert!(matches!(
                step,
                SignInStep::Next {
                    can_reset_password: true,
                    ..
                }
            ));
            let pending = next_of(step, NextStep::Password);

            let mut tx = server.begin().await.expect("a transaction");
            let locked_out = ForgottenPassword::LockedOut;
            assert!(matches!(
                keep_reset_password_in(&mut tx, &pending, "another horse battery", locked_out)
                    .await,
                Err(AccountError::PasswordResetNotAllowed)
            ));
            tx.commit().await.expect("committed");

            let error = sign_in_with_reset_password(&server, &mailer, &pending, "short")
                .await
                .expect_err("too short");
            assert!(matches!(
                error,
                AccountError::Credential(CredentialError::PasswordTooShort)
            ));
            let session =
                sign_in_with_reset_password(&server, &mailer, &pending, "another horse battery")
                    .await
                    .expect("reset and signed in");
            assert_eq!(session.user, alice);
            let mail = mailer.last_sent_to("alice@example.com").expect("mailed");
            assert_eq!(mail.subject, "Your password was changed");
            assert!(matches!(
                sign_in_with_reset_password(&server, &mailer, &pending, PASSWORD).await,
                Err(AccountError::SignInExpired)
            ));

            let sent = sign_in(&server, &mailer, "alice@example.com")
                .await
                .expect("signed in");
            let code = code_sent_to(&mailer, "alice@example.com");
            let step = sign_in_with_code(&server, &sent.pending, &code)
                .await
                .expect("the code matched");
            let pending = next_of(step, NextStep::Password);
            assert!(matches!(
                sign_in_with_password(&server, &pending, PASSWORD).await,
                Err(AccountError::PasswordDoesNotMatch)
            ));
            sign_in_with_password(&server, &pending, "another horse battery")
                .await
                .expect("the new one");
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_password_update_makes_the_first_and_changes_it_with_the_current_one() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let alice = signed_up(&server, &mailer, "alice@example.com").await.user;
            let first = PasswordUpdate {
                current: None,
                password: PASSWORD.to_string(),
            };
            let short = PasswordUpdate {
                current: None,
                password: "short".to_string(),
            };
            assert!(matches!(
                password_update(&server, &alice, &short).await,
                Err(AccountError::Credential(CredentialError::PasswordTooShort))
            ));
            password_update(&server, &alice, &first)
                .await
                .expect("their first password");
            assert_eq!(
                second_factors(&server, &alice).await.expect("looked"),
                vec![SecondFactor::Password]
            );

            let another = "another horse battery".to_string();
            for current in [None, Some("wrong horse battery".to_string())] {
                let update = PasswordUpdate {
                    current,
                    password: another.clone(),
                };
                assert!(matches!(
                    password_update(&server, &alice, &update).await,
                    Err(AccountError::PasswordDoesNotMatch)
                ));
            }
            let update = PasswordUpdate {
                current: Some(PASSWORD.to_string()),
                password: another.clone(),
            };
            password_update(&server, &alice, &update)
                .await
                .expect("changed");

            let sent = sign_in(&server, &mailer, "alice@example.com")
                .await
                .expect("signed in");
            let code = code_sent_to(&mailer, "alice@example.com");
            let step = sign_in_with_code(&server, &sent.pending, &code)
                .await
                .expect("the code matched");
            let pending = next_of(step, NextStep::Password);
            assert!(matches!(
                sign_in_with_password(&server, &pending, PASSWORD).await,
                Err(AccountError::PasswordDoesNotMatch)
            ));
            sign_in_with_password(&server, &pending, &another)
                .await
                .expect("the new one");
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_sign_up_refuses_a_bad_email_and_answers_a_taken_email_quietly() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let error = sign_up(&server, &mailer, "alice").await.expect_err("no @");
            assert!(matches!(
                error,
                AccountError::AccountUser(AccountUserError::InvalidEmail)
            ));
            assert!(mailer.sent().is_empty());

            let first = sign_up(&server, &mailer, "alice@example.com")
                .await
                .expect("signed up");
            let first_code = code_sent_to(&mailer, "alice@example.com");
            let again = sign_up(&server, &mailer, "alice@example.com")
                .await
                .expect("signed up again before confirming");
            assert!(
                matches!(
                    sign_in_with_code(&server, &first.pending, &first_code).await,
                    Err(AccountError::CodeDoesNotMatch)
                ),
                "the first pending sign-in was replaced"
            );
            let code = code_sent_to(&mailer, "alice@example.com");
            let step = sign_in_with_code(&server, &again.pending, &code)
                .await
                .expect("the second one works");
            let pending = next_of(step, NextStep::NewPassword);
            let session = sign_in_with_new_password(&server, &pending, None)
                .await
                .expect("skipped");
            assert_eq!(
                session.user.name, "alice",
                "named once, not again as alice1"
            );

            let sent_before = mailer.sent().len();
            let taken = sign_up(&server, &mailer, "alice@example.com")
                .await
                .expect("answered as a new one");
            assert_eq!(taken.pending.len(), 64);
            let mail = mailer.last_sent_to("alice@example.com").expect("mailed");
            assert_eq!(mailer.sent().len(), sent_before + 1);
            assert_eq!(mail.subject, "You already have an account");
            assert!(matches!(
                sign_in_with_code(&server, &taken.pending, "123456").await,
                Err(AccountError::CodeDoesNotMatch)
            ));
            assert_eq!(
                users::get(&server, session.user.id)
                    .await
                    .expect("alice")
                    .name,
                "alice",
                "the name was not touched"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_sign_in_answers_an_unknown_email_as_a_known_one_and_mails_that_it_has_none() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let error = sign_in(&server, &mailer, "nobody").await.expect_err("no @");
            assert!(matches!(
                error,
                AccountError::AccountUser(AccountUserError::InvalidEmail)
            ));
            let sent = sign_in(&server, &mailer, "nobody@example.com")
                .await
                .expect("answered as a known one");
            assert_eq!(sent.pending.len(), 64);
            let mail = mailer.last_sent_to("nobody@example.com").expect("mailed");
            assert!(mail.subject.starts_with("No "));
            assert!(mails::code_in(&mail).is_none());
            assert!(matches!(
                sign_in_with_code(&server, &sent.pending, "123456").await,
                Err(AccountError::CodeDoesNotMatch)
            ));
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_code_takes_five_wrong_tries_then_the_sign_in_is_gone() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let sent = sign_up(&server, &mailer, "alice@example.com")
                .await
                .expect("signed up");
            let code = code_sent_to(&mailer, "alice@example.com");
            let wrong = if code == "999999" { "999998" } else { "999999" };
            for _ in 0..MAX_CODE_ATTEMPTS {
                assert!(matches!(
                    sign_in_with_code(&server, &sent.pending, wrong).await,
                    Err(AccountError::CodeDoesNotMatch)
                ));
            }
            assert!(
                matches!(
                    sign_in_with_code(&server, &sent.pending, &code).await,
                    Err(AccountError::CodeDoesNotMatch)
                ),
                "the right code is too late"
            );
            let sent = sign_in(&server, &mailer, "alice@example.com")
                .await
                .expect("signed in again");
            let code = code_sent_to(&mailer, "alice@example.com");
            let step = sign_in_with_code(&server, &sent.pending, &code)
                .await
                .expect("a fresh code works");
            next_of(step, NextStep::NewPassword);
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_new_code_replaces_the_old_one_for_the_same_pending_sign_in() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let sent = sign_up(&server, &mailer, "alice@example.com")
                .await
                .expect("signed up");
            let first = code_sent_to(&mailer, "alice@example.com");
            resend_code(&server, &mailer, &sent.pending)
                .await
                .expect("a new code sent");
            let second = code_sent_to(&mailer, "alice@example.com");
            assert_eq!(mailer.sent().len(), 2);
            assert_eq!(
                resend_code_to(&server, &sent.pending)
                    .await
                    .expect("the address"),
                "alice@example.com"
            );
            if first != second {
                assert!(
                    matches!(
                        sign_in_with_code(&server, &sent.pending, &first).await,
                        Err(AccountError::CodeDoesNotMatch)
                    ),
                    "the old code is no longer good"
                );
            }
            sign_in_with_code(&server, &sent.pending, &second)
                .await
                .expect("the new one is");
            assert!(
                matches!(
                    resend_code(&server, &mailer, &sent.pending).await,
                    Err(AccountError::CodeDoesNotMatch)
                ),
                "nothing pending any more"
            );
            assert!(matches!(
                resend_code(&server, &mailer, "not a pending token").await,
                Err(AccountError::CodeDoesNotMatch)
            ));
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_device_code_needs_the_verifier_and_works_once() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let sent = sign_up(&server, &mailer, "alice@example.com")
                .await
                .expect("signed up");
            let code = code_sent_to(&mailer, "alice@example.com");
            let (verifier, challenge) = verifier_and_challenge();
            assert_eq!(challenge, challenge_of(&verifier));
            let step = sign_in_with_code_for_device(&server, &sent.pending, &code, &challenge)
                .await
                .expect("the code matched");
            let pending = next_of(step, NextStep::NewPassword);
            let device_code =
                sign_in_with_new_password_for_device(&server, &pending, None, &challenge)
                    .await
                    .expect("skipped making a password");
            assert_eq!(device_code.code.len(), 32);

            let error = sign_in_with_device_code(&server, &device_code.code, "another verifier")
                .await
                .expect_err("not without the verifier");
            assert!(matches!(error, AccountError::UnknownDeviceCode));
            let session = sign_in_with_device_code(&server, &device_code.code, &verifier)
                .await
                .expect("exchanged");
            assert_eq!(session.email, "alice@example.com");
            assert_eq!(
                users::get_by_token(&server, &session.token)
                    .await
                    .expect("an API token"),
                session.user
            );
            let error = sign_in_with_device_code(&server, &device_code.code, &verifier)
                .await
                .expect_err("once");
            assert!(matches!(error, AccountError::UnknownDeviceCode));
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_are_the_servers_alone() {
        let device = users::tests::device().await;
        let mailer = Mailer::memory();
        assert!(matches!(
            sign_up(&device, &mailer, "alice@example.com").await,
            Err(AccountError::ServerOnly)
        ));
        assert!(matches!(
            sign_in(&device, &mailer, "alice@example.com").await,
            Err(AccountError::ServerOnly)
        ));
        assert!(matches!(
            sign_in_with_code(&device, "pending", "123456").await,
            Err(AccountError::ServerOnly)
        ));
        assert!(matches!(
            sign_in_with_device_code(&device, "code", "verifier").await,
            Err(AccountError::ServerOnly)
        ));
        assert!(matches!(
            sign_out(&device, "a token").await,
            Err(AccountError::ServerOnly)
        ));
    }

    #[tokio::test]
    async fn accounts_a_device_signs_in_syncs_signs_out_and_resumes_on_the_next_sign_in() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let transport = InProcess { server: &server };
            let device = users::tests::device().await;
            assert!(matches!(
                sign_in_with_device_code_through(&transport, "no code", "no verifier").await,
                Err(AccountError::UnknownDeviceCode)
            ));

            let session = signed_up_for_a_device(&server, "alice@example.com").await;
            let alice = keep_signed_in(&device, session).await.expect("kept");
            assert_eq!(users::signed_in(&device).await.expect("alice"), alice);

            let post = posts::create(&device, &alice, "from the device", &[])
                .await
                .expect("a post");
            sync::run(&device, &transport).await.expect("synced");
            let cursor = sync::status(&device).await.expect("status").cursor;
            assert!(cursor.is_some(), "the first pull set the cursor");
            assert_eq!(
                posts::get(&server, &alice, post.id)
                    .await
                    .expect("on the server")
                    .post
                    .body,
                "from the device"
            );

            let (_, token) = users::signed_in_with_token(&device)
                .await
                .expect("the token");
            sign_out_on_device(&device, &transport)
                .await
                .expect("signed out");
            assert!(matches!(
                users::signed_in(&device).await,
                Err(UserError::NotSignedIn)
            ));
            assert!(matches!(
                users::get_by_token(&server, &token).await,
                Err(UserError::UnknownToken)
            ));
            let mut tx = device.begin().await.expect("a transaction");
            assert!(
                posts::get_in(&mut tx, post.id)
                    .await
                    .expect("looked")
                    .is_some(),
                "the device's copy stays"
            );
            tx.commit().await.expect("committed");
            assert!(
                sync::run(&device, &transport)
                    .await
                    .expect("ran")
                    .signed_out,
                "and nothing syncs meanwhile"
            );

            let (device_code, verifier) =
                signed_in_for_a_device(&server, "alice@example.com").await;
            let session =
                sign_in_with_device_code_through(&transport, &device_code.code, &verifier)
                    .await
                    .expect("exchanged through the transport");
            let again = keep_signed_in(&device, session).await.expect("kept again");
            assert_eq!(again, alice);
            assert_eq!(
                sync::status(&device).await.expect("status").cursor,
                cursor,
                "the cursor was kept through the sign-out, so the sync resumes"
            );
            let report = sync::run(&device, &transport).await.expect("synced");
            assert!(!report.resynced);
            assert_eq!(
                posts::list(&device, &alice).await.expect("her posts").len(),
                1
            );
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_sign_out_keeps_a_pending_change_for_the_next_sign_in_to_push() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let transport = InProcess { server: &server };
            let device = users::tests::device().await;
            let session = signed_up_for_a_device(&server, "alice@example.com").await;
            let alice = keep_signed_in(&device, session).await.expect("kept");
            let post = posts::create(&device, &alice, "written, then signed out", &[])
                .await
                .expect("a post");
            sign_out_on_device(&device, &transport)
                .await
                .expect("signed out");
            assert!(
                posts::get(&server, &alice, post.id).await.is_err(),
                "not on the server yet"
            );
            assert_eq!(sync::status(&device).await.expect("status").pending, 1);

            let (device_code, verifier) =
                signed_in_for_a_device(&server, "alice@example.com").await;
            let session =
                sign_in_with_device_code_through(&transport, &device_code.code, &verifier)
                    .await
                    .expect("exchanged");
            keep_signed_in(&device, session)
                .await
                .expect("signed in again");
            let report = sync::run(&device, &transport).await.expect("synced");
            assert_eq!(report.pushed, 1);
            assert!(posts::get(&server, &alice, post.id).await.is_ok());
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_401_is_a_sign_out_done_elsewhere_and_the_data_stays() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let transport = InProcess { server: &server };
            let session = signed_up_for_a_device(&server, "alice@example.com").await;
            let device = users::tests::device().await;
            assert!(matches!(
                users::signed_in(&device).await,
                Err(UserError::NotSignedIn)
            ));
            let alice = keep_signed_in(&device, session.clone())
                .await
                .expect("kept");
            assert_eq!(users::signed_in(&device).await.expect("alice"), alice);
            posts::create(&device, &alice, "kept through it", &[])
                .await
                .expect("a post");

            sign_out(&server, &session.token)
                .await
                .expect("the token deleted elsewhere");
            let error = sync::run(&device, &transport)
                .await
                .expect_err("the server no longer knows the token");
            assert!(matches!(error, SyncError::Unauthorized));
            forget_signed_in(&device).await.expect("forgotten");
            assert!(matches!(
                users::signed_in(&device).await,
                Err(UserError::NotSignedIn)
            ));
            assert_eq!(sync::status(&device).await.expect("status").pending, 1);
        })
        .await;
    }
}
