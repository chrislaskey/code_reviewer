//! The app's synced tables, which sync, conflicts, pending changes,
//! purge and a local user's move run over, through `sync::tables`: a
//! table the app adds is listed here, see `sync::synced_table`. `accounts_preferences` is the
//! one accounts table among them, see `docs/design/locales.md`.

use crate::accounts::preferences;
use crate::app::{communities, memberships, users};
use crate::sync::synced_table::SyncedTable;

/// Every synced table, parents before the rows that point at them: the
/// order a device writes a page in, and, reversed, the order purge
/// removes rows in.
pub fn list() -> &'static [&'static dyn SyncedTable] {
    &[
        &users::Synced,
        &preferences::Synced,
        &communities::Synced,
        &memberships::Synced,
    ]
}
