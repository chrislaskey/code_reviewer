//! The conformance suite of the backend protocol, see
//! `docs/design/backend-seam.md`: what a device sees of sync, files,
//! memberships, trials, preferences, passwords and deletions, set up
//! only by what the protocol serves. It is the files example's, so it
//! goes with it; an app adds a test for each resource its backend
//! serves. Each test runs on the Rust backend with the other tests, and
//! against a remote backend when asked, its device syncing through the
//! server's own functions as the web server calls them:
//!
//! ```sh
//! CONFORMANCE_BACKEND_URL=http://localhost:4100 \
//!   cargo test -p app_core --features object-storage,remote-backend conformance -- --ignored --test-threads=1
//! ```
//!
//! The tests run one at a time there: they share the backend's data, and
//! a pull would see another test's writes.
//!
//! `CONFORMANCE_BACKEND_SERVICE_TOKEN` is the service token,
//! `dev-service-token` when unset. The server keeps the bucket in memory,
//! as Rust keeps the bucket beside a remote backend.

use std::collections::BTreeSet;
use std::future::Future;

use serde_json::json;
use uuid::Uuid;

use crate::accounts::password::PasswordUpdate;
use crate::accounts::{self, Session, preferences};
use crate::app::files::{self, FileFilter};
use crate::app::{communities, memberships, users};
use crate::data::accounts_preferences;
use crate::data::app_events::Event;
use crate::data::app_sync_state::Cursor;
use crate::database::with_each_database;
use crate::deployment::Deployment;
use crate::services::mailer::{Mail, Mailer, mails};
use crate::services::web_server::InProcess;
use crate::sync::{self, PullRequest, Report, conflicts};

/// Runs the test on the Rust backend, on each database.
async fn on_rust<F, Fut>(test: F)
where
    F: Fn(Deployment) -> Fut,
    Fut: Future<Output = ()>,
{
    with_each_database(|database| test(files::tests::server(database).0)).await;
}

/// Runs the test against the remote backend at `CONFORMANCE_BACKEND_URL`.
#[cfg(feature = "remote-backend")]
async fn on_remote<F, Fut>(test: F)
where
    F: Fn(Deployment) -> Fut,
    Fut: Future<Output = ()>,
{
    use crate::deployment::FileStore;
    use crate::services::backend::RemoteBackend;
    use crate::services::storage::Storage;

    let url = std::env::var("CONFORMANCE_BACKEND_URL").expect("CONFORMANCE_BACKEND_URL");
    let token = std::env::var("CONFORMANCE_BACKEND_SERVICE_TOKEN")
        .unwrap_or_else(|_| "dev-service-token".to_string());
    let backend = RemoteBackend::new(&url, token.into()).expect("a backend");
    let server = Deployment::new_with_remote_backend(backend)
        .with_file_store(FileStore::Bucket(Storage::memory()));
    test(server).await;
}

/// One test of the suite, on each backend.
macro_rules! conformance {
    ($name:ident) => {
        mod $name {
            #[tokio::test]
            async fn on_rust() {
                super::on_rust(super::$name).await;
            }

            #[cfg(feature = "remote-backend")]
            #[tokio::test]
            #[ignore]
            async fn on_remote() {
                super::on_remote(super::$name).await;
            }
        }
    };
}

/// A new person signed up and in, as the web's sign-in does: the code
/// mailed, the code, the last step.
async fn signed_up(server: &Deployment) -> Session {
    let email = format!("{}@example.com", Uuid::now_v7().simple());
    let mailer = Mailer::memory();
    let sent = accounts::sign_in(server, &mailer, &email, true, "en")
        .await
        .expect("a sign-in");
    let mail = mailer.last_sent_to(&email).expect("mailed");
    let code = mails::code_in(&mail).expect("a code");
    let matched = accounts::sign_in_with_code(server, &sent.pending, &code)
        .await
        .expect("the code");
    accounts::sign_in_confirm(server, &matched.pending)
        .await
        .expect("signed in")
}

