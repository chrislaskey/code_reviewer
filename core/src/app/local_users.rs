//! Local users: a person using the app on a desktop or phone without
//! signing up, see `docs/design/accounts-local-and-trial-users.md`.
//! `create` makes one in a device's database, with a community of their
//! own, and signs them in with no token. From then on they are a user
//! like any other to every core function, with two differences, both
//! decided here: their writes queue no event, `events::queue` asks
//! `is_signed_in_in`, and a sync does nothing for them, since there is
//! no account on the server to sync with.
//!
//! When they sign up or sign in, `move_to_account` gives everything
//! they made to that account, in one transaction: every row becomes
//! the account's and is queued as a create, as if the account had made
//! it all on this device while offline, and the standard sync pushes
//! it. Nothing a local user made exists anywhere else, so none of it
//! conflicts with the server's rows. On a server with a single
//! community, the local user's communities are merged into the
//! account's community before anything is pushed, so the account still
//! has one community.

use crate::data::app_communities::{self, Community};
use crate::data::app_users_communities::{self, Membership};
use crate::data::local_users;
use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::accounts::Session;
use crate::app::communities::{self, CommunityAndMembership, CommunityError};
use crate::app::memberships::{self, MembershipError};
use crate::app::users::{self, SignedIn, UserError};
use crate::data::app_events::Event;
use crate::data::app_users::{self, User};
use crate::data::device_settings::{
    self, CURRENT_COMMUNITY, DeviceSettingsError, LOCAL_USER_MOVED, SIGNED_IN,
};
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::helpers::random;
use crate::sync::events;
use crate::sync::synced_table::TableError;
use crate::sync::tables;

/// Whether a device offers to skip signing in. The app's choice, to
/// change in a fork.
pub const ALLOWED: bool = true;

/// The community a local user starts with when the device has no name.
pub const COMMUNITY_NAME_WITHOUT_A_DEVICE_NAME: &str = "This device";

const NAME_PREFIX: &str = "local_";
const EMAIL_DOMAIN: &str = "example.com";

/// The account's community that a local user's communities are merged
/// into when they move to the account on a server with a single
/// community, as the server has it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeTargetCommunity {
    pub community: Community,
    pub membership: Membership,
}

impl MergeTargetCommunity {
    /// The community the account has been a member of the longest, so
    /// every device of the account picks the same one; none when the
    /// account is in no community.
    pub fn oldest_membership(listed: Vec<CommunityAndMembership>) -> Option<MergeTargetCommunity> {
        listed
            .into_iter()
            .filter_map(
                |CommunityAndMembership {
                     community,
                     membership,
                 }| {
                    let membership =
                        membership.filter(|membership| membership.deleted_at.is_none())?;
                    Some(MergeTargetCommunity {
                        community,
                        membership,
                    })
                },
            )
            .min_by_key(|joined| (joined.membership.inserted_at, joined.membership.id))
    }
}

/// The account a local user's records moved to, kept in the device's
/// database until the app has updated its list of accounts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moved {
    pub from_user_id: Uuid,
    pub user_id: Uuid,
    pub email: String,
}

