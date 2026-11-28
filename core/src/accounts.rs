//! Accounts: signing up, signing in and signing out, one flow for both.
//! The server does the work, and the emailed code is what proves who
//! someone is, see `docs/design/accounts-sign-in.md`. A sign-in,
//! `sign_in`, finds the account by its email, or, for an email with
//! none, writes the person's account, `users`, unconfirmed, and their
//! user's row with their name, `crate::app::users`; then it makes a
//! pending sign-in, `tokens`, and mails its six-digit code, `mails`,
//! answering the pending token the page holds. The code,
//! `sign_in_with_code`, answers whether the account is new and the last
//! step's pending token; that step, `sign_in_confirm`, confirms a new
//! account's email and ends in `finish_in`. A password is a way in of
//! its own, `sign_in_with_password`, for an account that made one while
//! signed in, `password`, and ends there too: a session token for the
//! browser's cookie, or, for a
//! sign-in a device started in the browser, a device code the device
//! exchanges for its API token, `sign_in_with_device_code`, with the
//! verifier only it holds. A sign-out deletes the token. A trial user,
//! `trial_users`, signs up with `sign_in_from_trial`, which keeps their
//! user and gives their account the address once they finish.
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
pub mod password;
#[cfg(feature = "stripe")]
pub mod plans;
pub mod preferences;
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
use crate::app::users::{SignedIn, UserError};
use crate::data::accounts_credentials::{self, CredentialError};
use crate::data::accounts_tokens::{self, Context};
use crate::data::app_users::User;
use crate::data::device_settings::{self, DeviceSettingsError, SIGNED_IN};
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::helpers::random;
use crate::services::mailer::mails;
use crate::services::mailer::{MailError, Mailer};
use crate::services::web_server::Transport;
use crate::sync::SyncError;

use crate::data::accounts_users::{self, AccountUser, AccountUserError};

/// Wrong codes a pending sign-in takes before it is deleted.
pub const MAX_CODE_ATTEMPTS: i64 = 5;

/// The version of the terms of service and privacy policy the sign-in's
/// last step asks everyone to accept: their effective date, on
/// `svelte/src/public/terms.html` and `privacy.html`, changed with them.
/// Each account records the version it last accepted, and when.
pub const TERMS_VERSION: &str = "2026-11-22";

/// The name of the community a person who signs up starts with: every
/// new user has one of their own, so an app may ask for a current
/// community everywhere. An ordinary community, theirs to rename or
/// delete.
pub const SIGN_UP_COMMUNITY_NAME: &str = "Personal";

/// What the emailed code answers once it matched: the pending token
/// of the last step, `sign_in_confirm`, and what the page shows for it.
/// Nobody is signed in, and no account made, until that step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeMatched {
    pub pending: String,
    /// Whether finishing makes the account: "Create account", not
    /// "Log in".
    pub new_account: bool,
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

/// What a sign-in answers: the pending token the page sends back with
/// the code that was emailed.
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
    #[error("That email and password don't match")]
    EmailOrPasswordDoesNotMatch,
    #[error("That sign-in has expired; sign in again")]
    SignInExpired,
    #[error("That code is wrong or has expired; try again, or send a new code")]
    CodeDoesNotMatch,
    #[error("That code is wrong or has expired; sign in again")]
    UnknownDeviceCode,
    #[error("Only a trial signs up this way")]
    NotATrial,
    #[error("Trials are full right now; sign up instead")]
    TrialsFull,
    #[error("no account has the email {email}")]
    NoAccount { email: String },
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
    /// The remote backend failed, or answered what this error has no
    /// variant for.
    #[cfg(feature = "remote-backend")]
    #[error(transparent)]
    Backend(#[from] crate::services::backend::BackendError),
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

/// Mails the person a code, on the server, whether or not they have an
/// account: one sign-in for both. An email with an account, confirmed
/// or not, gets a code for it. An email with none, when the server
/// takes sign-ups, is registered, unconfirmed and named from the email,
/// `users::name_from_email_in`, and gets a code; the account is
/// confirmed when the person finishes, `sign_in_confirm`. Every answer
/// looks the same, so it tells nobody whether an email has an account:
/// an email with none on a server that takes no sign-ups is emailed
/// so, and a trial user's made-up address is answered and mailed
/// nothing. The mail is in the account's language, else `requested`,
/// the request's, see `preferences::mail_locale_in`.
pub async fn sign_in(
    deployment: &Deployment,
    mailer: &Mailer,
    email: &str,
    sign_up_enabled: bool,
    requested: &str,
) -> Result<CodeSent, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let email = accounts_users::checked_email(email)?;
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        return from_backend::sign_in(backend, mailer, &email, sign_up_enabled, requested).await;
    }
    let mut tx = deployment.begin().await?;
    let (mail, pending) = match accounts_users::get_by_email(&mut tx, &email).await? {
        // A trial user's address is made up: nothing to send, nobody to tell
        Some(account) if account.trial_expires_at.is_some() => (None, random::hex(32)),
        Some(account) => {
            let (pending, code) =
                accounts_tokens::create_sign_in_code(&mut tx, account.id, None).await?;
            let locale = preferences::mail_locale_in(&mut tx, Some(account.id), requested).await?;
            (
                Some(mails::sign_in_code(locale, &account.email, &code)),
                pending,
            )
        }
        None if sign_up_enabled => {
            // Only a sign-up writes a synced row: its user
            crate::data::app_sync_versions::lock(&mut tx).await?;
            let account = accounts_users::create(&mut tx, Uuid::now_v7(), &email, false).await?;
            let name = crate::app::users::name_from_email_in(&mut tx, &email, account.id).await?;
            crate::app::users::create_in(&mut tx, account.id, &name).await?;
            let (pending, code) =
                accounts_tokens::create_sign_in_code(&mut tx, account.id, None).await?;
            let locale = preferences::mail_locale_in(&mut tx, None, requested).await?;
            (
                Some(mails::sign_in_code(locale, &account.email, &code)),
                pending,
            )
        }
        None => {
            let locale = preferences::mail_locale_in(&mut tx, None, requested).await?;
            (Some(mails::no_account(locale, &email)), random::hex(32))
        }
    };
    tx.commit().await?;
    if let Some(mail) = mail {
        mailer.send(mail).await?;
    }
    Ok(CodeSent { pending })
}

