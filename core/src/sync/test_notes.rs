//! Test notes: the framework's own synced table, built for tests alone,
//! so sync, conflicts, pending changes, purge, a local user's move and
//! an account's delete are tested on a table no app deletes, see
//! `docs/design/examples-delete.md`. A note is some text with an owner,
//! in one community or none. Its owner writes, edits and deletes it; its
//! owner and the community's live members see it. It goes with its
//! owner's account and with its community, and with its owner's
//! membership there, which rejoining brings back.
//!
//! Built with `#[cfg(any(test, feature = "test-tables"))]`: the web
//! server's and the app's tests turn the feature on as a dev-dependency,
//! so it never ships. Its table is created after the migrations, by
//! `SQLITE_TABLE_CREATE_IF_NOT_EXISTS` or
//! `POSTGRES_TABLE_CREATE_IF_NOT_EXISTS`, since a test may open a file
//! again.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::data::app_communities::Community;
use crate::data::app_events::{self, Conflicts, Event, EventKind, Fields};
use crate::data::app_sync_state::Cursor;
use crate::data::app_users::User;
use crate::data::app_users_communities::{self, Membership};
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::helpers::json;
use crate::sync::TableRow;
use crate::sync::events::{self, Answer, AnswerStatus};
use crate::sync::synced_table::{CurrentFields, LocalCreate, SyncedTable, TableError};

#[cfg(feature = "postgres")]
mod postgres;
mod sqlite;

pub const TABLE: &str = "app_test_notes";
/// The longest body, in characters.
pub const MAX_BODY_LENGTH: usize = 10_000;

/// The fields an event may set when it creates a note.
const CREATE_FIELDS: &[&str] = &["body", "community_id"];
/// The fields an event may change on a note.
const UPDATE_FIELDS: &[&str] = &["body", "community_id", "deleted_at"];

/// The table, on SQLite.
pub const SQLITE_TABLE_CREATE_IF_NOT_EXISTS: &str = "
CREATE TABLE IF NOT EXISTS app_test_notes (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES app_users (id) ON DELETE CASCADE,
  community_id TEXT REFERENCES app_communities (id) ON DELETE CASCADE,
  body TEXT NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  last_synced_at TEXT,
  deleted_at TEXT,
  deleted_by_id TEXT
);
CREATE INDEX IF NOT EXISTS app_test_notes_last_synced_at ON app_test_notes (last_synced_at, id);
";

/// The table, on PostgreSQL.
pub const POSTGRES_TABLE_CREATE_IF_NOT_EXISTS: &str = "
CREATE TABLE IF NOT EXISTS app_test_notes (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES app_users (id) ON DELETE CASCADE,
  community_id uuid REFERENCES app_communities (id) ON DELETE CASCADE,
  body text NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  version bigint NOT NULL DEFAULT 0,
  last_synced_at timestamptz,
  deleted_at timestamptz,
  deleted_by_id uuid
);
CREATE INDEX IF NOT EXISTS app_test_notes_last_synced_at ON app_test_notes (last_synced_at, id);
";

/// A note, with the sync columns every synced row carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: Uuid,
    pub user_id: Uuid,
    pub community_id: Option<Uuid>,
    pub body: String,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i64,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_id: Option<Uuid>,
}

