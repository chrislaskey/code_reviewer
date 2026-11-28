//! The device's settings as a whole, see `data::device_settings` for
//! each one: bringing them up to date when the app opens an account's
//! database.

use crate::data::device_settings::{self, DeviceSettingsError};
use crate::deployment::Deployment;

/// Brings the device's settings up to date with
/// `data::device_settings::defaults`: inserts each one that is missing
/// with its first value, and leaves the rest as the person set them. The
/// place for a release that renames or reshapes a setting to rewrite
/// what an earlier release stored. Returns how many were inserted.
pub async fn upgrade(deployment: &Deployment) -> Result<usize, DeviceSettingsError> {
    let mut tx = deployment.begin().await?;
    let inserted = device_settings::insert_missing_defaults(&mut tx).await?;
    tx.commit().await?;
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::users::SignedIn;
    use crate::data::device_settings::{SIGNED_IN, WORK_OFFLINE, defaults, get, set};
    use crate::database::with_each_database;
    use crate::deployment::Role;

    #[tokio::test]
    async fn device_settings_upgrade_adds_what_is_missing_and_keeps_what_is_set() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Device);
            let mut tx = deployment.begin().await.expect("a transaction");
            let error = get::<bool>(&mut tx, WORK_OFFLINE)
                .await
                .expect_err("not set up");
            assert!(matches!(error, DeviceSettingsError::Missing(key) if key == WORK_OFFLINE));
            tx.commit().await.expect("committed");

            assert_eq!(
                upgrade(&deployment).await.expect("upgraded"),
                defaults().len()
            );
            let mut tx = deployment.begin().await.expect("a transaction");
            assert!(!get::<bool>(&mut tx, WORK_OFFLINE).await.expect("read"));
            assert_eq!(
                get::<Option<SignedIn>>(&mut tx, SIGNED_IN)
                    .await
                    .expect("nobody"),
                None
            );
            set(&mut tx, WORK_OFFLINE, &true).await.expect("set");
            tx.commit().await.expect("committed");

            assert_eq!(upgrade(&deployment).await.expect("upgraded again"), 0);
            let mut tx = deployment.begin().await.expect("a transaction");
            assert!(get::<bool>(&mut tx, WORK_OFFLINE).await.expect("read"));
            let error = get::<String>(&mut tx, WORK_OFFLINE)
                .await
                .expect_err("not a string");
            assert!(matches!(error, DeviceSettingsError::Invalid { .. }));
            tx.commit().await.expect("committed");
        })
        .await;
    }
}
