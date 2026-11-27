//! Conflicts: the changes a device made that the server did not keep,
//! kept until the person resolves each, see "Conflicts" in
//! `docs/design/offline-sync.md`. Two kinds end up here. A change to a
//! field someone else changed first, which the person reapplies or lets
//! go. And a change the server refused, or said no to
//! `app_events::MAX_ATTEMPTS` times. A refused create keeps its row on the
//! device, a post or a file and its bytes, and the person tries again or
//! lets it go, see `docs/design/offline-files-decisions.md`. Never is a
//! refused create dropped without the person, see "What the person must
//! still be able to see" in `docs/design/backend-protocol.md`.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::data::app_events::{self, Event, EventKind, EventStatus, Fields, StoredEvent};
use crate::data::app_users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, now};
use crate::sync::bytes::{self, BytesError};
use crate::sync::events::AnswerStatus;
use crate::sync::synced_table::TableError;
use crate::sync::tables;

/// A conflicted change as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub id: Uuid,
    pub table: String,
    pub row_id: Uuid,
    /// When the change was made, on the device's clock.
    pub made_at: DateTime<Utc>,
    /// What it was about, a post's first line or a file's name; none
    /// when the row is gone.
    pub about: Option<String>,
    /// Why the server refused it, when it did.
    pub reason: Option<String>,
    pub fields: Vec<ConflictField>,
    pub resolve: Resolve,
}

/// What the person can do about a conflict, besides letting it go.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolve {
    /// Queue their values again over the row as it now stands.
    Reapply,
    /// Send the refused create again as it was.
    TryAgain,
    /// The row is gone: nothing but letting it go.
    LetGoOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictField {
    pub name: String,
    /// The person's value.
    pub yours: Value,
    /// The value on this device now, the server's as of the last sync.
    pub now: Value,
}

