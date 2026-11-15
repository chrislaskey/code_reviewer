//! The purge: hard-deletes what was soft-deleted more than the retention
//! window ago, on the server, see "Purge and resync" in
//! `docs/design/offline-sync.md`. A row goes once both its delete and
//! its last write are older than the horizon, so every device that
//! pulled within the window has seen it go; children first, so a
//! deleted post's community rows, deleted at the same moment, go with
//! it; the events of rows that are gone go too, and so do the tokens
//! that have expired, see `tokens`, the users who signed up more than a
//! day ago and never entered their code, see `accounts`, whom no device
//! ever pulled, and the accounts of the users removed. Each run records
//! its horizon, and a device whose cursor is older than the latest one
//! may have missed a deletion, so the pull tells it to start over.
//!
//! It runs daily, before the sweeps, see `jobs`.

use crate::data::events as events_records;
use crate::data::purges as purges_records;
use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::deployment::{Deployment, Role};
use crate::{accounts, tokens, users};

/// A record assembled by this resource's workflow.
pub(crate) struct AcceptedPurge<'a>(&'a Purge);

impl AcceptedPurge<'_> {
    /// Borrows the assembled record for persistence.
    pub(crate) fn row(&self) -> &Purge {
        self.0
    }
}

/// How long a deleted row is kept, and how long a device may stay away.
pub const RETENTION_DAYS: i64 = 90;
/// How long a sign-up has to enter its code before the user goes.
pub const UNCONFIRMED_DAYS: i64 = 1;

/// The tables purged, children first.
const TABLES: &[purges_records::Table] = &[
    purges_records::Table::PostsCommunities,
    purges_records::Table::Posts,
    purges_records::Table::Files,
    purges_records::Table::Memberships,
    purges_records::Table::Communities,
    purges_records::Table::Users,
];

/// One run as `app_purges` keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Purge {
    pub id: Uuid,
    pub horizon: DateTime<Utc>,
    pub inserted_at: DateTime<Utc>,
}

/// What a run removed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurgeReport {
    pub horizon: DateTime<Utc>,
    /// Rows removed, by table, with the events of rows gone under
    /// `app_events`.
    pub removed: BTreeMap<String, u64>,
}

