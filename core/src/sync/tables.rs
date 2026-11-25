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

#[cfg(test)]
mod tests {
    use super::*;
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
             WHERE tables.type = 'table' AND columns.name = 'last_synced_at'",
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
    }
}