#[derive(Debug, Error)]
pub enum NoteError {
    #[error("note {id} not found")]
    NotFound { id: Uuid },
    #[error("only the owner can change a note")]
    NotOwner,
    #[error("{0}")]
    Invalid(String),
    #[error("the change was refused: {0}")]
    Rejected(String),
    #[error(transparent)]
    Record(#[from] crate::sync::events::RecordError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<serde_json::Error> for NoteError {
    fn from(error: serde_json::Error) -> NoteError {
        NoteError::Invalid(error.to_string())
    }
}

macro_rules! query {
    ($tx:expr, $function:ident($($argument:expr),* $(,)?)) => {
        match $tx {
            Transaction::Sqlite(tx) => sqlite::$function(&mut **tx, $($argument),*).await,
            #[cfg(feature = "postgres")]
            Transaction::Postgres(tx) => postgres::$function(&mut **tx, $($argument),*).await,
        }
    };
}

/// Every note the user sees, not deleted, newest first.
pub async fn list(deployment: &Deployment, user: &User) -> Result<Vec<Note>, NoteError> {
    let mut tx = deployment.begin().await?;
    let notes = query!(&mut tx, list_visible(user.id))?;
    tx.commit().await?;
    Ok(notes)
}

/// One note the user sees, by its id, unless deleted.
pub async fn get(deployment: &Deployment, user: &User, id: Uuid) -> Result<Note, NoteError> {
    let mut tx = deployment.begin().await?;
    let note = get_live(&mut tx, user, id).await?;
    tx.commit().await?;
    Ok(note)
}

/// Writes a new note of the user's, in the community, which the user
/// must be in, or in none.
pub async fn create(
    deployment: &Deployment,
    user: &User,
    body: &str,
    community_id: Option<Uuid>,
) -> Result<Note, NoteError> {
    let event = Event::create(
        TABLE,
        Uuid::now_v7(),
        json::object_fields(json!({ "body": body, "community_id": community_id })),
        now(),
    );
    let mut tx = deployment.begin().await?;
    let note = apply_in(&mut tx, deployment.role, user, &event)
        .await?
        .note()?;
    tx.commit().await?;
    Ok(note)
}

/// Writes a new body over the user's note.
pub async fn update(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
    body: &str,
) -> Result<Note, NoteError> {
    let mut tx = deployment.begin().await?;
    let note = get_own(&mut tx, user, id).await?;
    let event = Event::update(
        TABLE,
        id,
        note.version,
        json::object_fields(json!({ "body": note.body })),
        json::object_fields(json!({ "body": body })),
        now(),
    );
    let note = apply_in(&mut tx, deployment.role, user, &event)
        .await?
        .note()?;
    tx.commit().await?;
    Ok(note)
}

/// Moves the user's note into the community, or out of every one.
pub async fn update_community(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
    community_id: Option<Uuid>,
) -> Result<Note, NoteError> {
    let mut tx = deployment.begin().await?;
    let note = get_own(&mut tx, user, id).await?;
    let event = Event::update(
        TABLE,
        id,
        note.version,
        json::object_fields(json!({ "community_id": note.community_id })),
        json::object_fields(json!({ "community_id": community_id })),
        now(),
    );
    let note = apply_in(&mut tx, deployment.role, user, &event)
        .await?
        .note()?;
    tx.commit().await?;
    Ok(note)
}

/// Deletes the user's note: a soft delete, so it syncs like any edit.
pub async fn delete(deployment: &Deployment, user: &User, id: Uuid) -> Result<(), NoteError> {
    let mut tx = deployment.begin().await?;
    let note = get_own(&mut tx, user, id).await?;
    let applied = apply_in(&mut tx, deployment.role, user, &deleted_event(&note)).await?;
    tx.commit().await?;
    applied.note().map(|_| ())
}

/// Applies an event of the user's to its note and answers it.
pub async fn apply_event(
    deployment: &Deployment,
    user: &User,
    event: &Event,
) -> Result<Answer, NoteError> {
    let mut tx = deployment.begin().await?;
    let applied = apply_in(&mut tx, deployment.role, user, event).await?;
    tx.commit().await?;
    Ok(applied.answer)
}

/// The note as it travels on the wire.
pub fn table_row(note: &Note) -> Result<TableRow, NoteError> {
    Ok(TableRow {
        table: TABLE.to_string(),
        row: serde_json::to_value(note)?,
    })
}

/// The row by its id, deleted or not, whoever sees it.
pub async fn get_in(tx: &mut Transaction, id: Uuid) -> Result<Option<Note>, NoteError> {
    Ok(query!(tx, get(id))?)
}

async fn get_live(tx: &mut Transaction, user: &User, id: Uuid) -> Result<Note, NoteError> {
    match query!(tx, get_visible(user.id, id))? {
        Some(note) if note.deleted_at.is_none() => Ok(note),
        _ => Err(NoteError::NotFound { id }),
    }
}

async fn get_own(tx: &mut Transaction, user: &User, id: Uuid) -> Result<Note, NoteError> {
    let note = get_live(tx, user, id).await?;
    if note.user_id != user.id {
        return Err(NoteError::NotOwner);
    }
    Ok(note)
}

fn deleted_event(note: &Note) -> Event {
    Event::update(
        TABLE,
        note.id,
        note.version,
        json::object_fields(json!({ "deleted_at": null })),
        json::object_fields(json!({ "deleted_at": now() })),
        now(),
    )
}

/// The body as it is kept: trimmed, not blank and not too long.
fn checked_body(body: &str) -> Result<String, String> {
    let body = body.trim();
    if body.is_empty() {
        return Err("a note needs some text".to_string());
    }
    if body.chars().count() > MAX_BODY_LENGTH {
        return Err(format!("a note has {MAX_BODY_LENGTH} characters at most"));
    }
    Ok(body.to_string())
}

/// Whether the user is a live member of the note's community, when it
/// is in one.
async fn in_its_community(
    tx: &mut Transaction,
    user: &User,
    note: &Note,
) -> Result<bool, NoteError> {
    let Some(community_id) = note.community_id else {
        return Ok(true);
    };
    let memberships =
        app_users_communities::list(&mut *tx, app_users_communities::Filter::User(user.id)).await?;
    Ok(memberships
        .iter()
        .any(|membership| membership.community_id == community_id))
}

/// An event applied: the answer, and the note as it now stands.
struct Applied {
    answer: Answer,
    note: Option<Note>,
}

impl Applied {
    fn note(self) -> Result<Note, NoteError> {
        match (self.answer.status, self.note) {
            (AnswerStatus::Rejected, _) => Err(NoteError::Rejected(
                self.answer.reason.unwrap_or_else(|| "rejected".to_string()),
            )),
            (_, Some(note)) => Ok(note),
            (_, None) => Err(NoteError::NotFound { id: self.answer.id }),
        }
    }
}

/// Applies the event in the transaction, as `posts::apply_in` does: checks
/// the fields it changes, the owner and the community, merges it into
/// the row, writes it, and records or queues it.
async fn apply_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
) -> Result<Applied, NoteError> {
    if role == Role::Server
        && let Some(recorded) = app_events::get(tx, event.id).await?
    {
        let note = get_in(tx, event.row_id).await?;
        let server = note.as_ref().map(table_row).transpose()?;
        return Ok(Applied {
            answer: recorded.answer(server),
            note,
        });
    }

    let allowed = match event.kind {
        EventKind::Create => CREATE_FIELDS,
        EventKind::Update => UPDATE_FIELDS,
    };
    if let Some(reason) = events::not_editable(&event.changes, allowed) {
        let note = get_in(tx, event.row_id).await?;
        return rejected(event, reason, note);
    }

    let now = now();
    match event.kind {
        EventKind::Create => {
            if let Some(existing) = get_in(tx, event.row_id).await? {
                return rejected(event, "exists", Some(existing));
            }
            let (version, last_synced_at) = match role {
                Role::Server => (1, Some(now)),
                Role::Device => (0, None),
            };
            let blank = Note {
                id: event.row_id,
                user_id: user.id,
                community_id: None,
                body: String::new(),
                inserted_at: event.row_inserted_at(now),
                updated_at: now,
                version,
                last_synced_at,
                deleted_at: None,
                deleted_by_id: None,
            };
            let mut note = match blank.with_fields(&event.changes) {
                Ok(note) => note,
                Err(error) => return rejected(event, format!("invalid: {error}"), None),
            };
            note.body = match checked_body(&note.body) {
                Ok(body) => body,
                Err(error) => return rejected(event, format!("invalid: {error}"), None),
            };
            if !in_its_community(tx, user, &note).await? {
                return rejected(event, "not_a_member", None);
            }
            query!(tx, insert(&note))?;
            finish(tx, role, user, event, Conflicts::new(), note).await
        }
        EventKind::Update => {
            let Some(note) = get_in(tx, event.row_id).await? else {
                return rejected(event, "unknown_note", None);
            };
            if note.user_id != user.id {
                return rejected(event, "not_owner", Some(note));
            }
            let (changes, conflicts) = events::merge(
                &note.fields(UPDATE_FIELDS)?,
                note.version,
                event.base_version,
                &event.base,
                &event.changes,
            );
            if changes.is_empty() {
                return finish(tx, role, user, event, conflicts, note).await;
            }
            let mut updated = match note.with_fields(&changes) {
                Ok(updated) => updated,
                Err(error) => return rejected(event, format!("invalid: {error}"), Some(note)),
            };
            updated.body = match checked_body(&updated.body) {
                Ok(body) => body,
                Err(error) => return rejected(event, format!("invalid: {error}"), Some(note)),
            };
            if changes.contains_key("community_id") && !in_its_community(tx, user, &updated).await?
            {
                return rejected(event, "not_a_member", Some(note));
            }
            updated.updated_at = now;
            if changes.contains_key("deleted_at") {
                updated.deleted_by_id = updated.deleted_at.map(|_| user.id);
            }
            if role == Role::Server {
                updated.version += 1;
                updated.last_synced_at = Some(now);
            }
            query!(tx, update(&updated))?;
            finish(tx, role, user, event, conflicts, updated).await
        }
    }
}