/// `sign_in` from a trial user's browser, on a server that takes
/// sign-ups. An address with a confirmed account gets a code for that
/// account, and the trial is left as it is. Any other address is the
/// trial user signing up: a code is mailed to it, and when they finish,
/// `sign_in_confirm`, their account takes that address and is a trial no
/// longer; the user and everything they made stay as they are. Their
/// made-up name gives way at once to one from the address,
/// `users::name_from_email_in`. The mail is in the account's language,
/// else the trial's, else `requested`, the request's.
pub async fn sign_in_from_trial(
    deployment: &Deployment,
    mailer: &Mailer,
    trial: &User,
    email: &str,
    requested: &str,
) -> Result<CodeSent, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let email = accounts_users::checked_email(email)?;
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        return from_backend::sign_in_from_trial(backend, mailer, trial, &email, requested).await;
    }
    let mut tx = deployment.begin().await?;
    let account = live_account_in(&mut tx, trial.id).await?;
    if account.trial_expires_at.is_none() {
        return Err(AccountError::NotATrial);
    }
    let (mail, pending) = match accounts_users::get_by_email(&mut tx, &email).await? {
        Some(taken) if taken.trial_expires_at.is_some() => (None, random::hex(32)),
        Some(taken) if taken.confirmed_at.is_some() => {
            let (pending, code) =
                accounts_tokens::create_sign_in_code(&mut tx, taken.id, None).await?;
            let locale = preferences::mail_locale_in(&mut tx, Some(taken.id), requested).await?;
            (
                Some(mails::sign_in_code(locale, &taken.email, &code)),
                pending,
            )
        }
        _ => {
            let name = crate::app::users::name_from_email_in(&mut tx, &email, trial.id).await?;
            crate::app::users::update_name_in(&mut tx, trial.id, &name).await?;
            let (pending, code) =
                accounts_tokens::create_sign_in_code(&mut tx, trial.id, Some(&email)).await?;
            let locale = preferences::mail_locale_in(&mut tx, Some(trial.id), requested).await?;
            (Some(mails::sign_in_code(locale, &email, &code)), pending)
        }
    };
    tx.commit().await?;
    if let Some(mail) = mail {
        mailer.send(mail).await?;
    }
    Ok(CodeSent { pending })
}

/// Mails the pending sign-in a new code, on the server, its ten minutes
/// starting over and its wrong attempts forgotten; the old code is no
/// longer good. For the "Send a new code" button; how often is the web
/// server's limit, see `docs/design/accounts-sign-in.md`. The mail is in
/// the account's language, else `requested`, the request's.
pub async fn resend_code(
    deployment: &Deployment,
    mailer: &Mailer,
    pending: &str,
    requested: &str,
) -> Result<(), AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        return from_backend::resend_code(backend, mailer, pending, requested).await;
    }
    let mut tx = deployment.begin().await?;
    let row = accounts_tokens::get_valid(
        &mut tx,
        &accounts_tokens::hash(pending),
        Context::SignInCode,
        now(),
    )
    .await?
    .ok_or(AccountError::CodeDoesNotMatch)?;
    let account = live_account_in(&mut tx, row.user_id).await?;
    let code = accounts_tokens::reset_code(&mut tx, row.id).await?;
    let locale = preferences::mail_locale_in(&mut tx, Some(row.user_id), requested).await?;
    tx.commit().await?;
    let sent_to = row.sent_to.unwrap_or(account.email);
    mailer
        .send(mails::sign_in_code(locale, &sent_to, &code))
        .await?;
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
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        return from_backend::resend_code_to(backend, pending).await;
    }
    let mut tx = deployment.begin().await?;
    let row = accounts_tokens::get_valid(
        &mut tx,
        &accounts_tokens::hash(pending),
        Context::SignInCode,
        now(),
    )
    .await?
    .ok_or(AccountError::CodeDoesNotMatch)?;
    let account = live_account_in(&mut tx, row.user_id).await?;
    tx.commit().await?;
    Ok(row.sent_to.unwrap_or(account.email))
}

/// Takes the code that was emailed for the pending sign-in, on the
/// server: the last step's pending token, and whether that step makes
/// the account and asks for its password. A wrong code counts; after
/// `MAX_CODE_ATTEMPTS` the pending sign-in is gone.
pub async fn sign_in_with_code(
    deployment: &Deployment,
    pending: &str,
    code: &str,
) -> Result<CodeMatched, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        return from_backend::sign_in_with_code(backend, pending, code).await;
    }
    let mut tx = deployment.begin().await?;
    let matched = match_code_in(&mut tx, pending, code).await;
    tx.commit().await?;
    matched
}

