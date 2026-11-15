//! A user's picture, uploaded from the person's device straight to object
//! storage, see `storage`, never through the server:
//!
//! 1. `create_upload`: the server checks what the person picked, its
//!    content type and size, and answers a signed URL to PUT that one
//!    file to, at a key under the user's own prefix.
//! 2. The page PUTs the file to the URL.
//! 3. `update`: the server checks the file arrived and its first bytes
//!    are the image it said it was, then sets `avatar_url` on the user,
//!    an event in its log the devices pull like any change.
//!
//! Online only for now: a device asks the server for both steps, see
//! `create_upload_on_device` and `update_on_device`. A file nobody points
//! at, an upload never confirmed, a picture since replaced or one whose
//! user the purge removed, stays in the bucket until
//! `delete_unreferenced`, see `docs/design/uploads.md`.

use garde::Validate;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::field_errors::FieldErrors;
#[cfg(feature = "sync-http")]
use crate::sync::Http;
use crate::sync::SyncError;
use crate::users::UserError;
#[cfg(any(feature = "object-storage", feature = "sync-http"))]
use crate::{
    deployment::Deployment,
    users::{self, User},
};
#[cfg(feature = "object-storage")]
use crate::{
    deployment::Role,
    storage::{Storage, StorageError},
};

/// The largest picture, 5 MB.
pub const MAX_BYTES: u64 = 5 * 1024 * 1024;
/// How long a signed upload URL works.
#[cfg(feature = "object-storage")]
const UPLOAD_URL_LIFETIME: std::time::Duration = std::time::Duration::from_secs(10 * 60);
/// How long after its key was made an upload may still be confirmed;
/// the sweep leaves files far younger than that, so it never deletes
/// one that could still become someone's picture.
#[cfg(feature = "object-storage")]
const CONFIRM_WITHIN: chrono::Duration = chrono::Duration::hours(1);
/// A file nobody points at is deleted once it is older than this.
pub const UNREFERENCED_DAYS: i64 = 1;
/// Where pictures go in the bucket: `avatars/<user id>/<id>.<extension>`.
#[cfg(feature = "object-storage")]
const KEY_PREFIX: &str = "avatars";

/// The images a picture may be: its content type, the extension its key
/// ends with, and how its first bytes begin.
#[cfg_attr(not(feature = "object-storage"), allow(dead_code))]
struct ImageType {
    content_type: &'static str,
    extension: &'static str,
    starts_like: fn(&[u8]) -> bool,
}

const IMAGE_TYPES: &[ImageType] = &[
    ImageType {
        content_type: "image/png",
        extension: "png",
        starts_like: |bytes| bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
    },
    ImageType {
        content_type: "image/jpeg",
        extension: "jpg",
        starts_like: |bytes| bytes.starts_with(b"\xff\xd8\xff"),
    },
    ImageType {
        content_type: "image/gif",
        extension: "gif",
        starts_like: |bytes| bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
    },
    ImageType {
        content_type: "image/webp",
        extension: "webp",
        starts_like: |bytes| {
            bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
        },
    },
];

/// The file the person picked, as the page reads it before uploading.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Validate)]
pub struct AvatarUploadNew {
    #[garde(custom(is_image_type))]
    pub content_type: String,
    #[garde(custom(is_avatar_size))]
    pub size: u64,
}

/// Where to PUT the file, with the `Content-Type` header the URL was
/// signed for, and the key to hand back to `update` once it is there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvatarUpload {
    pub upload_url: String,
    pub content_type: String,
    pub key: String,
}

/// The upload to make the user's picture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvatarUpdate {
    pub key: String,
}

