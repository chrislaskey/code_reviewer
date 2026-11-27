use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use super::*;
use crate::database::Database;
use crate::deployment::Role;
use crate::helpers::json::object_fields;

#[tokio::test]
async fn versions_commit_in_order_and_rollback_with_their_rows() {
    let directory = tempfile::tempdir().expect("a disposable SQLite directory");
    let sqlite = Database::open_sqlite_file(&directory.path().join("server.db"))
        .await
        .expect("a file-backed database");
    assert_writer_order(sqlite).await;
    #[cfg(feature = "postgres")]
    crate::database::with_test_postgres(assert_writer_order).await;
}

async fn assert_writer_order(database: Database) {
    let server = Deployment::new(database, Role::Server);
    let alice = users::create(&server, "alice@example.com", "Alice")
        .await
        .expect("a user");

    for commit in [true, false] {
        let before = pull(&server, &alice, PullRequest::default())
            .await
            .expect("a baseline")
            .cursor
            .expect("its cursor");

        let mut first = server.begin().await.expect("writer A");
        let event = Event::create(
            test_notes::TABLE,
            Uuid::now_v7(),
            object_fields(json!({"body": "A"})),
            now(),
        );
        let answer = test_notes::Synced
            .apply_in(&mut first, Role::Server, &alice, &event)
            .await
            .expect("A written ahead of commit");
        let first_version = answer
            .server
            .expect("A's row")
            .global_sync_version()
            .expect("a stamp");
        assert_eq!(first_version, before.global_sync_version + 1);

        let (started, waiting) = tokio::sync::oneshot::channel();
        let mut second = tokio::spawn({
            let server = server.clone();
            let alice = alice.clone();
            async move {
                started.send(()).expect("test is collecting");
                test_notes::create(&server, &alice, "B", None)
                    .await
                    .expect("B committed")
            }
        });
        waiting.await.expect("B started");
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut second)
                .await
                .is_err()
        );

        let page = pull(
            &server,
            &alice,
            PullRequest {
                cursor: Some(before),
                ..PullRequest::default()
            },
        )
        .await
        .expect("a reader can pull while A holds the writer lock");
        assert!(page.rows.is_empty());
        assert_eq!(page.cursor, Some(before));

        match (commit, first) {
            (true, first) => first.commit().await.expect("commit A"),
            (false, Transaction::Sqlite(tx)) => tx.rollback().await.expect("roll back A"),
            #[cfg(feature = "postgres")]
            (false, Transaction::Postgres(tx)) => tx.rollback().await.expect("roll back A"),
        }
        let second = second.await.expect("B finished");
        let second_version = second.global_sync_version.expect("B's stamp");
        assert_eq!(second_version, first_version + i64::from(commit));

        let page = pull(
            &server,
            &alice,
            PullRequest {
                cursor: Some(before),
                ..PullRequest::default()
            },
        )
        .await
        .expect("both committed changes are available");
        let notes: Vec<_> = page
            .rows
            .iter()
            .filter(|row| row.table == test_notes::TABLE)
            .map(|row| row.row["body"].as_str().expect("a note body"))
            .collect();
        assert_eq!(notes, if commit { vec!["A", "B"] } else { vec!["B"] });
        assert_eq!(
            page.cursor.expect("advanced").global_sync_version,
            second_version
        );
    }
}

#[tokio::test]
async fn versions_pull_advances_through_a_private_page_and_keeps_its_bound() {
    crate::database::with_each_database(|database| async move {
        let server = Deployment::new(database, Role::Server);
        let alice = users::create(&server, "alice@example.com", "Alice")
            .await
            .expect("Alice");
        let bob = users::create(&server, "bob@example.com", "Bob")
            .await
            .expect("Bob");
        let baseline = pull(&server, &alice, PullRequest::default())
            .await
            .expect("a baseline")
            .cursor;

        let private = test_notes::create(&server, &bob, "private", None)
            .await
            .expect("Bob's");
        let shared = test_notes::create(&server, &alice, "first", None)
            .await
            .expect("Alice's");
        let page = pull(
            &server,
            &alice,
            PullRequest {
                cursor: baseline,
                limit: Some(1),
                ..PullRequest::default()
            },
        )
        .await
        .expect("another page");
        assert_eq!(
            page.cursor.map(|cursor| cursor.global_sync_version),
            private.global_sync_version
        );
        assert!(page.rows.is_empty());

        let new = test_notes::create(&server, &alice, "second", None)
            .await
            .expect("after the bound");
        let page = pull(
            &server,
            &alice,
            PullRequest {
                after: page.next,
                limit: Some(1),
                ..PullRequest::default()
            },
        )
        .await
        .expect("through the earlier bound");
        assert!(page.next.is_none());
        assert_eq!(
            page.cursor.map(|cursor| cursor.global_sync_version),
            shared.global_sync_version
        );
        assert!(page.rows.iter().any(|row| row.id() == Some(shared.id)));
        assert!(!page.rows.iter().any(|row| row.id() == Some(new.id)));
        let next = pull(
            &server,
            &alice,
            PullRequest {
                cursor: page.cursor,
                ..PullRequest::default()
            },
        )
        .await
        .expect("the next bounded pull");
        assert!(next.rows.iter().any(|row| row.id() == Some(new.id)));
    })
    .await;
}

