//! The current community: the one community a person works in, chosen
//! per device, or per browser on the web. Only the choice is kept here,
//! as an id; it holds while the person is a live member of it, and
//! otherwise, or when nothing was chosen, it is their first community by
//! name, or none when they are in none. A device keeps the choice as a
//! device setting, in the signed-in account's own file; the web server
//! keeps it in a cookie, so it passes the stored id in and stores what
//! comes back.

use crate::data::app_users_communities;
use thiserror::Error;
use uuid::Uuid;

use crate::app::communities::{self, Community, CommunityError};
use crate::app::users::User;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role};
use crate::device_settings::{self, CURRENT_COMMUNITY, DeviceSettingsError};

#[derive(Debug, Error)]
pub enum CurrentCommunityError {
    #[error("only a member can make a community their current one")]
    NotAMember,
    #[error("the current community is kept by a device, not the server")]
    DeviceOnly,
    #[error(transparent)]
    Community(#[from] CommunityError),
    #[error(transparent)]
    DeviceSettings(#[from] DeviceSettingsError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The current community for the stored choice: the chosen one while
/// the user is a live member of it, else their first by name.
pub async fn get(
    deployment: &Deployment,
    user: &User,
    chosen_id: Option<Uuid>,
) -> Result<Option<Community>, CurrentCommunityError> {
    let mut tx = deployment.begin().await?;
    let community = resolve(&mut tx, user, chosen_id).await?;
    tx.commit().await?;
    Ok(community)
}

/// The community, once checked the user is a live member of it, for the
/// caller to store as the choice.
pub async fn update(
    deployment: &Deployment,
    user: &User,
    community_id: Uuid,
) -> Result<Community, CurrentCommunityError> {
    let mut tx = deployment.begin().await?;
    let community = member_community(&mut tx, user, community_id).await?;
    tx.commit().await?;
    community.ok_or(CurrentCommunityError::NotAMember)
}

/// The current community on this device, from its setting, which is
/// stored again when it no longer holds, so the choice stays put.
pub async fn get_on_device(
    deployment: &Deployment,
    user: &User,
) -> Result<Option<Community>, CurrentCommunityError> {
    device_only(deployment)?;
    let mut tx = deployment.begin().await?;
    let chosen_id: Option<Uuid> = device_settings::get(&mut tx, CURRENT_COMMUNITY).await?;
    let community = resolve(&mut tx, user, chosen_id).await?;
    let resolved_id = community.as_ref().map(|community| community.id);
    if resolved_id != chosen_id {
        device_settings::set(&mut tx, CURRENT_COMMUNITY, &resolved_id).await?;
    }
    tx.commit().await?;
    Ok(community)
}

/// Makes the community the current one on this device.
pub async fn update_on_device(
    deployment: &Deployment,
    user: &User,
    community_id: Uuid,
) -> Result<Community, CurrentCommunityError> {
    device_only(deployment)?;
    let mut tx = deployment.begin().await?;
    let community = member_community(&mut tx, user, community_id)
        .await?
        .ok_or(CurrentCommunityError::NotAMember)?;
    device_settings::set(&mut tx, CURRENT_COMMUNITY, &Some(community.id)).await?;
    tx.commit().await?;
    Ok(community)
}

fn device_only(deployment: &Deployment) -> Result<(), CurrentCommunityError> {
    match deployment.role {
        Role::Device => Ok(()),
        Role::Server => Err(CurrentCommunityError::DeviceOnly),
    }
}

async fn resolve(
    tx: &mut Transaction,
    user: &User,
    chosen_id: Option<Uuid>,
) -> Result<Option<Community>, CurrentCommunityError> {
    let mine = list_mine(tx, user).await?;
    let chosen = chosen_id.and_then(|id| mine.iter().find(|community| community.id == id));
    Ok(chosen.or(mine.first()).cloned())
}

async fn member_community(
    tx: &mut Transaction,
    user: &User,
    community_id: Uuid,
) -> Result<Option<Community>, CurrentCommunityError> {
    Ok(list_mine(tx, user)
        .await?
        .into_iter()
        .find(|community| community.id == community_id))
}

/// The live communities the user is a live member of, by name.
async fn list_mine(
    tx: &mut Transaction,
    user: &User,
) -> Result<Vec<Community>, CurrentCommunityError> {
    let memberships =
        app_users_communities::list(&mut *tx, app_users_communities::Filter::User(user.id)).await?;
    let ids: Vec<Uuid> = memberships
        .iter()
        .map(|membership| membership.community_id)
        .collect();
    let mut mine: Vec<Community> = communities::list_by_ids(tx, &ids)
        .await?
        .into_iter()
        .filter(|community| community.deleted_at.is_none())
        .collect();
    mine.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Ok(mine)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{memberships, users};
    use crate::database::{Database, with_each_database};

    #[tokio::test]
    async fn current_community_get_is_the_choice_while_a_member_else_the_first_by_name() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let alice = users::create(&deployment, "alice@example.com", "Alice")
                .await
                .expect("a user");
            assert_eq!(get(&deployment, &alice, None).await.expect("none"), None);

            let weeding = communities::create(&deployment, &alice, "Weeding")
                .await
                .expect("a community");
            let baking = communities::create(&deployment, &alice, "Baking")
                .await
                .expect("a community");
            assert_eq!(
                get(&deployment, &alice, None).await.expect("the first"),
                Some(baking.clone())
            );
            assert_eq!(
                get(&deployment, &alice, Some(weeding.id))
                    .await
                    .expect("the choice"),
                Some(weeding.clone())
            );
            assert_eq!(
                get(&deployment, &alice, Some(Uuid::now_v7()))
                    .await
                    .expect("unknown, so the first"),
                Some(baking.clone())
            );

            memberships::leave(&deployment, &alice, weeding.id)
                .await
                .expect("left");
            assert_eq!(
                get(&deployment, &alice, Some(weeding.id))
                    .await
                    .expect("left, so the first"),
                Some(baking.clone())
            );
            let error = update(&deployment, &alice, weeding.id)
                .await
                .expect_err("not a member");
            assert!(matches!(error, CurrentCommunityError::NotAMember));
            assert_eq!(
                update(&deployment, &alice, baking.id)
                    .await
                    .expect("a member"),
                baking
            );
        })
        .await;
    }

