//! The last visited page: the path and query of the page the person was
//! last on in an account, kept as a device setting in that account's own
//! file. A switch to the account opens it again; signing in and starting
//! the app still open Home. The pages decide which paths are worth
//! keeping; here a path is only checked to be one of the app's own.

use thiserror::Error;

use crate::data::device_settings::{self, DeviceSettingsError, LAST_VISITED_PAGE};
use crate::deployment::{Deployment, Role};

/// The longest path kept; a longer one is refused.
pub const MAX_PATH_LENGTH: usize = 2048;

#[derive(Debug, Error)]
pub enum LastVisitedPageError {
    #[error("the last visited page must be a path of the app, such as /posts")]
    InvalidPath,
    #[error("the last visited page is kept by a device, not the server")]
    DeviceOnly,
    #[error(transparent)]
    DeviceSettings(#[from] DeviceSettingsError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The last visited page in this account on this device, if any.
pub async fn get_on_device(
    deployment: &Deployment,
) -> Result<Option<String>, LastVisitedPageError> {
    device_only(deployment)?;
    let mut tx = deployment.begin().await?;
    let path: Option<String> = device_settings::get(&mut tx, LAST_VISITED_PAGE).await?;
    tx.commit().await?;
    Ok(path)
}

/// Keeps the path as the last visited page in this account on this device.
pub async fn update_on_device(
    deployment: &Deployment,
    path: &str,
) -> Result<(), LastVisitedPageError> {
    device_only(deployment)?;
    if !is_app_path(path) {
        return Err(LastVisitedPageError::InvalidPath);
    }
    let mut tx = deployment.begin().await?;
    device_settings::set(&mut tx, LAST_VISITED_PAGE, &Some(path)).await?;
    tx.commit().await?;
    Ok(())
}

/// A path on the app itself: starts with one `/`, so never another site.
fn is_app_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && !path.contains('\\')
        && path.len() <= MAX_PATH_LENGTH
}

fn device_only(deployment: &Deployment) -> Result<(), LastVisitedPageError> {
    match deployment.role {
        Role::Device => Ok(()),
        Role::Server => Err(LastVisitedPageError::DeviceOnly),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::users::tests::device;
    use crate::database::Database;

    #[tokio::test]
    async fn last_visited_page_update_then_get() {
        let device = device().await;
        assert_eq!(get_on_device(&device).await.expect("read"), None);

        update_on_device(&device, "/posts/1?tab=comments")
            .await
            .expect("kept");
        assert_eq!(
            get_on_device(&device).await.expect("read").as_deref(),
            Some("/posts/1?tab=comments")
        );
        update_on_device(&device, "/home").await.expect("kept");
        assert_eq!(
            get_on_device(&device).await.expect("read").as_deref(),
            Some("/home")
        );
    }

    #[tokio::test]
    async fn last_visited_page_update_refuses_a_path_off_the_app() {
        let device = device().await;
        let too_long = format!("/{}", "a".repeat(MAX_PATH_LENGTH));
        for path in [
            "",
            "posts",
            "//evil.example.com",
            "/\\evil.example.com",
            "https://evil.example.com",
            &too_long,
        ] {
            let error = update_on_device(&device, path).await.expect_err("refused");
            assert!(matches!(error, LastVisitedPageError::InvalidPath), "{path}");
        }
        assert_eq!(get_on_device(&device).await.expect("read"), None);
    }

    #[tokio::test]
    async fn last_visited_page_is_the_device_alone() {
        let database = Database::open_sqlite_in_memory()
            .await
            .expect("an in-memory database");
        let server = Deployment::new(database, Role::Server);
        assert!(matches!(
            get_on_device(&server).await,
            Err(LastVisitedPageError::DeviceOnly)
        ));
        assert!(matches!(
            update_on_device(&server, "/home").await,
            Err(LastVisitedPageError::DeviceOnly)
        ));
    }
}
