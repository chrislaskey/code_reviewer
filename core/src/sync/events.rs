//! Events: one change a person made to a row, the only way a synced row
//! changes, see "Events" in `docs/design/offline-sync.md`. A resource
//! builds one for each save and applies it with the merge here; the
//! server answers a pushed event the same way, since both run the
//! resource's `apply_event`. The `app_events` table is the server's log
//! of every event answered and a device's queue of events not yet
//! answered, with the conflicts the person has not resolved; its records
//! are `data::app_events`.
//!
//! An event in the queue is tried by every sync, but not for ever: each
//! time the server or the bucket says no to it and it stays pending
//! counts, `count_attempt`, and at `MAX_ATTEMPTS` it is refused, kept
//! as a conflict with the reason until the person tries it again or
//! lets it go. Not reaching the server counts nothing, and neither does
//! a server or a bucket that is failing, so an outage of any length
//! refuses nothing.

use crate::data::app_events::{
    self, Conflicts, Event, EventStatus, FieldConflict, Fields, StoredEvent,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::database::Transaction;
use crate::sync::TableRow;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnswerStatus {
    Applied,
    Conflict,
    Rejected,
}

/// The server's answer to one event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    pub id: Uuid,
    pub status: AnswerStatus,
    /// The row's version after the event.
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub conflicts: Conflicts,
    /// Why it was rejected.
    #[serde(default)]
    pub reason: Option<String>,
    /// The row as it now stands on the server, when it has one.
    #[serde(default)]
    pub server: Option<TableRow>,
}

impl Answer {
    /// Applied, or a conflict when some field did not apply.
    pub fn applied(id: Uuid, version: i64, conflicts: Conflicts, server: TableRow) -> Answer {
        let status = if conflicts.is_empty() {
            AnswerStatus::Applied
        } else {
            AnswerStatus::Conflict
        };
        Answer {
            id,
            status,
            version: Some(version),
            conflicts,
            reason: None,
            server: Some(server),
        }
    }

    pub fn rejected(id: Uuid, reason: impl Into<String>, server: Option<TableRow>) -> Answer {
        Answer {
            id,
            status: AnswerStatus::Rejected,
            version: None,
            conflicts: Conflicts::new(),
            reason: Some(reason.into()),
            server,
        }
    }
}

impl StoredEvent {
    /// The answer the server recorded, with the row as it stands now.
    pub fn answer(&self, server: Option<TableRow>) -> Answer {
        let status = match self.status {
            EventStatus::Applied => AnswerStatus::Applied,
            EventStatus::Conflict => AnswerStatus::Conflict,
            EventStatus::Rejected | EventStatus::Pending => AnswerStatus::Rejected,
        };
        Answer {
            id: self.event.id,
            status,
            version: self.version,
            conflicts: self.conflicts.clone(),
            reason: self.last_error.clone(),
            server,
        }
    }
}

/// Merges an event's `changes` into a row's `current` values at
/// `version`, field by field. At the server's version each changed value
/// applies. Behind it, a field the row already holds the device's value
/// for is skipped; a field still at the device's `base` value applies,
/// since nobody else changed it; any other field was changed by someone
/// else first, and the device's value is a conflict.
pub fn merge(
    current: &Fields,
    version: i64,
    base_version: i64,
    base: &Fields,
    changes: &Fields,
) -> (Fields, Conflicts) {
    if base_version == version {
        let applied = changes
            .iter()
            .filter(|(field, value)| current.get(*field) != Some(*value))
            .map(|(field, value)| (field.clone(), value.clone()))
            .collect();
        return (applied, Conflicts::new());
    }
    let mut applied = Fields::new();
    let mut conflicts = Conflicts::new();
    for (field, value) in changes {
        let server = current.get(field).cloned().unwrap_or(Value::Null);
        if server == *value {
            continue;
        }
        if base.get(field) == Some(&server) {
            applied.insert(field.clone(), value.clone());
        } else {
            conflicts.insert(
                field.clone(),
                FieldConflict {
                    yours: value.clone(),
                    server,
                },
            );
        }
    }
    (applied, conflicts)
}

/// The reason to refuse an event that changes a field it may not, or
/// nothing when every field is allowed.
pub fn not_editable(changes: &Fields, allowed: &[&str]) -> Option<String> {
    let mut refused: Vec<&str> = changes
        .keys()
        .map(String::as_str)
        .filter(|field| !allowed.contains(field))
        .collect();
    if refused.is_empty() {
        return None;
    }
    refused.sort_unstable();
    Some(format!("not_editable: {}", refused.join(", ")))
}

