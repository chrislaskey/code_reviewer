//! The posts commands: `app_core::app::posts` as the pages call it with `invoke`,
//! for the user signed in on this device. A command is named resource
//! first, `posts_list`, like the page's `postsList`, since the commands
//! are one namespace across resources, and that is its name on the wire.
//! A command's error is a `CommandError`, its message the one the page
//! shows, see `command_error`. A write starts a
//! sync in the background once it is on the device, see `sync`.

use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::app::posts::{self, PostWithOwnerAndCommunities};
use app_core::data::app_posts::Post;

use crate::command_error::CommandError;
use crate::device_accounts::DeviceAccounts;
use crate::sync;
use crate::user_auth::signed_in;

#[tauri::command]
pub async fn posts_list(
    device: State<'_, DeviceAccounts>,
) -> Result<Vec<PostWithOwnerAndCommunities>, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    Ok(posts::list(&deployment, &user).await?)
}

#[tauri::command]
pub async fn posts_get(
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<PostWithOwnerAndCommunities, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    Ok(posts::get(&deployment, &user, id).await?)
}

#[tauri::command]
pub async fn posts_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    body: String,
    community_ids: Vec<Uuid>,
) -> Result<Post, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    let post = posts::create(&deployment, &user, &body, &community_ids).await?;
    sync::spawn(app);
    Ok(post)
}

#[tauri::command]
pub async fn posts_update(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
    body: String,
    community_ids: Vec<Uuid>,
) -> Result<Post, CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    let post = posts::update(&deployment, &user, id, &body, &community_ids).await?;
    sync::spawn(app);
    Ok(post)
}

#[tauri::command]
pub async fn posts_delete(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<(), CommandError> {
    let (deployment, user) = signed_in(&device).await?;
    posts::delete(&deployment, &user, id).await?;
    sync::spawn(app);
    Ok(())
}
