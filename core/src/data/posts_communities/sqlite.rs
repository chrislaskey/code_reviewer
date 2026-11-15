//! The `app_posts_communities` table:
//! a post in a community, see `posts_communities`.

use chrono::{DateTime, Utc};
use sqlx::types::Json;
use sqlx::{FromRow, SqliteConnection};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use crate::posts_communities::PostCommunity;
use crate::sync::Cursor;

#[derive(FromRow)]
struct PostCommunityRow {
    id: Hyphenated,
    post_id: Hyphenated,
    community_id: Hyphenated,
    users_communities_id: Hyphenated,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i64,
    last_synced_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_id: Option<Hyphenated>,
}

impl From<PostCommunityRow> for PostCommunity {
    fn from(row: PostCommunityRow) -> PostCommunity {
        PostCommunity {
            id: row.id.into_uuid(),
            post_id: row.post_id.into_uuid(),
            community_id: row.community_id.into_uuid(),
            users_communities_id: row.users_communities_id.into_uuid(),
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
            version: row.version,
            last_synced_at: row.last_synced_at,
            deleted_at: row.deleted_at,
            deleted_by_id: row.deleted_by_id.map(Hyphenated::into_uuid),
        }
    }
}

macro_rules! select {
    ($($rest:expr),+ $(,)?) => {
        concat!(
            "SELECT id, post_id, community_id, users_communities_id, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id FROM app_posts_communities ",
            $($rest),+
        )
    };
}

/// The rows the user, `$1`, sees: those of their own posts, and every
/// one in a community they are a live member of.
macro_rules! visible_to {
    () => {
        "(community_id IN (SELECT community_id FROM app_users_communities WHERE user_id = $1 AND deleted_at IS NULL) OR post_id IN (SELECT id FROM app_posts WHERE user_id = $1))"
    };
}

/// The row by its id, deleted or not.
pub async fn get(
    connection: &mut SqliteConnection,
    id: Uuid,
) -> Result<Option<PostCommunity>, sqlx::Error> {
    let row: Option<PostCommunityRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id.hyphenated())
        .fetch_optional(connection)
        .await?;
    Ok(row.map(PostCommunity::from))
}

/// The post's row for the community, deleted or not: there is one at
/// most, revived by adding the post back.
pub async fn get_by_post_and_community(
    connection: &mut SqliteConnection,
    post_id: Uuid,
    community_id: Uuid,
) -> Result<Option<PostCommunity>, sqlx::Error> {
    let row: Option<PostCommunityRow> =
        sqlx::query_as(select!("WHERE post_id = $1 AND community_id = $2"))
            .bind(post_id.hyphenated())
            .bind(community_id.hyphenated())
            .fetch_optional(connection)
            .await?;
    Ok(row.map(PostCommunity::from))
}

/// The live rows of the posts.
pub async fn list_live_of_posts(
    connection: &mut SqliteConnection,
    ids: &[Uuid],
) -> Result<Vec<PostCommunity>, sqlx::Error> {
    let rows: Vec<PostCommunityRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL AND post_id IN (SELECT value FROM json_each($1)) ORDER BY inserted_at, id"
    ))
    .bind(Json(ids.iter().map(|id| id.hyphenated().to_string()).collect::<Vec<String>>()))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(PostCommunity::from).collect())
}

/// The live rows under the membership: the member's own posts in the
/// community.
pub async fn list_live_of_membership(
    connection: &mut SqliteConnection,
    membership_id: Uuid,
) -> Result<Vec<PostCommunity>, sqlx::Error> {
    let rows: Vec<PostCommunityRow> = sqlx::query_as(select!(
        "WHERE users_communities_id = $1 AND deleted_at IS NULL ORDER BY inserted_at, id"
    ))
    .bind(membership_id.hyphenated())
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(PostCommunity::from).collect())
}