async fn sync(device: &Deployment, server: &Deployment) -> Report {
    sync::run(device, &InProcess { server })
        .await
        .expect("a sync")
}

async fn file_names(device: &Deployment, session: &Session) -> Vec<String> {
    let filter = FileFilter {
        limit: 50,
        ..FileFilter::default()
    };
    files::list(device, &session.user, &filter)
        .await
        .expect("the files")
        .files
        .into_iter()
        .map(|shown| shown.file.name)
        .collect()
}

/// The community the sign-up made, the one the user is a member of.
async fn personal_community(server: &Deployment, session: &Session) -> Uuid {
    communities::list(server, &session.user)
        .await
        .expect("the communities")
        .into_iter()
        .find(|listed| listed.membership.is_some())
        .expect("a membership")
        .community
        .id
}

conformance!(conformance_a_sign_up_pulls_its_community_and_membership);
async fn conformance_a_sign_up_pulls_its_community_and_membership(server: Deployment) {
    let alice = signed_up(&server).await;
    let (a, _folder) = files::tests::device_with_folder(&alice.user, &alice.token).await;
    sync(&a, &server).await;
    let mine = memberships::list_mine(&a, &alice.user)
        .await
        .expect("the memberships");
    assert_eq!(mine.len(), 1);
    assert_eq!(
        mine[0].community_id,
        personal_community(&server, &alice).await
    );
}

conformance!(conformance_a_file_reaches_another_device_with_its_bytes);
async fn conformance_a_file_reaches_another_device_with_its_bytes(server: Deployment) {
    let alice = signed_up(&server).await;
    let (a, _a_folder) = files::tests::device_with_folder(&alice.user, &alice.token).await;
    let (b, _b_folder) = files::tests::device_with_folder(&alice.user, &alice.token).await;
    sync(&a, &server).await;
    let file = files::tests::with_bytes(&a, &alice.user, "notes.txt", "text/plain", b"hello")
        .await
        .expect("a file");
    let report = sync(&a, &server).await;
    assert_eq!(report.pending, 0, "{report:?}");

    sync(&b, &server).await;
    assert_eq!(file_names(&b, &alice).await, vec!["notes.txt"]);
    let path = files::local_copy(&b, &InProcess { server: &server }, &alice.user, file.id)
        .await
        .expect("the bytes");
    assert_eq!(tokio::fs::read(path).await.expect("read"), b"hello");
}

conformance!(conformance_a_rename_from_an_old_version_is_a_conflict_the_person_sees);
async fn conformance_a_rename_from_an_old_version_is_a_conflict_the_person_sees(
    server: Deployment,
) {
    let alice = signed_up(&server).await;
    let (a, _a_folder) = files::tests::device_with_folder(&alice.user, &alice.token).await;
    let (b, _b_folder) = files::tests::device_with_folder(&alice.user, &alice.token).await;
    sync(&a, &server).await;
    let file = files::tests::with_bytes(&a, &alice.user, "notes.txt", "text/plain", b"hello")
        .await
        .expect("a file");
    sync(&a, &server).await;
    sync(&b, &server).await;

    files::tests::rename(&a, &alice.user, file.id, "from a")
        .await
        .expect("renamed on a");
    files::tests::rename(&b, &alice.user, file.id, "from b")
        .await
        .expect("renamed on b");
    sync(&a, &server).await;
    let report = sync(&b, &server).await;
    assert_eq!(report.conflicts, 1, "{report:?}");
    assert_eq!(file_names(&b, &alice).await, vec!["from a"]);
    let seen = conflicts::list(&b).await.expect("the conflicts");
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].resolve, conflicts::Resolve::Reapply);
}