    #[tokio::test]
    async fn current_community_on_a_device_is_stored_and_kept() {
        let server = Deployment::new(
            Database::open_sqlite_in_memory().await.expect("a database"),
            Role::Server,
        );
        let alice = users::create(&server, "alice@example.com", "Alice")
            .await
            .expect("a user");
        let device = users::tests::device_of(&alice, "a token").await;
        assert_eq!(get_on_device(&device, &alice).await.expect("none"), None);

        let weeding = communities::create(&device, &alice, "Weeding")
            .await
            .expect("a community");
        assert_eq!(
            get_on_device(&device, &alice).await.expect("the only one"),
            Some(weeding.clone())
        );
        let baking = communities::create(&device, &alice, "Baking")
            .await
            .expect("a community");
        assert_eq!(
            get_on_device(&device, &alice)
                .await
                .expect("stored, so kept"),
            Some(weeding.clone()),
            "a new community does not take over"
        );

        update_on_device(&device, &alice, baking.id)
            .await
            .expect("chosen");
        assert_eq!(
            get_on_device(&device, &alice).await.expect("the choice"),
            Some(baking.clone())
        );
        communities::delete(&device, &alice, baking.id)
            .await
            .expect("deleted");
        assert_eq!(
            get_on_device(&device, &alice).await.expect("the one left"),
            Some(weeding)
        );

        let error = get_on_device(&server, &alice)
            .await
            .expect_err("the server keeps no choice");
        assert!(matches!(error, CurrentCommunityError::DeviceOnly));
    }
}
