//! Pending changes: the changes a device made that have not reached the
//! server yet, as the page lists them, so a person sees what a sign-out
//! would lose before choosing to. Each is an event still queued, see
//! `events::pending`; a sync sends them.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::communities::{self, CommunityError};
use crate::deployment::Deployment;
use crate::events::{self, EventKind, Fields, StoredEvent};
use crate::files::{self, FileError};
use crate::posts::{self, PostError};
use crate::users::{self, UserError};

/// A change waiting to reach the server, as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingChange {
    pub id: Uuid,
    pub table: String,
    pub row_id: Uuid,
    pub kind: EventKind,
    /// When the change was made, on the device's clock.
    pub made_at: DateTime<Utc>,
    /// What it was about, a post's first line or a file's name; none for
    /// another table.
    pub about: Option<String>,
    /// The values the person chose.
    pub changes: Fields,
}

#[derive(Debug, Error)]
pub enum PendingChangeError {
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

/// Every change waiting to reach the server, in the order they were made.
pub async fn list(deployment: &Deployment) -> Result<Vec<PendingChange>, PendingChangeError> {
    let mut tx = deployment.begin().await?;
    let mut changes = Vec::new();
    for stored in events::pending(&mut tx).await? {
        let about = match stored.event.table.as_str() {
            posts::TABLE => posts::get_in(&mut tx, stored.event.row_id)
                .await?
                .map(|post| post.body.lines().next().unwrap_or_default().to_string()),
            files::TABLE => files::get_in(&mut tx, stored.event.row_id)
                .await?
                .map(|file| file.name),
            communities::TABLE => communities::get_in(&mut tx, stored.event.row_id)
                .await?
                .map(|community| community.name),
            users::TABLE => users::get_in(&mut tx, stored.event.row_id)
                .await?
                .map(|user| user.name),
            _ => None,
        };
        changes.push(pending_change(stored, about));
    }
    tx.commit().await?;
    Ok(changes)
}

fn pending_change(stored: StoredEvent, about: Option<String>) -> PendingChange {
    PendingChange {
        id: stored.event.id,
        table: stored.event.table,
        row_id: stored.event.row_id,
        kind: stored.event.kind,
        made_at: stored.event.made_at,
        about,
        changes: stored.event.changes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::with_each_database;
    use crate::deployment::Role;
    use crate::sync::{self, InProcess};
    use crate::users;

    #[tokio::test]
    async fn pending_changes_list_what_waits_and_nothing_once_synced() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = sync::tests::user_with_token(&server, "alice@example.com").await;
            let device = users::tests::device_of(&alice, &token).await;
            let post = posts::create(&device, &alice, "first line\nsecond", &[])
                .await
                .expect("a post");
            posts::update(&device, &alice, post.id, "changed\nsecond", &[])
                .await
                .expect("updated");

            let changes = list(&device).await.expect("listed");
            assert_eq!(changes.len(), 2, "the create, then the update");
            assert_eq!(changes[0].kind, EventKind::Create);
            assert_eq!(changes[1].kind, EventKind::Update);
            assert_eq!(changes[1].about.as_deref(), Some("changed"));
            assert_eq!(changes[1].changes["body"], "changed\nsecond");

            sync::run(&device, &InProcess { server: &server })
                .await
                .expect("synced");
            assert!(list(&device).await.expect("listed").is_empty());
        })
        .await;
    }
}
