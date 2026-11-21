//! A file in a community: a row for each community a file is in, see
//! `docs/design/files-communities.md`. The rows are the file's life: a
//! file is created with its first rows, and the server deletes it when
//! its last row is deleted. A row is the community's, not a member's,
//! so leaving a community leaves its files there.
//!
//! A live member of a community the file is in puts it in another
//! community they are in, and a member of a community takes it out of
//! that community. Taking out soft-deletes the row and putting back
//! revives it. A user sees only the rows of their own communities, so
//! nothing tells them a file is elsewhere.

use crate::data::app_files;
use crate::data::app_files_communities;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::app::events::{self, Answer, AnswerStatus, Conflicts, Event, EventKind};
use crate::app::files::{self, File};
use crate::app::memberships::{self, id_field};
use crate::app::sync::{Cursor, TableRow};
use crate::data::app_communities::Community;
use crate::data::app_users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::helpers::json;

/// A row accepted by this resource's event or replication workflow.
/// Only this module can construct an accepted write.
pub(crate) struct AcceptedFileCommunity<'a>(&'a FileCommunity);

impl AcceptedFileCommunity<'_> {
    /// Borrows the accepted row for persistence.
    pub(crate) fn row(&self) -> &FileCommunity {
        self.0
    }
}

pub const TABLE: &str = "app_files_communities";
const CREATE_FIELDS: &[&str] = &["file_id", "community_id"];
const UPDATE_FIELDS: &[&str] = &["deleted_at"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileCommunity {
    pub id: Uuid,
    pub file_id: Uuid,
    pub community_id: Uuid,
    /// The member who put the file in the community.
    pub created_by_id: Uuid,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i64,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_id: Option<Uuid>,
}

/// A row a file's create carries: the id the device made for it, and
/// the community.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileCommunityNew {
    pub id: Uuid,
    pub community_id: Uuid,
}

#[derive(Debug, Error)]
pub enum FileCommunityError {
    #[error("file {id} not found")]
    UnknownFile { id: Uuid },
    #[error("not a member of community {community_id}")]
    NotAMember { community_id: Uuid },
    #[error("the file is not in community {community_id}")]
    NotFound { community_id: Uuid },
    #[error("{0}")]
    Invalid(String),
    #[error("the change was refused: {0}")]
    Rejected(String),
    #[error(transparent)]
    Record(#[from] crate::app::events::RecordError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<serde_json::Error> for FileCommunityError {
    fn from(error: serde_json::Error) -> FileCommunityError {
        FileCommunityError::Invalid(error.to_string())
    }
}

/// Puts a file the user sees in another community of theirs, or back in it.
pub async fn create(
    deployment: &Deployment,
    user: &User,
    file_id: Uuid,
    community_id: Uuid,
) -> Result<FileCommunity, FileCommunityError> {
    let mut tx = deployment.begin().await?;
    let row = create_in(&mut tx, deployment.role, user, file_id, community_id).await?;
    tx.commit().await?;
    Ok(row)
}

/// Takes a file out of a community of the user's; the server deletes the
/// file when that was its last.
pub async fn delete(
    deployment: &Deployment,
    user: &User,
    file_id: Uuid,
    community_id: Uuid,
) -> Result<(), FileCommunityError> {
    let mut tx = deployment.begin().await?;
    delete_in(&mut tx, deployment.role, user, file_id, community_id).await?;
    tx.commit().await?;
    Ok(())
}

/// Applies an event of the user's to a row and answers it.
pub async fn apply_event(
    deployment: &Deployment,
    user: &User,
    event: &Event,
) -> Result<Answer, FileCommunityError> {
    let mut tx = deployment.begin().await?;
    let applied = apply_in(&mut tx, deployment.role, user, event).await?;
    tx.commit().await?;
    Ok(applied.answer)
}

pub fn table_row(row: &FileCommunity) -> Result<TableRow, FileCommunityError> {
    Ok(TableRow {
        table: TABLE.to_string(),
        row: serde_json::to_value(row)?,
    })
}

pub(crate) async fn create_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    file_id: Uuid,
    community_id: Uuid,
) -> Result<FileCommunity, FileCommunityError> {
    let file = visible_file(tx, user, file_id).await?;
    member_of(tx, user, community_id).await?;
    let event = match app_files_communities::replication::get_by_file_and_community(
        &mut *tx,
        file.id,
        community_id,
    )
    .await?
    {
        None => Event::create(
            TABLE,
            Uuid::now_v7(),
            json::object_fields(json!({ "file_id": file.id, "community_id": community_id })),
            now(),
        ),
        Some(row) if row.deleted_at.is_none() => return Ok(row),
        Some(removed) => Event::update(
            TABLE,
            removed.id,
            removed.version,
            json::object_fields(json!({ "deleted_at": removed.deleted_at })),
            json::object_fields(json!({ "deleted_at": null })),
            now(),
        ),
    };
    apply_in(tx, role, user, &event).await?.row()
}