conformance!(conformance_a_file_for_a_community_left_meanwhile_is_refused_and_kept);
async fn conformance_a_file_for_a_community_left_meanwhile_is_refused_and_kept(server: Deployment) {
    let alice = signed_up(&server).await;
    let bob = signed_up(&server).await;
    let bobs = personal_community(&server, &bob).await;
    memberships::create(&server, &alice.user, bobs)
        .await
        .expect("joined");
    let (a, _folder) = files::tests::device_with_folder(&alice.user, &alice.token).await;
    sync(&a, &server).await;
    let file =
        files::create_with_bytes(&a, &alice.user, "late.txt", "text/plain", &[bobs], b"late")
            .await
            .expect("a file, offline");
    memberships::delete(&server, &alice.user, bobs)
        .await
        .expect("left meanwhile");

    let report = sync(&a, &server).await;
    assert_eq!(report.rejected, 1, "{report:?}");
    let refused = conflicts::list(&a).await.expect("the conflicts");
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].row_id, file.id);
    assert_eq!(refused[0].resolve, conflicts::Resolve::TryAgain);
    assert_eq!(refused[0].reason.as_deref(), Some("not_a_member"));
}

conformance!(conformance_joining_brings_the_communitys_files_and_leaving_takes_them);
async fn conformance_joining_brings_the_communitys_files_and_leaving_takes_them(
    server: Deployment,
) {
    let alice = signed_up(&server).await;
    let bob = signed_up(&server).await;
    let (b, _b_folder) = files::tests::device_with_folder(&bob.user, &bob.token).await;
    sync(&b, &server).await;
    files::tests::with_bytes(&b, &bob.user, "bobs.txt", "text/plain", b"bob's")
        .await
        .expect("a file");
    sync(&b, &server).await;

    let (a, _a_folder) = files::tests::device_with_folder(&alice.user, &alice.token).await;
    sync(&a, &server).await;
    assert!(file_names(&a, &alice).await.is_empty());
    let bobs = personal_community(&server, &bob).await;
    memberships::create(&server, &alice.user, bobs)
        .await
        .expect("joined");
    sync(&a, &server).await;
    assert_eq!(file_names(&a, &alice).await, vec!["bobs.txt"]);

    memberships::delete(&server, &alice.user, bobs)
        .await
        .expect("left");
    sync(&a, &server).await;
    assert!(file_names(&a, &alice).await.is_empty());
}

conformance!(conformance_a_pull_in_pages_of_one_row_gets_every_row);
/// A page holds its rows' parents too, so a parent may come again.
async fn conformance_a_pull_in_pages_of_one_row_gets_every_row(server: Deployment) {
    let alice = signed_up(&server).await;
    let (a, _folder) = files::tests::device_with_folder(&alice.user, &alice.token).await;
    sync(&a, &server).await;
    for name in ["one.txt", "two.txt", "three.txt"] {
        files::tests::with_bytes(&a, &alice.user, name, "text/plain", name.as_bytes())
            .await
            .expect("a file");
    }
    sync(&a, &server).await;

    let whole = sync::pull(&server, &alice.user, PullRequest::default())
        .await
        .expect("a page");
    let mut paged = Vec::new();
    let mut pages = 0;
    let mut request = PullRequest {
        limit: Some(1),
        ..PullRequest::default()
    };
    loop {
        let page = sync::pull(&server, &alice.user, request.clone())
            .await
            .expect("a page");
        pages += 1;
        paged.extend(page.rows);
        match page.next {
            Some(next) => {
                request.after = Some(next);
            }
            None => {
                assert_eq!(page.cursor, whole.cursor, "the same end");
                break;
            }
        }
    }
    let ids = |rows: &[sync::TableRow]| -> BTreeSet<(String, Option<String>)> {
        rows.iter()
            .map(|row| {
                let id = row.row.get("id").and_then(|id| id.as_str());
                (row.table.clone(), id.map(str::to_string))
            })
            .collect()
    };
    assert!(pages > whole.rows.len() / 2, "{pages} pages");
    assert_eq!(ids(&paged), ids(&whole.rows));
}

