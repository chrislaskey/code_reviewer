//! Where to PUT the picture the person picked: a URL the server signs
//! for one upload, under the user's own prefix, see
//! `docs/design/uploads.md`.

#[cfg(feature = "object-storage")]
use crate::app::avatars;
use crate::app::avatars::{AvatarError, AvatarUpload, AvatarUploadNew};
#[cfg(feature = "sync-http")]
use crate::app::users;
#[cfg(feature = "object-storage")]
use crate::data::app_users::User;
use crate::deployment::Deployment;
use crate::helpers::field_errors::FieldErrors;
#[cfg(feature = "sync-http")]
use crate::services::web_server::Http;

/// How long a signed upload URL works.
#[cfg(feature = "object-storage")]
const UPLOAD_URL_LIFETIME: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Answers where to upload the file the person picked, once it is a
/// picture this app takes: on the server, which holds the bucket's keys.
#[cfg(feature = "object-storage")]
pub async fn create(
    deployment: &Deployment,
    user: &User,
    request: &AvatarUploadNew,
) -> Result<AvatarUpload, AvatarError> {
    FieldErrors::check(request).map_err(AvatarError::Invalid)?;
    if crate::accounts::trial_users::is_trial(deployment, user).await? {
        return Err(AvatarError::TrialUser);
    }
    let storage = avatars::bucket(deployment)?;
    let image_type = avatars::image_type_of(&request.content_type).ok_or_else(|| {
        AvatarError::Invalid(FieldErrors::one("content_type", "is not a picture"))
    })?;
    let key = format!(
        "{}/{}/{}.{}",
        avatars::KEY_PREFIX,
        user.id,
        uuid::Uuid::now_v7(),
        image_type.extension
    );
    let upload_url = storage
        .upload_url(&key, image_type.content_type, None, UPLOAD_URL_LIFETIME)
        .await?;
    Ok(AvatarUpload {
        upload_url,
        content_type: image_type.content_type.to_string(),
        key,
    })
}

/// `create` asked of the server by the user signed in on this
/// device, after the same checks here, so a wrong file is told at once.
#[cfg(feature = "sync-http")]
pub async fn create_on_device(
    deployment: &Deployment,
    server: &Http,
    request: &AvatarUploadNew,
) -> Result<AvatarUpload, AvatarError> {
    FieldErrors::check(request).map_err(AvatarError::Invalid)?;
    let (_user, token) = users::signed_in_with_token(deployment).await?;
    Ok(server.avatars_uploads_create(&token, request).await?)
}