#[derive(Debug, Error)]
pub enum AvatarError {
    #[error("{0}")]
    Invalid(FieldErrors),
    #[error("that upload is not one of yours")]
    NotYours,
    #[error("A trial has no picture; sign up first")]
    TrialUser,
    #[error("the picture did not arrive; try again")]
    UploadMissing,
    #[error("that upload is too old; pick the picture again")]
    UploadExpired,
    #[error("the file is not the image it said it was")]
    NotTheImageType,
    #[error("the picture must be 5 MB or smaller")]
    TooLarge,
    #[error("only the server sets a picture")]
    ServerOnly,
    #[error("uploads are not set up on this server")]
    NoAvatarBucket,
    #[cfg(feature = "object-storage")]
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error(transparent)]
    User(#[from] UserError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Answers where to upload the file the person picked, once it is a
/// picture this app takes: on the server, which holds the bucket's keys.
#[cfg(feature = "object-storage")]
pub async fn create_upload(
    deployment: &Deployment,
    user: &User,
    request: &AvatarUploadNew,
) -> Result<AvatarUpload, AvatarError> {
    FieldErrors::check(request).map_err(AvatarError::Invalid)?;
    if crate::trial_users::is_trial(deployment, user).await? {
        return Err(AvatarError::TrialUser);
    }
    let storage = bucket(deployment)?;
    let image_type = image_type_of(&request.content_type).ok_or_else(|| {
        AvatarError::Invalid(FieldErrors::one("content_type", "is not a picture"))
    })?;
    let key = format!(
        "{KEY_PREFIX}/{}/{}.{}",
        user.id,
        uuid::Uuid::now_v7(),
        image_type.extension
    );
    let upload_url = storage
        .upload_url(&key, image_type.content_type, UPLOAD_URL_LIFETIME)
        .await?;
    Ok(AvatarUpload {
        upload_url,
        content_type: image_type.content_type.to_string(),
        key,
    })
}

/// Makes the uploaded file the user's picture, on the server, once it
/// is there, no larger than `MAX_BYTES`, and the image its key says; a
/// file that is not is deleted. The size is checked here because a
/// signed PUT URL cannot limit it.
#[cfg(feature = "object-storage")]
pub async fn update(
    deployment: &Deployment,
    user: &User,
    request: &AvatarUpdate,
) -> Result<User, AvatarError> {
    if deployment.role != Role::Server {
        return Err(AvatarError::ServerOnly);
    }
    let storage = bucket(deployment)?;
    let (image_type, key_made_at) =
        image_type_of_key(user, &request.key).ok_or(AvatarError::NotYours)?;
    if crate::deployment::now() - key_made_at > CONFIRM_WITHIN {
        return Err(AvatarError::UploadExpired);
    }
    let size = storage
        .size(&request.key)
        .await?
        .ok_or(AvatarError::UploadMissing)?;
    if size > MAX_BYTES {
        storage.delete(&request.key).await?;
        return Err(AvatarError::TooLarge);
    }
    let first_bytes = storage.first_bytes(&request.key, 12, size).await?;
    if !(image_type.starts_like)(&first_bytes) {
        storage.delete(&request.key).await?;
        return Err(AvatarError::NotTheImageType);
    }
    let mut tx = deployment.begin().await?;
    let user =
        users::update_avatar_url_in(&mut tx, user.id, &storage.public_url(&request.key)?).await?;
    tx.commit().await?;
    Ok(user)
}

/// What `delete_unreferenced` did: the files it left, and the ones it
/// deleted.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteUnreferencedReport {
    pub kept: usize,
    pub deleted: usize,
}

/// Deletes every picture of the user's from the bucket at once, for a
/// deleted account, see `accounts::deletions`; how many. A server with
/// no bucket has none.
#[cfg(feature = "object-storage")]
pub async fn delete_of_user(
    deployment: &Deployment,
    user_id: uuid::Uuid,
) -> Result<usize, AvatarError> {
    let Some(storage) = &deployment.avatar_bucket else {
        return Ok(0);
    };
    let pictures = storage.list(&format!("{KEY_PREFIX}/{user_id}")).await?;
    for picture in &pictures {
        storage.delete(&picture.key).await?;
    }
    Ok(pictures.len())
}

