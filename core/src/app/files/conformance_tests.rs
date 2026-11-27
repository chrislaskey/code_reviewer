//! The conformance suite of the backend protocol, see
//! `docs/design/backend-seam.md`: what a device sees of sync, files and
//! memberships, set up only by what the protocol serves. It is the
//! files example's, so it goes with it; an app adds a test for each
//! resource its backend serves. Each test runs
//! on the Rust backend with the other tests, and against a remote
//! backend when asked, its device syncing through the server's own
//! functions as the web server calls them:
//!
//! ```sh
//! CONFORMANCE_BACKEND_URL=http://localhost:4100 \
//!   cargo test -p app_core --features object-storage,remote-backend conformance -- --ignored
//! ```
//!
//! `CONFORMANCE_BACKEND_SERVICE_TOKEN` is the service token,
//! `dev-service-token` when unset. The server keeps the bucket in memory,
//! as Rust keeps the bucket beside a remote backend.

use std::collections::BTreeSet;
use std::future::Future;

use uuid::Uuid;

use crate::accounts::{self, Session};
use crate::app::files::{self, FileFilter};
use crate::app::{communities, memberships};
use crate::data::app_sync_state::Cursor;
use crate::database::with_each_database;
use crate::deployment::Deployment;
use crate::services::mailer::{Mailer, mails};
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
