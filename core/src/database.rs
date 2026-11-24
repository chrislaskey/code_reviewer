//! The database a deployment opens: a SQLite file on the device for the
//! desktop and mobile apps, and for the web server too by default, or
//! PostgreSQL for the web server, behind the `postgres` feature. Opening
//! runs the migrations under `migrations/`, so every deployment has the
//! same tables. A core function runs in one `Transaction` on it, and
//! each record context dispatches to its private SQLite or PostgreSQL
//! adapter.

use std::path::Path;
use std::str::FromStr;

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use thiserror::Error;

/// The SQLite migrations, embedded.
static SQLITE_MIGRATOR: Migrator = sqlx::migrate!("migrations/sqlite");
/// The PostgreSQL migrations, embedded.
#[cfg(feature = "postgres")]
static POSTGRES_MIGRATOR: Migrator = sqlx::migrate!("migrations/postgres");

#[derive(Clone, Debug)]
pub enum Database {
    Sqlite(SqlitePool),
    #[cfg(feature = "postgres")]
    Postgres(sqlx::PgPool),
}

/// One transaction: what every core function runs in, so a row and the
/// event that changed it are written together or not at all. The SQL
/// modules take the connection inside it, never a pool.
pub enum Transaction {
    Sqlite(sqlx::Transaction<'static, sqlx::Sqlite>),
    #[cfg(feature = "postgres")]
    Postgres(sqlx::Transaction<'static, sqlx::Postgres>),
}

impl Transaction {
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        match self {
            Transaction::Sqlite(tx) => tx.commit().await,
            #[cfg(feature = "postgres")]
            Transaction::Postgres(tx) => tx.commit().await,
        }
    }
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("could not open the database: {0}")]
    Open(#[source] sqlx::Error),
    #[error("could not migrate the database: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

impl Database {
    /// On SQLite the transaction takes the write lock as it begins, so
    /// two that read then write wait their turn by the busy timeout;
    /// begun as readers, the second to write would fail at once with
    /// "database is locked".
    pub async fn begin(&self) -> Result<Transaction, sqlx::Error> {
        Ok(match self {
            Database::Sqlite(pool) => {
                Transaction::Sqlite(pool.begin_with("BEGIN IMMEDIATE").await?)
            }
            #[cfg(feature = "postgres")]
            Database::Postgres(pool) => Transaction::Postgres(pool.begin().await?),
        })
    }

    /// Closes the connections, waiting for the ones in use to finish, so
    /// a SQLite file can be deleted or opened again.
    pub async fn close(&self) {
        match self {
            Database::Sqlite(pool) => pool.close().await,
            #[cfg(feature = "postgres")]
            Database::Postgres(pool) => pool.close().await,
        }
    }

    /// Opens the SQLite file, creating it and its directory when missing,
    /// and brings it up to date.
    pub async fn open_sqlite_file(path: &Path) -> Result<Database, DatabaseError> {
        if let Some(directory) = path.parent() {
            std::fs::create_dir_all(directory)
                .map_err(|error| DatabaseError::Open(error.into()))?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .map_err(DatabaseError::Open)?;
        Self::migrate_sqlite(pool).await
    }

    /// A SQLite database in memory, for tests. One connection, kept open:
    /// each SQLite connection to memory is a database of its own, so the
    /// pool holds a single one and never lets it go.
    pub async fn open_sqlite_in_memory() -> Result<Database, DatabaseError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .map_err(DatabaseError::Open)?
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options)
            .await
            .map_err(DatabaseError::Open)?;
        Self::migrate_sqlite(pool).await
    }

    async fn migrate_sqlite(pool: SqlitePool) -> Result<Database, DatabaseError> {
        SQLITE_MIGRATOR.run(&pool).await?;
        Ok(Database::Sqlite(pool))
    }

    /// Connects to the PostgreSQL database at the URL, which must exist,
    /// and brings it up to date.
    #[cfg(feature = "postgres")]
    pub async fn open_postgres(url: &str) -> Result<Database, DatabaseError> {
        let options =
            sqlx::postgres::PgConnectOptions::from_str(url).map_err(DatabaseError::Open)?;
        Self::open_postgres_with(options).await
    }

    #[cfg(feature = "postgres")]
    async fn open_postgres_with(
        options: sqlx::postgres::PgConnectOptions,
    ) -> Result<Database, DatabaseError> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_with(options)
            .await
            .map_err(DatabaseError::Open)?;
        POSTGRES_MIGRATOR.run(&pool).await?;
        Ok(Database::Postgres(pool))
    }
}