/// Deletes every picture no user points at, deleted users included, once
/// it is older than `UNREFERENCED_DAYS`: an upload never confirmed, a
/// picture replaced, the picture of a user the purge removed. Rows go
/// first and their files after, so whatever removed a row, a cascade
/// included, its file goes on the next run; a run cut short is finished
/// by the next. On the server, daily after `purge::run`, see `jobs`.
#[cfg(feature = "object-storage")]
pub async fn delete_unreferenced(
    deployment: &Deployment,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<DeleteUnreferencedReport, AvatarError> {
    if deployment.role != Role::Server {
        return Err(AvatarError::ServerOnly);
    }
    let storage = bucket(deployment)?;
    let mut tx = deployment.begin().await?;
    let referenced: std::collections::HashSet<String> = users::list_avatar_urls_in(&mut tx)
        .await?
        .iter()
        .filter_map(|url| key_of_url(url))
        .collect();
    tx.commit().await?;

    let cutoff = now - chrono::Duration::days(UNREFERENCED_DAYS);
    let mut report = DeleteUnreferencedReport::default();
    for file in storage.list(KEY_PREFIX).await? {
        if referenced.contains(&file.key) || file.last_modified > cutoff {
            report.kept += 1;
            continue;
        }
        storage.delete(&file.key).await?;
        report.deleted += 1;
    }
    Ok(report)
}

/// `create_upload` asked of the server by the user signed in on this
/// device, after the same checks here, so a wrong file is told at once.
#[cfg(feature = "sync-http")]
pub async fn create_upload_on_device(
    deployment: &Deployment,
    server: &Http,
    request: &AvatarUploadNew,
) -> Result<AvatarUpload, AvatarError> {
    FieldErrors::check(request).map_err(AvatarError::Invalid)?;
    let (_user, token) = users::signed_in_with_token(deployment).await?;
    Ok(server.avatars_create_upload(&token, request).await?)
}

/// `update` asked of the server by the user signed in on this device,
/// whose row the server answers is written here at once, rather than at
/// the next pull.
#[cfg(feature = "sync-http")]
pub async fn update_on_device(
    deployment: &Deployment,
    server: &Http,
    request: &AvatarUpdate,
) -> Result<User, AvatarError> {
    let (_user, token) = users::signed_in_with_token(deployment).await?;
    let user = server.avatars_update(&token, request).await?;
    let mut tx = deployment.begin().await?;
    users::upsert_in(&mut tx, &user).await?;
    tx.commit().await?;
    Ok(user)
}

fn image_type_of(content_type: &str) -> Option<&'static ImageType> {
    IMAGE_TYPES
        .iter()
        .find(|image_type| image_type.content_type == content_type)
}

/// The image type a key of the user's ends with, and when the key was
/// made, from its v7 UUID; none for a key that is not
/// `avatars/<their id>/<uuid v7>.<extension>`.
#[cfg(feature = "object-storage")]
fn image_type_of_key(
    user: &User,
    key: &str,
) -> Option<(&'static ImageType, chrono::DateTime<chrono::Utc>)> {
    let name = key.strip_prefix(&format!("{KEY_PREFIX}/{}/", user.id))?;
    let (id, extension) = name.split_once('.')?;
    let (seconds, nanos) = uuid::Uuid::parse_str(id).ok()?.get_timestamp()?.to_unix();
    let made_at = chrono::DateTime::from_timestamp(i64::try_from(seconds).ok()?, nanos)?;
    let image_type = IMAGE_TYPES
        .iter()
        .find(|image_type| image_type.extension == extension)?;
    Some((image_type, made_at))
}

/// The deployment's bucket of pictures, or `NoAvatarBucket`.
#[cfg(feature = "object-storage")]
fn bucket(deployment: &Deployment) -> Result<&Storage, AvatarError> {
    deployment
        .avatar_bucket
        .as_ref()
        .ok_or(AvatarError::NoAvatarBucket)
}

/// The key in a picture's URL, whatever the public URL in front of it,
/// so a bucket moved behind a CDN never makes a picture look unused.
#[cfg(feature = "object-storage")]
fn key_of_url(url: &str) -> Option<String> {
    let start = url.rfind(&format!("/{KEY_PREFIX}/"))?;
    Some(url[start + 1..].to_string())
}

