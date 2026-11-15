//! What a local user's move to an account does to the device's rows,
//! see `crate::local_users`.

use sqlx::SqliteConnection;
use uuid::Uuid;

/// Gives every row of one user to another, who must have their row.
pub async fn give_rows_to_user(
    connection: &mut SqliteConnection,
    from: Uuid,
    to: Uuid,
) -> Result<(), sqlx::Error> {
    for statement in [
        "UPDATE app_users_communities SET user_id = $2 WHERE user_id = $1",
        "UPDATE app_posts SET user_id = $2 WHERE user_id = $1",
        "UPDATE app_files SET user_id = $2 WHERE user_id = $1",
        "UPDATE app_events SET user_id = $2 WHERE user_id = $1",
    ] {
        sqlx::query(statement)
            .bind(from.hyphenated())
            .bind(to.hyphenated())
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}

/// Removes every soft-deleted row, children first: nobody else ever
/// saw a local user's rows, so there is nobody to tell.
pub async fn delete_deleted_rows(connection: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    for statement in [
        "DELETE FROM app_posts_communities WHERE deleted_at IS NOT NULL",
        "DELETE FROM app_posts WHERE deleted_at IS NOT NULL",
        "DELETE FROM app_files WHERE deleted_at IS NOT NULL",
        "DELETE FROM app_users_communities WHERE deleted_at IS NOT NULL",
        "DELETE FROM app_communities WHERE deleted_at IS NOT NULL",
    ] {
        sqlx::query(statement).execute(&mut *connection).await?;
    }
    Ok(())
}

pub async fn delete_user(connection: &mut SqliteConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_users WHERE id = $1")
        .bind(id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}

/// How many posts and files the user has, not deleted.
pub async fn count_items(
    connection: &mut SqliteConnection,
    user_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM app_posts WHERE user_id = $1 AND deleted_at IS NULL)
              + (SELECT COUNT(*) FROM app_files WHERE user_id = $1 AND deleted_at IS NULL)",
    )
    .bind(user_id.hyphenated())
    .fetch_one(connection)
    .await?;
    Ok(count)
}