conformance!(conformance_a_cursor_of_another_feed_or_beyond_its_head_resyncs);
async fn conformance_a_cursor_of_another_feed_or_beyond_its_head_resyncs(server: Deployment) {
    let alice = signed_up(&server).await;
    let whole = sync::pull(&server, &alice.user, PullRequest::default())
        .await
        .expect("a page");
    let head = whole.cursor.expect("a cursor");
    for cursor in [
        Cursor {
            feed_id: Uuid::now_v7(),
            ..head
        },
        Cursor {
            global_sync_version: head.global_sync_version + 1_000_000,
            ..head
        },
    ] {
        let request = PullRequest {
            cursor: Some(cursor),
            ..PullRequest::default()
        };
        let page = sync::pull(&server, &alice.user, request)
            .await
            .expect("a page");
        assert!(page.resync);
    }
}

/// The address and pending sign-in of a new sign-in for the address,
/// with the code mailed.
async fn sign_in_started(server: &Deployment, mailer: &Mailer, email: &str) -> (String, String) {
    let sent = accounts::sign_in(server, mailer, email, true, "en")
        .await
        .expect("a sign-in");
    let mail = mailer.last_sent_to(email).expect("mailed");
    (sent.pending, mails::code_in(&mail).expect("a code"))
}

conformance!(conformance_a_device_signs_in_with_a_code_it_collects_once);
async fn conformance_a_device_signs_in_with_a_code_it_collects_once(server: Deployment) {
    let alice = signed_up(&server).await;
    let (pending, code) = sign_in_started(&server, &Mailer::memory(), &alice.email).await;
    let matched = accounts::sign_in_with_code(&server, &pending, &code)
        .await
        .expect("the code");
    assert!(!matched.new_account);
    let verifier = "a verifier only the device knows";
    let device_code = accounts::sign_in_confirm_for_device(
        &server,
        &matched.pending,
        &accounts::challenge_of(verifier),
    )
    .await
    .expect("a device code");

    let wrong = accounts::sign_in_with_device_code(&server, &device_code.code, "another")
        .await
        .expect_err("another verifier");
    assert!(matches!(wrong, accounts::AccountError::UnknownDeviceCode));
    let session = accounts::sign_in_with_device_code(&server, &device_code.code, verifier)
        .await
        .expect("the device's session");
    assert_eq!(session.user.id, alice.user.id);
    let again = accounts::sign_in_with_device_code(&server, &device_code.code, verifier)
        .await
        .expect_err("once");
    assert!(matches!(again, accounts::AccountError::UnknownDeviceCode));
    let user = users::get_by_token(&server, &session.token)
        .await
        .expect("the token works");
    assert_eq!(user.id, alice.user.id);
}

conformance!(conformance_a_resent_code_replaces_the_first);
async fn conformance_a_resent_code_replaces_the_first(server: Deployment) {
    let alice = signed_up(&server).await;
    let mailer = Mailer::memory();
    let (pending, first) = sign_in_started(&server, &mailer, &alice.email).await;
    let sent_to = accounts::resend_code_to(&server, &pending)
        .await
        .expect("the address");
    assert_eq!(sent_to, alice.email);
    accounts::resend_code(&server, &mailer, &pending, "en")
        .await
        .expect("resent");
    let mail = mailer.last_sent_to(&alice.email).expect("mailed again");
    let second = mails::code_in(&mail).expect("a code");

    if first != second {
        let error = accounts::sign_in_with_code(&server, &pending, &first)
            .await
            .expect_err("the first code");
        assert!(matches!(error, accounts::AccountError::CodeDoesNotMatch));
    }
    accounts::sign_in_with_code(&server, &pending, &second)
        .await
        .expect("the second code");
}

conformance!(conformance_the_last_step_records_the_terms_accepted);
async fn conformance_the_last_step_records_the_terms_accepted(server: Deployment) {
    let alice = signed_up(&server).await;
    let account = accounts::users::get(&server, alice.user.id)
        .await
        .expect("the account");
    assert_eq!(
        account.terms_version.as_deref(),
        Some(accounts::TERMS_VERSION)
    );
    assert!(account.terms_accepted_at.is_some());
}