/// Finishes a sign-in whose code matched, on the server: the account
/// is made if it is new, and the token for the browser's cookie
/// answered.
pub async fn sign_in_confirm(
    deployment: &Deployment,
    pending: &str,
) -> Result<Session, AccountError> {
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        return from_backend::sign_in_confirm(backend, pending).await;
    }
    confirm(deployment, pending, Finish::Session).await
}

/// Finishes it for the device whose challenge it is: the device code it
/// collects, and no token for the browser.
pub async fn sign_in_confirm_for_device(
    deployment: &Deployment,
    pending: &str,
    challenge: &str,
) -> Result<DeviceCode, AccountError> {
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        return from_backend::sign_in_confirm_for_device(backend, pending, challenge).await;
    }
    confirm(deployment, pending, Finish::Device { challenge }).await
}

/// Signs in with the account's password, on the server, in place of an
/// emailed code. Every refusal is the same, after the same work: no
/// account, an unconfirmed or trial one, one with no password, or a
/// wrong password, so the answer tells nobody whether an email has an
/// account. How often an address may try is the web server's limit,
/// see `docs/design/accounts-sign-in.md`.
pub async fn sign_in_with_password(
    deployment: &Deployment,
    email: &str,
    password: &str,
) -> Result<Session, AccountError> {
    with_password(deployment, email, password, Finish::Session).await
}

/// Signs in with the password for the device whose challenge it is.
pub async fn sign_in_with_password_for_device(
    deployment: &Deployment,
    email: &str,
    password: &str,
    challenge: &str,
) -> Result<DeviceCode, AccountError> {
    with_password(deployment, email, password, Finish::Device { challenge }).await
}

/// Exchanges the device code, with the verifier the device kept, for
/// the device's session, once: the row is found by the code and the
/// verifier's hash together, see `accounts_tokens::create_device_code`, and
/// deleted.
pub async fn sign_in_with_device_code(
    deployment: &Deployment,
    code: &str,
    verifier: &str,
) -> Result<Session, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        return from_backend::sign_in_with_device_code(backend, code, &challenge_of(verifier))
            .await;
    }
    let mut tx = deployment.begin().await?;
    let token_hash = accounts_tokens::device_code_hash(code, &challenge_of(verifier));
    let row = accounts_tokens::get_valid(&mut tx, &token_hash, Context::DeviceCode, now())
        .await?
        .ok_or(AccountError::UnknownDeviceCode)?;
    accounts_tokens::delete_by_id(&mut tx, row.id).await?;
    let account = accounts_users::get(&mut tx, row.user_id)
        .await?
        .ok_or(AccountError::UnknownDeviceCode)?;
    let user = live_user_in(&mut tx, row.user_id)
        .await?
        .ok_or(AccountError::UnknownDeviceCode)?;
    let token = accounts_tokens::create(&mut tx, user.id, Context::Api).await?;
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
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        return from_backend::sign_out(backend, token).await;
    }
    let mut tx = deployment.begin().await?;
    accounts_tokens::delete(&mut tx, token).await?;
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
    accounts_tokens::hash(verifier)
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
/// server no longer knows is already forgotten. A server that cannot be
/// told, offline say, keeps the token until it expires, and the device
/// signs out all the same: it holds the only copy.
pub async fn sign_out_on_device<T: Transport>(
    deployment: &Deployment,
    transport: &T,
) -> Result<(), AccountError> {
    if deployment.role != Role::Device {
        return Err(AccountError::DeviceOnly);
    }
    let (user, token) = crate::app::users::signed_in_with_token(deployment).await?;
    match transport.sign_out(&token).await {
        Ok(()) | Err(SyncError::Unauthorized) | Err(SyncError::AccountDeleted) => {}
        Err(error) => tracing::warn!(
            "signed {} out on the device only; the server was not told: {error:?}",
            user.id
        ),
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

/// The last step of a code's sign-in: the pending step taken, the
/// account made if it is new, and the sign-in finished.
async fn confirm<Finished: FromFinish>(
    deployment: &Deployment,
    pending: &str,
    finish: Finish<'_>,
) -> Result<Finished, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let row = accounts_tokens::get_valid(
        &mut tx,
        &accounts_tokens::hash(pending),
        Context::SignInConfirm,
        now(),
    )
    .await?
    .ok_or(AccountError::SignInExpired)?;
    crate::data::app_sync_versions::lock(&mut tx).await?;
    accounts_tokens::delete_by_id(&mut tx, row.id).await?;
    let account = open_account_in(&mut tx, &row).await?;
    let finished = finish_proven_in(&mut tx, &account, finish).await?;
    tx.commit().await?;
    Ok(finished)
}

/// A sign-in with the password of a confirmed account: the same refusal
/// for every way it fails, after the password's work either way.
async fn with_password<Finished: FromFinish>(
    deployment: &Deployment,
    email: &str,
    password: &str,
    finish: Finish<'_>,
) -> Result<Finished, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let email = accounts_users::checked_email(email)?;
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        return from_backend::with_password(backend, &email, password, finish).await;
    }
    let mut tx = deployment.begin().await?;
    let account = accounts_users::get_by_email(&mut tx, &email)
        .await?
        .filter(|account| account.confirmed_at.is_some() && account.trial_expires_at.is_none());
    let matches = accounts_credentials::verify(
        &mut tx,
        account.as_ref().map(|account| account.id),
        password,
    )
    .await?;
    let account = match account {
        Some(account) if matches => account,
        _ => return Err(AccountError::EmailOrPasswordDoesNotMatch),
    };
    let finished = finish_proven_in(&mut tx, &account, finish).await?;
    tx.commit().await?;
    Ok(finished)
}