fn is_image_type(content_type: &str, _context: &()) -> garde::Result {
    if image_type_of(content_type).is_none() {
        return Err(garde::Error::new(
            "must be a PNG, JPEG, GIF or WebP picture",
        ));
    }
    Ok(())
}

fn is_avatar_size(size: &u64, _context: &()) -> garde::Result {
    if *size == 0 {
        return Err(garde::Error::new("is empty"));
    }
    if *size > MAX_BYTES {
        return Err(garde::Error::new("must be 5 MB or smaller"));
    }
    Ok(())
}

#[cfg(all(test, feature = "object-storage"))]
mod tests {
    use super::*;
    use crate::database::{Database, with_each_database};
    use crate::events::{self, EventStatus};

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nthe rest of a picture";

    fn upload(content_type: &str, size: u64) -> AvatarUploadNew {
        AvatarUploadNew {
            content_type: content_type.to_string(),
            size,
        }
    }

    /// A server with a bucket of pictures in memory, the bucket, and Alice.
    async fn server_with_alice(database: Database) -> (Deployment, Storage, User) {
        let storage = Storage::memory();
        let deployment =
            Deployment::new(database, Role::Server).with_avatar_bucket(storage.clone());
        let alice = users::create(&deployment, "alice@example.com", "Alice")
            .await
            .expect("alice");
        (deployment, storage, alice)
    }

    /// A server with a bucket of pictures in memory, for the checks that
    /// read no rows.
    async fn server() -> Deployment {
        let database = Database::open_sqlite_in_memory().await.expect("a database");
        Deployment::new(database, Role::Server).with_avatar_bucket(Storage::memory())
    }

    #[tokio::test]
    async fn avatars_create_upload_answers_field_errors_for_a_wrong_file() {
        let deployment = server().await;
        let alice = users::tests::alice();
        let error = create_upload(&deployment, &alice, &upload("text/html", MAX_BYTES + 1))
            .await
            .expect_err("neither field is right");
        let AvatarError::Invalid(errors) = error else {
            panic!("field errors, got {error:?}");
        };
        assert_eq!(
            errors.0["content_type"],
            ["must be a PNG, JPEG, GIF or WebP picture"]
        );
        assert_eq!(errors.0["size"], ["must be 5 MB or smaller"]);

        let error = create_upload(&deployment, &alice, &upload("image/png", 0))
            .await
            .expect_err("empty");
        assert!(matches!(error, AvatarError::Invalid(errors) if errors.0["size"] == ["is empty"]));
    }

    #[tokio::test]
    async fn avatars_create_upload_signs_a_key_under_the_users_prefix() {
        let deployment = server().await;
        let alice = users::tests::alice();
        let answer = create_upload(&deployment, &alice, &upload("image/jpeg", 1000))
            .await
            .expect("an upload");
        assert!(
            answer.key.starts_with(&format!("avatars/{}/", alice.id)),
            "{}",
            answer.key
        );
        assert!(answer.key.ends_with(".jpg"));
        assert_eq!(answer.content_type, "image/jpeg");
        assert_eq!(answer.upload_url, format!("memory:///{}", answer.key));
    }

    #[tokio::test]
    async fn avatars_update_sets_the_url_as_an_event_once_the_file_is_there() {
        with_each_database(|database| async move {
            let (deployment, storage, alice) = server_with_alice(database).await;
            let answer = create_upload(&deployment, &alice, &upload("image/png", PNG.len() as u64))
                .await
                .expect("an upload");
            let request = AvatarUpdate {
                key: answer.key.clone(),
            };

            let error = update(&deployment, &alice, &request)
                .await
                .expect_err("not uploaded yet");
            assert!(matches!(error, AvatarError::UploadMissing));

            storage
                .put(&answer.key, PNG.to_vec())
                .await
                .expect("the upload");
            let updated = update(&deployment, &alice, &request)
                .await
                .expect("the picture");
            assert_eq!(
                updated.avatar_url,
                Some(format!("memory:///{}", answer.key))
            );
            assert_eq!(updated.version, alice.version + 1);
            assert_eq!(
                users::get(&deployment, alice.id).await.expect("alice"),
                updated
            );
            let mut tx = deployment.begin().await.expect("a transaction");
            let applied = events::count(&mut tx, EventStatus::Applied)
                .await
                .expect("a count");
            assert_eq!(applied, 1, "the change is in the server's log");
        })
        .await;
    }