conformance!(conformance_a_signed_up_user_is_not_a_trial);
async fn conformance_a_signed_up_user_is_not_a_trial(server: Deployment) {
    let alice = signed_up(&server).await;
    let is_trial = accounts::trial_users::is_trial(&server, &alice.user)
        .await
        .expect("asked");
    assert!(!is_trial);
}

/// A trial user on a server whose cap leaves room.
async fn trial(server: &Deployment) -> Session {
    accounts::trial_users::create(server, 1_000_000)
        .await
        .expect("a trial")
}

conformance!(conformance_a_trial_has_its_own_community_nobody_else_joins);
async fn conformance_a_trial_has_its_own_community_nobody_else_joins(server: Deployment) {
    let guest = trial(&server).await;
    let alice = signed_up(&server).await;
    assert!(
        accounts::trial_users::is_trial(&server, &guest.user)
            .await
            .expect("asked")
    );
    let names: Vec<String> = communities::list(&server, &guest.user)
        .await
        .expect("the communities")
        .into_iter()
        .map(|listed| listed.community.name)
        .collect();
    assert_eq!(names, [accounts::trial_users::COMMUNITY_NAME]);
    let guest_community = personal_community(&server, &guest).await;
    let joined = memberships::create(&server, &alice.user, guest_community).await;
    assert!(
        matches!(&joined, Err(memberships::MembershipError::Rejected(reason)) if reason == "unknown_community"),
        "{joined:?}"
    );
}

conformance!(conformance_a_trial_signs_up_and_keeps_its_user);
async fn conformance_a_trial_signs_up_and_keeps_its_user(server: Deployment) {
    let guest = trial(&server).await;
    let email = format!("{}@example.com", Uuid::now_v7().simple());
    let mailer = Mailer::memory();
    let sent = accounts::sign_in_from_trial(&server, &mailer, &guest.user, &email, "en")
        .await
        .expect("a code");
    let code = mails::code_in(&mailer.last_sent_to(&email).expect("mailed")).expect("a code");
    let matched = accounts::sign_in_with_code(&server, &sent.pending, &code)
        .await
        .expect("the code");
    assert!(matched.new_account);
    let session = accounts::sign_in_confirm(&server, &matched.pending)
        .await
        .expect("signed up");
    assert_eq!(session.user.id, guest.user.id);
    assert_eq!(session.email, email);
    assert!(
        !accounts::trial_users::is_trial(&server, &session.user)
            .await
            .expect("asked")
    );
    assert!(users::get_by_token(&server, &guest.token).await.is_err());
}

conformance!(conformance_a_trials_writes_stop_at_the_limit);
async fn conformance_a_trials_writes_stop_at_the_limit(server: Deployment) {
    let guest = trial(&server).await;
    let community = personal_community(&server, &guest).await;
    let mut writes = 0;
    let refused = loop {
        let written = if writes % 2 == 0 {
            memberships::delete(&server, &guest.user, community).await
        } else {
            memberships::create(&server, &guest.user, community)
                .await
                .map(|_| ())
        };
        match written {
            Ok(()) => writes += 1,
            Err(error) => break error,
        }
        assert!(writes < accounts::trial_users::MAX_WRITES, "never refused");
    };
    assert!(
        matches!(
            refused,
            memberships::MembershipError::Record(crate::sync::events::RecordError::TrialWritesFull)
        ),
        "{refused:?}"
    );
    // Its community and membership were the first two
    assert_eq!(writes + 2, accounts::trial_users::MAX_WRITES);
}

conformance!(conformance_a_preference_reaches_the_device_and_one_made_there_the_server);
async fn conformance_a_preference_reaches_the_device_and_one_made_there_the_server(
    server: Deployment,
) {
    let alice = signed_up(&server).await;
    preferences::update(&server, &alice.user, "theme", json!("dark"))
        .await
        .expect("the theme");
    let (a, _folder) = files::tests::device_with_folder(&alice.user, &alice.token).await;
    sync(&a, &server).await;
    let on_device = preferences::get(&a, &alice.user)
        .await
        .expect("the device's");
    assert_eq!(on_device.theme.as_deref(), Some("dark"));

    preferences::update(&a, &alice.user, "locale", json!("es"))
        .await
        .expect("the language, offline");
    sync(&a, &server).await;
    let on_server = preferences::get(&server, &alice.user)
        .await
        .expect("the server's");
    assert_eq!(on_server.locale.as_deref(), Some("es"));
    assert_eq!(on_server.theme.as_deref(), Some("dark"));
}