/// Where a proven account ends: the terms it was shown recorded as
/// accepted, and the sign-in finished for its user.
async fn finish_proven_in<Finished: FromFinish>(
    tx: &mut Transaction,
    account: &AccountUser,
    finish: Finish<'_>,
) -> Result<Finished, AccountError> {
    accounts_users::accept_terms(tx, account.id, TERMS_VERSION).await?;
    let user = live_user_in(tx, account.id)
        .await?
        .ok_or(AccountError::SignInExpired)?;
    let secret = finish_in(tx, &user, finish).await?;
    Ok(Finished::from_finish(user, account.email.clone(), secret))
}

/// What `finish_in` answered, as the caller's kind of finished sign-in,
/// which a remote backend answers too.
trait FromFinish: serde::de::DeserializeOwned {
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

/// What the pending sign-in's code answers once it matches: the
/// pending sign-in is deleted for the last step's, which keeps the
/// address a trial user's code went to. A wrong code is counted, and
/// the one that reaches `MAX_CODE_ATTEMPTS` deletes the pending sign-in.
async fn match_code_in(
    tx: &mut Transaction,
    pending: &str,
    code: &str,
) -> Result<CodeMatched, AccountError> {
    let row = accounts_tokens::get_valid(
        tx,
        &accounts_tokens::hash(pending),
        Context::SignInCode,
        now(),
    )
    .await?
    .ok_or(AccountError::CodeDoesNotMatch)?;
    if row.code_hash.as_deref() != Some(accounts_tokens::hash(code.trim()).as_str()) {
        count_attempt_in(tx, row.id).await?;
        return Err(AccountError::CodeDoesNotMatch);
    }
    accounts_tokens::delete_by_id(tx, row.id).await?;
    let account = live_account_in(tx, row.user_id).await?;
    let pending =
        accounts_tokens::create_sign_in_confirm(tx, account.id, row.sent_to.as_deref()).await?;
    Ok(CodeMatched {
        pending,
        new_account: row.sent_to.is_some() || account.confirmed_at.is_none(),
    })
}

/// The account made, if the sign-in is its first: a trial user's
/// account as their own when the code went to a new address, or a new
/// account's email confirmed and its first community made.
async fn open_account_in(
    tx: &mut Transaction,
    row: &accounts_tokens::Token,
) -> Result<AccountUser, AccountError> {
    let account = live_account_in(tx, row.user_id).await?;
    match row.sent_to.as_deref() {
        Some(email) => sign_up_trial_in(tx, account, email).await,
        None if account.confirmed_at.is_none() => {
            accounts_users::confirm(tx, account.id).await?;
            let user = live_user_in(tx, account.id)
                .await?
                .ok_or(AccountError::SignInExpired)?;
            communities::create_in(tx, Role::Server, &user, SIGN_UP_COMMUNITY_NAME).await?;
            Ok(account)
        }
        None => Ok(account),
    }
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
    if let Some(other) = accounts_users::get_by_email(tx, email).await? {
        if other.confirmed_at.is_some() {
            return Err(AccountError::CodeDoesNotMatch);
        }
        accounts_users::delete_unconfirmed(tx, other.id).await?;
    }
    accounts_users::sign_up_trial(tx, account.id, email).await?;
    crate::accounts::trial_users::show_communities_in(tx, account.id).await?;
    accounts_tokens::delete_of_user(tx, account.id, Context::Session).await?;
    live_account_in(tx, account.id).await
}

/// Counts a wrong attempt against the pending token; the one that
/// reaches `MAX_CODE_ATTEMPTS` deletes it.
async fn count_attempt_in(tx: &mut Transaction, id: Uuid) -> Result<(), AccountError> {
    if accounts_tokens::count_attempt(tx, id).await? >= MAX_CODE_ATTEMPTS {
        accounts_tokens::delete_by_id(tx, id).await?;
    }
    Ok(())
}

/// The sign-in's steps answered by the remote backend, see
/// `docs/design/backend-protocol.md`. Rust sends the mail the backend
/// answers, as the Rust server does after its transaction.
#[cfg(feature = "remote-backend")]
mod from_backend {
    use serde_json::json;

    use super::*;
    use crate::services::backend::{BackendError, RemoteBackend};
    use crate::services::mailer::Mail;

    /// The mail a sign-in answers for Rust to send.
    #[derive(Deserialize)]
    struct MailToSend {
        kind: String,
        to: String,
        locale: String,
        code: Option<String>,
    }

    #[derive(Deserialize)]
    struct Sent {
        pending: String,
        mail: Option<MailToSend>,
    }

    pub(super) async fn sign_in(
        backend: &RemoteBackend,
        mailer: &Mailer,
        email: &str,
        sign_up_enabled: bool,
        requested: &str,
    ) -> Result<CodeSent, AccountError> {
        let request = json!({
            "email": email,
            "sign_up_enabled": sign_up_enabled,
            "locale": requested,
        });
        let sent: Sent = backend
            .call("accounts/sign_in", &request)
            .await
            .map_err(account_error)?;
        if let Some(mail) = sent.mail {
            mailer.send(mail_of(mail)?).await?;
        }
        Ok(CodeSent {
            pending: sent.pending,
        })
    }

    pub(super) async fn sign_in_from_trial(
        backend: &RemoteBackend,
        mailer: &Mailer,
        trial: &User,
        email: &str,
        requested: &str,
    ) -> Result<CodeSent, AccountError> {
        let request = json!({ "trial_id": trial.id, "email": email, "locale": requested });
        let sent: Sent = backend
            .call("accounts/sign_in_from_trial", &request)
            .await
            .map_err(account_error)?;
        if let Some(mail) = sent.mail {
            mailer.send(mail_of(mail)?).await?;
        }
        Ok(CodeSent {
            pending: sent.pending,
        })
    }

