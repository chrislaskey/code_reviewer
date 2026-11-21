//! Event records: the `Event` a person made to a synced row, as
//! `app_events` keeps it, `StoredEvent`, and the reads and writes of the
//! server's log and a device's queue. How an event is applied, merged and
//! answered is `app::events` and each resource's `apply_event`.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::database::Transaction;

#[cfg(feature = "postgres")]
mod postgres;
mod sqlite;

macro_rules! query {
    ($tx:expr, $function:ident($($argument:expr),* $(,)?)) => {
        match $tx {
            Transaction::Sqlite(tx) => sqlite::$function(&mut **tx, $($argument),*).await,
            #[cfg(feature = "postgres")]
            Transaction::Postgres(tx) => postgres::$function(&mut **tx, $($argument),*).await,
        }
    };
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

/// Keeps an event in the log or the queue.
pub async fn create(tx: &mut Transaction, event: &StoredEvent) -> Result<(), sqlx::Error> {
    query!(tx, insert(event))
}

pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<StoredEvent>, sqlx::Error> {
    query!(tx, get(id))
}

/// The events not yet answered, in the order they were made.
pub async fn pending(tx: &mut Transaction) -> Result<Vec<StoredEvent>, sqlx::Error> {
    query!(tx, list_by_status(EventStatus::Pending))
}

/// The conflicts the person has not resolved, newest first.
pub async fn conflicts(tx: &mut Transaction) -> Result<Vec<StoredEvent>, sqlx::Error> {
    let mut conflicts = query!(tx, list_by_status(EventStatus::Conflict))?;
    conflicts.reverse();
    Ok(conflicts)
}

/// The events of the table's rows, in the order they were made.
pub async fn list_of_table(
    tx: &mut Transaction,
    table: &str,
) -> Result<Vec<StoredEvent>, sqlx::Error> {
    query!(tx, list_by_table(table))
}

pub async fn count(tx: &mut Transaction, status: EventStatus) -> Result<i64, sqlx::Error> {
    query!(tx, count_by_status(status))
}

/// Whether the row has an event not yet answered, in which case a row
/// from the server must not replace it.
pub async fn has_pending(
    tx: &mut Transaction,
    table: &str,
    row_id: Uuid,
) -> Result<bool, sqlx::Error> {
    query!(tx, has_pending(table, row_id))
}

pub async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
    query!(tx, delete(id))
}

pub async fn delete_pending_of_row(
    tx: &mut Transaction,
    table: &str,
    row_id: Uuid,
) -> Result<(), sqlx::Error> {
    query!(tx, delete_pending_of_row(table, row_id))
}

/// Removes every event of the row, whatever its status.
pub async fn delete_of_row(
    tx: &mut Transaction,
    table: &str,
    row_id: Uuid,
) -> Result<(), sqlx::Error> {
    query!(tx, delete_of_row(table, row_id))
}

pub async fn mark_conflict(
    tx: &mut Transaction,
    id: Uuid,
    conflicts: &Conflicts,
) -> Result<(), sqlx::Error> {
    query!(tx, mark_conflict(id, conflicts))
}

/// Keeps why a pending event did not go, without counting a try: the
/// server or the bucket was not reached, or is failing.
pub async fn mark_error(tx: &mut Transaction, id: Uuid, reason: &str) -> Result<(), sqlx::Error> {
    query!(tx, mark_error(id, reason))
}

/// Counts a try the server or the bucket said no to, with why. At
/// `MAX_ATTEMPTS` the event is refused; true when it now is.
pub async fn count_attempt(
    tx: &mut Transaction,
    id: Uuid,
    reason: &str,
) -> Result<bool, sqlx::Error> {
    let attempts = query!(&mut *tx, add_attempt(id, reason))?;
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
    let Some(stored) = get(&mut *tx, id).await? else {
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
    query!(tx, mark_refused(id, &conflicts, reason))
}

/// Puts a refused event back in the queue, its tries counted from none.
pub async fn try_again(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
    query!(tx, mark_pending(id))
}

/// Explicit maintenance operations; bulk SQL and cascades do not run per-row hooks.
pub(crate) mod maintenance {
    use super::*;

    /// Removes events for business rows that no longer exist.
    pub(crate) async fn delete_of_missing_rows(tx: &mut Transaction) -> Result<u64, sqlx::Error> {
        query!(tx, delete_of_missing_rows())
    }
}
