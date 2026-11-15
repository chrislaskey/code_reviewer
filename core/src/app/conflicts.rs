//! Conflicts: the changes a device made that the server did not keep,
//! kept until the person resolves each, see "Conflicts" in
//! `docs/design/offline-sync.md`. Two kinds end up here. A change to a
//! field someone else changed first, which the person reapplies or lets
//! go. And a change the server refused, or said no to
//! `events::MAX_ATTEMPTS` times: for a file added on this device, whose
//! bytes are still here, the person tries again or lets it go, see
//! `docs/design/offline-files-decisions.md`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::communities::{self, CommunityError};
use crate::database::Transaction;
use crate::deployment::{Deployment, now};
use crate::events::{self, AnswerStatus, Event, EventKind, EventStatus, Fields, StoredEvent};
use crate::files::{self, FileError};
use crate::posts::{self, PostError};
use crate::users::{self, User, UserError};

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
    #[error(transparent)]
    Post(#[from] PostError),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    Community(#[from] CommunityError),
    #[error(transparent)]
    User(#[from] UserError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Every conflict, newest first.
pub async fn list(deployment: &Deployment) -> Result<Vec<Conflict>, ConflictError> {
    let mut tx = deployment.begin().await?;
    let mut conflicts = Vec::new();
    for stored in events::conflicts(&mut tx).await? {
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
    let answer = match stored.event.table.as_str() {
        files::TABLE => {
            files::apply_in(&mut tx, deployment.role, user, &event, true)
                .await?
                .answer
        }
        communities::TABLE => {
            communities::apply_in(&mut tx, deployment.role, user, &event)
                .await?
                .answer
        }
        users::TABLE => {
            users::apply_in(&mut tx, deployment.role, user, &event)
                .await?
                .answer
        }
        _ => {
            posts::apply_in(&mut tx, deployment.role, user, &event)
                .await?
                .answer
        }
    };
    if answer.status == AnswerStatus::Rejected {
        return Err(ConflictError::Rejected(answer.reason.unwrap_or_default()));
    }
    events::delete(&mut tx, id).await?;
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
    events::try_again(&mut tx, id).await?;
    tx.commit().await?;
    Ok(())
}

/// Drops the conflict; nothing is sent. A file the server refused goes
/// from the device with it, its row, its other events and its bytes,
/// since nothing of it ever reached the server.
pub async fn let_go(deployment: &Deployment, id: Uuid) -> Result<(), ConflictError> {
    let mut tx = deployment.begin().await?;
    let stored = get(&mut tx, id).await?;
    let refused_file = stored.event.table == files::TABLE && stored.event.kind == EventKind::Create;
    if refused_file {
        files::forget_in(&mut tx, stored.event.row_id).await?;
    } else {
        events::delete(&mut tx, id).await?;
    }
    tx.commit().await?;
    if refused_file {
        files::delete_local_copy(deployment, stored.event.row_id).await?;
    }
    Ok(())
}

async fn get(tx: &mut Transaction, id: Uuid) -> Result<StoredEvent, ConflictError> {
    match events::get(tx, id).await? {
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
    Ok(match event.table.as_str() {
        posts::TABLE => posts::get_in(tx, event.row_id)
            .await?
            .filter(|post| post.deleted_at.is_none())
            .map(|post| {
                Ok::<Row, PostError>(Row {
                    about: post.body.lines().next().unwrap_or_default().to_string(),
                    version: post.version,
                    fields: post.fields(&["body", "deleted_at"])?,
                })
            })
            .transpose()?,
        files::TABLE => files::get_in(tx, event.row_id)
            .await?
            .filter(|file| file.deleted_at.is_none())
            .map(|file| {
                Ok::<Row, FileError>(Row {
                    about: file.name.clone(),
                    version: file.version,
                    fields: file.fields(&["name", "deleted_at"])?,
                })
            })
            .transpose()?,
        communities::TABLE => communities::get_in(tx, event.row_id)
            .await?
            .filter(|community| community.deleted_at.is_none())
            .map(|community| {
                Ok::<Row, CommunityError>(Row {
                    about: community.name.clone(),
                    version: community.version,
                    fields: community.fields(communities::UPDATE_FIELDS)?,
                })
            })
            .transpose()?,
        users::TABLE => users::get_in(tx, event.row_id)
            .await?
            .filter(|user| user.deleted_at.is_none())
            .map(|user| Row {
                about: user.name.clone(),
                version: user.version,
                fields: Fields::from_iter([("name".to_string(), Value::String(user.name))]),
            }),
        _ => None,
    })
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