#[derive(Debug, Error)]
pub enum PurgeError {
    #[error("only the server purges")]
    ServerOnly,
    #[error(transparent)]
    User(#[from] crate::users::UserError),
    #[error(transparent)]
    AccountUser(#[from] crate::accounts::users::AccountUserError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Purges every table as of `now` and records the horizon, in one
/// transaction.
pub async fn run(deployment: &Deployment, now: DateTime<Utc>) -> Result<PurgeReport, PurgeError> {
    if deployment.role != Role::Server {
        return Err(PurgeError::ServerOnly);
    }
    let horizon = now - Duration::days(RETENTION_DAYS);
    let mut tx = deployment.begin().await?;
    let mut removed = BTreeMap::new();
    for table in TABLES {
        let count =
            purges_records::maintenance::delete_deleted_before(&mut tx, *table, horizon).await?;
        removed.insert(table.name().to_string(), count);
    }
    let events = events_records::maintenance::delete_of_missing_rows(&mut tx).await?;
    removed.insert("app_events".to_string(), events);
    let expired_tokens = tokens::delete_expired(&mut tx, now).await?;
    removed.insert("accounts_tokens".to_string(), expired_tokens);
    let unconfirmed_before = now - Duration::days(UNCONFIRMED_DAYS);
    users::delete_of_unconfirmed_accounts_before(&mut tx, unconfirmed_before).await?;
    let unconfirmed =
        accounts::users::delete_unconfirmed_before(&mut tx, unconfirmed_before).await?;
    removed.insert("accounts_users unconfirmed".to_string(), unconfirmed);
    let accounts = accounts::users::delete_without_user(&mut tx).await?;
    removed.insert(accounts::users::TABLE.to_string(), accounts);
    let purge = Purge {
        id: Uuid::now_v7(),
        horizon,
        inserted_at: now,
    };
    purges_records::create(&mut tx, AcceptedPurge(&purge)).await?;
    tx.commit().await?;
    Ok(PurgeReport { horizon, removed })
}

/// The latest run's horizon: the oldest deletion a device may still
/// learn of, or none before the first run.
pub async fn horizon(deployment: &Deployment) -> Result<Option<DateTime<Utc>>, PurgeError> {
    let mut tx = deployment.begin().await?;
    let horizon = purges_records::get(&mut tx).await?;
    tx.commit().await?;
    Ok(horizon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{Database, with_each_database};
    use crate::deployment::now;
    use crate::events::{self, EventStatus};
    use crate::{communities, posts, posts_communities};

    #[tokio::test]
    async fn purge_removes_what_was_deleted_before_the_horizon_with_its_rows_and_events() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let gardening = communities::create(&deployment, &alice, "Gardening")
                .await
                .expect("a community");
            let gone = posts::create(&deployment, &alice, "gone", &[gardening.id])
                .await
                .expect("a post");
            let kept = posts::create(&deployment, &alice, "kept", &[gardening.id])
                .await
                .expect("a post");
            posts::delete(&deployment, &alice, gone.id)
                .await
                .expect("deleted");
            assert_eq!(horizon(&deployment).await.expect("none"), None);

            let report = run(&deployment, now()).await.expect("purged");
            assert_eq!(
                report.removed.values().sum::<u64>(),
                0,
                "nothing is old yet"
            );
            assert_eq!(
                horizon(&deployment).await.expect("one"),
                Some(report.horizon)
            );

            let later = now() + Duration::days(RETENTION_DAYS + 1);
            let report = run(&deployment, later).await.expect("purged");
            assert_eq!(report.removed[posts::TABLE], 1);
            assert_eq!(report.removed[posts_communities::TABLE], 1);
            assert_eq!(report.removed[communities::TABLE], 0);
            assert_eq!(
                report.removed["app_events"], 3,
                "the post's create, delete and its row's"
            );
            let mut tx = deployment.begin().await.expect("a transaction");
            assert!(
                posts::get_in(&mut tx, gone.id)
                    .await
                    .expect("looked")
                    .is_none()
            );
            assert!(
                posts::get_in(&mut tx, kept.id)
                    .await
                    .expect("looked")
                    .is_some()
            );
            assert_eq!(
                events::count(&mut tx, EventStatus::Applied)
                    .await
                    .expect("counted"),
                4,
                "alice's community, her membership, the kept post and its row"
            );
            tx.commit().await.expect("committed");
            assert_eq!(
                horizon(&deployment).await.expect("the latest"),
                Some(report.horizon)
            );
        })
        .await;
    }

    #[tokio::test]
    async fn purge_removes_a_sign_up_that_never_entered_its_code_after_a_day() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let mailer = crate::mailer::Mailer::memory();
            crate::accounts::sign_up(&deployment, &mailer, "bob@example.com", "Bob")
                .await
                .expect("bob signed up and stopped there");
            let mut tx = deployment.begin().await.expect("a transaction");
            let bob = crate::accounts::users::get_by_email_in(&mut tx, "bob@example.com")
                .await
                .expect("looked")
                .expect("bob's account");
            tx.commit().await.expect("committed");
            let alice =
                crate::accounts::tests::signed_up_for_a_device(&deployment, "alice@example.com")
                    .await
                    .user;

            let report = run(&deployment, now()).await.expect("purged");
            assert_eq!(
                report.removed["accounts_users unconfirmed"], 0,
                "bob has a day"
            );
            let tomorrow = now() + Duration::days(UNCONFIRMED_DAYS + 1);
            let report = run(&deployment, tomorrow).await.expect("purged");
            assert_eq!(report.removed["accounts_users unconfirmed"], 1);
            let mut tx = deployment.begin().await.expect("a transaction");
            assert!(
                crate::accounts::users::get_by_email_in(&mut tx, "bob@example.com")
                    .await
                    .expect("looked")
                    .is_none(),
                "bob is gone, with his pending sign-in"
            );
            assert!(
                users::get_in(&mut tx, bob.id)
                    .await
                    .expect("looked")
                    .is_none(),
                "and so is his user's row"
            );
            assert!(
                users::get_in(&mut tx, alice.id)
                    .await
                    .expect("looked")
                    .is_some(),
                "alice, confirmed, stays"
            );
            tx.commit().await.expect("committed");
            assert!(
                crate::accounts::sign_up(&deployment, &mailer, "bob@example.com", "Bob")
                    .await
                    .is_ok(),
                "and the email is free again"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn purge_is_the_servers_alone() {
        let device = users::tests::device().await;
        let error = run(&device, now()).await.expect_err("a device");
        assert!(matches!(error, PurgeError::ServerOnly));
        let _ = Database::open_sqlite_in_memory;
    }
}