/// A table or column name for SQL that runs over every synced table,
/// which cannot be bound as a value: only lowercase letters and
/// underscores pass, so a name is never more than a name.
pub(crate) fn identifier(name: &str) -> Result<&str, sqlx::Error> {
    let plain = !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_');
    if plain {
        Ok(name)
    } else {
        Err(sqlx::Error::Protocol(format!(
            "not a table or column name: {name}"
        )))
    }
}

/// Runs a test against every database the tests can reach: SQLite in
/// memory always, and PostgreSQL when `TEST_POSTGRES_URL` names a database
/// to use, each test in a schema of its own that is dropped afterwards.
/// Every resource test runs through here, so the two databases' modules
/// never drift apart.
#[cfg(test)]
pub(crate) async fn with_each_database<Test, Run>(test: Test)
where
    Test: Fn(Database) -> Run,
    Run: Future<Output = ()>,
{
    let sqlite = Database::open_sqlite_in_memory()
        .await
        .expect("an in-memory SQLite database");
    test(sqlite).await;

    #[cfg(feature = "postgres")]
    if let Ok(url) = std::env::var("TEST_POSTGRES_URL") {
        let schema = format!("test_{}", uuid::Uuid::now_v7().simple());
        let options = sqlx::postgres::PgConnectOptions::from_str(&url)
            .expect("TEST_POSTGRES_URL is a PostgreSQL URL")
            .options([("search_path", schema.as_str())]);
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_with(options.clone())
            .await
            .expect("a connection to TEST_POSTGRES_URL");
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&pool)
            .await
            .expect("a schema for the test");
        let postgres = Database::open_postgres_with(options)
            .await
            .expect("a migrated PostgreSQL schema");
        test(postgres).await;
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
            .execute(&pool)
            .await
            .expect("the schema dropped");
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The database an environment variable names, `sqlite:<path>` or,
    /// with the feature, `postgres://...`, for a test against a running
    /// server's own database.
    pub(crate) async fn open_from_env(variable: &str) -> Database {
        let url = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} is needed"));
        if let Some(path) = url.strip_prefix("sqlite:") {
            return Database::open_sqlite_file(Path::new(path))
                .await
                .expect("the server's SQLite file");
        }
        #[cfg(feature = "postgres")]
        if url.starts_with("postgres") {
            return Database::open_postgres(&url)
                .await
                .expect("the server's PostgreSQL database");
        }
        panic!("{variable} must be sqlite:<path> or, with the postgres feature, postgres://...");
    }

    #[tokio::test]
    // Without the postgres feature, SQLite is the only variant
    #[allow(irrefutable_let_patterns)]
    async fn database_transactions_that_read_then_write_at_once_wait_their_turn() {
        let folder = tempfile::tempdir().expect("a folder");
        let database = Database::open_sqlite_file(&folder.path().join("app.db"))
            .await
            .expect("a SQLite file");
        let Database::Sqlite(pool) = &database else {
            panic!("a SQLite database");
        };
        sqlx::query("CREATE TABLE counts (n INTEGER)")
            .execute(pool)
            .await
            .expect("a table");

        let write = |database: Database| async move {
            let mut tx = database.begin().await?;
            let Transaction::Sqlite(sqlite) = &mut tx else {
                panic!("a SQLite transaction");
            };
            sqlx::query("SELECT count(*) FROM counts")
                .execute(&mut **sqlite)
                .await?;
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            sqlx::query("INSERT INTO counts (n) VALUES (1)")
                .execute(&mut **sqlite)
                .await?;
            tx.commit().await
        };
        let (first, second) = tokio::join!(write(database.clone()), write(database.clone()));
        first.expect("the first write");
        second.expect("the second write, after the first");
    }
}