/// Why the server did not record an event, and so not the write.
#[derive(Debug, Error)]
pub enum RecordError {
    #[error("This trial has reached its limit. Sign up to keep going.")]
    TrialWritesFull,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Records an event the server answered, in the same transaction as the
/// row, so a retry gets the same answer. Refused for a trial user who
/// has made all the writes a trial may, see `trial_users::MAX_WRITES`:
/// the caller's transaction ends there and nothing is written.
pub async fn record(
    tx: &mut Transaction,
    user_id: Uuid,
    event: &Event,
    answer: &Answer,
) -> Result<(), RecordError> {
    if crate::accounts::trial_users::writes_are_full_in(tx, user_id).await? {
        return Err(RecordError::TrialWritesFull);
    }
    let status = match answer.status {
        AnswerStatus::Applied => EventStatus::Applied,
        AnswerStatus::Conflict => EventStatus::Conflict,
        AnswerStatus::Rejected => EventStatus::Rejected,
    };
    let stored = StoredEvent {
        event: event.clone(),
        user_id,
        status,
        conflicts: answer.conflicts.clone(),
        version: answer.version,
        last_error: answer.reason.clone(),
        attempts: 0,
    };
    Ok(app_events::create(tx, &stored).await?)
}

/// Queues an event a device applied, for the next push. A local user's
/// is not queued: they have no server to push to, and their rows are
/// queued as creates if they ever move to an account, see `local_users`.
pub async fn queue(tx: &mut Transaction, user_id: Uuid, event: &Event) -> Result<(), sqlx::Error> {
    if crate::app::local_users::is_signed_in_in(tx).await? {
        return Ok(());
    }
    let stored = StoredEvent {
        event: event.clone(),
        user_id,
        status: EventStatus::Pending,
        conflicts: Conflicts::new(),
        version: None,
        last_error: None,
        attempts: 0,
    };
    app_events::create(tx, &stored).await
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::sync::test_notes;

    fn fields(value: Value) -> Fields {
        match value {
            Value::Object(fields) => fields,
            _ => panic!("fields are an object"),
        }
    }

    #[tokio::test]
    async fn events_retry_transitions_roll_back_and_restart_without_stale_metadata() {
        crate::database::with_each_database(|database| async move {
            let deployment =
                crate::deployment::Deployment::new(database, crate::deployment::Role::Server);
            let user = crate::app::users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("user");
            let event = Event::create(
                test_notes::TABLE,
                Uuid::now_v7(),
                fields(json!({"body": "pending"})),
                crate::deployment::now(),
            );
            let mut tx = deployment.begin().await.expect("transaction");
            queue(&mut tx, user.id, &event).await.expect("queue");
            tx.commit().await.expect("commit");
            let mut tx = deployment.begin().await.expect("transaction");
            for attempt in 1..=app_events::MAX_ATTEMPTS {
                assert_eq!(
                    app_events::count_attempt(&mut tx, event.id, "refused")
                        .await
                        .expect("attempt"),
                    attempt == app_events::MAX_ATTEMPTS
                );
            }
            let refused = app_events::get(&mut tx, event.id)
                .await
                .expect("read")
                .expect("event");
            assert_eq!(refused.status, EventStatus::Conflict);
            assert_eq!(refused.attempts, app_events::MAX_ATTEMPTS);
            assert!(!refused.conflicts.is_empty());
            drop(tx);
            let mut tx = deployment.begin().await.expect("transaction");
            let pending = app_events::get(&mut tx, event.id)
                .await
                .expect("read")
                .expect("event");
            assert_eq!(pending.status, EventStatus::Pending);
            assert_eq!(pending.attempts, 0);
            assert!(pending.last_error.is_none());
            app_events::refuse(&mut tx, event.id, "refused")
                .await
                .expect("refuse");
            app_events::try_again(&mut tx, event.id)
                .await
                .expect("retry");
            tx.commit().await.expect("commit");
            let mut tx = deployment.begin().await.expect("transaction");
            let retried = app_events::get(&mut tx, event.id)
                .await
                .expect("read")
                .expect("event");
            assert_eq!(retried.status, EventStatus::Pending);
            assert_eq!(retried.attempts, 0);
            assert!(retried.conflicts.is_empty());
            assert!(retried.last_error.is_none());
            assert_eq!(retried.event, pending.event);
        })
        .await;
    }

    #[test]
    fn events_merge_applies_everything_at_the_servers_version() {
        let (applied, conflicts) = merge(
            &fields(json!({"body": "a"})),
            2,
            2,
            &fields(json!({"body": "a"})),
            &fields(json!({"body": "b"})),
        );
        assert_eq!(applied, fields(json!({"body": "b"})));
        assert!(conflicts.is_empty());
    }

    #[test]
    fn events_merge_keeps_the_change_that_reached_the_server_first() {
        let (applied, conflicts) = merge(
            &fields(json!({"body": "b"})),
            2,
            1,
            &fields(json!({"body": "a"})),
            &fields(json!({"body": "c"})),
        );
        assert!(applied.is_empty());
        assert_eq!(
            conflicts.get("body"),
            Some(&FieldConflict {
                yours: json!("c"),
                server: json!("b")
            })
        );
    }

    #[test]
    fn events_merge_applies_a_field_nobody_else_changed() {
        let (applied, conflicts) = merge(
            &fields(json!({"body": "b", "deleted_at": null})),
            2,
            1,
            &fields(json!({"deleted_at": null})),
            &fields(json!({"deleted_at": "2026-11-08T00:00:00Z"})),
        );
        assert_eq!(
            applied,
            fields(json!({"deleted_at": "2026-11-08T00:00:00Z"}))
        );
        assert!(conflicts.is_empty());
    }

    #[test]
    fn events_merge_skips_a_value_the_server_already_holds() {
        let (applied, conflicts) = merge(
            &fields(json!({"body": "c"})),
            2,
            1,
            &fields(json!({"body": "a"})),
            &fields(json!({"body": "c"})),
        );
        assert!(applied.is_empty());
        assert!(conflicts.is_empty());
    }

    #[test]
    fn events_not_editable_names_the_refused_fields() {
        let changes = fields(json!({"body": "x", "version": 9, "id": "y"}));
        assert_eq!(
            not_editable(&changes, &["body"]),
            Some("not_editable: id, version".to_string())
        );
        assert_eq!(not_editable(&changes, &["body", "id", "version"]), None);
    }
}
