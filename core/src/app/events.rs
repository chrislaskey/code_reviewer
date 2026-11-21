//! Events: one change a person made to a row, the only way a synced row
//! changes, see "Events" in `docs/design/offline-sync.md`. A resource
//! builds one for each save and applies it with the merge here; the
//! server answers a pushed event the same way, since both run the
//! resource's `apply_event`. The `app_events` table is the server's log
//! of every event answered and a device's queue of events not yet
//! answered, with the conflicts the person has not resolved.
//!
//! An event in the queue is tried by every sync, but not for ever: each
//! time the server or the bucket says no to it and it stays pending
//! counts, `count_attempt`, and at `MAX_ATTEMPTS` it is refused, kept
//! as a conflict with the reason until the person tries it again or
//! lets it go. Not reaching the server counts nothing, and neither does
//! a server or a bucket that is failing, so an outage of any length
//! refuses nothing.

use crate::data::events as events_records;
use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;
use uuid::Uuid;

use crate::app::sync::TableRow;
use crate::database::Transaction;

/// A record assembled by this resource's workflow.
pub(crate) struct AcceptedEvent<'a>(&'a StoredEvent);

impl AcceptedEvent<'_> {
    /// Borrows the assembled record for persistence.
    pub(crate) fn row(&self) -> &StoredEvent {
        self.0
    }
}

/// How many times the server or the bucket may say no to an event
/// before a device stops trying it.
pub const MAX_ATTEMPTS: i64 = 30;

/// A row's fields as they travel on the wire, by name.
pub type Fields = Map<String, Value>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    Create,
    Update,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::Create => "create",
            EventKind::Update => "update",
        }
    }

    pub fn parse(text: &str) -> Option<EventKind> {
        match text {
            "create" => Some(EventKind::Create),
            "update" => Some(EventKind::Update),
            _ => None,
        }
    }
}

/// One change, sent once and answered once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// From the device that made it: the idempotency key.
    pub id: Uuid,
    pub table: String,
    pub row_id: Uuid,
    #[serde(rename = "type")]
    pub kind: EventKind,
    /// The row's version the change started from, 0 for a create.
    pub base_version: i64,
    /// The changed fields' values as the device saw them before the change.
    pub base: Fields,
    /// The values the person chose.
    pub changes: Fields,
    /// The clock of the device that made it, for ordering and display only.
    pub made_at: DateTime<Utc>,
}

impl Event {
    pub fn create(table: &str, row_id: Uuid, changes: Fields, made_at: DateTime<Utc>) -> Event {
        Event {
            id: Uuid::now_v7(),
            table: table.to_string(),
            row_id,
            kind: EventKind::Create,
            base_version: 0,
            base: Fields::new(),
            changes,
            made_at,
        }
    }

    /// When the row a create makes was made: the clock of the device
    /// the person made it on, so a row made offline keeps its time, or
    /// this deployment's `now` when the device's clock is ahead of it.
    pub fn row_inserted_at(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        self.made_at.min(now)
    }

    pub fn update(
        table: &str,
        row_id: Uuid,
        base_version: i64,
        base: Fields,
        changes: Fields,
        made_at: DateTime<Utc>,
    ) -> Event {
        Event {
            id: Uuid::now_v7(),
            table: table.to_string(),
            row_id,
            kind: EventKind::Update,
            base_version,
            base,
            changes,
            made_at,
        }
    }
}

/// A field someone else changed first: the person's value, and the
/// server's that stays.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldConflict {
    pub yours: Value,
    pub server: Value,
}

pub type Conflicts = BTreeMap<String, FieldConflict>;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventStatus {
    /// On a device: not yet answered by the server.
    Pending,
    Applied,
    Conflict,
    Rejected,
}

impl EventStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            EventStatus::Pending => "pending",
            EventStatus::Applied => "applied",
            EventStatus::Conflict => "conflict",
            EventStatus::Rejected => "rejected",
        }
    }

    pub fn parse(text: &str) -> Option<EventStatus> {
        match text {
            "pending" => Some(EventStatus::Pending),
            "applied" => Some(EventStatus::Applied),
            "conflict" => Some(EventStatus::Conflict),
            "rejected" => Some(EventStatus::Rejected),
            _ => None,
        }
    }
}

/// An event as `app_events` keeps it: with its answer on the server, or
/// its place in the queue on a device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredEvent {
    #[serde(flatten)]
    pub event: Event,
    /// Who made it: on the server the user the token stood for, on a
    /// device the user signed in.
    pub user_id: Uuid,
    pub status: EventStatus,
    #[serde(default)]
    pub conflicts: Conflicts,
    /// On the server, the row's version after the event.
    pub version: Option<i64>,
    /// On the server, why it was rejected; on a device, the reason of a
    /// rejection that kept the event pending, or why it was refused.
    pub last_error: Option<String>,
    /// On a device, how many times the server or the bucket said no to
    /// it while it stayed pending.
    #[serde(default)]
    pub attempts: i64,
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
/// `version`, field by field. At the server's version every change
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
        return (changes.clone(), Conflicts::new());
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
    Ok(events_records::create(tx, AcceptedEvent(&stored)).await?)
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
    events_records::create(tx, AcceptedEvent(&stored)).await
}

pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<StoredEvent>, sqlx::Error> {
    events_records::get(tx, id).await
}

