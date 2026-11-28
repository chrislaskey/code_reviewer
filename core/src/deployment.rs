//! A deployment: the database this web server or app opened, its role,
//! where it keeps the bytes of files, see `files`, and on the server the
//! bucket of users' pictures, see `avatars`. The web server is
//! the system of record; a desktop or mobile app is a device with a copy
//! that syncs with it, see `docs/design/offline-sync.md`. Reads are the
//! same in both roles. A write differs in two places: the server
//! increments the row's version and records the event in its log, a
//! device leaves the version as the server's and queues the event for
//! the next push.

use std::path::PathBuf;

use chrono::{DateTime, SubsecRound, Utc};

use crate::database::{Database, Transaction};
#[cfg(feature = "object-storage")]
use crate::services::storage::Storage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The web server, the system of record.
    Server,
    /// A desktop or mobile app, syncing its copy with the server.
    Device,
}

/// Where a deployment keeps the bytes of files: the server in its
/// bucket, a device in the folder of the account its database is.
#[derive(Clone, Debug, Default)]
pub enum FileStore {
    /// A server with no bucket, or a test that adds no file.
    #[default]
    None,
    Folder(PathBuf),
    #[cfg(feature = "object-storage")]
    Bucket(Storage),
}

#[derive(Clone, Debug)]
pub struct Deployment {
    pub database: Database,
    pub role: Role,
    pub file_store: FileStore,
    /// The public bucket users' pictures go to, on a server that has one.
    #[cfg(feature = "object-storage")]
    pub avatar_bucket: Option<Storage>,
    /// Stripe, on a server that sells, for the jobs that ask it.
    #[cfg(feature = "stripe")]
    pub stripe: Option<crate::services::stripe::Stripe>,
    /// The App Store, on a server that sells in the iPhone app.
    #[cfg(feature = "app-store")]
    pub app_store: Option<crate::services::app_store::AppStore>,
    /// The remote backend that owns the database, on a web server that
    /// has one: the public functions it serves call it instead, see
    /// `docs/design/backend-seam.md`. The database is then `None`.
    #[cfg(feature = "remote-backend")]
    pub remote_backend: Option<crate::services::backend::RemoteBackend>,
}

impl Deployment {
    /// A deployment that keeps no files' bytes, until `with_file_store`,
    /// and no pictures, until `with_avatar_bucket`.
    pub fn new(database: Database, role: Role) -> Deployment {
        Deployment {
            database,
            role,
            file_store: FileStore::None,
            #[cfg(feature = "object-storage")]
            avatar_bucket: None,
            #[cfg(feature = "stripe")]
            stripe: None,
            #[cfg(feature = "app-store")]
            app_store: None,
            #[cfg(feature = "remote-backend")]
            remote_backend: None,
        }
    }

    /// A web server whose remote backend owns the database: it opens
    /// none.
    #[cfg(feature = "remote-backend")]
    pub fn new_with_remote_backend(
        remote_backend: crate::services::backend::RemoteBackend,
    ) -> Deployment {
        Deployment {
            remote_backend: Some(remote_backend),
            ..Deployment::new(Database::None, Role::Server)
        }
    }

    /// Whether a remote backend owns the database, so only what it
    /// serves is answered.
    pub fn has_remote_backend(&self) -> bool {
        #[cfg(feature = "remote-backend")]
        return self.remote_backend.is_some();
        #[cfg(not(feature = "remote-backend"))]
        false
    }

    pub fn with_file_store(self, file_store: FileStore) -> Deployment {
        Deployment { file_store, ..self }
    }

    #[cfg(feature = "stripe")]
    pub fn with_stripe(self, stripe: crate::services::stripe::Stripe) -> Deployment {
        Deployment {
            stripe: Some(stripe),
            ..self
        }
    }

    #[cfg(feature = "app-store")]
    pub fn with_app_store(self, app_store: crate::services::app_store::AppStore) -> Deployment {
        Deployment {
            app_store: Some(app_store),
            ..self
        }
    }

    #[cfg(feature = "object-storage")]
    pub fn with_avatar_bucket(self, avatar_bucket: Storage) -> Deployment {
        Deployment {
            avatar_bucket: Some(avatar_bucket),
            ..self
        }
    }

    /// Begins the one transaction a core function runs in.
    pub async fn begin(&self) -> Result<Transaction, sqlx::Error> {
        self.database.begin().await
    }
}

/// The time now on this deployment's clock, to the microsecond, which is
/// what the databases keep.
pub fn now() -> DateTime<Utc> {
    Utc::now().trunc_subsecs(6)
}
