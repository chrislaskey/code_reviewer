//! Scheduled server cleanup of expired sync entries and soft-deleted
//! rows. The purge commits the discarded version floor with its deletions.
//! A device below the floor starts with a fresh set of visible rows.
//! Synced children go first and any remaining child keeps its parent.
//!
//! It runs daily, before the sweeps, see `jobs`.

use crate::data::app_purges::{self, Purge};
use crate::data::{app_events, app_sync_changes, app_sync_versions};
use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::accounts::preferences;
use crate::app::users;
use crate::data::accounts_preferences;
use crate::data::accounts_tokens;
use crate::data::accounts_users;
use crate::deployment::{Deployment, Role};
use crate::sync::tables;

/// How long a deleted row is kept, and how long a device may stay away.
pub const RETENTION_DAYS: i64 = 90;
/// How long a sign-up has to enter its code before the user goes.
pub const UNCONFIRMED_DAYS: i64 = 1;

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
    User(#[from] crate::app::users::UserError),
    #[error(transparent)]
    AccountUser(#[from] crate::data::accounts_users::AccountUserError),
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
    app_sync_versions::lock(&mut tx).await?;
    let floor = app_sync_changes::maintenance::delete_written_before(&mut tx, horizon).await?;
    let mut removed = BTreeMap::new();
    // Children first, so a purged parent's rows have already gone
    for synced in tables::list().iter().rev() {
        let table = synced.table();
        let count =
            app_purges::maintenance::delete_deleted_before(&mut tx, table, horizon, floor).await?;
        removed.insert(table.to_string(), count);
    }
    let mut events = 0;
    for synced in tables::list() {
        events += app_events::maintenance::delete_of_missing_rows(&mut tx, synced.table()).await?;
    }
    removed.insert("app_events".to_string(), events);
    let expired_tokens = accounts_tokens::delete_expired(&mut tx, now).await?;
    removed.insert("accounts_tokens".to_string(), expired_tokens);
    let unconfirmed_before = now - Duration::days(UNCONFIRMED_DAYS);
    users::delete_of_unconfirmed_accounts_before(&mut tx, unconfirmed_before).await?;
    let unconfirmed =
        accounts_users::maintenance::delete_unconfirmed_before(&mut tx, unconfirmed_before).await?;
    removed.insert("accounts_users unconfirmed".to_string(), unconfirmed);
    let accounts = accounts_users::maintenance::delete_without_user(&mut tx).await?;
    removed.insert(accounts_users::TABLE.to_string(), accounts);
    let preferences = preferences::delete_without_account_in(&mut tx).await?;
    removed.insert(accounts_preferences::TABLE.to_string(), preferences);
    let purge = Purge {
        id: Uuid::now_v7(),
        horizon,
        inserted_at: now,
        minimum_global_sync_version: floor,
    };
    app_purges::create(&mut tx, &purge).await?;
    tx.commit().await?;
    Ok(PurgeReport { horizon, removed })
}

/// The latest run's horizon: the oldest deletion a device may still
/// learn of, or none before the first run.
pub async fn horizon(deployment: &Deployment) -> Result<Option<DateTime<Utc>>, PurgeError> {
    let mut tx = deployment.begin().await?;
    let horizon = app_purges::get(&mut tx).await?;
    tx.commit().await?;
    Ok(horizon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::communities;
    use crate::data::app_communities;
    use crate::data::app_events::EventStatus;
    use crate::database::{Database, with_each_database};
    use crate::deployment::now;
    use crate::sync::test_notes;

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
            let gone = test_notes::create(&deployment, &alice, "gone", Some(gardening.id))
                .await
                .expect("a note");
            let kept = test_notes::create(&deployment, &alice, "kept", Some(gardening.id))
                .await
                .expect("a note");
            test_notes::delete(&deployment, &alice, gone.id)
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
            assert_eq!(report.removed[test_notes::TABLE], 1);
            assert_eq!(report.removed[app_communities::TABLE], 0);
            assert_eq!(
                report.removed["app_events"], 2,
                "the note's create and delete"
            );
            let mut tx = deployment.begin().await.expect("a transaction");
            assert!(
                test_notes::get_in(&mut tx, gone.id)
                    .await
                    .expect("looked")
                    .is_none()
            );
            assert!(
                test_notes::get_in(&mut tx, kept.id)
                    .await
                    .expect("looked")
                    .is_some()
            );
            assert_eq!(
                app_events::count(&mut tx, EventStatus::Applied)
                    .await
                    .expect("counted"),
                3,
                "alice's community, her membership and the kept note"
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
    async fn purge_removes_a_sign_up_that_never_finished_after_a_day() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let mailer = crate::services::mailer::Mailer::memory();
            crate::accounts::sign_in(
                &deployment,
                &mailer,
                "bob@example.com",
                true,
                crate::helpers::locales::DEFAULT,
            )
            .await
            .expect("bob started a sign-up and stopped there");
            let mut tx = deployment.begin().await.expect("a transaction");
            let bob = crate::data::accounts_users::get_by_email(&mut tx, "bob@example.com")
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
                crate::data::accounts_users::get_by_email(&mut tx, "bob@example.com")
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
                crate::accounts::sign_in(
                    &deployment,
                    &mailer,
                    "bob@example.com",
                    true,
                    crate::helpers::locales::DEFAULT
                )
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