/// The events not yet answered, in the order they were made.
pub async fn pending(tx: &mut Transaction) -> Result<Vec<StoredEvent>, sqlx::Error> {
    events_records::list(tx, events_records::Filter::Status(EventStatus::Pending)).await
}

/// The conflicts the person has not resolved, newest first.
pub async fn conflicts(tx: &mut Transaction) -> Result<Vec<StoredEvent>, sqlx::Error> {
    let mut conflicts =
        events_records::list(tx, events_records::Filter::Status(EventStatus::Conflict)).await?;
    conflicts.reverse();
    Ok(conflicts)
}

/// The events of the table's rows, in the order they were made.
pub async fn list_of_table(
    tx: &mut Transaction,
    table: &str,
) -> Result<Vec<StoredEvent>, sqlx::Error> {
    events_records::list(tx, events_records::Filter::Table(table)).await
}

pub async fn count(tx: &mut Transaction, status: EventStatus) -> Result<i64, sqlx::Error> {
    events_records::count(tx, status).await
}

/// Whether the row has an event not yet answered, in which case a row
/// from the server must not replace it.
pub async fn has_pending(
    tx: &mut Transaction,
    table: &str,
    row_id: Uuid,
) -> Result<bool, sqlx::Error> {
    events_records::has_pending(tx, table, row_id).await
}

pub async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
    events_records::delete(tx, id).await
}

pub async fn delete_pending_of_row(
    tx: &mut Transaction,
    table: &str,
    row_id: Uuid,
) -> Result<(), sqlx::Error> {
    events_records::delete_pending_of_row(tx, table, row_id).await
}

/// Removes every event of the row, whatever its status.
pub async fn delete_of_row(
    tx: &mut Transaction,
    table: &str,
    row_id: Uuid,
) -> Result<(), sqlx::Error> {
    events_records::delete_of_row(tx, table, row_id).await
}

pub async fn mark_conflict(
    tx: &mut Transaction,
    id: Uuid,
    conflicts: &Conflicts,
) -> Result<(), sqlx::Error> {
    events_records::processing::mark_conflict(tx, id, conflicts).await
}

/// Keeps why a pending event did not go, without counting a try: the
/// server or the bucket was not reached, or is failing.
pub async fn mark_error(tx: &mut Transaction, id: Uuid, reason: &str) -> Result<(), sqlx::Error> {
    events_records::processing::mark_error(tx, id, reason).await
}

/// Counts a try the server or the bucket said no to, with why. At
/// `MAX_ATTEMPTS` the event is refused; true when it now is.
pub async fn count_attempt(
    tx: &mut Transaction,
    id: Uuid,
    reason: &str,
) -> Result<bool, sqlx::Error> {
    let attempts = events_records::processing::add_attempt(&mut *tx, id, reason).await?;
    if attempts < MAX_ATTEMPTS {
        return Ok(false);
    }
    refuse(tx, id, &format!("tried {attempts} times: {reason}")).await?;
    Ok(true)
}

/// Stops trying a pending event: it is kept as a conflict with nothing
/// on the server's side, and why, until the person tries it again or
/// lets it go.
pub async fn refuse(tx: &mut Transaction, id: Uuid, reason: &str) -> Result<(), sqlx::Error> {
    let Some(stored) = get(tx, id).await? else {
        return Ok(());
    };
    let conflicts: Conflicts = stored
        .event
        .changes
        .iter()
        .map(|(field, yours)| {
            (
                field.clone(),
                FieldConflict {
                    yours: yours.clone(),
                    server: Value::Null,
                },
            )
        })
        .collect();
    events_records::processing::mark_refused(tx, id, &conflicts, reason).await
}

/// Puts a refused event back in the queue, its tries counted from none.
pub async fn try_again(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
    events_records::processing::mark_pending(tx, id).await
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

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
                crate::app::posts::TABLE,
                Uuid::now_v7(),
                fields(json!({"body": "pending"})),
                crate::deployment::now(),
            );
            let mut tx = deployment.begin().await.expect("transaction");
            queue(&mut tx, user.id, &event).await.expect("queue");
            tx.commit().await.expect("commit");
            let mut tx = deployment.begin().await.expect("transaction");
            for attempt in 1..=MAX_ATTEMPTS {
                assert_eq!(
                    count_attempt(&mut tx, event.id, "refused")
                        .await
                        .expect("attempt"),
                    attempt == MAX_ATTEMPTS
                );
            }
            let refused = get(&mut tx, event.id).await.expect("read").expect("event");
            assert_eq!(refused.status, EventStatus::Conflict);
            assert_eq!(refused.attempts, MAX_ATTEMPTS);
            assert!(!refused.conflicts.is_empty());
            drop(tx);
            let mut tx = deployment.begin().await.expect("transaction");
            let pending = get(&mut tx, event.id).await.expect("read").expect("event");
            assert_eq!(pending.status, EventStatus::Pending);
            assert_eq!(pending.attempts, 0);
            assert!(pending.last_error.is_none());
            refuse(&mut tx, event.id, "refused").await.expect("refuse");
            try_again(&mut tx, event.id).await.expect("retry");
            tx.commit().await.expect("commit");
            let mut tx = deployment.begin().await.expect("transaction");
            let retried = get(&mut tx, event.id).await.expect("read").expect("event");
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