#[test]
fn versions_use_decimal_strings_on_the_wire() {
    let cursor = Cursor {
        feed_id: Uuid::now_v7(),
        global_sync_version: 9_007_199_254_740_993,
    };
    let wire = serde_json::to_value(cursor).expect("serialize");
    assert_eq!(wire["global_sync_version"], "9007199254740993");
    assert_eq!(
        serde_json::from_value::<Cursor>(wire).expect("deserialize"),
        cursor
    );
    assert!(
        serde_json::from_value::<Cursor>(json!({
            "feed_id": cursor.feed_id, "global_sync_version": "-1"
        }))
        .is_err()
    );
}

#[tokio::test]
async fn versions_purge_sets_the_floor_and_bootstrap_keeps_live_rows() {
    crate::database::with_each_database(|database| async move {
        let server = Deployment::new(database, Role::Server);
        let user = users::create(&server, "reader@example.com", "Reader")
            .await
            .expect("the user");
        let note = test_notes::create(&server, &user, "live", None)
            .await
            .expect("a live note");
        let before = pull(&server, &user, PullRequest::default())
            .await
            .expect("before purge")
            .cursor
            .expect("the cursor");
        let gone = test_notes::create(&server, &user, "deleted", None)
            .await
            .expect("a note to remove");
        test_notes::delete(&server, &user, gone.id)
            .await
            .expect("delete");
        let after = pull(&server, &user, PullRequest::default())
            .await
            .expect("after delete")
            .cursor
            .expect("the cursor");
        let report = purge::run(
            &server,
            now() + chrono::Duration::days(purge::RETENTION_DAYS + 1),
        )
        .await
        .expect("purge old changes");
        assert_eq!(report.removed[test_notes::TABLE], 1);
        for (cursor, resync) in [
            (before, true),
            (after, false),
            (
                Cursor {
                    feed_id: Uuid::now_v7(),
                    ..after
                },
                true,
            ),
        ] {
            let page = pull(
                &server,
                &user,
                PullRequest {
                    cursor: Some(cursor),
                    ..PullRequest::default()
                },
            )
            .await
            .expect("a page");
            assert_eq!(page.resync, resync);
            assert!(page.rows.is_empty());
        }
        let page = pull(&server, &user, PullRequest::default())
            .await
            .expect("a fresh bootstrap");
        assert_eq!(page.cursor, Some(after));
        assert!(page.rows.iter().any(|row| row.id() == Some(note.id)));
        assert!(!page.rows.iter().any(|row| row.id() == Some(gone.id)));
    })
    .await;
}

#[tokio::test]
async fn versions_publish_each_community_exposed_when_a_guest_signs_up() {
    crate::database::with_each_database(|database| async move {
        let server = Deployment::new(database, Role::Server);
        let reader = users::create(&server, "reader@example.com", "Reader")
            .await
            .expect("a reader");
        let guest = crate::accounts::trial_users::create(&server, 2)
            .await
            .expect("a guest")
            .user;
        let second = crate::app::communities::create(&server, &guest, "Second")
            .await
            .expect("two communities");
        let page = pull(&server, &reader, PullRequest::default())
            .await
            .expect("a baseline");
        assert!(!page.rows.iter().any(|row| row.id() == Some(second.id)));

        let mailer = crate::services::mailer::Mailer::memory();
        let email = "guest@example.com";
        let sent = crate::accounts::sign_in_from_trial(&server, &mailer, &guest, email, "en")
            .await
            .expect("a signup code");
        let mail = mailer.last_sent_to(email).expect("sent");
        let code = crate::services::mailer::mails::code_in(&mail).expect("a code");
        let matched = crate::accounts::sign_in_with_code(&server, &sent.pending, &code)
            .await
            .expect("the code matched");
        crate::accounts::sign_in_confirm(&server, &matched.pending)
            .await
            .expect("signed up");

        let after = pull(
            &server,
            &reader,
            PullRequest {
                cursor: page.cursor,
                ..PullRequest::default()
            },
        )
        .await
        .expect("incremental pull");
        let communities: Vec<_> = after
            .rows
            .iter()
            .filter(|row| row.table == "app_communities")
            .collect();
        assert_eq!(communities.len(), 2);
        assert_ne!(
            communities[0].global_sync_version().unwrap(),
            communities[1].global_sync_version().unwrap()
        );
        assert!(communities.iter().any(|row| row.id() == Some(second.id)));
    })
    .await;
}