    pub(super) async fn sign_in_with_code(
        backend: &RemoteBackend,
        pending: &str,
        code: &str,
    ) -> Result<CodeMatched, AccountError> {
        let request = json!({ "pending": pending, "code": code });
        backend
            .call("accounts/sign_in_with_code", &request)
            .await
            .map_err(account_error)
    }

    /// The address the pending sign-in's code went to.
    pub(super) async fn resend_code_to(
        backend: &RemoteBackend,
        pending: &str,
    ) -> Result<String, AccountError> {
        #[derive(Deserialize)]
        struct SentTo {
            email: String,
        }
        let sent_to: SentTo = backend
            .call("accounts/resend_code_to", &json!({ "pending": pending }))
            .await
            .map_err(account_error)?;
        Ok(sent_to.email)
    }

    pub(super) async fn resend_code(
        backend: &RemoteBackend,
        mailer: &Mailer,
        pending: &str,
        requested: &str,
    ) -> Result<(), AccountError> {
        #[derive(Deserialize)]
        struct Resent {
            mail: MailToSend,
        }
        let request = json!({ "pending": pending, "locale": requested });
        let resent: Resent = backend
            .call("accounts/resend_code", &request)
            .await
            .map_err(account_error)?;
        mailer.send(mail_of(resent.mail)?).await?;
        Ok(())
    }

    /// The last step records the terms the person accepted: the version
    /// Rust shows, `TERMS_VERSION`.
    pub(super) async fn sign_in_confirm(
        backend: &RemoteBackend,
        pending: &str,
    ) -> Result<Session, AccountError> {
        let request = json!({ "pending": pending, "terms_version": TERMS_VERSION });
        backend
            .call("accounts/sign_in_confirm", &request)
            .await
            .map_err(account_error)
    }

    pub(super) async fn sign_in_confirm_for_device(
        backend: &RemoteBackend,
        pending: &str,
        challenge: &str,
    ) -> Result<DeviceCode, AccountError> {
        let request = json!({
            "pending": pending,
            "challenge": challenge,
            "terms_version": TERMS_VERSION,
        });
        backend
            .call("accounts/sign_in_confirm_for_device", &request)
            .await
            .map_err(account_error)
    }

    /// `challenge` is the device's verifier turned into its challenge, as
    /// Rust checks a device code.
    pub(super) async fn sign_in_with_device_code(
        backend: &RemoteBackend,
        code: &str,
        challenge: &str,
    ) -> Result<Session, AccountError> {
        let request = json!({ "code": code, "challenge": challenge });
        backend
            .call("accounts/sign_in_with_device_code", &request)
            .await
            .map_err(account_error)
    }

    /// A sign-in with a password: the backend keeps the hash, and Rust
    /// checks the password against it, as `with_password` does, so every
    /// refusal is the same after the same work. Then the backend finishes
    /// the sign-in.
    pub(super) async fn with_password<Finished: FromFinish>(
        backend: &RemoteBackend,
        email: &str,
        password: &str,
        finish: Finish<'_>,
    ) -> Result<Finished, AccountError> {
        #[derive(Deserialize)]
        struct Hash {
            user_id: Option<Uuid>,
            password_hash: Option<String>,
        }
        let hash: Hash = backend
            .call("accounts/password/get_hash", &json!({ "email": email }))
            .await
            .map_err(account_error)?;
        let matches = accounts_credentials::verify_hash(hash.password_hash, password).await?;
        let user_id = match hash.user_id {
            Some(user_id) if matches => user_id,
            _ => return Err(AccountError::EmailOrPasswordDoesNotMatch),
        };
        let (action, request) = match finish {
            Finish::Session => (
                "accounts/sign_in_with_password",
                json!({ "user_id": user_id, "terms_version": TERMS_VERSION }),
            ),
            Finish::Device { challenge } => (
                "accounts/sign_in_with_password_for_device",
                json!({ "user_id": user_id, "challenge": challenge, "terms_version": TERMS_VERSION }),
            ),
        };
        backend.call(action, &request).await.map_err(account_error)
    }

    /// Rust hashes the password and the backend keeps the hash; Rust
    /// mails the account that it changed.
    pub(in crate::accounts) async fn password_update(
        backend: &RemoteBackend,
        mailer: &Mailer,
        user: &User,
        password: &str,
        requested: &str,
    ) -> Result<(), AccountError> {
        #[derive(Deserialize)]
        struct Updated {
            mail: MailToSend,
        }
        let password_hash = accounts_credentials::checked_hash(password).await?;
        let request = json!({
            "user_id": user.id,
            "password_hash": password_hash,
            "locale": requested,
        });
        let updated: Updated = backend
            .call("accounts/password/update", &request)
            .await
            .map_err(account_error)?;
        mailer.send(mail_of(updated.mail)?).await?;
        Ok(())
    }

    pub(super) async fn sign_out(backend: &RemoteBackend, token: &str) -> Result<(), AccountError> {
        let _: serde_json::Value = backend
            .call("accounts/sign_out", &json!({ "token": token }))
            .await
            .map_err(account_error)?;
        Ok(())
    }