    #[tokio::test]
    async fn avatars_update_refuses_another_users_key_and_deletes_a_file_too_large_or_not_its_type()
    {
        with_each_database(|database| async move {
            let (deployment, storage, alice) = server_with_alice(database).await;
            let bob = users::create(&deployment, "bob@example.com", "Bob")
                .await
                .expect("bob");

            let bobs = create_upload(&deployment, &bob, &upload("image/png", PNG.len() as u64))
                .await
                .expect("bob's upload");
            storage.put(&bobs.key, PNG.to_vec()).await.expect("put");
            let error = update(&deployment, &alice, &AvatarUpdate { key: bobs.key })
                .await
                .expect_err("bob's");
            assert!(matches!(error, AvatarError::NotYours));

            let answer = create_upload(&deployment, &alice, &upload("image/png", PNG.len() as u64))
                .await
                .expect("an upload");
            let mut huge = PNG.to_vec();
            huge.resize(MAX_BYTES as usize + 1, 0);
            storage.put(&answer.key, huge).await.expect("put");
            let request = AvatarUpdate {
                key: answer.key.clone(),
            };
            let error = update(&deployment, &alice, &request)
                .await
                .expect_err("larger than it said");
            assert!(matches!(error, AvatarError::TooLarge));
            assert_eq!(storage.size(&answer.key).await.expect("a head"), None);

            let html = b"<html><script>alert(1)</script></html>".to_vec();
            let answer =
                create_upload(&deployment, &alice, &upload("image/png", html.len() as u64))
                    .await
                    .expect("an upload");
            storage.put(&answer.key, html).await.expect("put");
            let request = AvatarUpdate {
                key: answer.key.clone(),
            };
            let error = update(&deployment, &alice, &request)
                .await
                .expect_err("not a PNG");
            assert!(matches!(error, AvatarError::NotTheImageType));
            assert_eq!(storage.size(&answer.key).await.expect("a head"), None);
            let alice = users::get(&deployment, alice.id).await.expect("alice");
            assert_eq!(alice.avatar_url, None);
        })
        .await;
    }

    #[tokio::test]
    async fn avatars_update_refuses_an_upload_older_than_an_hour() {
        with_each_database(|database| async move {
            let (deployment, storage, alice) = server_with_alice(database).await;
            let two_hours_ago = (crate::deployment::now() - chrono::Duration::hours(2)).timestamp();
            let old_id = uuid::Uuid::new_v7(uuid::Timestamp::from_unix(
                uuid::NoContext,
                u64::try_from(two_hours_ago).expect("after 1970"),
                0,
            ));
            let key = format!("avatars/{}/{old_id}.png", alice.id);
            storage.put(&key, PNG.to_vec()).await.expect("put");
            let error = update(&deployment, &alice, &AvatarUpdate { key })
                .await
                .expect_err("too old");
            assert!(matches!(error, AvatarError::UploadExpired));
        })
        .await;
    }

    /// Alice's upload, confirmed as her picture or not.
    async fn uploaded(
        deployment: &Deployment,
        storage: &Storage,
        alice: &User,
        confirmed: bool,
    ) -> String {
        let answer = create_upload(deployment, alice, &upload("image/png", PNG.len() as u64))
            .await
            .expect("an upload");
        storage.put(&answer.key, PNG.to_vec()).await.expect("put");
        if confirmed {
            let request = AvatarUpdate {
                key: answer.key.clone(),
            };
            update(deployment, alice, &request)
                .await
                .expect("her picture");
        }
        answer.key
    }

