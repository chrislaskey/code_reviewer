//! The `app_users_communities` table, the counterpart of `sqlite::memberships`:
//! a user in a community, see `memberships`.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::memberships::Membership;
use crate::sync::Cursor;

#[derive(FromRow)]
struct MembershipRow {
    id: Uuid,
    user_id: Uuid,
    community_id: Uuid,
    role: String,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i64,
    last_synced_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_id: Option<Uuid>,
}

impl From<MembershipRow> for Membership {
    fn from(row: MembershipRow) -> Membership {
        Membership {
            id: row.id,
            user_id: row.user_id,
            community_id: row.community_id,
            role: row.role,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
            version: row.version,
            last_synced_at: row.last_synced_at,
            deleted_at: row.deleted_at,
            deleted_by_id: row.deleted_by_id,
        }
    }
}

macro_rules! select {
    ($($rest:expr),+ $(,)?) => {
        concat!(
            "SELECT id, user_id, community_id, role, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id FROM app_users_communities ",
            $($rest),+
        )
    };
}

/// The memberships the user, `$1`, sees: their own, and every one in
/// a community they are a live member of.
macro_rules! visible_to {
    () => {
        "(user_id = $1 OR community_id IN (SELECT community_id FROM app_users_communities WHERE user_id = $1 AND deleted_at IS NULL))"
    };
}

/// The row by its id, deleted or not.
pub async fn get(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<Option<Membership>, sqlx::Error> {
    let row: Option<MembershipRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id)
        .fetch_optional(connection)
        .await?;
    Ok(row.map(Membership::from))
}

/// The user's row for the community, deleted or not: there is one at
/// most, revived by a rejoin.
pub async fn get_by_user_and_community(
    connection: &mut PgConnection,
    user_id: Uuid,
    community_id: Uuid,
) -> Result<Option<Membership>, sqlx::Error> {
    let row: Option<MembershipRow> =
        sqlx::query_as(select!("WHERE user_id = $1 AND community_id = $2"))
            .bind(user_id)
            .bind(community_id)
            .fetch_optional(connection)
            .await?;
    Ok(row.map(Membership::from))
}

/// The user's live memberships.
pub async fn list_live_of_user(
    connection: &mut PgConnection,
    user_id: Uuid,
) -> Result<Vec<Membership>, sqlx::Error> {
    let rows: Vec<MembershipRow> = sqlx::query_as(select!(
        "WHERE user_id = $1 AND deleted_at IS NULL ORDER BY inserted_at, id"
    ))
    .bind(user_id)
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Membership::from).collect())
}

/// The community's live memberships.
pub async fn list_live_of_community(
    connection: &mut PgConnection,
    community_id: Uuid,
) -> Result<Vec<Membership>, sqlx::Error> {
    let rows: Vec<MembershipRow> = sqlx::query_as(select!(
        "WHERE community_id = $1 AND deleted_at IS NULL ORDER BY inserted_at, id"
    ))
    .bind(community_id)
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Membership::from).collect())
}

/// The rows the server wrote after the cursor that the user sees, in
/// cursor order, deleted or not: a page of a pull.
pub async fn list_synced_after(
    connection: &mut PgConnection,
    user_id: Uuid,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<Membership>, sqlx::Error> {
    let rows: Vec<MembershipRow> = sqlx::query_as(select!(
        "WHERE last_synced_at IS NOT NULL AND (last_synced_at, id) > ($2, $3) AND ",
        visible_to!(),
        " ORDER BY last_synced_at, id LIMIT $4"
    ))
    .bind(user_id)
    .bind(after.last_synced_at)
    .bind(after.id)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Membership::from).collect())
}

pub async fn insert(
    connection: &mut PgConnection,
    membership: &Membership,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_users_communities (id, user_id, community_id, role, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(membership.id)
    .bind(membership.user_id)
    .bind(membership.community_id)
    .bind(&membership.role)
    .bind(membership.inserted_at)
    .bind(membership.updated_at)
    .bind(membership.version)
    .bind(membership.last_synced_at)
    .bind(membership.deleted_at)
    .bind(membership.deleted_by_id)
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes the membership over its row; false when there is no such row.
pub async fn update(
    connection: &mut PgConnection,
    membership: &Membership,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE app_users_communities SET role = $2, updated_at = $3, version = $4, last_synced_at = $5, deleted_at = $6, deleted_by_id = $7
         WHERE id = $1",
    )
    .bind(membership.id)
    .bind(&membership.role)
    .bind(membership.updated_at)
    .bind(membership.version)
    .bind(membership.last_synced_at)
    .bind(membership.deleted_at)
    .bind(membership.deleted_by_id)
    .execute(connection)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Writes the server's row over the device's, or inserts it.
pub async fn upsert(
    connection: &mut PgConnection,
    membership: &Membership,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_users_communities (id, user_id, community_id, role, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (id) DO UPDATE SET
           user_id = excluded.user_id, community_id = excluded.community_id, role = excluded.role,
           inserted_at = excluded.inserted_at, updated_at = excluded.updated_at, version = excluded.version,
           last_synced_at = excluded.last_synced_at, deleted_at = excluded.deleted_at, deleted_by_id = excluded.deleted_by_id",
    )
    .bind(membership.id)
    .bind(membership.user_id)
    .bind(membership.community_id)
    .bind(&membership.role)
    .bind(membership.inserted_at)
    .bind(membership.updated_at)
    .bind(membership.version)
    .bind(membership.last_synced_at)
    .bind(membership.deleted_at)
    .bind(membership.deleted_by_id)
    .execute(connection)
    .await?;
    Ok(())
}

/// Removes the row itself, when a device's row was replaced by the
/// server's under another id, or its create refused.
pub async fn delete_row(connection: &mut PgConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_users_communities WHERE id = $1")
        .bind(id)
        .execute(connection)
        .await?;
    Ok(())
}
