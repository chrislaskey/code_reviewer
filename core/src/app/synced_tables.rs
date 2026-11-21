//! The app's synced tables, which sync, conflicts, pending changes,
//! purge and a local user's move run over: a table the app adds is
//! listed here, see `sync::synced_table`.

use crate::app::{
    communities, files, files_communities, memberships, posts, posts_communities, users,
};
use crate::sync::synced_table::SyncedTable;

/// Every synced table, parents before the rows that point at them: the
/// order a device writes a page in, and, reversed, the order purge
/// removes rows in.
pub fn list() -> &'static [&'static dyn SyncedTable] {
    &[
        &users::Synced,
        &communities::Synced,
        &memberships::Synced,
        &posts::Synced,
        &posts_communities::Synced,
        &files::Synced,
        &files_communities::Synced,
    ]
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
    /// of `list` would never sync, be purged or move with a local user.
    #[tokio::test]
    async fn synced_tables_list_every_table_with_sync_columns() {
        let Database::Sqlite(pool) = Database::open_sqlite_in_memory()
            .await
            .expect("an in-memory SQLite database")
        else {
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
