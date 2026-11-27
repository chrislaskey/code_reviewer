//! The synced tables sync runs over: the app's, `app::synced_tables`,
//! and in tests the framework's own `test_notes` after them.

use crate::app::synced_tables;
use crate::sync::synced_table::SyncedTable;

/// Every synced table, parents before the rows that point at them: the
/// order a device writes a page in, and, reversed, the order purge
/// removes rows in.
#[cfg(not(any(test, feature = "test-tables")))]
pub fn list() -> &'static [&'static dyn SyncedTable] {
    synced_tables::list()
}

/// Every synced table, the test notes last.
#[cfg(any(test, feature = "test-tables"))]
pub fn list() -> &'static [&'static dyn SyncedTable] {
    static LIST: std::sync::LazyLock<Vec<&'static dyn SyncedTable>> =
        std::sync::LazyLock::new(|| {
            let mut list = synced_tables::list().to_vec();
            list.push(&crate::sync::test_notes::Synced);
            list
        });
    &LIST
}

/// The synced table of the name, as an event or a row on the wire names it.
pub fn get(table: &str) -> Option<&'static dyn SyncedTable> {
    list()
        .iter()
        .copied()
        .find(|synced| synced.table() == table)
}

/// The table's place in `list`, parents first; after every table for an
/// unknown one.
pub fn position(table: &str) -> usize {
    list()
        .iter()
        .position(|synced| synced.table() == table)
        .unwrap_or(list().len())
}

/// Restricts physical deletion to rows without surviving synced dependents.
pub(crate) fn without_dependents(table: &str) -> Result<String, sqlx::Error> {
    use crate::database::identifier;
    use std::fmt::Write;

    let table = identifier(table)?;
    let mut predicate = String::new();
    for child in list() {
        let name = identifier(child.table())?;
        for &(column, parent) in child.foreign_keys() {
            if parent == table {
                let column = identifier(column)?;
                write!(
                    predicate,
                    " AND NOT EXISTS (SELECT 1 FROM {name} WHERE {name}.{column} = {table}.id)"
                )
                .map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
            }
        }
    }
    Ok(predicate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_list_only_tables_this_server_serves() {
        let unserved: Vec<_> = list()
            .iter()
            .filter(|synced| synced.server().is_none())
            .map(|synced| synced.table())
            .collect();
        assert!(unserved.is_empty(), "no server part: {unserved:?}");
    }
    use crate::database::Database;

    /// A table the app adds that carries the sync columns but is left out
    /// of `app::synced_tables` would never sync, be purged or move with a
    /// local user.
    #[tokio::test]
    async fn tables_list_every_table_with_sync_columns() {
        let database = Database::open_sqlite_in_memory()
            .await
            .expect("an in-memory SQLite database");
        #[allow(irrefutable_let_patterns)]
        let Database::Sqlite(pool) = database else {
            unreachable!("an in-memory database is SQLite");
        };
        let mut with_sync_columns: Vec<String> = sqlx::query_scalar(
            "SELECT tables.name FROM sqlite_master AS tables
               JOIN pragma_table_info(tables.name) AS columns
             WHERE tables.type = 'table' AND columns.name = 'global_sync_version' AND tables.name NOT IN ('app_sync_changes', 'app_sync_state', 'app_sync_versions')",
        )
        .fetch_all(&pool)
        .await
        .expect("the tables");
        with_sync_columns.sort();
        let mut listed: Vec<String> = list()
            .iter()
            .map(|synced| synced.table().to_string())
            .collect();
        listed.sort();
        assert_eq!(listed, with_sync_columns);

        for child in list() {
            let mut declared = child.foreign_keys().to_vec();
            declared.sort_unstable();
            let keys: Vec<(String, String)> = sqlx::query_as(
                r#"SELECT "from", "table" FROM pragma_foreign_key_list($1)
                 WHERE "to" = 'id' ORDER BY "from", "table""#,
            )
            .bind(child.table())
            .fetch_all(&pool)
            .await
            .expect("foreign keys");
            let registered: Vec<_> = keys
                .iter()
                .filter(|(_, parent)| get(parent).is_some())
                .map(|(column, parent)| (column.as_str(), parent.as_str()))
                .collect();
            assert_eq!(
                declared,
                registered,
                "{}'s synced foreign keys",
                child.table()
            );
        }
    }
}
