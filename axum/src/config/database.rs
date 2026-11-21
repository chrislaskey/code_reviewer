//! The web server's database: what `DATABASE_URL` names, `sqlite:<path>`
//! relative to this crate, or `postgres://...`; the file under `data/`
//! when unset. Opened as the server, the system of record the apps sync
//! with.

use std::env;
use std::path::PathBuf;

use app_core::database::Database;
use app_core::deployment::{Deployment, Role};

pub async fn open() -> Result<Deployment, String> {
    let database = open_database().await?;
    Ok(Deployment::new(database, Role::Server))
}

async fn open_database() -> Result<Database, String> {
    let url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/app.db".to_string());
    if let Some(path) = url.strip_prefix("sqlite:") {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
        tracing::info!("database {}", path.display());
        return Database::open_sqlite_file(&path)
            .await
            .map_err(|error| error.to_string());
    }
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        tracing::info!("database PostgreSQL");
        return Database::open_postgres(&url)
            .await
            .map_err(|error| error.to_string());
    }
    Err(format!(
        "DATABASE_URL must be sqlite:<path> or postgres://..., got '{url}'"
    ))
}