    /// The mail in the language the backend chose, if core has it.
    fn mail_of(mail: MailToSend) -> Result<Mail, AccountError> {
        let locale = preferences::supported_locale(&mail.locale);
        match (mail.kind.as_str(), mail.code) {
            ("sign_in_code", Some(code)) => Ok(mails::sign_in_code(locale, &mail.to, &code)),
            ("no_account", _) => Ok(mails::no_account(locale, &mail.to)),
            ("password_changed", _) => Ok(mails::password_changed(locale, &mail.to)),
            (kind, _) => Err(BackendError::Malformed(format!("a mail of kind {kind}")).into()),
        }
    }

    /// The account's error for each code the protocol gives a sign-in.
    pub(in crate::accounts) fn account_error(error: BackendError) -> AccountError {
        match error.code() {
            Some("wrong_code") => AccountError::CodeDoesNotMatch,
            Some("sign_in_expired") => AccountError::SignInExpired,
            Some("unknown_device_code") => AccountError::UnknownDeviceCode,
            Some("not_a_trial") => AccountError::NotATrial,
            Some("account_deleted") => AccountError::User(UserError::AccountDeleted),
            _ => error.into(),
        }
    }
}

/// Where every proven user ends: the browser's token, or the device
/// code for the device whose challenge was given.
async fn finish_in(
    tx: &mut Transaction,
    user: &User,
    finish: Finish<'_>,
) -> Result<String, AccountError> {
    Ok(match finish {
        Finish::Session => accounts_tokens::create(tx, user.id, Context::Session).await?,
        Finish::Device { challenge } => {
            accounts_tokens::create_device_code(tx, user.id, challenge).await?
        }
    })
}

async fn live_account_in(tx: &mut Transaction, id: Uuid) -> Result<AccountUser, AccountError> {
    accounts_users::get(tx, id)
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
    use crate::accounts::password::{self, PasswordUpdate};
    use crate::app::users;
    use crate::database::with_each_database;
    use crate::services::web_server::InProcess;
    use crate::sync;
    use crate::sync::test_notes;

    pub(crate) const PASSWORD: &str = "correct horse battery";

    /// The code the last email to the address carried.
    pub(crate) fn code_sent_to(mailer: &Mailer, email: &str) -> String {
        let mail = mailer.last_sent_to(email).expect("an email");
        mails::code_in(&mail).expect("a code in it")
    }

    /// A sign-in of the email on a server that takes sign-ups, its code
    /// matched: the last step still to take.
    async fn code_matched(server: &Deployment, mailer: &Mailer, email: &str) -> CodeMatched {
        let sent = sign_in(
            server,
            mailer,
            email,
            true,
            crate::helpers::locales::DEFAULT,
        )
        .await
        .expect("signed in");
        let code = code_sent_to(mailer, email);
        sign_in_with_code(server, &sent.pending, &code)
            .await
            .expect("the code matched")
    }

    /// A user signed up on the server with no password, their email
    /// confirmed: the browser's session.
    async fn signed_up(server: &Deployment, mailer: &Mailer, email: &str) -> Session {
        let matched = code_matched(server, mailer, email).await;
        assert!(matched.new_account);
        sign_in_confirm(server, &matched.pending)
            .await
            .expect("signed up")
    }

    /// A sign-in of an account with no password, as a device: the
    /// device code and the verifier to exchange it with.
    async fn signed_in_for_a_device(server: &Deployment, email: &str) -> (DeviceCode, String) {
        let mailer = Mailer::memory();
        let matched = code_matched(server, &mailer, email).await;
        let (verifier, challenge) = verifier_and_challenge();
        let device_code = sign_in_confirm_for_device(server, &matched.pending, &challenge)
            .await
            .expect("finished");
        (device_code, verifier)
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
    async fn accounts_sign_in_of_a_new_email_makes_the_account_at_the_last_step_then_signs_in_and_out()
     {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let sent = sign_in(
                &server,
                &mailer,
                " Alice@Example.com ",
                true,
                crate::helpers::locales::DEFAULT,
            )
            .await
            .expect("signed in");
            assert_eq!(sent.pending.len(), 64);
            let mail = mailer.last_sent_to("alice@example.com").expect("mailed");
            let code = mails::code_in(&mail).expect("a code");
            assert!(mail.subject.contains("code"));

            let error = sign_in_with_code(&server, &sent.pending, "000000")
                .await
                .expect_err("wrong, unless the code is 000000");
            assert!(matches!(error, AccountError::CodeDoesNotMatch) || code == "000000");

            let matched = sign_in_with_code(&server, &sent.pending, &format!(" {code} "))
                .await
                .expect("the code matched, trimmed");
            assert!(matched.new_account);
            assert!(
                matches!(
                    sign_in_with_code(&server, &sent.pending, &code).await,
                    Err(AccountError::CodeDoesNotMatch)
                ),
                "a code is used once"
            );
            let mut tx = server.begin().await.expect("a transaction");
            let pending_account = accounts_users::get_by_email(&mut tx, "alice@example.com")
                .await
                .expect("looked")
                .expect("registered");
            tx.commit().await.expect("committed");
            assert!(
                pending_account.confirmed_at.is_none(),
                "not made until the last step"
            );

            let signed_up = sign_in_confirm(&server, &matched.pending)
                .await
                .expect("the account made");
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
                    sign_in_confirm(&server, &matched.pending).await,
                    Err(AccountError::SignInExpired)
                ),
                "the step is taken once"
            );
            assert!(
                !password::get(&server, &signed_up.user)
                    .await
                    .expect("looked")
                    .is_set
            );

            let matched = code_matched(&server, &mailer, "alice@example.com").await;
            assert!(!matched.new_account, "a sign-in now");
            let signed_in = sign_in_confirm(&server, &matched.pending)
                .await
                .expect("signed in");
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
    async fn accounts_the_last_step_records_the_terms_version_accepted_and_when() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let matched = code_matched(&server, &mailer, "alice@example.com").await;
            let account_of = |id| {
                let server = server.clone();
                async move {
                    let mut tx = server.begin().await.expect("a transaction");
                    let account = accounts_users::get(&mut tx, id)
                        .await
                        .expect("looked")
                        .expect("the account");
                    tx.commit().await.expect("committed");
                    account
                }
            };
            let mut tx = server.begin().await.expect("a transaction");
            let pending_account = accounts_users::get_by_email(&mut tx, "alice@example.com")
                .await
                .expect("looked")
                .expect("registered");
            tx.commit().await.expect("committed");
            assert_eq!(
                pending_account.terms_version, None,
                "not until the last step"
            );
            assert_eq!(pending_account.terms_accepted_at, None);

            let alice = sign_in_confirm(&server, &matched.pending)
                .await
                .expect("signed up")
                .user;
            let accepted = account_of(alice.id).await;
            assert_eq!(accepted.terms_version.as_deref(), Some(TERMS_VERSION));
            let accepted_at = accepted.terms_accepted_at.expect("when");

            let matched = code_matched(&server, &mailer, "alice@example.com").await;
            sign_in_confirm(&server, &matched.pending)
                .await
                .expect("signed in");
            assert_eq!(
                account_of(alice.id).await.terms_accepted_at,
                Some(accepted_at),
                "the same version keeps when it was first accepted"
            );

            let mut tx = server.begin().await.expect("a transaction");
            accounts_users::accept_terms(&mut tx, alice.id, "a later version")
                .await
                .expect("accepted");
            tx.commit().await.expect("committed");
            let later = account_of(alice.id).await;
            assert_eq!(later.terms_version.as_deref(), Some("a later version"));
            assert!(later.terms_accepted_at.expect("when") >= accepted_at);
        })
        .await;
    }

    /// Gives the user a password, as they would signed in.
    async fn password_set(server: &Deployment, mailer: &Mailer, user: &User, new: &str) {
        let update = PasswordUpdate {
            password: new.to_string(),
        };
        password::update(
            server,
            mailer,
            user,
            &update,
            crate::helpers::locales::DEFAULT,
        )
        .await
        .expect("a password");
    }

    #[tokio::test]
    async fn accounts_a_password_signs_in_in_place_of_the_code() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let alice = signed_up(&server, &mailer, "alice@example.com").await.user;
            password_set(&server, &mailer, &alice, PASSWORD).await;
            assert!(password::get(&server, &alice).await.expect("looked").is_set);

            let sent_before = mailer.sent().len();
            let session = sign_in_with_password(&server, " Alice@Example.com ", PASSWORD)
                .await
                .expect("the password matched");
            assert_eq!(session.user, alice);
            assert_eq!(session.email, "alice@example.com");
            assert_eq!(
                users::get_by_token(&server, &session.token)
                    .await
                    .expect("stands for alice"),
                alice
            );
            assert_eq!(mailer.sent().len(), sent_before, "no code mailed");

            let (verifier, challenge) = verifier_and_challenge();
            let device_code = sign_in_with_password_for_device(
                &server,
                "alice@example.com",
                PASSWORD,
                &challenge,
            )
            .await
            .expect("the password matched");
            let session = sign_in_with_device_code(&server, &device_code.code, &verifier)
                .await
                .expect("exchanged");
            assert_eq!(session.user, alice);
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_password_sign_in_refuses_every_way_alike() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let alice = signed_up(&server, &mailer, "alice@example.com").await.user;
            password_set(&server, &mailer, &alice, PASSWORD).await;
            signed_up(&server, &mailer, "bob@example.com").await;
            sign_in(
                &server,
                &mailer,
                "carol@example.com",
                true,
                crate::helpers::locales::DEFAULT,
            )
            .await
            .expect("registered, never confirmed");

            for (email, attempt) in [
                ("alice@example.com", "wrong horse battery"),
                ("alice@example.com", ""),
                ("bob@example.com", PASSWORD),
                ("carol@example.com", PASSWORD),
                ("nobody@example.com", PASSWORD),
            ] {
                assert!(
                    matches!(
                        sign_in_with_password(&server, email, attempt).await,
                        Err(AccountError::EmailOrPasswordDoesNotMatch)
                    ),
                    "{email} with {attempt:?}"
                );
            }
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_password_update_needs_no_current_one_and_mails_the_change() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let alice = signed_up(&server, &mailer, "alice@example.com").await.user;
            let short = PasswordUpdate {
                password: "short".to_string(),
            };
            assert!(matches!(
                password::update(
                    &server,
                    &mailer,
                    &alice,
                    &short,
                    crate::helpers::locales::DEFAULT
                )
                .await,
                Err(AccountError::Credential(CredentialError::PasswordTooShort))
            ));
            password_set(&server, &mailer, &alice, PASSWORD).await;
            let mail = mailer.last_sent_to("alice@example.com").expect("mailed");
            assert_eq!(mail.subject, "Your password was changed");

            let another = "another horse battery";
            password_set(&server, &mailer, &alice, another).await;
            assert!(matches!(
                sign_in_with_password(&server, "alice@example.com", PASSWORD).await,
                Err(AccountError::EmailOrPasswordDoesNotMatch)
            ));
            sign_in_with_password(&server, "alice@example.com", another)
                .await
                .expect("the new one");
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_sign_in_refuses_a_bad_email_and_a_second_one_replaces_the_first() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let error = sign_in(
                &server,
                &mailer,
                "alice",
                true,
                crate::helpers::locales::DEFAULT,
            )
            .await
            .expect_err("no @");
            assert!(matches!(
                error,
                AccountError::AccountUser(AccountUserError::InvalidEmail)
            ));
            assert!(mailer.sent().is_empty());

            let first = sign_in(
                &server,
                &mailer,
                "alice@example.com",
                true,
                crate::helpers::locales::DEFAULT,
            )
            .await
            .expect("signed in");
            let first_code = code_sent_to(&mailer, "alice@example.com");
            let again = sign_in(
                &server,
                &mailer,
                "alice@example.com",
                true,
                crate::helpers::locales::DEFAULT,
            )
            .await
            .expect("signed in again before finishing");
            assert!(
                matches!(
                    sign_in_with_code(&server, &first.pending, &first_code).await,
                    Err(AccountError::CodeDoesNotMatch)
                ),
                "the first pending sign-in was replaced"
            );
            let code = code_sent_to(&mailer, "alice@example.com");
            let matched = sign_in_with_code(&server, &again.pending, &code)
                .await
                .expect("the second one works");
            assert!(matched.new_account);
            let session = sign_in_confirm(&server, &matched.pending)
                .await
                .expect("the account made");
            assert_eq!(
                session.user.name, "alice",
                "named once, not again as alice1"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_sign_in_without_sign_ups_answers_an_unknown_email_as_a_known_one() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let sent = sign_in(
                &server,
                &mailer,
                "nobody@example.com",
                false,
                crate::helpers::locales::DEFAULT,
            )
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
            let mut tx = server.begin().await.expect("a transaction");
            assert!(
                accounts_users::get_by_email(&mut tx, "nobody@example.com")
                    .await
                    .expect("looked")
                    .is_none(),
                "no account made"
            );
            tx.commit().await.expect("committed");

            signed_up(&server, &mailer, "alice@example.com").await;
            let sent = sign_in(
                &server,
                &mailer,
                "alice@example.com",
                false,
                crate::helpers::locales::DEFAULT,
            )
            .await
            .expect("an account signs in still");
            let code = code_sent_to(&mailer, "alice@example.com");
            let matched = sign_in_with_code(&server, &sent.pending, &code)
                .await
                .expect("the code matched");
            assert!(!matched.new_account);
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_code_takes_five_wrong_tries_then_the_sign_in_is_gone() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let sent = sign_in(
                &server,
                &mailer,
                "alice@example.com",
                true,
                crate::helpers::locales::DEFAULT,
            )
            .await
            .expect("signed in");
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
            let matched = code_matched(&server, &mailer, "alice@example.com").await;
            assert!(matched.new_account, "a fresh code works");
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_new_code_replaces_the_old_one_for_the_same_pending_sign_in() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let sent = sign_in(
                &server,
                &mailer,
                "alice@example.com",
                true,
                crate::helpers::locales::DEFAULT,
            )
            .await
            .expect("signed in");
            let first = code_sent_to(&mailer, "alice@example.com");
            resend_code(
                &server,
                &mailer,
                &sent.pending,
                crate::helpers::locales::DEFAULT,
            )
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
                    resend_code(
                        &server,
                        &mailer,
                        &sent.pending,
                        crate::helpers::locales::DEFAULT
                    )
                    .await,
                    Err(AccountError::CodeDoesNotMatch)
                ),
                "nothing pending any more"
            );
            assert!(matches!(
                resend_code(
                    &server,
                    &mailer,
                    "not a pending token",
                    crate::helpers::locales::DEFAULT
                )
                .await,
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
            let matched = code_matched(&server, &mailer, "alice@example.com").await;
            let (verifier, challenge) = verifier_and_challenge();
            assert_eq!(challenge, challenge_of(&verifier));
            let device_code = sign_in_confirm_for_device(&server, &matched.pending, &challenge)
                .await
                .expect("the account made");
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
            sign_in(
                &device,
                &mailer,
                "alice@example.com",
                true,
                crate::helpers::locales::DEFAULT
            )
            .await,
            Err(AccountError::ServerOnly)
        ));
        assert!(matches!(
            sign_in_with_code(&device, "pending", "123456").await,
            Err(AccountError::ServerOnly)
        ));
        assert!(matches!(
            sign_in_confirm(&device, "pending").await,
            Err(AccountError::ServerOnly)
        ));
        assert!(matches!(
            sign_in_with_password(&device, "alice@example.com", PASSWORD).await,
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

            let note = test_notes::create(&device, &alice, "from the device", None)
                .await
                .expect("a note");
            sync::run(&device, &transport).await.expect("synced");
            let cursor = sync::status(&device).await.expect("status").cursor;
            assert!(cursor.is_some(), "the first pull set the cursor");
            assert_eq!(
                test_notes::get(&server, &alice, note.id)
                    .await
                    .expect("on the server")
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
                test_notes::get_in(&mut tx, note.id)
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
                test_notes::list(&device, &alice)
                    .await
                    .expect("her notes")
                    .len(),
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
            let note = test_notes::create(&device, &alice, "written, then signed out", None)
                .await
                .expect("a note");
            sign_out_on_device(&device, &transport)
                .await
                .expect("signed out");
            assert!(
                test_notes::get(&server, &alice, note.id).await.is_err(),
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
            assert!(test_notes::get(&server, &alice, note.id).await.is_ok());
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
            test_notes::create(&device, &alice, "kept through it", None)
                .await
                .expect("a note");

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