conformance!(conformance_a_value_a_key_does_not_take_is_refused);
async fn conformance_a_value_a_key_does_not_take_is_refused(server: Deployment) {
    let alice = signed_up(&server).await;
    let (a, _folder) = files::tests::device_with_folder(&alice.user, &alice.token).await;
    sync(&a, &server).await;
    let event = Event::create(
        accounts_preferences::TABLE,
        Uuid::now_v7(),
        crate::helpers::json::object_fields(
            json!({ "user_id": alice.user.id, "key": "theme", "value": "plaid" }),
        ),
        crate::deployment::now(),
    );
    let answers = sync::push(
        &server,
        &alice.user,
        sync::PushRequest {
            events: vec![event],
        },
    )
    .await
    .expect("answered");
    assert_eq!(
        answers.events[0].status,
        sync::events::AnswerStatus::Rejected
    );
    let on_server = preferences::get(&server, &alice.user)
        .await
        .expect("the server's");
    assert_eq!(on_server.theme, None);
}

conformance!(conformance_a_code_is_mailed_in_the_language_chosen);
async fn conformance_a_code_is_mailed_in_the_language_chosen(server: Deployment) {
    let alice = signed_up(&server).await;
    preferences::update(&server, &alice.user, "locale", json!("es"))
        .await
        .expect("the language");
    let mailer = Mailer::memory();
    accounts::sign_in(&server, &mailer, &alice.email, true, "en")
        .await
        .expect("a sign-in");
    let mail = mailer.last_sent_to(&alice.email).expect("mailed");
    let code = mails::code_in(&mail).expect("a code");
    assert_eq!(
        mail.subject,
        mails::sign_in_code("es", &alice.email, &code).subject
    );
}

conformance!(conformance_a_rename_reaches_the_device_and_one_made_there_the_server);
async fn conformance_a_rename_reaches_the_device_and_one_made_there_the_server(server: Deployment) {
    let alice = signed_up(&server).await;
    let renamed = users::update(&server, &alice.user, "Alice on the web")
        .await
        .expect("renamed");
    assert_eq!(renamed.version, alice.user.version + 1);
    let (a, _folder) = files::tests::device_with_folder(&alice.user, &alice.token).await;
    sync(&a, &server).await;
    let on_device = users::get(&a, alice.user.id).await.expect("the device's");
    assert_eq!(on_device.name, "Alice on the web");

    users::update(&a, &on_device, "Alice offline")
        .await
        .expect("renamed offline");
    sync(&a, &server).await;
    let on_server = users::get_by_token(&server, &alice.token)
        .await
        .expect("the server's");
    assert_eq!(on_server.name, "Alice offline");
}

conformance!(conformance_renaming_oneself_from_an_old_version_is_a_conflict_the_person_sees);
async fn conformance_renaming_oneself_from_an_old_version_is_a_conflict_the_person_sees(
    server: Deployment,
) {
    let alice = signed_up(&server).await;
    let (a, _folder) = files::tests::device_with_folder(&alice.user, &alice.token).await;
    sync(&a, &server).await;
    let on_device = users::get(&a, alice.user.id).await.expect("the device's");
    users::update(&server, &alice.user, "From the web")
        .await
        .expect("renamed on the web");
    users::update(&a, &on_device, "From the device")
        .await
        .expect("renamed offline");
    let report = sync(&a, &server).await;
    assert_eq!(report.conflicts, 1, "{report:?}");
    let on_server = users::get_by_token(&server, &alice.token)
        .await
        .expect("the server's");
    assert_eq!(on_server.name, "From the web");
    let seen = conflicts::list(&a).await.expect("the conflicts");
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].resolve, conflicts::Resolve::Reapply);
}