async fn finish(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
    conflicts: Conflicts,
    note: Note,
) -> Result<Applied, NoteError> {
    let answer = Answer::applied(event.id, note.version, conflicts, table_row(&note)?);
    match role {
        Role::Server => events::record(tx, user.id, event, &answer).await?,
        Role::Device => events::queue(tx, user.id, event).await?,
    }
    Ok(Applied {
        answer,
        note: Some(note),
    })
}

fn rejected(
    event: &Event,
    reason: impl Into<String>,
    note: Option<Note>,
) -> Result<Applied, NoteError> {
    let server = note.as_ref().map(table_row).transpose()?;
    Ok(Applied {
        answer: Answer::rejected(event.id, reason, server),
        note,
    })
}

/// Writes a delete copied down from a parent, or its undoing, as the
/// role does: not logged, the server's version moved on.
async fn write_copied(
    tx: &mut Transaction,
    role: Role,
    mut note: Note,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_id: Option<Uuid>,
) -> Result<(), NoteError> {
    note.deleted_at = deleted_at;
    note.deleted_by_id = deleted_by_id;
    note.updated_at = now();
    if role == Role::Server {
        note.version += 1;
        note.last_synced_at = Some(note.updated_at);
    }
    Ok(query!(tx, update(&note))?)
}

