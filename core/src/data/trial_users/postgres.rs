//! What tells a trial user apart, see `crate::trial_users`: an account
//! with `trial_expires_at`. A device has no accounts, so each of these
//! finds nothing there.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

/// The trial users there are, expired or not.
pub async fn count(connection: &mut PgConnection) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM accounts_users WHERE trial_expires_at IS NOT NULL")
            .fetch_one(connection)
            .await?;
    Ok(count)
}

/// How many writes of the user the server has recorded.
pub async fn count_writes(
    connection: &mut PgConnection,
    user_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM app_events WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(connection)
        .await?;
    Ok(count)
}

/// Whether a user joining a community would put a trial user and
/// anyone else in one community: the user is a trial and someone else
/// is or was a member, or someone who is or was a member is a trial.
pub async fn join_is_walled_off(
    connection: &mut PgConnection,
    user_id: Uuid,
    community_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let (walled_off,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (
           SELECT 1 FROM app_users_communities uc
           WHERE uc.community_id = $2 AND uc.user_id != $1
             AND ((SELECT trial_expires_at FROM accounts_users WHERE id = $1) IS NOT NULL
               OR (SELECT trial_expires_at FROM accounts_users WHERE id = uc.user_id) IS NOT NULL))",
    )
    .bind(user_id)
    .bind(community_id)
    .fetch_one(connection)
    .await?;
    Ok(walled_off)
}

/// Whether the community is not the viewer's to see: a trial user's
/// other than the viewer, or anyone's the viewer, a trial user, was
/// never in.
pub async fn community_is_hidden_from(
    connection: &mut PgConnection,
    community_id: Uuid,
    viewer_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let (hidden,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (
           SELECT 1 FROM app_users_communities uc
           JOIN accounts_users a ON a.id = uc.user_id
           WHERE uc.community_id = $1 AND uc.user_id != $2 AND a.trial_expires_at IS NOT NULL)
         OR ((SELECT trial_expires_at FROM accounts_users WHERE id = $2) IS NOT NULL
           AND NOT EXISTS (
             SELECT 1 FROM app_users_communities
             WHERE community_id = $1 AND user_id = $2))",
    )
    .bind(community_id)
    .bind(viewer_id)
    .fetch_one(connection)
    .await?;
    Ok(hidden)
}

/// Writes the user's communities as changed now, so the next pull of
/// everyone who could not see them while the user was a trial brings
/// them.
pub async fn touch_communities_of(
    connection: &mut PgConnection,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE app_communities SET version = version + 1, last_synced_at = $2
         WHERE id IN (SELECT community_id FROM app_users_communities WHERE user_id = $1)",
    )
    .bind(user_id)
    .bind(now)
    .execute(connection)
    .await?;
    Ok(())
}

/// Deletes outright the trial user, as `delete_expired_before` does,
/// when they still are one: a trial deleting itself. Whether they were.
pub async fn delete_one(connection: &mut PgConnection, user_id: Uuid) -> Result<bool, sqlx::Error> {
    let trial = "SELECT id FROM accounts_users WHERE id = $1 AND trial_expires_at IS NOT NULL";
    for statement in [
        format!("DELETE FROM app_events WHERE user_id IN ({trial})"),
        format!(
            "DELETE FROM app_communities WHERE id IN
               (SELECT community_id FROM app_users_communities WHERE user_id IN ({trial}))"
        ),
        format!("DELETE FROM app_users WHERE id IN ({trial})"),
    ] {
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .bind(user_id)
            .execute(&mut *connection)
            .await?;
    }
    let result =
        sqlx::query("DELETE FROM accounts_users WHERE id = $1 AND trial_expires_at IS NOT NULL")
            .bind(user_id)
            .execute(connection)
            .await?;
    Ok(result.rows_affected() > 0)
}

/// Deletes outright the trial users whose trial ended before the time,
/// with everything of theirs: their events, the communities they are
/// in, which are theirs alone, their user's row and with it their
/// posts, files and memberships, and their account with its tokens.
/// How many users.
pub async fn delete_expired_before(
    connection: &mut PgConnection,
    time: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    for statement in [
        "DELETE FROM app_events WHERE user_id IN
           (SELECT id FROM accounts_users WHERE trial_expires_at < $1)",
        "DELETE FROM app_communities WHERE id IN
           (SELECT community_id FROM app_users_communities WHERE user_id IN
             (SELECT id FROM accounts_users WHERE trial_expires_at < $1))",
        "DELETE FROM app_users WHERE id IN
           (SELECT id FROM accounts_users WHERE trial_expires_at < $1)",
    ] {
        sqlx::query(statement)
            .bind(time)
            .execute(&mut *connection)
            .await?;
    }
    let result = sqlx::query("DELETE FROM accounts_users WHERE trial_expires_at < $1")
        .bind(time)
        .execute(connection)
        .await?;
    Ok(result.rows_affected())
}