/// The rows under the membership deleted at the moment it was: what
/// left the community with the member.
pub async fn list_of_membership_deleted_at(
    connection: &mut SqliteConnection,
    membership_id: Uuid,
    deleted_at: DateTime<Utc>,
) -> Result<Vec<PostCommunity>, sqlx::Error> {
    let rows: Vec<PostCommunityRow> = sqlx::query_as(select!(
        "WHERE users_communities_id = $1 AND deleted_at = $2 ORDER BY inserted_at, id"
    ))
    .bind(membership_id.hyphenated())
    .bind(deleted_at)
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(PostCommunity::from).collect())
}

/// The rows the server wrote after the cursor that the user sees, in
/// cursor order, deleted or not: a page of a pull.
pub async fn list_synced_after(
    connection: &mut SqliteConnection,
    user_id: Uuid,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<PostCommunity>, sqlx::Error> {
    let rows: Vec<PostCommunityRow> = sqlx::query_as(select!(
        "WHERE last_synced_at IS NOT NULL AND (last_synced_at, id) > ($2, $3) AND ",
        visible_to!(),
        " ORDER BY last_synced_at, id LIMIT $4"
    ))
    .bind(user_id.hyphenated())
    .bind(after.last_synced_at)
    .bind(after.id.hyphenated())
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(PostCommunity::from).collect())
}

/// The live rows of the community after the cursor, in cursor order: a
/// page of a backfill.
pub async fn list_live_of_community_after(
    connection: &mut SqliteConnection,
    community_id: Uuid,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<PostCommunity>, sqlx::Error> {
    let rows: Vec<PostCommunityRow> = sqlx::query_as(select!(
        "WHERE community_id = $1 AND deleted_at IS NULL AND last_synced_at IS NOT NULL
           AND (last_synced_at, id) > ($2, $3)
         ORDER BY last_synced_at, id LIMIT $4"
    ))
    .bind(community_id.hyphenated())
    .bind(after.last_synced_at)
    .bind(after.id.hyphenated())
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(PostCommunity::from).collect())
}

pub async fn insert(
    connection: &mut SqliteConnection,
    row: &PostCommunity,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_posts_communities (id, post_id, community_id, users_communities_id, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(row.id.hyphenated())
    .bind(row.post_id.hyphenated())
    .bind(row.community_id.hyphenated())
    .bind(row.users_communities_id.hyphenated())
    .bind(row.inserted_at)
    .bind(row.updated_at)
    .bind(row.version)
    .bind(row.last_synced_at)
    .bind(row.deleted_at)
    .bind(row.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes the row over itself; false when there is no such row.
pub async fn update(
    connection: &mut SqliteConnection,
    row: &PostCommunity,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE app_posts_communities SET users_communities_id = $2, updated_at = $3, version = $4, last_synced_at = $5, deleted_at = $6, deleted_by_id = $7
         WHERE id = $1",
    )
    .bind(row.id.hyphenated())
    .bind(row.users_communities_id.hyphenated())
    .bind(row.updated_at)
    .bind(row.version)
    .bind(row.last_synced_at)
    .bind(row.deleted_at)
    .bind(row.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Writes the server's row over the device's, or inserts it.
pub async fn upsert(
    connection: &mut SqliteConnection,
    row: &PostCommunity,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_posts_communities (id, post_id, community_id, users_communities_id, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (id) DO UPDATE SET
           post_id = excluded.post_id, community_id = excluded.community_id, users_communities_id = excluded.users_communities_id,
           inserted_at = excluded.inserted_at, updated_at = excluded.updated_at, version = excluded.version,
           last_synced_at = excluded.last_synced_at, deleted_at = excluded.deleted_at, deleted_by_id = excluded.deleted_by_id",
    )
    .bind(row.id.hyphenated())
    .bind(row.post_id.hyphenated())
    .bind(row.community_id.hyphenated())
    .bind(row.users_communities_id.hyphenated())
    .bind(row.inserted_at)
    .bind(row.updated_at)
    .bind(row.version)
    .bind(row.last_synced_at)
    .bind(row.deleted_at)
    .bind(row.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(())
}

/// Removes the row itself, when a device's row was replaced by the
/// server's under another id, or its create refused.
pub async fn delete_row(connection: &mut SqliteConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_posts_communities WHERE id = $1")
        .bind(id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}