conformance!(conformance_nobody_renames_someone_else);
async fn conformance_nobody_renames_someone_else(server: Deployment) {
    let alice = signed_up(&server).await;
    let bob = signed_up(&server).await;
    let event = Event::update(
        crate::data::app_users::TABLE,
        alice.user.id,
        alice.user.version,
        crate::helpers::json::object_fields(json!({ "name": alice.user.name })),
        crate::helpers::json::object_fields(json!({ "name": "Not Alice" })),
        crate::deployment::now(),
    );
    let answers = sync::push(
        &server,
        &bob.user,
        sync::PushRequest {
            events: vec![event],
        },
    )
    .await
    .expect("answered");
    assert_eq!(
        answers.events[0].status,
        sync::events::AnswerStatus::Rejected
    );
    assert_eq!(answers.events[0].reason.as_deref(), Some("not_yours"));
    let on_server = users::get_by_token(&server, &alice.token)
        .await
        .expect("the server's");
    assert_eq!(on_server.name, alice.user.name);
}

const PASSWORD: &str = "correct horse battery";

/// Gives the user a password, as they would signed in, and answers the
/// mail that tells them.
async fn password_set(server: &Deployment, session: &Session, password: &str) -> Mail {
    let mailer = Mailer::memory();
    let update = PasswordUpdate {
        password: password.to_string(),
    };
    accounts::password::update(server, &mailer, &session.user, &update, "en")
        .await
        .expect("a password");
    mailer.last_sent_to(&session.email).expect("mailed")
}

conformance!(conformance_a_password_signs_in_on_the_web_and_on_a_device);
async fn conformance_a_password_signs_in_on_the_web_and_on_a_device(server: Deployment) {
    let alice = signed_up(&server).await;
    let status = accounts::password::get(&server, &alice.user)
        .await
        .expect("asked");
    assert!(!status.is_set);
    let mail = password_set(&server, &alice, PASSWORD).await;
    assert_eq!(
        mail.subject,
        mails::password_changed("en", &alice.email).subject
    );
    let status = accounts::password::get(&server, &alice.user)
        .await
        .expect("asked");
    assert!(status.is_set);

    let session = accounts::sign_in_with_password(&server, &alice.email.to_uppercase(), PASSWORD)
        .await
        .expect("the password matched");
    assert_eq!(session.user.id, alice.user.id);
    let user = users::get_by_token(&server, &session.token)
        .await
        .expect("the token works");
    assert_eq!(user.id, alice.user.id);

    let verifier = "a verifier only the device knows";
    let device_code = accounts::sign_in_with_password_for_device(
        &server,
        &alice.email,
        PASSWORD,
        &accounts::challenge_of(verifier),
    )
    .await
    .expect("a device code");
    let session = accounts::sign_in_with_device_code(&server, &device_code.code, verifier)
        .await
        .expect("the device's session");
    assert_eq!(session.user.id, alice.user.id);
}

conformance!(conformance_a_password_sign_in_refuses_every_way_alike);
async fn conformance_a_password_sign_in_refuses_every_way_alike(server: Deployment) {
    let alice = signed_up(&server).await;
    password_set(&server, &alice, PASSWORD).await;
    password_set(&server, &alice, "another horse battery").await;
    let bob = signed_up(&server).await;
    let guest = trial(&server).await;
    let nobody = format!("{}@example.com", Uuid::now_v7().simple());
    for (email, attempt) in [
        (alice.email.as_str(), PASSWORD),
        (alice.email.as_str(), ""),
        (bob.email.as_str(), PASSWORD),
        (guest.email.as_str(), PASSWORD),
        (nobody.as_str(), PASSWORD),
    ] {
        let refused = accounts::sign_in_with_password(&server, email, attempt).await;
        assert!(
            matches!(
                refused,
                Err(accounts::AccountError::EmailOrPasswordDoesNotMatch)
            ),
            "{email} with {attempt:?}: {refused:?}"
        );
    }
    accounts::sign_in_with_password(&server, &alice.email, "another horse battery")
        .await
        .expect("the new one");
}