impl Note {
    fn fields(&self, names: &[&str]) -> Result<Fields, NoteError> {
        let mut all = json::object_fields(serde_json::to_value(self)?);
        all.retain(|name, _| names.contains(&name.as_str()));
        Ok(all)
    }

    fn with_fields(&self, changes: &Fields) -> Result<Note, NoteError> {
        let mut all = json::object_fields(serde_json::to_value(self)?);
        for (name, value) in changes {
            all.insert(name.clone(), value.clone());
        }
        Ok(serde_json::from_value(Value::Object(all))?)
    }
}

/// Test notes as a synced table, listed after the app's by
/// `sync::tables`.
pub struct Synced;

#[async_trait]
impl SyncedTable for Synced {
    fn table(&self) -> &'static str {
        TABLE
    }

    fn owner_column(&self) -> Option<&'static str> {
        Some("user_id")
    }

    async fn apply_in(
        &self,
        tx: &mut Transaction,
        role: Role,
        user: &User,
        event: &Event,
    ) -> Result<Answer, TableError> {
        Ok(apply_in(tx, role, user, event).await?.answer)
    }

    async fn list_synced_after(
        &self,
        tx: &mut Transaction,
        user: &User,
        after: &Cursor,
        limit: usize,
    ) -> Result<Vec<TableRow>, TableError> {
        let notes = query!(tx, list_synced_after(user.id, after, limit))?;
        Ok(notes.iter().map(table_row).collect::<Result<_, _>>()?)
    }

    async fn parents(
        &self,
        tx: &mut Transaction,
        row: &TableRow,
    ) -> Result<Vec<TableRow>, TableError> {
        let note: Note = serde_json::from_value(row.row.clone())?;
        let mut parents = Vec::new();
        if let Some(owner) = crate::app::users::get_in(tx, note.user_id).await? {
            parents.push(crate::app::users::table_row(&owner)?);
        }
        if let Some(community_id) = note.community_id
            && let Some(community) = crate::app::communities::get_in(tx, community_id).await?
        {
            parents.push(crate::app::communities::table_row(&community)?);
        }
        Ok(parents)
    }

    async fn apply_server_row(
        &self,
        tx: &mut Transaction,
        _user: &User,
        row: &TableRow,
    ) -> Result<bool, TableError> {
        let note: Note = serde_json::from_value(row.row.clone())?;
        if app_events::has_pending(tx, TABLE, note.id).await? {
            return Ok(false);
        }
        query!(tx, upsert(&note))?;
        Ok(true)
    }

    async fn delete_row(&self, tx: &mut Transaction, id: Uuid) -> Result<(), TableError> {
        Ok(query!(tx, delete_row(id))?)
    }

    async fn local_user_creates(
        &self,
        tx: &mut Transaction,
        user: &User,
    ) -> Result<Vec<LocalCreate>, TableError> {
        let notes = query!(tx, list_visible(user.id))?;
        Ok(notes
            .into_iter()
            .map(|note| LocalCreate {
                id: note.id,
                inserted_at: note.inserted_at,
                parents: note.community_id.into_iter().collect(),
                changes: json::object_fields(
                    json!({ "body": note.body, "community_id": note.community_id }),
                ),
            })
            .collect())
    }

    async fn label(&self, tx: &mut Transaction, id: Uuid) -> Result<Option<String>, TableError> {
        let note = get_in(tx, id).await?;
        Ok(note.map(|note| note.body.lines().next().unwrap_or_default().to_string()))
    }

    async fn current_fields(
        &self,
        tx: &mut Transaction,
        id: Uuid,
    ) -> Result<Option<CurrentFields>, TableError> {
        let Some(note) = get_in(tx, id).await? else {
            return Ok(None);
        };
        if note.deleted_at.is_some() {
            return Ok(None);
        }
        Ok(Some(CurrentFields {
            version: note.version,
            fields: note.fields(UPDATE_FIELDS)?,
        }))
    }

    async fn list_live_in_community_after(
        &self,
        tx: &mut Transaction,
        community_id: Uuid,
        after: &Cursor,
        limit: usize,
    ) -> Result<Vec<TableRow>, TableError> {
        let notes = query!(tx, list_live_in_community_after(community_id, after, limit))?;
        Ok(notes.iter().map(table_row).collect::<Result<_, _>>()?)
    }

    async fn move_rows_to_community(
        &self,
        tx: &mut Transaction,
        from_community_id: Uuid,
        to: &Membership,
    ) -> Result<(), TableError> {
        Ok(query!(
            tx,
            update_community_of_all(from_community_id, to.community_id)
        )?)
    }

    /// A note is its owner's, so it goes with their account.
    async fn delete_of_user(
        &self,
        tx: &mut Transaction,
        role: Role,
        user: &User,
    ) -> Result<(), TableError> {
        for note in query!(tx, list_live_of_user(user.id))? {
            apply_in(tx, role, user, &deleted_event(&note))
                .await?
                .note()?;
        }
        Ok(())
    }

    /// The owner's notes in the community go with their membership, at
    /// its moment.
    async fn delete_with_membership(
        &self,
        tx: &mut Transaction,
        role: Role,
        membership: &Membership,
    ) -> Result<(), TableError> {
        let notes = query!(
            tx,
            list_live_of_user_in_community(membership.user_id, membership.community_id)
        )?;
        for note in notes {
            write_copied(
                tx,
                role,
                note,
                membership.deleted_at,
                membership.deleted_by_id,
            )
            .await?;
        }
        Ok(())
    }

    /// The notes that went with the membership at `left_at` come back.
    async fn revive_with_membership(
        &self,
        tx: &mut Transaction,
        role: Role,
        membership: &Membership,
        left_at: DateTime<Utc>,
    ) -> Result<(), TableError> {
        let notes = query!(
            tx,
            list_of_user_in_community_deleted_at(
                membership.user_id,
                membership.community_id,
                left_at
            )
        )?;
        for note in notes {
            write_copied(tx, role, note, None, None).await?;
        }
        Ok(())
    }

    /// A note in a community goes with it, at its moment.
    async fn delete_with_community(
        &self,
        tx: &mut Transaction,
        role: Role,
        community: &Community,
    ) -> Result<(), TableError> {
        for note in query!(tx, list_live_in_community(community.id))? {
            write_copied(
                tx,
                role,
                note,
                community.deleted_at,
                community.deleted_by_id,
            )
            .await?;
        }
        Ok(())
    }

    async fn prune(&self, tx: &mut Transaction, user: &User) -> Result<u64, TableError> {
        Ok(query!(tx, delete_unreachable(user.id))?)
    }

    fn counted_before_a_move(&self) -> bool {
        true
    }
}