pub(crate) async fn delete_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    file_id: Uuid,
    community_id: Uuid,
) -> Result<FileCommunity, FileCommunityError> {
    let file = visible_file(tx, user, file_id).await?;
    let row = match app_files_communities::replication::get_by_file_and_community(
        &mut *tx,
        file.id,
        community_id,
    )
    .await?
    {
        Some(row) if row.deleted_at.is_none() => row,
        _ => return Err(FileCommunityError::NotFound { community_id }),
    };
    member_of(tx, user, community_id).await?;
    let event = Event::update(
        TABLE,
        row.id,
        row.version,
        json::object_fields(json!({ "deleted_at": null })),
        json::object_fields(json!({ "deleted_at": now() })),
        now(),
    );
    apply_in(tx, role, user, &event).await?.row()
}

/// Makes the file's communities among the user's exactly these: puts it
/// in what is missing and takes it out of the rest of the user's. Rows
/// in communities the user is not in are not theirs to change.
pub(crate) async fn set_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    file_id: Uuid,
    community_ids: &[Uuid],
) -> Result<(), FileCommunityError> {
    let current = list_visible_of_files(tx, user, &[file_id]).await?;
    for community_id in community_ids {
        if !current.iter().any(|row| row.community_id == *community_id) {
            create_in(tx, role, user, file_id, *community_id).await?;
        }
    }
    for row in &current {
        if !community_ids.contains(&row.community_id) {
            delete_in(tx, role, user, file_id, row.community_id).await?;
        }
    }
    Ok(())
}

/// Why the rows a file's create carries are refused, if they are: there
/// must be one at least, each in a community the user is a live member
/// of, under an id no row has.
pub(crate) async fn refusal_of_new(
    tx: &mut Transaction,
    user: &User,
    rows: &[FileCommunityNew],
) -> Result<Option<&'static str>, FileCommunityError> {
    if rows.is_empty() {
        return Ok(Some("no_community"));
    }
    for (index, row) in rows.iter().enumerate() {
        if rows[..index]
            .iter()
            .any(|earlier| earlier.community_id == row.community_id)
        {
            return Ok(Some("invalid: a community is named twice"));
        }
        if memberships::get_live_in(tx, user.id, row.community_id)
            .await
            .map_err(invalid)?
            .is_none()
        {
            return Ok(Some("not_a_member"));
        }
        if get_in(tx, row.id).await?.is_some() {
            return Ok(Some("exists"));
        }
    }
    Ok(None)
}

/// Writes the rows a file's create carries, with the file, as the file
/// is written: checked first by `refusal_of_new`.
pub(crate) async fn create_with_file(
    tx: &mut Transaction,
    user: &User,
    file: &File,
    rows: &[FileCommunityNew],
) -> Result<(), FileCommunityError> {
    for new in rows {
        let row = FileCommunity {
            id: new.id,
            file_id: file.id,
            community_id: new.community_id,
            created_by_id: user.id,
            inserted_at: file.inserted_at,
            updated_at: file.updated_at,
            version: file.version,
            last_synced_at: file.last_synced_at,
            deleted_at: None,
            deleted_by_id: None,
        };
        app_files_communities::create(&mut *tx, AcceptedFileCommunity(&row)).await?;
    }
    Ok(())
}

/// The live rows of the files in the communities the user is in.
pub(crate) async fn list_visible_of_files(
    tx: &mut Transaction,
    user: &User,
    file_ids: &[Uuid],
) -> Result<Vec<FileCommunity>, FileCommunityError> {
    Ok(app_files_communities::list(
        tx,
        app_files_communities::Filter::FilesVisibleTo {
            user_id: user.id,
            file_ids,
        },
    )
    .await?)
}

/// Deletes the community's live rows with it, at its moment, and each
/// file that leaves with no row.
pub(crate) async fn delete_with_community(
    tx: &mut Transaction,
    role: Role,
    community: &Community,
) -> Result<(), FileCommunityError> {
    for mut row in app_files_communities::list(
        &mut *tx,
        app_files_communities::Filter::Community(community.id),
    )
    .await?
    {
        row.deleted_at = community.deleted_at;
        row.deleted_by_id = community.deleted_by_id;
        write_changed(tx, role, &mut row).await?;
        delete_file_if_in_none(tx, role, &row).await?;
    }
    Ok(())
}

