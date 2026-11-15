//! Embedded sqlite migrations. Record adapters live privately under `data`.

use sqlx::migrate::Migrator;

pub static MIGRATOR: Migrator = sqlx::migrate!("migrations/sqlite");