/// Asks for the code that confirms deleting the account, and answers it.
async fn delete_code(server: &Deployment, session: &Session) -> String {
    let mailer = Mailer::memory();
    accounts::deletions::code::create(server, &mailer, &session.user, "en")
        .await
        .expect("a code");
    let mail = mailer.last_sent_to(&session.email).expect("mailed");
    mails::code_in(&mail).expect("a code")
}

conformance!(conformance_a_deleted_account_takes_its_community_and_leaves_the_rest);
async fn conformance_a_deleted_account_takes_its_community_and_leaves_the_rest(server: Deployment) {
    let alice = signed_up(&server).await;
    let bob = signed_up(&server).await;
    let alices = personal_community(&server, &alice).await;
    let bobs = personal_community(&server, &bob).await;
    memberships::create(&server, &alice.user, bobs)
        .await
        .expect("alice joins bob's");
    let code = delete_code(&server, &alice).await;
    let mailer = Mailer::memory();
    accounts::deletions::create(&server, &mailer, &alice.user, Some(&code), "en")
        .await
        .expect("deleted");
    let mail = mailer.last_sent_to(&alice.email).expect("told");
    assert_eq!(
        mail.subject,
        mails::account_deleted("en", &alice.email).subject
    );

    let refused = users::get_by_token(&server, &alice.token).await;
    assert!(
        matches!(refused, Err(users::UserError::AccountDeleted)),
        "{refused:?}"
    );
    let seen: Vec<Uuid> = communities::list(&server, &bob.user)
        .await
        .expect("bob's communities")
        .into_iter()
        .map(|listed| listed.community.id)
        .collect();
    assert!(!seen.contains(&alices), "alice's community went");
    assert!(seen.contains(&bobs));

    let (b, _folder) = files::tests::device_with_folder(&bob.user, &bob.token).await;
    sync(&b, &server).await;
    let members = memberships::list_mine(&b, &bob.user)
        .await
        .expect("bob's memberships");
    assert_eq!(members.len(), 1);
}

conformance!(conformance_a_delete_needs_the_code_mailed);
async fn conformance_a_delete_needs_the_code_mailed(server: Deployment) {
    let alice = signed_up(&server).await;
    let mailer = Mailer::memory();
    let refused =
        accounts::deletions::create(&server, &mailer, &alice.user, Some("123456"), "en").await;
    assert!(
        matches!(
            refused,
            Err(accounts::deletions::DeletionError::CodeDoesNotMatch)
        ),
        "{refused:?}"
    );
    let code = delete_code(&server, &alice).await;
    let wrong = if code == "000000" { "111111" } else { "000000" };
    let refused =
        accounts::deletions::create(&server, &mailer, &alice.user, Some(wrong), "en").await;
    assert!(matches!(
        refused,
        Err(accounts::deletions::DeletionError::CodeDoesNotMatch)
    ));
    users::get_by_token(&server, &alice.token)
        .await
        .expect("still there");
    accounts::deletions::create(&server, &mailer, &alice.user, Some(&code), "en")
        .await
        .expect("deleted with the code");
}

conformance!(conformance_a_trial_is_deleted_without_a_code);
async fn conformance_a_trial_is_deleted_without_a_code(server: Deployment) {
    let guest = trial(&server).await;
    let mailer = Mailer::memory();
    let refused = accounts::deletions::code::create(&server, &mailer, &guest.user, "en").await;
    assert!(matches!(
        refused,
        Err(accounts::deletions::DeletionError::TrialHasNoCode)
    ));
    accounts::deletions::create(&server, &mailer, &guest.user, None, "en")
        .await
        .expect("deleted");
    assert!(mailer.sent().is_empty(), "nothing mailed");
    assert!(users::get_by_token(&server, &guest.token).await.is_err());
}
