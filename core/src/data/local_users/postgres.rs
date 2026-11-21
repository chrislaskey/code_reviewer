//! What a local user's move to an account does to the device's rows,
//! see `crate::app::local_users`; the counterpart of
//! `sqlite::local_users`, which is the one a device runs.

use sqlx::{AssertSqlSafe, PgConnection};
use uuid::Uuid;

use crate::database::identifier;

/// Gives every row of one user to another, who must have their row:
/// the column naming its maker in each table, and the events.
pub async fn give_rows_to_user(
    connection: &mut PgConnection,
    owner_columns: &[(&str, &str)],
    from: Uuid,
    to: Uuid,
) -> Result<(), sqlx::Error> {
    let mut statements = Vec::new();
    for (table, column) in owner_columns {
        let (table, column) = (identifier(table)?, identifier(column)?);
        statements.push(format!(
            "UPDATE {table} SET {column} = $2 WHERE {column} = $1"
        ));
    }
    statements.push("UPDATE app_events SET user_id = $2 WHERE user_id = $1".to_string());
    for statement in statements {
        sqlx::query(AssertSqlSafe(statement))
            .bind(from)
            .bind(to)
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}

/// Removes every soft-deleted row of the tables, in the order given:
/// nobody else ever saw a local user's rows, so there is nobody to tell.
pub async fn delete_deleted_rows(
    connection: &mut PgConnection,
    tables: &[&str],
) -> Result<(), sqlx::Error> {
    for table in tables {
        let table = identifier(table)?;
        sqlx::query(AssertSqlSafe(format!(
            "DELETE FROM {table} WHERE deleted_at IS NOT NULL"
        )))
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

pub async fn delete_user(connection: &mut PgConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_users WHERE id = $1")
        .bind(id)
        .execute(connection)
        .await?;
    Ok(())
}

/// How many live rows the user made in the tables, each named with the
/// column naming its maker.
pub async fn count_items(
    connection: &mut PgConnection,
    owner_columns: &[(&str, &str)],
    user_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let mut count = 0;
    for (table, column) in owner_columns {
        let (table, column) = (identifier(table)?, identifier(column)?);
        let (rows,): (i64,) = sqlx::query_as(AssertSqlSafe(format!(
            "SELECT COUNT(*) FROM {table} WHERE {column} = $1 AND deleted_at IS NULL"
        )))
        .bind(user_id)
        .fetch_one(&mut *connection)
        .await?;
        count += rows;
    }
    Ok(count)
}
