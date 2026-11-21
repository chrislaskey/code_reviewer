//! Conflicts: the changes a device made that the server did not keep,
//! kept until the person resolves each, see "Conflicts" in
//! `docs/design/offline-sync.md`. Two kinds end up here. A change to a
//! field someone else changed first, which the person reapplies or lets
//! go. And a change the server refused, or said no to
//! `app_events::MAX_ATTEMPTS` times: for a file added on this device, whose
//! bytes are still here, the person tries again or lets it go, see
//! `docs/design/offline-files-decisions.md`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::app::synced_tables;
use crate::data::app_events::{self, Event, EventKind, EventStatus, Fields, StoredEvent};
use crate::data::app_users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, now};
use crate::sync::bytes::{self, BytesError};
use crate::sync::events::AnswerStatus;
use crate::sync::synced_table::TableError;

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
    let synced = synced_tables::get(&stored.event.table).ok_or(ConflictError::Gone)?;
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

/// Drops the conflict; nothing is sent. A file the server refused goes
/// from the device with it, its row, its other events and its bytes,
/// since nothing of it ever reached the server.
pub async fn let_go(deployment: &Deployment, id: Uuid) -> Result<(), ConflictError> {
    let mut tx = deployment.begin().await?;
    let stored = get(&mut tx, id).await?;
    let refused_with_bytes =
        stored.event.kind == EventKind::Create && bytes::of_table(&stored.event.table).is_ok();
    if refused_with_bytes {
        tx.commit().await?;
        return Ok(bytes::forget(deployment, &stored.event.table, stored.event.row_id).await?);
    }
    app_events::delete(&mut tx, id).await?;
    tx.commit().await?;
    Ok(())
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
    let Some(synced) = synced_tables::get(&event.table) else {
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