    #[tokio::test]
    async fn avatars_delete_unreferenced_deletes_old_files_nobody_points_at() {
        with_each_database(|database| async move {
            let (deployment, storage, alice) = server_with_alice(database).await;
            let never_confirmed = uploaded(&deployment, &storage, &alice, false).await;
            let replaced = uploaded(&deployment, &storage, &alice, true).await;
            let current = uploaded(&deployment, &storage, &alice, true).await;
            storage
                .put("elsewhere/a.txt", b"not a picture".to_vec())
                .await
                .expect("put");

            let now = crate::deployment::now();
            let report = delete_unreferenced(&deployment, now)
                .await
                .expect("a sweep");
            assert_eq!(
                report,
                DeleteUnreferencedReport {
                    kept: 3,
                    deleted: 0
                },
                "nothing a day old yet"
            );

            let tomorrow =
                now + chrono::Duration::days(UNREFERENCED_DAYS) + chrono::Duration::seconds(1);
            let report = delete_unreferenced(&deployment, tomorrow)
                .await
                .expect("a sweep");
            assert_eq!(
                report,
                DeleteUnreferencedReport {
                    kept: 1,
                    deleted: 2
                }
            );
            assert_eq!(storage.size(&never_confirmed).await.expect("a head"), None);
            assert_eq!(storage.size(&replaced).await.expect("a head"), None);
            assert!(storage.size(&current).await.expect("a head").is_some());
            assert!(
                storage
                    .size("elsewhere/a.txt")
                    .await
                    .expect("a head")
                    .is_some(),
                "only pictures are swept"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn avatars_delete_unreferenced_keeps_a_deleted_users_picture_until_the_purge() {
        with_each_database(|database| async move {
            let (deployment, storage, alice) = server_with_alice(database).await;
            let key = uploaded(&deployment, &storage, &alice, true).await;
            let now = crate::deployment::now();

            let deleted = User {
                deleted_at: Some(now),
                ..users::get(&deployment, alice.id).await.expect("alice")
            };
            let mut tx = deployment.begin().await.expect("a transaction");
            users::upsert_in(&mut tx, &deleted)
                .await
                .expect("soft deleted");
            tx.commit().await.expect("committed");
            let report = delete_unreferenced(&deployment, now + chrono::Duration::days(2))
                .await
                .expect("a sweep");
            assert_eq!(report.deleted, 0, "a soft-deleted row still points at it");

            let after_the_window = now + chrono::Duration::days(crate::purge::RETENTION_DAYS + 1);
            crate::purge::run(&deployment, after_the_window)
                .await
                .expect("the purge");
            let report = delete_unreferenced(&deployment, after_the_window)
                .await
                .expect("a sweep");
            assert_eq!(
                report.deleted, 1,
                "the purge took the row, so its file goes"
            );
            assert_eq!(storage.size(&key).await.expect("a head"), None);
        })
        .await;
    }

    #[test]
    fn avatars_key_of_url_ignores_where_the_bucket_is_served_from() {
        let key =
            "avatars/0192f7a1-0000-7000-8000-000000000000/0192f7a1-0000-7000-8000-000000000001.png";
        for base in [
            "http://starter-pack.web.localhost:6002",
            "https://cdn.example.com/files",
        ] {
            assert_eq!(key_of_url(&format!("{base}/{key}")).as_deref(), Some(key));
        }
        assert_eq!(key_of_url("https://example.com/other.png"), None);
    }

    #[tokio::test]
    async fn avatars_update_is_the_servers_alone() {
        let alice = users::tests::alice();
        let device = users::tests::device_of(&alice, "a token").await;
        let error = update(
            &device,
            &alice,
            &AvatarUpdate {
                key: format!("avatars/{}/x.png", alice.id),
            },
        )
        .await
        .expect_err("a device");
        assert!(matches!(error, AvatarError::ServerOnly));
    }
}