#[derive(Debug, Error)]
pub enum LocalUserError {
    #[error("only a device has local users")]
    DeviceOnly,
    #[error("This device is set up without local users")]
    NotAllowed,
    #[error("The user on this device is not a local user")]
    NotLocal,
    #[error(transparent)]
    User(#[from] UserError),
    #[error(transparent)]
    Community(#[from] CommunityError),
    #[error(transparent)]
    Membership(#[from] MembershipError),
    #[error(transparent)]
    DeviceSettings(#[from] DeviceSettingsError),
    /// A synced table's own module failed.
    #[error(transparent)]
    Table(#[from] TableError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// A name for a new local user, `local_85a3ycZae3b`.
pub fn new_name() -> String {
    format!("{NAME_PREFIX}{}", random::letters_and_digits(11))
}

/// The email a device lists a local user by. Nothing is ever sent to
/// it: `example.com` is reserved for this.
pub fn email_of(name: &str) -> String {
    format!("{name}@{EMAIL_DOMAIN}")
}

/// Makes the local user in a device's database, with their own
/// community, named for the device, and signs them in.
pub async fn create(
    deployment: &Deployment,
    id: Uuid,
    name: &str,
    device_name: &str,
) -> Result<User, LocalUserError> {
    if deployment.role != Role::Device {
        return Err(LocalUserError::DeviceOnly);
    }
    if !ALLOWED {
        return Err(LocalUserError::NotAllowed);
    }
    let now = now();
    let user = User {
        id,
        name: app_users::checked_name(name).map_err(UserError::from)?,
        inserted_at: now,
        updated_at: now,
        version: 0,
        last_synced_at: None,
        global_sync_version: None,
        deleted_at: None,
        deleted_by_id: None,
        avatar_url: None,
    };
    let community_name: String = match device_name.trim() {
        "" => COMMUNITY_NAME_WITHOUT_A_DEVICE_NAME.to_string(),
        device_name => device_name
            .chars()
            .take(app_communities::MAX_NAME_LENGTH)
            .collect(),
    };
    let mut tx = deployment.begin().await?;
    users::upsert_in(&mut tx, &user).await?;
    let signed_in = SignedIn {
        user_id: user.id,
        token: None,
    };
    device_settings::set(&mut tx, SIGNED_IN, &Some(signed_in)).await?;
    communities::create_in(&mut tx, Role::Device, &user, &community_name).await?;
    tx.commit().await?;
    Ok(user)
}

/// Whether the user signed in on this device is a local user.
pub async fn is_signed_in(deployment: &Deployment) -> Result<bool, LocalUserError> {
    let mut tx = deployment.begin().await?;
    let local = is_signed_in_in(&mut tx).await?;
    tx.commit().await?;
    Ok(local)
}

/// `is_signed_in` in the caller's transaction: someone is signed in and
/// has no token. On a server, and with nobody signed in, false.
pub(crate) async fn is_signed_in_in(tx: &mut Transaction) -> Result<bool, sqlx::Error> {
    let signed_in = device_settings::get_stored(&mut *tx, SIGNED_IN).await?;
    Ok(signed_in.is_some_and(|signed_in| {
        signed_in.is_object() && signed_in.get("token").is_none_or(Value::is_null)
    }))
}

/// How many things the local user made, in the tables that count them,
/// for the question asked before a move.
pub async fn count_items(deployment: &Deployment) -> Result<i64, LocalUserError> {
    let mut tx = deployment.begin().await?;
    let local = local_user_in(&mut tx).await?;
    let counted: Vec<_> = owner_columns()
        .into_iter()
        .filter(|(table, _)| {
            tables::get(table).is_some_and(|synced| synced.counted_before_a_move())
        })
        .collect();
    let count = local_users::count_items(&mut tx, &counted, local).await?;
    tx.commit().await?;
    Ok(count)
}

/// Gives everything the local user made to the account of the session,
/// which is signed in in their place: the account's row is written,
/// every row of the local user becomes the account's, the local user
/// and every deleted row go, and each row left is queued as a create,
/// made when the row was, for the next sync to push. The account's
/// name and picture stay the account's. With a merge target, the
/// local user's communities are merged into it first, see
/// `merge_local_communities_into`.
pub async fn move_to_account(
    deployment: &Deployment,
    session: Session,
    merge_target: Option<MergeTargetCommunity>,
) -> Result<User, LocalUserError> {
    if deployment.role != Role::Device {
        return Err(LocalUserError::DeviceOnly);
    }
    let account = session.user;
    let mut tx = deployment.begin().await?;
    let local = local_user_in(&mut tx).await?;
    users::upsert_in(&mut tx, &account).await?;
    local_users::maintenance::give_rows_to_user(&mut tx, &owner_columns(), local, account.id)
        .await?;
    let children_first: Vec<&str> = tables::list()
        .iter()
        .rev()
        .map(|synced| synced.table())
        .filter(|table| *table != app_users::TABLE)
        .collect();
    local_users::maintenance::delete_deleted_rows(&mut tx, &children_first).await?;
    local_users::maintenance::delete_user(&mut tx, local).await?;
    let on_server = match &merge_target {
        Some(merge_target) => {
            merge_local_communities_into(&mut tx, &account, merge_target).await?;
            vec![merge_target.community.id, merge_target.membership.id]
        }
        None => Vec::new(),
    };
    let signed_in = SignedIn {
        user_id: account.id,
        token: Some(session.token),
    };
    device_settings::set(&mut tx, SIGNED_IN, &Some(signed_in)).await?;
    let moved = Moved {
        from_user_id: local,
        user_id: account.id,
        email: session.email,
    };
    device_settings::set(&mut tx, LOCAL_USER_MOVED, &Some(moved)).await?;
    queue_creates_in(&mut tx, &account, &on_server).await?;
    tx.commit().await?;
    Ok(account)
}

/// The move the app has not yet updated its list of accounts for.
pub async fn moved(deployment: &Deployment) -> Result<Option<Moved>, LocalUserError> {
    let mut tx = deployment.begin().await?;
    let moved = device_settings::get(&mut tx, LOCAL_USER_MOVED).await?;
    tx.commit().await?;
    Ok(moved)
}

/// Forgets the move, once the app's list of accounts names the account.
pub async fn moved_forget(deployment: &Deployment) -> Result<(), LocalUserError> {
    let mut tx = deployment.begin().await?;
    device_settings::set(&mut tx, LOCAL_USER_MOVED, &None::<Moved>).await?;
    tx.commit().await?;
    Ok(())
}

/// Writes the account's community and membership as the server has
/// them, moves every row in the local user's communities to it, deletes
/// those communities, which were never synced to the server, and makes
/// it the current community.
async fn merge_local_communities_into(
    tx: &mut Transaction,
    account: &User,
    merge_target: &MergeTargetCommunity,
) -> Result<(), LocalUserError> {
    let MergeTargetCommunity {
        community,
        membership,
    } = merge_target;
    communities::apply_server_row(
        tx,
        &serde_json::to_value(community).map_err(CommunityError::from)?,
    )
    .await?;
    memberships::apply_server_row(
        tx,
        &serde_json::to_value(membership).map_err(MembershipError::from)?,
    )
    .await?;
    let local_community_ids: Vec<Uuid> =
        app_users_communities::list(&mut *tx, app_users_communities::Filter::User(account.id))
            .await?
            .into_iter()
            .map(|local| local.community_id)
            .filter(|community_id| *community_id != community.id)
            .collect();
    for from_community_id in local_community_ids {
        for synced in tables::list() {
            synced
                .move_rows_to_community(tx, from_community_id, membership)
                .await?;
        }
        communities::delete_row(tx, from_community_id).await?;
    }
    device_settings::set(tx, CURRENT_COMMUNITY, &Some(community.id)).await?;
    Ok(())
}

/// The id of the local user signed in.
async fn local_user_in(tx: &mut Transaction) -> Result<Uuid, LocalUserError> {
    let signed_in: Option<SignedIn> = device_settings::get(tx, SIGNED_IN).await?;
    match signed_in {
        Some(SignedIn {
            user_id,
            token: None,
        }) => Ok(user_id),
        _ => Err(LocalUserError::NotLocal),
    }
}

/// Each synced table whose rows name their maker, with that column.
fn owner_columns() -> Vec<(&'static str, &'static str)> {
    tables::list()
        .iter()
        .filter_map(|synced| Some((synced.table(), synced.owner_column()?)))
        .collect()
}

/// Queues a create for every row on the device, except those already on
/// the server, table by table, parents first, see `synced_tables`. The
/// queue is pushed in the order the events were made, so each is made at
/// the time its row was, and after the rows it points at.
async fn queue_creates_in(
    tx: &mut Transaction,
    user: &User,
    on_server: &[Uuid],
) -> Result<(), LocalUserError> {
    let mut made_at_of: BTreeMap<Uuid, DateTime<Utc>> = BTreeMap::new();
    let mut queue = Vec::new();
    for synced in tables::list() {
        for create in synced.local_user_creates(tx, user).await? {
            if on_server.contains(&create.id) {
                continue;
            }
            let after_parents = create
                .parents
                .iter()
                .filter_map(|parent| made_at_of.get(parent))
                .max()
                .map(|parent| *parent + Duration::microseconds(1));
            let made_at =
                after_parents.map_or(create.inserted_at, |after| after.max(create.inserted_at));
            made_at_of.insert(create.id, made_at);
            queue.push(Event::create(
                synced.table(),
                create.id,
                create.changes,
                made_at,
            ));
        }
    }
    for event in &queue {
        events::queue(tx, user.id, event).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts;
    use crate::database::with_each_database;
    use crate::services::web_server::InProcess;
    use crate::sync;
    use crate::sync::test_notes;

    fn server(database: crate::database::Database) -> Deployment {
        Deployment::new(database, Role::Server)
    }

    /// A device where someone skipped signing in.
    async fn device_with_a_local_user() -> (Deployment, User) {
        let device = users::tests::device().await;
        let name = new_name();
        let user = create(&device, Uuid::now_v7(), &name, "Alice's Laptop")
            .await
            .expect("a local user");
        (device, user)
    }

    async fn pending(device: &Deployment) -> i64 {
        sync::status(device).await.expect("the status").pending
    }

    #[tokio::test]
    async fn local_users_create_signs_in_a_user_with_a_community_who_queues_nothing_and_never_syncs()
     {
        with_each_database(|database| async move {
            let server = server(database);
            let (device, local) = device_with_a_local_user().await;
            assert!(local.name.starts_with("local_") && local.name.len() == 17);
            assert_eq!(email_of(&local.name), format!("{}@example.com", local.name));
            assert_eq!(users::signed_in(&device).await.expect("signed in"), local);
            assert!(is_signed_in(&device).await.expect("asked"));
            assert!(matches!(
                users::signed_in_with_token(&device).await,
                Err(UserError::LocalUser)
            ));

            let communities = communities::list(&device, &local).await.expect("listed");
            assert_eq!(communities.len(), 1);
            assert_eq!(communities[0].community.name, "Alice's Laptop");
            assert!(communities[0].membership.is_some());

            let note = test_notes::create(&device, &local, "written here", None)
                .await
                .expect("a note");
            test_notes::update(&device, &local, note.id, "edited here")
                .await
                .expect("edited");
            users::update(&device, &local, "Alice")
                .await
                .expect("renamed");
            assert_eq!(pending(&device).await, 0, "nothing is queued");
            assert_eq!(note.version, 0);
            assert_eq!(count_items(&device).await.expect("counted"), 1);

            let report = sync::run(&device, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert!(report.local);
            assert_eq!(report.pushed + report.pulled, 0);
            assert_eq!(
                test_notes::list(&device, &local)
                    .await
                    .expect("notes")
                    .len(),
                1,
                "and nothing was pruned"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn local_users_create_without_a_device_name_names_the_community_this_device() {
        let device = users::tests::device().await;
        let local = create(&device, Uuid::now_v7(), &new_name(), "  ")
            .await
            .expect("a local user");
        let communities = communities::list(&device, &local).await.expect("listed");
        assert_eq!(
            communities[0].community.name,
            COMMUNITY_NAME_WITHOUT_A_DEVICE_NAME
        );
    }

    /// What the test's local user made: a note in their first
    /// community, a note in none, a second community, and a
    /// note since deleted.
    struct Made {
        in_community: test_notes::Note,
        in_none: test_notes::Note,
        deleted: test_notes::Note,
    }

    async fn made_by(device: &Deployment, local: &User) -> Made {
        let first = communities::list(device, local).await.expect("listed")[0]
            .community
            .id;
        let in_community = test_notes::create(device, local, "in the laptop's", Some(first))
            .await
            .expect("a note");
        let in_none = test_notes::create(device, local, "in none", None)
            .await
            .expect("a note");
        communities::create(device, local, "Garden")
            .await
            .expect("a community");
        let deleted = test_notes::create(device, local, "thought better of", Some(first))
            .await
            .expect("a note");
        test_notes::delete(device, local, deleted.id)
            .await
            .expect("deleted");
        Made {
            in_community,
            in_none,
            deleted,
        }
    }

    async fn names_of_communities(deployment: &Deployment, user: &User) -> Vec<String> {
        communities::list(deployment, user)
            .await
            .expect("listed")
            .into_iter()
            .filter(|listed| listed.membership.is_some())
            .map(|listed| listed.community.name)
            .collect()
    }

    async fn notes_of(deployment: &Deployment, user: &User) -> Vec<test_notes::Note> {
        let mut notes = test_notes::list(deployment, user).await.expect("the notes");
        notes.sort_by_key(|note| note.id);
        notes
    }

    #[tokio::test]
    async fn local_users_move_to_a_new_account_pushes_every_live_row_with_its_time() {
        with_each_database(|database| async move {
            let server = server(database);
            let (device, local) = device_with_a_local_user().await;
            let made = made_by(&device, &local).await;
            let session =
                accounts::tests::signed_up_for_a_device(&server, "alice@example.com").await;

            let alice = move_to_account(&device, session.clone(), None)
                .await
                .expect("moved");
            assert_eq!(alice, session.user, "the account's name, not the local one");
            assert_eq!(users::signed_in(&device).await.expect("signed in"), alice);
            assert!(!is_signed_in(&device).await.expect("asked"));
            assert!(matches!(
                users::get(&device, local.id).await,
                Err(UserError::NotFound { .. })
            ));
            assert_eq!(
                moved(&device).await.expect("asked"),
                Some(Moved {
                    from_user_id: local.id,
                    user_id: alice.id,
                    email: "alice@example.com".to_string(),
                })
            );
            assert_eq!(
                pending(&device).await,
                6,
                "two communities, two memberships and two notes"
            );

            let report = sync::run(&device, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(report.push_error, None);
            assert_eq!(
                (report.pushed, report.rejected, report.conflicts),
                (6, 0, 0)
            );
            assert_eq!(report.pending, 0);

            assert_eq!(
                names_of_communities(&server, &alice).await,
                ["Alice's Laptop", "Garden", "Personal"],
                "what she made, beside her sign-up's"
            );
            let on_server = notes_of(&server, &alice).await;
            assert_eq!(on_server.len(), 2, "the deleted note never left the device");
            assert!(on_server.iter().all(|note| note.id != made.deleted.id));
            for before in [&made.in_community, &made.in_none] {
                let after = on_server
                    .iter()
                    .find(|note| note.id == before.id)
                    .expect("on the server");
                assert_eq!(after.user_id, alice.id);
                assert_eq!(
                    after.inserted_at, before.inserted_at,
                    "made when it was made"
                );
                assert_eq!(after.version, 1);
            }
            let in_community = test_notes::get(&server, &alice, made.in_community.id)
                .await
                .expect("the note");
            assert!(in_community.community_id.is_some());
            assert_eq!(in_community.community_id, made.in_community.community_id);

            assert_eq!(notes_of(&device, &alice).await, on_server);
            assert_eq!(
                communities::list(&device, &alice).await.expect("listed"),
                communities::list(&server, &alice).await.expect("listed"),
                "the device matches the server"
            );
            moved_forget(&device).await.expect("forgotten");
            assert_eq!(moved(&device).await.expect("asked"), None);
        })
        .await;
    }

    #[tokio::test]
    async fn local_users_move_to_an_account_that_has_data_keeps_both() {
        with_each_database(|database| async move {
            let server = server(database);
            let session =
                accounts::tests::signed_up_for_a_device(&server, "alice@example.com").await;
            let alice = session.user.clone();
            test_notes::create(&server, &alice, "written on the web", None)
                .await
                .expect("a note");
            communities::create(&server, &alice, "Baking")
                .await
                .expect("a community");

            let (device, local) = device_with_a_local_user().await;
            made_by(&device, &local).await;
            move_to_account(&device, session, None)
                .await
                .expect("moved");
            let report = sync::run(&device, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(
                (report.pushed, report.rejected, report.conflicts),
                (6, 0, 0)
            );

            assert_eq!(
                names_of_communities(&server, &alice).await,
                ["Alice's Laptop", "Baking", "Garden", "Personal"]
            );
            let on_server = notes_of(&server, &alice).await;
            assert_eq!(on_server.len(), 3);
            assert_eq!(notes_of(&device, &alice).await, on_server);
            assert_eq!(
                names_of_communities(&device, &alice).await,
                names_of_communities(&server, &alice).await
            );
        })
        .await;
    }

    #[tokio::test]
    async fn local_users_move_to_a_single_community_account_merges_every_community_into_its_own() {
        with_each_database(|database| async move {
            let server = server(database);
            let session =
                accounts::tests::signed_up_for_a_device(&server, "alice@example.com").await;
            let alice = session.user.clone();
            let merge_target = MergeTargetCommunity::oldest_membership(
                communities::list(&server, &alice).await.expect("listed"),
            )
            .expect("her sign-up's community");
            assert_eq!(merge_target.community.name, "Personal");

            let (device, local) = device_with_a_local_user().await;
            let made = made_by(&device, &local).await;
            let garden = communities::list(&device, &local)
                .await
                .expect("listed")
                .into_iter()
                .find(|listed| listed.community.name == "Garden")
                .expect("the second community")
                .community
                .id;
            let in_garden = test_notes::create(&device, &local, "in the garden", Some(garden))
                .await
                .expect("a note");
            move_to_account(&device, session, Some(merge_target.clone()))
                .await
                .expect("moved");
            assert_eq!(names_of_communities(&device, &alice).await, ["Personal"]);
            assert_eq!(
                crate::app::current_community::get_on_device(&device, &alice)
                    .await
                    .expect("asked"),
                Some(merge_target.community.clone())
            );

            let report = sync::run(&device, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(report.push_error, None);
            assert_eq!((report.rejected, report.conflicts), (0, 0));
            assert_eq!(report.pending, 0);

            assert_eq!(names_of_communities(&server, &alice).await, ["Personal"]);
            for note in [&made.in_community, &in_garden] {
                let shown = test_notes::get(&server, &alice, note.id)
                    .await
                    .expect("the note");
                assert_eq!(
                    shown.community_id,
                    Some(merge_target.community.id),
                    "{}",
                    note.body
                );
            }
            assert_eq!(
                notes_of(&device, &alice).await,
                notes_of(&server, &alice).await
            );
            assert_eq!(
                communities::list(&device, &alice).await.expect("listed"),
                communities::list(&server, &alice).await.expect("listed"),
                "the device matches the server"
            );
        })
        .await;
    }

    #[test]
    fn local_users_merge_target_community_is_the_oldest_live_membership() {
        let at = |seconds| DateTime::from_timestamp(seconds, 0).expect("a time");
        let listed = |name: &str, joined: Option<(i64, bool)>| {
            let community = Community {
                id: Uuid::now_v7(),
                name: name.to_string(),
                inserted_at: at(0),
                updated_at: at(0),
                version: 1,
                last_synced_at: None,
                global_sync_version: None,
                deleted_at: None,
                deleted_by_id: None,
            };
            let membership = joined.map(|(seconds, left)| Membership {
                id: Uuid::now_v7(),
                user_id: Uuid::nil(),
                community_id: community.id,
                role: "member".to_string(),
                inserted_at: at(seconds),
                updated_at: at(seconds),
                version: 1,
                last_synced_at: None,
                global_sync_version: None,
                deleted_at: left.then(|| at(seconds + 1)),
                deleted_by_id: None,
            });
            CommunityAndMembership {
                community,
                membership,
            }
        };
        let first = MergeTargetCommunity::oldest_membership(vec![
            listed("Baking", Some((20, false))),
            listed("Left", Some((5, true))),
            listed("Not mine", None),
            listed("Personal", Some((10, false))),
        ]);
        assert_eq!(
            first.map(|joined| joined.community.name),
            Some("Personal".to_string())
        );
        assert_eq!(
            MergeTargetCommunity::oldest_membership(vec![listed("Not mine", None)]),
            None
        );
    }

    #[tokio::test]
    async fn local_users_move_to_account_is_only_for_a_local_user() {
        with_each_database(|database| async move {
            let server = server(database);
            let session =
                accounts::tests::signed_up_for_a_device(&server, "alice@example.com").await;
            let device = users::tests::device_of(&session.user, &session.token).await;
            test_notes::create(&device, &session.user, "hers already", None)
                .await
                .expect("a note");

            let error = move_to_account(&device, session, None)
                .await
                .expect_err("she has an account");
            assert!(matches!(error, LocalUserError::NotLocal));
            assert_eq!(pending(&device).await, 1, "and nothing changed");
            assert!(matches!(
                count_items(&device).await,
                Err(LocalUserError::NotLocal)
            ));
        })
        .await;
    }
}
