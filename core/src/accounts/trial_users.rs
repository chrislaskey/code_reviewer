//! Trial users: a person using the app on the web without signing up,
//! see `docs/design/accounts-local-and-trial-users.md`. `create` makes
//! one on the server: an account with a made-up email and
//! `trial_expires_at`, a user, a community of their own named `Trial`,
//! and a session. They use the app as anyone does, with these
//! differences, each decided by one question, whether an account has
//! `trial_expires_at`:
//!
//! - nobody else sees their communities or joins them, and they join
//!   nobody else's (`data::app_communities`, `memberships`), so nothing
//!   of theirs ever reaches a device or another person;
//! - they see only their own communities (`data::app_communities`);
//! - they have no files and no picture (`files::uploads::create`,
//!   `avatars::uploads::create`);
//! - they make `MAX_WRITES` writes and no more (`events::record`);
//! - `delete_expired`, an hourly job, removes them outright with
//!   everything of theirs once the trial ended `KEPT_AFTER_HOURS` ago.
//!
//! Signing up, `accounts::sign_in_from_trial`, makes the same user a
//! full one: nothing moves, since every row is already theirs.

use crate::data::trial_users;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::accounts::{AccountError, Session};
use crate::app::communities::{self, CommunityError};
use crate::app::users::{self, UserError};
use crate::data::accounts_tokens::{self, Context};
use crate::data::accounts_users::{self, AccountUserError};
use crate::data::app_users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::helpers::random;

/// How long the pages say a trial lasts.
pub const SHOWN_HOURS: i64 = 24;
/// How long after that a trial user is kept, to finish signing up.
pub const KEPT_AFTER_HOURS: i64 = 24;
/// How many writes a trial user makes in the trial's life, the two
/// their first community takes included; `events::record` refuses the
/// next. With the cap on trial users and the lengths a row may have,
/// this bounds what trials can make the server hold.
pub const MAX_WRITES: i64 = 500;
/// The community a trial user starts with, named as the pages call
/// them: a guest.
pub const COMMUNITY_NAME: &str = "Guest";

const NAME_PREFIX: &str = "guest_";
const EMAIL_DOMAIN: &str = "example.com";

/// What `delete_expired` removed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrialUsersDeleted {
    pub users: u64,
}