#[derive(Debug, Error)]
pub enum ConflictError {
    #[error("conflict {id} not found")]
    NotFound { id: Uuid },
    #[error("what the change was about is gone; it can only be let go")]
    Gone,
    #[error("this change is tried again, not reapplied")]
    NotReapplied,
    #[error("only a refused change is tried again")]
    NotTriedAgain,
    #[error("the change was refused: {0}")]
    Rejected(String),
    /// A synced table's own module failed.
    #[error(transparent)]
    Table(#[from] TableError),
    #[error(transparent)]
    Bytes(#[from] BytesError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Every conflict, newest first.
pub async fn list(deployment: &Deployment) -> Result<Vec<Conflict>, ConflictError> {
    let mut tx = deployment.begin().await?;
    let mut conflicts = Vec::new();
    for stored in app_events::conflicts(&mut tx).await? {
        let row = row_of(&mut tx, &stored.event).await?;
        conflicts.push(conflict(stored, row));
    }
    tx.commit().await?;
    Ok(conflicts)
}

/// Queues the person's values again as a new change of the user's from
/// the row as it stands now, and drops the conflict.
pub async fn reapply(deployment: &Deployment, user: &User, id: Uuid) -> Result<(), ConflictError> {
    let mut tx = deployment.begin().await?;
    let stored = get(&mut tx, id).await?;
    if stored.event.kind == EventKind::Create {
        return Err(ConflictError::NotReapplied);
    }
    let Some(row) = row_of(&mut tx, &stored.event).await? else {
        return Err(ConflictError::Gone);
    };
    let changes: Fields = stored
        .conflicts
        .iter()
        .map(|(name, conflict)| (name.clone(), conflict.yours.clone()))
        .collect();
    let mut base = row.fields;
    base.retain(|name, _| changes.contains_key(name));
    let event = Event::update(
        &stored.event.table,
        stored.event.row_id,
        row.version,
        base,
        changes,
        now(),
    );
    let synced = tables::get(&stored.event.table).ok_or(ConflictError::Gone)?;
    let answer = synced
        .apply_in(&mut tx, deployment.role, user, &event)
        .await?;
    if answer.status == AnswerStatus::Rejected {
        return Err(ConflictError::Rejected(answer.reason.unwrap_or_default()));
    }
    app_events::delete(&mut tx, id).await?;
    tx.commit().await?;
    Ok(())
}

/// Puts a refused create back in the queue as it was, its tries counted
/// from none, for the next sync to send: after the server's rules or
/// the app changed, say.
pub async fn try_again(deployment: &Deployment, id: Uuid) -> Result<(), ConflictError> {
    let mut tx = deployment.begin().await?;
    let stored = get(&mut tx, id).await?;
    if stored.event.kind != EventKind::Create {
        return Err(ConflictError::NotTriedAgain);
    }
    app_events::try_again(&mut tx, id).await?;
    tx.commit().await?;
    Ok(())
}

/// Deletes the conflict, which the page calls letting it go; nothing is
/// sent. A refused create goes from the device with its row, its other
/// events and the rows waiting with it, since nothing of them ever
/// reached the server; a file's bytes go too.
pub async fn delete(deployment: &Deployment, id: Uuid) -> Result<(), ConflictError> {
    let mut tx = deployment.begin().await?;
    let stored = get(&mut tx, id).await?;
    if stored.event.kind != EventKind::Create {
        app_events::delete(&mut tx, id).await?;
        tx.commit().await?;
        return Ok(());
    }
    if bytes::of_table(&stored.event.table).is_ok() {
        tx.commit().await?;
        return Ok(bytes::forget(deployment, &stored.event.table, stored.event.row_id).await?);
    }
    let rows = rows_waiting_with(&mut tx, &stored.event).await?;
    // The rows pointing at others go first
    for (table, row_id) in rows.iter().rev() {
        app_events::delete_of_row(&mut tx, table, *row_id).await?;
        if let Some(synced) = tables::get(table) {
            synced.delete_row(&mut tx, *row_id).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

/// Adds the rows whose events wait for the person: those whose create
/// the server refused, and those whose creates point at a row held back,
/// which the server would answer by a row it never had.
pub(crate) async fn hold_back_refused(
    tx: &mut Transaction,
    held_back: &mut BTreeSet<Uuid>,
) -> Result<(), sqlx::Error> {
    let conflicts = app_events::conflicts(&mut *tx).await?;
    let refused = conflicts
        .iter()
        .filter(|stored| stored.event.kind == EventKind::Create)
        .map(|stored| stored.event.row_id);
    held_back.extend(refused);
    let pending = app_events::pending(&mut *tx).await?;
    loop {
        let pointing: Vec<Uuid> = pending
            .iter()
            .filter(|stored| !held_back.contains(&stored.event.row_id))
            .filter(|stored| creates_row_pointing_at(&stored.event, held_back))
            .map(|stored| stored.event.row_id)
            .collect();
        if pointing.is_empty() {
            return Ok(());
        }
        held_back.extend(pointing);
    }
}

/// Whether the event creates a row whose foreign keys point at one of
/// the rows, a post's row in a community at its post, say.
pub(crate) fn creates_row_pointing_at(event: &Event, rows: &BTreeSet<Uuid>) -> bool {
    let Some(synced) = tables::get(&event.table) else {
        return false;
    };
    event.kind == EventKind::Create
        && synced.foreign_keys().iter().any(|(column, _)| {
            event
                .changes
                .get(*column)
                .and_then(Value::as_str)
                .and_then(|id| Uuid::parse_str(id).ok())
                .is_some_and(|id| rows.contains(&id))
        })
}

/// The refused create's row and the rows whose creates wait with it, in
/// the order they point: each after the row it points at.
async fn rows_waiting_with(
    tx: &mut Transaction,
    refused: &Event,
) -> Result<Vec<(String, Uuid)>, sqlx::Error> {
    let mut events = app_events::pending(&mut *tx).await?;
    events.extend(app_events::conflicts(&mut *tx).await?);
    let mut rows = vec![(refused.table.clone(), refused.row_id)];
    let mut ids = BTreeSet::from([refused.row_id]);
    loop {
        let pointing: Vec<&Event> = events
            .iter()
            .map(|stored| &stored.event)
            .filter(|event| !ids.contains(&event.row_id))
            .filter(|event| creates_row_pointing_at(event, &ids))
            .collect();
        if pointing.is_empty() {
            return Ok(rows);
        }
        for event in pointing {
            ids.insert(event.row_id);
            rows.push((event.table.clone(), event.row_id));
        }
    }
}

async fn get(tx: &mut Transaction, id: Uuid) -> Result<StoredEvent, ConflictError> {
    match app_events::get(tx, id).await? {
        Some(stored) if stored.status == EventStatus::Conflict => Ok(stored),
        _ => Err(ConflictError::NotFound { id }),
    }
}

/// The row a conflict is about, as much of it as the page and a reapply
/// need.
struct Row {
    /// A post's first line, a file's or a community's name.
    about: String,
    version: i64,
    /// The fields an event may change, as they stand now.
    fields: Fields,
}

/// The live row the event is about, of a table whose conflicts are
/// shown; none when it is gone or deleted.
async fn row_of(tx: &mut Transaction, event: &Event) -> Result<Option<Row>, ConflictError> {
    let Some(synced) = tables::get(&event.table) else {
        return Ok(None);
    };
    let Some(current) = synced.current_fields(tx, event.row_id).await? else {
        return Ok(None);
    };
    let about = synced.label(tx, event.row_id).await?.unwrap_or_default();
    Ok(Some(Row {
        about,
        version: current.version,
        fields: current.fields,
    }))
}

fn conflict(stored: StoredEvent, row: Option<Row>) -> Conflict {
    let resolve = match (stored.event.kind, &row) {
        (EventKind::Create, _) => Resolve::TryAgain,
        (EventKind::Update, Some(_)) => Resolve::Reapply,
        (EventKind::Update, None) => Resolve::LetGoOnly,
    };
    let now = row.as_ref().map(|row| &row.fields);
    Conflict {
        id: stored.event.id,
        table: stored.event.table,
        row_id: stored.event.row_id,
        made_at: stored.event.made_at,
        fields: stored
            .conflicts
            .iter()
            .map(|(name, field)| ConflictField {
                name: name.clone(),
                yours: field.yours.clone(),
                now: now
                    .and_then(|fields| fields.get(name))
                    .cloned()
                    .unwrap_or(Value::Null),
            })
            .collect(),
        about: row.map(|row| row.about),
        reason: stored.last_error,
        resolve,
    }
}