/// Writes the server's row over the device's, unless an event of the
/// device's for it is still pending, or the device does not have its
/// file: a row taken out of a community before this device saw the
/// file; true when it was written.
pub(crate) async fn apply_server_row(
    tx: &mut Transaction,
    row: &Value,
) -> Result<bool, FileCommunityError> {
    let row: FileCommunity = serde_json::from_value(row.clone())?;
    if events::has_pending(tx, TABLE, row.id).await? {
        return Ok(false);
    }
    if app_files::replication::get(tx, row.file_id)
        .await?
        .is_none()
    {
        return Ok(false);
    }
    app_files_communities::replication::upsert(&mut *tx, AcceptedFileCommunity(&row)).await?;
    Ok(true)
}

pub(crate) async fn delete_row(tx: &mut Transaction, id: Uuid) -> Result<(), FileCommunityError> {
    Ok(app_files_communities::maintenance::delete(tx, id).await?)
}

pub(crate) async fn get_in(
    tx: &mut Transaction,
    id: Uuid,
) -> Result<Option<FileCommunity>, FileCommunityError> {
    Ok(app_files_communities::replication::get(tx, id).await?)
}

pub(crate) async fn list_synced_after(
    tx: &mut Transaction,
    user: &User,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<FileCommunity>, FileCommunityError> {
    Ok(app_files_communities::replication::list_synced_after(tx, user.id, after, limit).await?)
}

/// A page of the community's live rows after the cursor: a backfill.
pub(crate) async fn list_live_of_community_after(
    tx: &mut Transaction,
    community_id: Uuid,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<FileCommunity>, FileCommunityError> {
    Ok(
        app_files_communities::replication::list_live_of_community_after(
            tx,
            community_id,
            after,
            limit,
        )
        .await?,
    )
}

/// The live file, if the user sees it; one they do not see is unknown.
async fn visible_file(
    tx: &mut Transaction,
    user: &User,
    file_id: Uuid,
) -> Result<File, FileCommunityError> {
    app_files::get(tx, user.id, file_id)
        .await?
        .ok_or(FileCommunityError::UnknownFile { id: file_id })
}

async fn member_of(
    tx: &mut Transaction,
    user: &User,
    community_id: Uuid,
) -> Result<(), FileCommunityError> {
    match memberships::get_live_in(tx, user.id, community_id)
        .await
        .map_err(invalid)?
    {
        Some(_) => Ok(()),
        None => Err(FileCommunityError::NotAMember { community_id }),
    }
}

struct Applied {
    answer: Answer,
    row: Option<FileCommunity>,
}

impl Applied {
    fn row(self) -> Result<FileCommunity, FileCommunityError> {
        match (self.answer.status, self.row) {
            (AnswerStatus::Rejected, _) => Err(FileCommunityError::Rejected(
                self.answer.reason.unwrap_or_else(|| "rejected".to_string()),
            )),
            (_, Some(row)) => Ok(row),
            (_, None) => Err(FileCommunityError::Invalid("no row".to_string())),
        }
    }
}

/// Applies the event in the transaction, as what the person meant: a
/// create puts the file in the community, an update of `deleted_at`
/// takes it out or puts it back, whatever the server already holds for
/// the pair. Only a file the user sees, in a community they are in.
async fn apply_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
) -> Result<Applied, FileCommunityError> {
    if role == Role::Server
        && let Some(recorded) = events::get(tx, event.id).await?
    {
        let row = get_in(tx, event.row_id).await?;
        let server = row.as_ref().map(table_row).transpose()?;
        return Ok(Applied {
            answer: recorded.answer(server),
            row,
        });
    }

    let allowed = match event.kind {
        EventKind::Create => CREATE_FIELDS,
        EventKind::Update => UPDATE_FIELDS,
    };
    if let Some(reason) = events::not_editable(&event.changes, allowed) {
        return rejected(event, reason, None);
    }

    let row = match event.kind {
        EventKind::Create => {
            let Some(file_id) = id_field(&event.changes, "file_id") else {
                return rejected(event, "unknown_file", None);
            };
            let Some(community_id) = id_field(&event.changes, "community_id") else {
                return rejected(event, "unknown_community", None);
            };
            if app_files::get(tx, user.id, file_id).await?.is_none() {
                return rejected(event, "unknown_file", None);
            }
            if memberships::get_live_in(tx, user.id, community_id)
                .await
                .map_err(invalid)?
                .is_none()
            {
                return rejected(event, "not_a_member", None);
            }
            match app_files_communities::replication::get_by_file_and_community(
                &mut *tx,
                file_id,
                community_id,
            )
            .await?
            {
                Some(existing) if existing.deleted_at.is_none() => existing,
                Some(removed) => put_back(tx, role, user, removed).await?,
                None => {
                    if get_in(tx, event.row_id).await?.is_some() {
                        return rejected(event, "exists", None);
                    }
                    let now = now();
                    let (version, last_synced_at) = match role {
                        Role::Server => (1, Some(now)),
                        Role::Device => (0, None),
                    };
                    let row = FileCommunity {
                        id: event.row_id,
                        file_id,
                        community_id,
                        created_by_id: user.id,
                        inserted_at: event.row_inserted_at(now),
                        updated_at: now,
                        version,
                        last_synced_at,
                        deleted_at: None,
                        deleted_by_id: None,
                    };
                    app_files_communities::create(&mut *tx, AcceptedFileCommunity(&row)).await?;
                    row
                }
            }
        }
        EventKind::Update => {
            // A row of a community the user is not in is none of their business
            let row = match get_in(tx, event.row_id).await? {
                Some(row)
                    if memberships::get_live_in(tx, user.id, row.community_id)
                        .await
                        .map_err(invalid)?
                        .is_some() =>
                {
                    row
                }
                Some(_) => return rejected(event, "not_a_member", None),
                None => return rejected(event, "unknown_row", None),
            };
            let deleted_at: Option<DateTime<Utc>> = match event.changes.get("deleted_at") {
                Some(value) => match serde_json::from_value(value.clone()) {
                    Ok(deleted_at) => deleted_at,
                    Err(error) => return rejected(event, format!("invalid: {error}"), Some(row)),
                },
                None => row.deleted_at,
            };
            match (row.deleted_at, deleted_at) {
                (Some(_), None) => {
                    let file = app_files::replication::get(tx, row.file_id).await?;
                    if file.is_none_or(|file| file.deleted_at.is_some()) {
                        return rejected(event, "unknown_file", Some(row));
                    }
                    put_back(tx, role, user, row).await?
                }
                (None, Some(at)) => {
                    let mut removed = row;
                    removed.deleted_at = Some(at);
                    removed.deleted_by_id = Some(user.id);
                    write_changed(tx, role, &mut removed).await?;
                    delete_file_if_in_none(tx, role, &removed).await?;
                    removed
                }
                _ => row,
            }
        }
    };
    let answer = Answer::applied(event.id, row.version, Conflicts::new(), table_row(&row)?);
    match role {
        Role::Server => events::record(tx, user.id, event, &answer).await?,
        Role::Device => events::queue(tx, user.id, event).await?,
    }
    Ok(Applied {
        answer,
        row: Some(row),
    })
}

/// Revives the row, by the user, now.
async fn put_back(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    mut row: FileCommunity,
) -> Result<FileCommunity, FileCommunityError> {
    row.created_by_id = user.id;
    row.deleted_at = None;
    row.deleted_by_id = None;
    write_changed(tx, role, &mut row).await?;
    Ok(row)
}

/// Deletes the row's file when it is in no community any more, at the
/// row's moment: on the server, the file is gone; on a device, it is in
/// none of the user's, and a row the server sends later puts it back.
async fn delete_file_if_in_none(
    tx: &mut Transaction,
    role: Role,
    row: &FileCommunity,
) -> Result<(), FileCommunityError> {
    let live = app_files_communities::list(
        &mut *tx,
        app_files_communities::Filter::Files(&[row.file_id]),
    )
    .await?;
    if !live.is_empty() {
        return Ok(());
    }
    files::delete_with_last_community(tx, role, row.file_id, row.deleted_at, row.deleted_by_id)
        .await
        .map_err(|error| FileCommunityError::Invalid(error.to_string()))
}

/// Writes the row over itself as the role does: the server increments
/// the version and sets the sync time.
async fn write_changed(
    tx: &mut Transaction,
    role: Role,
    row: &mut FileCommunity,
) -> Result<(), FileCommunityError> {
    let now = now();
    row.updated_at = now;
    if role == Role::Server {
        row.version += 1;
        row.last_synced_at = Some(now);
    }
    app_files_communities::update(&mut *tx, AcceptedFileCommunity(row)).await?;
    Ok(())
}

fn rejected(
    event: &Event,
    reason: impl Into<String>,
    row: Option<FileCommunity>,
) -> Result<Applied, FileCommunityError> {
    let server = row.as_ref().map(table_row).transpose()?;
    Ok(Applied {
        answer: Answer::rejected(event.id, reason, server),
        row,
    })
}

fn invalid(error: memberships::MembershipError) -> FileCommunityError {
    match error {
        memberships::MembershipError::Database(error) => FileCommunityError::Database(error),
        other => FileCommunityError::Invalid(other.to_string()),
    }
}

#[cfg(all(test, feature = "object-storage"))]
mod tests {
    use super::*;
    use crate::app::files::tests::{device_with_folder, new_file, server};
    use crate::app::files::{FileError, FileNew};
    use crate::app::sync::tests::user_with_token;
    use crate::app::sync::{self, BackfillRequest, InProcess, PullRequest, SyncError};
    use crate::app::{communities, files, memberships, users};
    use crate::database::with_each_database;
    use crate::services::storage::Storage;

    /// A file the user created on the server in the communities, its
    /// bytes put in the bucket as an upload would have.
    async fn file_in(
        deployment: &Deployment,
        storage: &Storage,
        user: &User,
        name: &str,
        community_ids: &[Uuid],
    ) -> File {
        let new = FileNew {
            community_ids: community_ids.to_vec(),
            ..new_file(name, b"bytes", Uuid::nil())
        };
        storage
            .put(&files::storage_key(user.id, new.id), b"bytes".to_vec())
            .await
            .expect("the upload");
        files::create(deployment, user, &new).await.expect("a file")
    }

    async fn user(deployment: &Deployment, name: &str) -> User {
        users::create(deployment, &format!("{name}@example.com"), name)
            .await
            .expect("a user")
    }

    async fn community(deployment: &Deployment, user: &User, name: &str) -> Uuid {
        communities::create(deployment, user, name)
            .await
            .expect("a community")
            .id
    }

    async fn names(deployment: &Deployment, user: &User) -> Vec<String> {
        files::list(deployment, user)
            .await
            .expect("the files")
            .into_iter()
            .map(|listed| listed.file.name)
            .collect()
    }

    async fn community_names(deployment: &Deployment, user: &User, id: Uuid) -> Vec<String> {
        files::get(deployment, user, id)
            .await
            .expect("the file")
            .communities
            .into_iter()
            .map(|community| community.name)
            .collect()
    }

    #[tokio::test]
    async fn files_communities_a_file_is_seen_in_its_communities_only() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let alice = user(&server, "Alice").await;
            let bob = user(&server, "Bob").await;
            let carol = user(&server, "Carol").await;
            let garden = community(&server, &alice, "Garden").await;
            let cooking = community(&server, &carol, "Cooking").await;
            memberships::join(&server, &bob, garden)
                .await
                .expect("joined");
            let file = file_in(&server, &storage, &alice, "notes.txt", &[garden]).await;

            assert_eq!(names(&server, &bob).await, ["notes.txt"]);
            let shown = files::get(&server, &bob, file.id).await.expect("in garden");
            assert_eq!(
                shown.creator.map(|creator| creator.name),
                Some("Alice".into())
            );
            assert_eq!(community_names(&server, &bob, file.id).await, ["Garden"]);

            assert!(names(&server, &carol).await.is_empty());
            for error in [
                files::get(&server, &carol, file.id)
                    .await
                    .expect_err("not hers"),
                files::update(&server, &carol, file.id, "hers.txt", &[cooking])
                    .await
                    .expect_err("not hers"),
                files::delete(&server, &carol, file.id)
                    .await
                    .expect_err("not hers"),
                files::content_url(&server, &carol, file.id, false)
                    .await
                    .expect_err("not hers"),
            ] {
                assert!(matches!(error, FileError::NotFound { .. }), "{error}");
            }
            let error = create(&server, &carol, file.id, cooking)
                .await
                .expect_err("not hers to share");
            assert!(matches!(error, FileCommunityError::UnknownFile { .. }));
            let error = delete(&server, &carol, file.id, garden)
                .await
                .expect_err("not hers to take out");
            assert!(matches!(error, FileCommunityError::UnknownFile { .. }));

            let rename = Event::update(
                files::TABLE,
                file.id,
                file.version,
                json::object_fields(json!({ "name": file.name })),
                json::object_fields(json!({ "name": "hers.txt" })),
                now(),
            );
            let answer = files::apply_event(&server, &carol, &rename)
                .await
                .expect("answered");
            assert_eq!(answer.status, AnswerStatus::Rejected);
            assert_eq!(answer.reason.as_deref(), Some("unknown_file"));
            assert_eq!(answer.server, None, "nothing of the file is told");
        })
        .await;
    }

    #[tokio::test]
    async fn files_communities_a_file_is_created_in_communities_its_creator_is_in() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let alice = user(&server, "Alice").await;
            let bob = user(&server, "Bob").await;
            let garden = community(&server, &alice, "Garden").await;
            let woodwork = community(&server, &bob, "Woodwork").await;

            let error = files::create(&server, &alice, &new_file("x.txt", b"x", garden))
                .await
                .map(|_| ())
                .expect_err("no bytes");
            assert!(matches!(error, FileError::NotUploaded));
            for community_ids in [vec![], vec![woodwork], vec![garden, woodwork]] {
                let new = FileNew {
                    community_ids: community_ids.clone(),
                    ..new_file("x.txt", b"x", garden)
                };
                storage
                    .put(&files::storage_key(alice.id, new.id), b"x".to_vec())
                    .await
                    .expect("the upload");
                let error = files::create(&server, &alice, &new)
                    .await
                    .expect_err("not in a community of hers");
                let refused = match &error {
                    FileError::Invalid(errors) => errors.0.contains_key("community_ids"),
                    FileError::Rejected(reason) => reason == "not_a_member",
                    _ => false,
                };
                assert!(refused, "{community_ids:?}: {error}");
                let mut tx = server.begin().await.expect("a transaction");
                assert!(
                    files::get_in(&mut tx, new.id)
                        .await
                        .expect("read")
                        .is_none()
                );
                tx.commit().await.expect("committed");
            }
        })
        .await;
    }

    #[tokio::test]
    async fn files_communities_any_member_shares_and_renames_and_a_creator_who_left_loses_it() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let alice = user(&server, "Alice").await;
            let bob = user(&server, "Bob").await;
            let dave = user(&server, "Dave").await;
            let garden = community(&server, &alice, "Garden").await;
            let cooking = community(&server, &dave, "Cooking").await;
            memberships::join(&server, &bob, garden)
                .await
                .expect("joined");
            memberships::join(&server, &bob, cooking)
                .await
                .expect("joined");
            let file = file_in(&server, &storage, &alice, "notes.txt", &[garden]).await;

            files::update(
                &server,
                &bob,
                file.id,
                "bob's notes.txt",
                &[garden, cooking],
            )
            .await
            .expect("renamed and shared by a member");
            assert_eq!(names(&server, &dave).await, ["bob's notes.txt"]);
            assert_eq!(
                community_names(&server, &dave, file.id).await,
                ["Cooking"],
                "garden is not dave's to know of"
            );
            assert_eq!(
                community_names(&server, &bob, file.id).await,
                ["Garden", "Cooking"]
            );

            memberships::leave(&server, &alice, garden)
                .await
                .expect("left");
            assert!(
                names(&server, &alice).await.is_empty(),
                "no access of her own"
            );
            assert!(matches!(
                files::get(&server, &alice, file.id).await,
                Err(FileError::NotFound { .. })
            ));
            let shown = files::get(&server, &bob, file.id).await.expect("stays");
            assert_eq!(shown.creator, None, "a former member");
            assert_eq!(shown.file.created_by_id, alice.id);
            assert_eq!(names(&server, &dave).await, ["bob's notes.txt"]);
        })
        .await;
    }

    #[tokio::test]
    async fn files_communities_taking_a_file_out_of_its_last_community_deletes_it() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let alice = user(&server, "Alice").await;
            let bob = user(&server, "Bob").await;
            let garden = community(&server, &alice, "Garden").await;
            let cooking = community(&server, &alice, "Cooking").await;
            memberships::join(&server, &bob, garden)
                .await
                .expect("joined");
            let file = file_in(&server, &storage, &alice, "notes.txt", &[garden, cooking]).await;

            files::delete(&server, &bob, file.id)
                .await
                .expect("taken out of bob's");
            assert!(names(&server, &bob).await.is_empty());
            assert_eq!(community_names(&server, &alice, file.id).await, ["Cooking"]);

            files::delete(&server, &alice, file.id)
                .await
                .expect("taken out of the last");
            let mut tx = server.begin().await.expect("a transaction");
            let row = files::get_in(&mut tx, file.id)
                .await
                .expect("read")
                .expect("kept until the purge");
            let last = app_files_communities::replication::get_by_file_and_community(
                &mut tx, file.id, cooking,
            )
            .await
            .expect("read")
            .expect("kept");
            tx.commit().await.expect("committed");
            assert_eq!(row.deleted_at, last.deleted_at, "the one moment");
            assert_eq!(row.deleted_by_id, Some(alice.id));
            assert_eq!(row.version, 2);
            let error = create(&server, &alice, file.id, garden)
                .await
                .expect_err("a deleted file is gone");
            assert!(matches!(error, FileCommunityError::UnknownFile { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn files_communities_deleting_a_community_deletes_the_files_only_it_had() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let alice = user(&server, "Alice").await;
            let garden = community(&server, &alice, "Garden").await;
            let cooking = community(&server, &alice, "Cooking").await;
            let only = file_in(&server, &storage, &alice, "only.txt", &[garden]).await;
            let shared = file_in(&server, &storage, &alice, "shared.txt", &[garden, cooking]).await;

            communities::delete(&server, &alice, garden)
                .await
                .expect("deleted");
            assert_eq!(names(&server, &alice).await, ["shared.txt"]);
            assert_eq!(
                community_names(&server, &alice, shared.id).await,
                ["Cooking"]
            );
            let mut tx = server.begin().await.expect("a transaction");
            let gone = files::get_in(&mut tx, only.id)
                .await
                .expect("read")
                .expect("kept until the purge");
            tx.commit().await.expect("committed");
            assert!(gone.deleted_at.is_some());
        })
        .await;
    }

    #[tokio::test]
    async fn files_communities_a_device_gets_only_the_files_and_rows_of_its_communities() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let (alice, _) = user_with_token(&server, "alice@example.com").await;
            let (bob, bobs_token) = user_with_token(&server, "bob@example.com").await;
            let carol = user(&server, "Carol").await;
            let garden = community(&server, &alice, "Garden").await;
            let cooking = community(&server, &alice, "Cooking").await;
            memberships::join(&server, &bob, garden)
                .await
                .expect("joined");
            memberships::join(&server, &carol, cooking)
                .await
                .expect("joined");
            let shared = file_in(&server, &storage, &alice, "shared.txt", &[garden, cooking]).await;
            let cookings = file_in(&server, &storage, &carol, "cooking's.txt", &[cooking]).await;

            let page = sync::pull(&server, &bob, PullRequest::default())
                .await
                .expect("a page");
            let files_sent: Vec<Value> = page
                .rows
                .iter()
                .filter(|row| row.table == files::TABLE)
                .map(|row| row.row["id"].clone())
                .collect();
            assert_eq!(files_sent, [json!(shared.id)]);
            let communities_sent: Vec<Value> = page
                .rows
                .iter()
                .filter(|row| row.table == TABLE)
                .map(|row| row.row["community_id"].clone())
                .collect();
            assert_eq!(communities_sent, [json!(garden)], "nothing of cooking");

            let error = sync::backfill(
                &server,
                &bob,
                BackfillRequest {
                    community_id: cooking,
                    after: None,
                    limit: None,
                },
            )
            .await
            .expect_err("not his");
            assert!(matches!(error, SyncError::NotAMember { .. }));
            let error = files::content_url(&server, &bob, cookings.id, false)
                .await
                .expect_err("not his");
            assert!(matches!(error, FileError::NotFound { .. }));

            let (device, _folder) = device_with_folder(&bob, &bobs_token).await;
            sync::run(&device, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(names(&device, &bob).await, ["shared.txt"]);
            assert_eq!(community_names(&device, &bob, shared.id).await, ["Garden"]);
            let mut tx = device.begin().await.expect("a transaction");
            assert!(
                files::get_in(&mut tx, cookings.id)
                    .await
                    .expect("read")
                    .is_none()
            );
            tx.commit().await.expect("committed");
        })
        .await;
    }

    #[tokio::test]
    async fn files_communities_leaving_prunes_a_file_from_a_device_and_rejoining_brings_it_back() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let (alice, _) = user_with_token(&server, "alice@example.com").await;
            let (bob, bobs_token) = user_with_token(&server, "bob@example.com").await;
            let garden = community(&server, &alice, "Garden").await;
            memberships::join(&server, &bob, garden)
                .await
                .expect("joined");
            let file = file_in(&server, &storage, &alice, "notes.txt", &[garden]).await;
            let (device, folder) = device_with_folder(&bob, &bobs_token).await;
            let transport = InProcess { server: &server };
            sync::run(&device, &transport).await.expect("a sync");
            files::local_copy(&device, &transport, &bob, file.id)
                .await
                .expect("fetched");

            memberships::leave(&device, &bob, garden)
                .await
                .expect("left");
            let report = sync::run(&device, &transport).await.expect("a sync");
            assert_eq!(report.pruned, 1);
            assert_eq!(report.cleaned, 1);
            let mut tx = device.begin().await.expect("a transaction");
            assert!(
                files::get_in(&mut tx, file.id)
                    .await
                    .expect("read")
                    .is_none()
            );
            tx.commit().await.expect("committed");
            assert_eq!(
                std::fs::read_dir(folder.path())
                    .expect("the folder")
                    .count(),
                0
            );
            assert_eq!(names(&server, &alice).await, ["notes.txt"], "it stays");

            memberships::join(&device, &bob, garden)
                .await
                .expect("joined again");
            sync::run(&device, &transport).await.expect("a sync");
            assert_eq!(names(&device, &bob).await, ["notes.txt"]);
        })
        .await;
    }

    #[tokio::test]
    async fn files_communities_a_row_taken_out_before_a_device_saw_the_file_brings_nothing() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let (alice, _) = user_with_token(&server, "alice@example.com").await;
            let (bob, bobs_token) = user_with_token(&server, "bob@example.com").await;
            let garden = community(&server, &alice, "Garden").await;
            let cooking = community(&server, &alice, "Cooking").await;
            let file = file_in(&server, &storage, &alice, "notes.txt", &[garden, cooking]).await;
            delete(&server, &alice, file.id, garden)
                .await
                .expect("taken out of garden");
            memberships::join(&server, &bob, garden)
                .await
                .expect("joined after");

            let page = sync::pull(&server, &bob, PullRequest::default())
                .await
                .expect("a page");
            assert!(
                page.rows.iter().all(|row| row.table != files::TABLE),
                "the file is in cooking only, which bob is not in"
            );
            let (device, _folder) = device_with_folder(&bob, &bobs_token).await;
            sync::run(&device, &InProcess { server: &server })
                .await
                .expect("a sync");
            let mut tx = device.begin().await.expect("a transaction");
            assert!(
                files::get_in(&mut tx, file.id)
                    .await
                    .expect("read")
                    .is_none()
            );
            tx.commit().await.expect("committed");
        })
        .await;
    }

    #[tokio::test]
    async fn files_communities_a_waiting_file_shared_on_a_device_reaches_both_communities() {
        with_each_database(|database| async move {
            let (server, _storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let garden = community(&server, &alice, "Garden").await;
            let (device, _folder) = device_with_folder(&alice, &token).await;
            let transport = InProcess { server: &server };
            sync::run(&device, &transport).await.expect("a sync");
            let personal = community(&device, &alice, "Personal").await;
            let file = files::create_with_bytes(
                &device,
                &alice,
                "notes.txt",
                "text/plain",
                &[personal],
                b"notes",
            )
            .await
            .expect("a file");
            files::update(&device, &alice, file.id, "notes.txt", &[personal, garden])
                .await
                .expect("shared while waiting");

            let report = sync::run(&device, &transport).await.expect("a sync");
            assert_eq!(report.pending, 0, "{report:?}");
            assert_eq!(report.rejected, 0, "{report:?}");
            let mut on_server = community_names(&server, &alice, file.id).await;
            on_server.sort();
            assert_eq!(on_server, ["Garden", "Personal"]);
        })
        .await;
    }

    #[tokio::test]
    async fn files_list_duplicates_finds_the_same_bytes_among_the_files_a_user_sees_only() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let alice = user(&server, "Alice").await;
            let bob = user(&server, "Bob").await;
            let garden = community(&server, &alice, "Garden").await;
            let woodwork = community(&server, &bob, "Woodwork").await;
            let alices = file_in(&server, &storage, &alice, "roses.jpg", &[garden]).await;
            let bobs = file_in(&server, &storage, &bob, "the same.jpg", &[woodwork]).await;
            assert_eq!(alices.sha256, bobs.sha256, "the same bytes");
            let other = crate::helpers::sha256::hex(b"other bytes");

            let found = files::list_duplicates(&server, &alice, &[other.clone(), alices.sha256])
                .await
                .expect("found");
            let names: Vec<&str> = found.iter().map(|shown| shown.file.name.as_str()).collect();
            assert_eq!(names, ["roses.jpg"], "nothing of bob's is told");
            assert_eq!(found[0].communities[0].id, garden);
            assert!(
                files::list_duplicates(&server, &alice, std::slice::from_ref(&other))
                    .await
                    .expect("looked")
                    .is_empty()
            );
            assert!(
                files::list_duplicates(&server, &alice, &[])
                    .await
                    .expect("looked")
                    .is_empty()
            );
            let error = files::list_duplicates(&server, &alice, &["not a checksum".to_string()])
                .await
                .expect_err("not a SHA-256");
            assert!(matches!(error, FileError::Invalid(_)));
            let too_many = vec![other; files::MAX_DUPLICATES_CHECKED + 1];
            let error = files::list_duplicates(&server, &alice, &too_many)
                .await
                .expect_err("too many");
            assert!(matches!(error, FileError::Invalid(_)));
        })
        .await;
    }
}