#[derive(Debug, Error)]
pub enum TrialUserError {
    #[error("only the server has trial users")]
    ServerOnly,
    #[error("Trials are full right now; sign up instead")]
    Full,
    #[error(transparent)]
    User(#[from] UserError),
    #[error(transparent)]
    AccountUser(#[from] AccountUserError),
    #[error(transparent)]
    Community(#[from] CommunityError),
    /// The remote backend failed, or does not have trials yet: the
    /// sample answers `not_implemented`.
    #[cfg(feature = "remote-backend")]
    #[error(transparent)]
    Backend(#[from] crate::services::backend::BackendError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<TrialUserError> for AccountError {
    fn from(error: TrialUserError) -> AccountError {
        match error {
            TrialUserError::ServerOnly => AccountError::ServerOnly,
            TrialUserError::Full => AccountError::TrialsFull,
            TrialUserError::User(error) => error.into(),
            TrialUserError::AccountUser(error) => error.into(),
            TrialUserError::Community(error) => error.into(),
            #[cfg(feature = "remote-backend")]
            TrialUserError::Backend(error) => error.into(),
            TrialUserError::Database(error) => error.into(),
        }
    }
}

/// How long a trial user's rows are kept from the moment they are made:
/// what the browser's cookie lasts.
pub fn kept_for() -> Duration {
    Duration::hours(SHOWN_HOURS + KEPT_AFTER_HOURS)
}

/// Makes a trial user on the server and signs them in: the session for
/// the browser's cookie. Refused while `max` trial users exist, which
/// also turns trials off at 0.
pub async fn create(deployment: &Deployment, max: i64) -> Result<Session, TrialUserError> {
    if deployment.role != Role::Server {
        return Err(TrialUserError::ServerOnly);
    }
    if max <= 0 {
        return Err(TrialUserError::Full);
    }
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        let request = serde_json::json!({ "max": max });
        return match backend.call("trial_users/create", &request).await {
            Ok(session) => Ok(session),
            Err(error) if error.code() == Some("full") => Err(TrialUserError::Full),
            Err(error) => Err(error.into()),
        };
    }
    let mut tx = deployment.begin().await?;
    crate::data::app_sync_versions::lock(&mut tx).await?;
    if trial_users::count(&mut tx).await? >= max {
        return Err(TrialUserError::Full);
    }
    let name = format!("{NAME_PREFIX}{}", random::letters_and_digits(11));
    let email = format!("{name}@{EMAIL_DOMAIN}");
    let trial_expires_at = now() + Duration::hours(SHOWN_HOURS);
    let account =
        accounts_users::create_trial(&mut tx, Uuid::now_v7(), &email, trial_expires_at).await?;
    let user = users::create_in(&mut tx, account.id, &name).await?;
    communities::create_in(&mut tx, Role::Server, &user, COMMUNITY_NAME).await?;
    let token = accounts_tokens::create(&mut tx, user.id, Context::Session).await?;
    tx.commit().await?;
    Ok(Session {
        user,
        email: account.email,
        token,
    })
}

/// How many trial users there are, for the cap.
pub async fn count(deployment: &Deployment) -> Result<i64, TrialUserError> {
    #[cfg(feature = "remote-backend")]
    if let Some(backend) = &deployment.remote_backend {
        #[derive(serde::Deserialize)]
        struct Count {
            count: i64,
        }
        let answer: Count = backend
            .call("trial_users/count", &serde_json::json!({}))
            .await?;
        return Ok(answer.count);
    }
    let mut tx = deployment.begin().await?;
    let count = trial_users::count(&mut tx).await?;
    tx.commit().await?;
    Ok(count)
}

/// Whether the user is a trial user. On a device, never.
pub async fn is_trial(deployment: &Deployment, user: &User) -> Result<bool, sqlx::Error> {
    // A remote backend has no trials yet: they are not in the sample
    if deployment.has_remote_backend() {
        return Ok(false);
    }
    let mut tx = deployment.begin().await?;
    let account = accounts_users::lifecycle::get(&mut tx, user.id).await?;
    tx.commit().await?;
    Ok(account.is_some_and(|account| account.trial_expires_at.is_some()))
}

/// Whether the user is a trial user who has made `MAX_WRITES` writes.
pub(crate) async fn writes_are_full_in(
    tx: &mut Transaction,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let account = accounts_users::lifecycle::get(&mut *tx, user_id).await?;
    if !account.is_some_and(|account| account.trial_expires_at.is_some()) {
        return Ok(false);
    }
    Ok(trial_users::count_writes(tx, user_id).await? >= MAX_WRITES)
}

/// Removes outright every trial user whose trial ended more than
/// `KEPT_AFTER_HOURS` before `now`, with everything of theirs. Nobody
/// else ever saw a row of theirs, so there is no device to tell. A
/// trial that signed up is no longer one and is never touched.
pub async fn delete_expired(
    deployment: &Deployment,
    now: DateTime<Utc>,
) -> Result<TrialUsersDeleted, TrialUserError> {
    if deployment.role != Role::Server {
        return Err(TrialUserError::ServerOnly);
    }
    let ended_before = now - Duration::hours(KEPT_AFTER_HOURS);
    let mut tx = deployment.begin().await?;
    crate::data::app_sync_versions::lock(&mut tx).await?;
    let users = trial_users::maintenance::delete_expired_before(&mut tx, ended_before).await?;
    tx.commit().await?;
    Ok(TrialUsersDeleted { users })
}

/// Removes the trial user outright now, with everything of theirs, as
/// `delete_expired` would later: a trial deleting itself, see
/// `accounts::deletions`. Whether they were a trial.
pub(crate) async fn delete_in(tx: &mut Transaction, user_id: Uuid) -> Result<bool, sqlx::Error> {
    trial_users::maintenance::delete_one(tx, user_id).await
}

/// Shows a trial user's communities to everyone, once they signed up:
/// written as changed now, so the next pull of anyone brings them.
pub(crate) async fn show_communities_in(
    tx: &mut Transaction,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    trial_users::maintenance::touch_communities_of(tx, user_id, now()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::{self, CodeMatched};
    use crate::app::memberships;
    use crate::data::app_communities;
    use crate::database::with_each_database;
    use crate::services::mailer::Mailer;
    use crate::services::mailer::mails;
    use crate::sync::test_notes;
    use crate::sync::{self, PullRequest};

    fn server(database: crate::database::Database) -> Deployment {
        Deployment::new(database, Role::Server)
    }

    /// A full user, signed up and confirmed.
    async fn full_user(server: &Deployment, email: &str) -> User {
        accounts::tests::signed_up_for_a_device(server, email)
            .await
            .user
    }

    async fn names_seen_by(server: &Deployment, user: &User) -> Vec<String> {
        communities::list(server, user)
            .await
            .expect("listed")
            .into_iter()
            .map(|listed| listed.community.name)
            .collect()
    }

    async fn tables_pulled_by(server: &Deployment, user: &User) -> Vec<(String, Uuid)> {
        let request = PullRequest {
            cursor: None,
            after: None,
            limit: None,
        };
        sync::pull(server, user, request)
            .await
            .expect("a page")
            .rows
            .into_iter()
            .map(|row| {
                let id = row.row["id"].as_str().expect("an id");
                (row.table.clone(), id.parse().expect("a uuid"))
            })
            .collect()
    }

    #[tokio::test]
    async fn trial_users_create_makes_a_user_with_a_trial_community_and_a_session() {
        with_each_database(|database| async move {
            let server = server(database);
            let before = now();
            let session = create(&server, 10).await.expect("a trial");
            assert!(session.user.name.starts_with("guest_"));
            assert_eq!(
                session.email,
                format!("{}@example.com", session.user.name.to_lowercase())
            );
            let user = users::get_by_token(&server, &session.token)
                .await
                .expect("the session's user");
            assert_eq!(user, session.user);
            assert!(is_trial(&server, &user).await.expect("asked"));
            let account = crate::accounts::users::get(&server, user.id)
                .await
                .expect("theirs");
            assert_eq!(account.confirmed_at, None);
            let expires = account.trial_expires_at.expect("a trial");
            assert!(expires >= before + Duration::hours(SHOWN_HOURS));
            assert_eq!(names_seen_by(&server, &user).await, ["Guest"]);
            assert_eq!(
                memberships::list_mine(&server, &user)
                    .await
                    .expect("theirs")
                    .len(),
                1
            );

            test_notes::create(&server, &user, "trying it", None)
                .await
                .expect("a trial user notes like anyone");
            assert_eq!(count(&server).await.expect("counted"), 1);
        })
        .await;
    }

    #[tokio::test]
    async fn trial_users_create_is_refused_at_the_cap_and_off_at_zero() {
        with_each_database(|database| async move {
            let server = server(database);
            assert!(matches!(
                create(&server, 0).await,
                Err(TrialUserError::Full)
            ));
            create(&server, 1).await.expect("the first");
            assert!(matches!(
                create(&server, 1).await,
                Err(TrialUserError::Full)
            ));
            create(&server, 2).await.expect("under a higher cap");
        })
        .await;
    }

    #[tokio::test]
    async fn trial_users_are_seen_by_nobody_else_and_join_nobody() {
        with_each_database(|database| async move {
            let server = server(database);
            let alice = full_user(&server, "alice@example.com").await;
            let baking = communities::create(&server, &alice, "Baking")
                .await
                .expect("a community");
            let trial = create(&server, 10).await.expect("a trial").user;
            let theirs = communities::list(&server, &trial).await.expect("listed");
            let trial_community = theirs
                .iter()
                .find(|listed| listed.community.name == "Guest")
                .expect("their own")
                .community
                .clone();
            test_notes::create(&server, &trial, "in the trial", Some(trial_community.id))
                .await
                .expect("a note");

            assert_eq!(names_seen_by(&server, &alice).await, ["Baking", "Personal"]);
            assert!(matches!(
                communities::get(&server, &alice, trial_community.id).await,
                Err(CommunityError::NotFound { .. })
            ));
            assert!(
                memberships::create(&server, &alice, trial_community.id)
                    .await
                    .is_err(),
                "nobody joins a trial user's community"
            );
            assert!(
                memberships::create(&server, &trial, baking.id)
                    .await
                    .is_err(),
                "and a trial user joins nobody's"
            );
            let pulled = tables_pulled_by(&server, &alice).await;
            assert!(
                pulled
                    .iter()
                    .all(|(_, id)| *id != trial_community.id && *id != trial.id),
                "nothing of the trial's reaches her devices"
            );
            assert!(
                test_notes::list(&server, &alice)
                    .await
                    .expect("hers")
                    .is_empty()
            );

            assert_eq!(
                names_seen_by(&server, &trial).await,
                ["Guest"],
                "a trial user sees only their own"
            );
            assert!(matches!(
                communities::get(&server, &trial, baking.id).await,
                Err(CommunityError::NotFound { .. })
            ));
            assert!(
                tables_pulled_by(&server, &trial)
                    .await
                    .iter()
                    .all(|(_, id)| *id != baking.id),
                "nor pulls anyone else's"
            );
            let second = communities::create(&server, &trial, "Second")
                .await
                .expect("a trial user makes more of their own");
            assert!(!names_seen_by(&server, &alice).await.contains(&second.name));
        })
        .await;
    }

    #[tokio::test]
    async fn trial_users_make_the_maximum_of_writes_and_no_more_until_they_sign_up() {
        with_each_database(|database| async move {
            let server = server(database);
            let trial = create(&server, 10).await.expect("a trial").user;
            let mut tx = server.begin().await.expect("a transaction");
            let made = trial_users::count_writes(&mut tx, trial.id)
                .await
                .expect("counted");
            tx.commit().await.expect("committed");
            assert_eq!(made, 2, "their community and their membership");
            for number in made..MAX_WRITES {
                test_notes::create(&server, &trial, &format!("note {number}"), None)
                    .await
                    .expect("within the maximum");
            }

            let error = test_notes::create(&server, &trial, "one more", None)
                .await
                .expect_err("refused");
            assert!(matches!(
                error,
                test_notes::NoteError::Record(crate::sync::events::RecordError::TrialWritesFull)
            ));
            assert_eq!(
                error.to_string(),
                "This trial has reached its limit. Sign up to keep going."
            );
            let pushed = crate::data::app_events::Event::create(
                test_notes::TABLE,
                Uuid::now_v7(),
                crate::data::app_events::Fields::from_iter([("body".to_string(), "pushed".into())]),
                now(),
            );
            let request = sync::PushRequest {
                events: vec![pushed],
            };
            assert!(
                sync::push(&server, &trial, request).await.is_err(),
                "a push is a write like any other"
            );
            assert!(
                communities::create(&server, &trial, "Another")
                    .await
                    .is_err(),
                "whatever the resource"
            );
            let mut tx = server.begin().await.expect("a transaction");
            let recorded = trial_users::count_writes(&mut tx, trial.id)
                .await
                .expect("counted");
            tx.commit().await.expect("committed");
            assert_eq!(recorded, MAX_WRITES, "a refused write leaves no row");
            let notes_made = usize::try_from(MAX_WRITES - made).expect("a count");
            assert_eq!(
                test_notes::list(&server, &trial)
                    .await
                    .expect("theirs")
                    .len(),
                notes_made
            );
            assert_eq!(names_seen_by(&server, &trial).await, ["Guest"]);

            let matched = signed_up(&server, &trial, "bob@example.com").await;
            accounts::sign_in_confirm(&server, &matched.pending)
                .await
                .expect("signed up");
            let bob = users::get(&server, trial.id).await.expect("the same user");
            test_notes::create(&server, &bob, "signed up", None)
                .await
                .expect("a full user has no maximum");
        })
        .await;
    }

    /// The trial user signed up with the email, to the last step.
    async fn signed_up(server: &Deployment, trial: &User, email: &str) -> CodeMatched {
        let mailer = Mailer::memory();
        let sent = accounts::sign_in_from_trial(
            server,
            &mailer,
            trial,
            email,
            crate::helpers::locales::DEFAULT,
        )
        .await
        .expect("a code sent");
        let sent_to = email.trim().to_lowercase();
        let code = mails::code_in(&mailer.last_sent_to(&sent_to).expect("mailed")).expect("a code");
        accounts::sign_in_with_code(server, &sent.pending, &code)
            .await
            .expect("the code matched")
    }

    #[tokio::test]
    async fn trial_users_sign_up_keeps_the_user_and_every_row_and_shows_their_communities() {
        with_each_database(|database| async move {
            let server = server(database);
            let alice = full_user(&server, "alice@example.com").await;
            let cursor_before = sync::pull(
                &server,
                &alice,
                PullRequest {
                    cursor: None,
                    after: None,
                    limit: None,
                },
            )
            .await
            .expect("a page")
            .cursor;
            let session = create(&server, 10).await.expect("a trial");
            let trial = session.user.clone();
            let note = test_notes::create(&server, &trial, "made on a trial", None)
                .await
                .expect("a note");

            let matched = signed_up(&server, &trial, " Bob@Example.com ").await;
            assert!(matched.new_account, "a sign-up");
            assert!(
                is_trial(&server, &trial).await.expect("asked"),
                "a trial until the last step"
            );
            let signed_up = accounts::sign_in_confirm(&server, &matched.pending)
                .await
                .expect("signed up");
            assert_eq!(signed_up.user.id, trial.id, "the same user");
            assert_eq!(signed_up.user.name, "bob", "named from the address");
            assert_eq!(signed_up.email, "bob@example.com");
            let account = crate::accounts::users::get(&server, trial.id)
                .await
                .expect("theirs");
            assert_eq!(account.trial_expires_at, None);
            assert!(account.confirmed_at.is_some());
            assert!(!is_trial(&server, &signed_up.user).await.expect("asked"));
            assert!(matches!(
                users::get_by_token(&server, &session.token).await,
                Err(UserError::UnknownToken)
            ));

            let kept = test_notes::get(&server, &signed_up.user, note.id)
                .await
                .expect("the note");
            assert_eq!(kept, note);
            assert_eq!(
                names_seen_by(&server, &signed_up.user).await,
                ["Guest", "Personal"],
                "alice's, and their own, still called Guest, with no Personal of their own"
            );
            assert_eq!(names_seen_by(&server, &alice).await, ["Guest", "Personal"]);
            let page = sync::pull(
                &server,
                &alice,
                PullRequest {
                    cursor: cursor_before,
                    after: None,
                    limit: None,
                },
            )
            .await
            .expect("a page");
            assert!(
                page.rows
                    .iter()
                    .any(|row| row.table == app_communities::TABLE),
                "and it reaches a device that had pulled before"
            );

            let far_later = now() + Duration::days(365);
            let deleted = delete_expired(&server, far_later).await.expect("ran");
            assert_eq!(deleted.users, 0, "a signed-up trial is never removed");
        })
        .await;
    }

    #[tokio::test]
    async fn trial_users_sign_in_with_a_taken_email_signs_in_to_it_and_leaves_the_trial() {
        with_each_database(|database| async move {
            let server = server(database);
            let alice = full_user(&server, "alice@example.com").await;
            let trial = create(&server, 10).await.expect("a trial").user;
            let matched = signed_up(&server, &trial, "alice@example.com").await;
            assert!(!matched.new_account, "a sign-in");
            let session = accounts::sign_in_confirm(&server, &matched.pending)
                .await
                .expect("signed in");
            assert_eq!(session.user.id, alice.id);
            assert!(is_trial(&server, &trial).await.expect("asked"));
            assert_eq!(
                users::get(&server, trial.id).await.expect("the trial").name,
                trial.name
            );

            let mailer = Mailer::memory();
            let carol = full_user(&server, "carol@example.com").await;
            assert!(matches!(
                accounts::sign_in_from_trial(
                    &server,
                    &mailer,
                    &carol,
                    "new@example.com",
                    crate::helpers::locales::DEFAULT
                )
                .await,
                Err(AccountError::NotATrial)
            ));
        })
        .await;
    }

    #[tokio::test]
    async fn trial_users_sign_up_takes_an_email_a_sign_up_never_proved() {
        with_each_database(|database| async move {
            let server = server(database);
            accounts::sign_in(
                &server,
                &Mailer::memory(),
                "bob@example.com",
                true,
                crate::helpers::locales::DEFAULT,
            )
            .await
            .expect("signed up and stopped there");
            let trial = create(&server, 10).await.expect("a trial").user;
            let matched = signed_up(&server, &trial, "bob@example.com").await;
            accounts::sign_in_confirm(&server, &matched.pending)
                .await
                .expect("signed up");
            let account = crate::accounts::users::get(&server, trial.id)
                .await
                .expect("theirs");
            assert_eq!(account.email, "bob@example.com");
        })
        .await;
    }

    #[tokio::test]
    async fn trial_users_delete_expired_removes_a_trial_whole_after_the_kept_hours() {
        with_each_database(|database| async move {
            let server = server(database);
            let alice = full_user(&server, "alice@example.com").await;
            let session = create(&server, 10).await.expect("a trial");
            let trial = session.user.clone();
            let community = communities::list(&server, &trial).await.expect("listed");
            let community = community
                .iter()
                .find(|listed| listed.membership.is_some())
                .expect("their own");
            let note =
                test_notes::create(&server, &trial, "soon gone", Some(community.community.id))
                    .await
                    .expect("a note");
            let made = now();

            let an_hour_short = made + Duration::hours(SHOWN_HOURS + KEPT_AFTER_HOURS - 1);
            let deleted = delete_expired(&server, an_hour_short).await.expect("ran");
            assert_eq!(deleted.users, 0, "47 hours in, kept");
            assert!(users::get_by_token(&server, &session.token).await.is_ok());

            let an_hour_over = made + Duration::hours(SHOWN_HOURS + KEPT_AFTER_HOURS + 1);
            let deleted = delete_expired(&server, an_hour_over).await.expect("ran");
            assert_eq!(deleted.users, 1, "49 hours in, gone");
            assert!(matches!(
                users::get_by_token(&server, &session.token).await,
                Err(UserError::UnknownToken)
            ));
            assert!(matches!(
                users::get(&server, trial.id).await,
                Err(UserError::NotFound { .. })
            ));
            assert!(matches!(
                crate::accounts::users::get(&server, trial.id).await,
                Err(AccountUserError::NotFound { .. })
            ));
            let mut tx = server.begin().await.expect("a transaction");
            assert!(
                test_notes::get_in(&mut tx, note.id)
                    .await
                    .expect("looked")
                    .is_none()
            );
            assert!(
                communities::get_in(&mut tx, community.community.id)
                    .await
                    .expect("looked")
                    .is_none()
            );
            assert_eq!(
                crate::data::app_events::list_of_table(&mut tx, test_notes::TABLE)
                    .await
                    .expect("the log")
                    .len(),
                0,
                "their events too"
            );
            tx.commit().await.expect("committed");
            assert_eq!(count(&server).await.expect("counted"), 0);
            assert_eq!(names_seen_by(&server, &alice).await, ["Personal"]);
            assert!(users::get(&server, alice.id).await.is_ok());
        })
        .await;
    }

    #[tokio::test]
    async fn trial_users_are_not_purged_as_unconfirmed_sign_ups() {
        with_each_database(|database| async move {
            let server = server(database);
            let trial = create(&server, 10).await.expect("a trial").user;
            let a_day_and_a_half = now() + Duration::hours(36);
            crate::sync::purge::run(&server, a_day_and_a_half)
                .await
                .expect("purged");
            assert!(
                users::get(&server, trial.id).await.is_ok(),
                "36 hours in: the purge leaves a trial to its own job"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn trial_users_have_no_sign_in_by_their_made_up_email() {
        with_each_database(|database| async move {
            let server = server(database);
            let session = create(&server, 10).await.expect("a trial");
            let mailer = Mailer::memory();
            let sent = accounts::sign_in(
                &server,
                &mailer,
                &session.email,
                true,
                crate::helpers::locales::DEFAULT,
            )
            .await
            .expect("answered as any sign-in");
            assert_eq!(sent.pending.len(), 64);
            assert!(
                mailer.sent().is_empty(),
                "nothing is sent to a made-up address"
            );
        })
        .await;
    }
}
