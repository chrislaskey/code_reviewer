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
//! it. Nothing a local user made exists anywhere else, so there is
//! nothing to merge.

use crate::data::app_communities;
use crate::data::local_users;
use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::accounts::Session;
use crate::app::communities::{self, CommunityError};
use crate::app::events;
use crate::app::synced_table::TableError;
use crate::app::synced_tables;
use crate::app::users::{self, SignedIn, UserError};
use crate::data::app_events::Event;
use crate::data::app_users::{self, User};
use crate::data::device_settings::{self, DeviceSettingsError, LOCAL_USER_MOVED, SIGNED_IN};
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::helpers::random;

/// Whether a device offers to skip signing in. The app's choice, to
/// change in a fork.
pub const ALLOWED: bool = true;

/// The community a local user starts with when the device has no name.
pub const COMMUNITY_NAME_WITHOUT_A_DEVICE_NAME: &str = "This device";

const NAME_PREFIX: &str = "local_";
const EMAIL_DOMAIN: &str = "example.com";

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

/// How many things the local user made, posts and files say, in the
/// tables that count them, for the question asked before a move.
pub async fn count_items(deployment: &Deployment) -> Result<i64, LocalUserError> {
    let mut tx = deployment.begin().await?;
    let local = local_user_in(&mut tx).await?;
    let counted: Vec<_> = owner_columns()
        .into_iter()
        .filter(|(table, _)| {
            synced_tables::get(table).is_some_and(|synced| synced.counted_before_a_move())
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
/// name and picture stay the account's.
pub async fn move_to_account(
    deployment: &Deployment,
    session: Session,
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
    let children_first: Vec<&str> = synced_tables::list()
        .iter()
        .rev()
        .map(|synced| synced.table())
        .filter(|table| *table != app_users::TABLE)
        .collect();
    local_users::maintenance::delete_deleted_rows(&mut tx, &children_first).await?;
    local_users::maintenance::delete_user(&mut tx, local).await?;
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
    queue_creates_in(&mut tx, &account).await?;
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
    synced_tables::list()
        .iter()
        .filter_map(|synced| Some((synced.table(), synced.owner_column()?)))
        .collect()
}

/// Queues a create for every row the device holds, table by table,
/// parents first, see `synced_tables`. The queue is pushed in the order
/// the events were made, so each is made at the time its row was, and
/// after the rows it points at.
async fn queue_creates_in(tx: &mut Transaction, user: &User) -> Result<(), LocalUserError> {
    let mut made_at_of: BTreeMap<Uuid, DateTime<Utc>> = BTreeMap::new();
    let mut queue = Vec::new();
    for synced in synced_tables::list() {
        for create in synced.local_user_creates(tx, user).await? {
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

#[cfg(all(test, feature = "object-storage"))]
mod tests {
    use super::*;
    use crate::accounts;
    use crate::app::files;
    use crate::app::files::tests::server;
    use crate::app::posts;
    use crate::app::sync;
    use crate::data::{app_files, app_posts};
    use crate::database::with_each_database;
    use crate::deployment::FileStore;
    use crate::services::storage::Storage;
    use crate::services::web_server::InProcess;

    /// A device where someone skipped signing in, with a folder for
    /// its files' bytes.
    async fn device_with_a_local_user() -> (Deployment, User, tempfile::TempDir) {
        let folder = tempfile::tempdir().expect("a folder");
        let device = users::tests::device()
            .await
            .with_file_store(FileStore::Folder(folder.path().to_path_buf()));
        let name = new_name();
        let user = create(&device, Uuid::now_v7(), &name, "Alice's Laptop")
            .await
            .expect("a local user");
        (device, user, folder)
    }

    async fn pending(device: &Deployment) -> i64 {
        sync::status(device).await.expect("the status").pending
    }

    #[tokio::test]
    async fn local_users_create_signs_in_a_user_with_a_community_who_queues_nothing_and_never_syncs()
     {
        with_each_database(|database| async move {
            let (server, _storage) = server(database);
            let (device, local, _folder) = device_with_a_local_user().await;
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

            let post = posts::create(&device, &local, "written here", &[])
                .await
                .expect("a post");
            posts::update(&device, &local, post.id, "edited here", &[])
                .await
                .expect("edited");
            users::update(&device, &local, "Alice")
                .await
                .expect("renamed");
            files::tests::with_bytes(&device, &local, "notes.txt", "text/plain", b"notes")
                .await
                .expect("a file");
            assert_eq!(pending(&device).await, 0, "nothing is queued");
            assert_eq!(post.version, 0);
            assert_eq!(count_items(&device).await.expect("counted"), 2);

            let report = sync::run(&device, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert!(report.local);
            assert_eq!(report.pushed + report.pulled, 0);
            assert_eq!(
                posts::list(&device, &local).await.expect("posts").len(),
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

    /// What the test's local user made: a post in their first
    /// community, a post in none, a second community, a file, and a
    /// post since deleted.
    struct Made {
        in_community: app_posts::Post,
        in_none: app_posts::Post,
        deleted: app_posts::Post,
        file: app_files::File,
    }

    async fn made_by(device: &Deployment, local: &User) -> Made {
        let first = communities::list(device, local).await.expect("listed")[0]
            .community
            .id;
        let in_community = posts::create(device, local, "in the laptop's", &[first])
            .await
            .expect("a post");
        let in_none = posts::create(device, local, "in none", &[])
            .await
            .expect("a post");
        communities::create(device, local, "Garden")
            .await
            .expect("a community");
        let deleted = posts::create(device, local, "thought better of", &[first])
            .await
            .expect("a post");
        posts::delete(device, local, deleted.id)
            .await
            .expect("deleted");
        let file = files::tests::with_bytes(device, local, "notes.txt", "text/plain", b"notes")
            .await
            .expect("a file");
        Made {
            in_community,
            in_none,
            deleted,
            file,
        }
    }

    async fn bucket_has(storage: &Storage, user: &User, id: Uuid) -> bool {
        storage
            .size(&files::storage_key(user.id, id))
            .await
            .expect("looked")
            .is_some()
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

    async fn posts_of(deployment: &Deployment, user: &User) -> Vec<app_posts::Post> {
        let mut posts: Vec<app_posts::Post> = posts::list(deployment, user)
            .await
            .expect("the posts")
            .into_iter()
            .map(|listed| listed.post)
            .collect();
        posts.sort_by_key(|post| post.id);
        posts
    }

    #[tokio::test]
    async fn local_users_move_to_a_new_account_pushes_every_live_row_with_its_time() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let (device, local, _folder) = device_with_a_local_user().await;
            let made = made_by(&device, &local).await;
            let session =
                accounts::tests::signed_up_for_a_device(&server, "alice@example.com").await;

            let alice = move_to_account(&device, session.clone())
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
                8,
                "two communities, two memberships, two posts, one of them in a community, a file"
            );

            let report = sync::run(&device, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(report.push_error, None);
            assert_eq!(
                (report.pushed, report.rejected, report.conflicts),
                (8, 0, 0)
            );
            assert_eq!(report.uploaded, 1);
            assert_eq!(report.pending, 0);

            assert_eq!(
                names_of_communities(&server, &alice).await,
                ["Alice's Laptop", "Garden", "Personal"],
                "what she made, beside her sign-up's"
            );
            let on_server = posts_of(&server, &alice).await;
            assert_eq!(on_server.len(), 2, "the deleted post never left the device");
            assert!(on_server.iter().all(|post| post.id != made.deleted.id));
            for before in [&made.in_community, &made.in_none] {
                let after = on_server
                    .iter()
                    .find(|post| post.id == before.id)
                    .expect("on the server");
                assert_eq!(after.user_id, alice.id);
                assert_eq!(
                    after.inserted_at, before.inserted_at,
                    "made when it was made"
                );
                assert_eq!(after.version, 1);
            }
            let in_community = posts::get(&server, &alice, made.in_community.id)
                .await
                .expect("the post");
            assert_eq!(in_community.communities[0].name, "Alice's Laptop");
            assert!(bucket_has(&storage, &alice, made.file.id).await);
            let file = files::get(&server, &alice, made.file.id)
                .await
                .expect("the file");
            assert_eq!(file.file.inserted_at, made.file.inserted_at);

            assert_eq!(posts_of(&device, &alice).await, on_server);
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
            let (server, _storage) = server(database);
            let session =
                accounts::tests::signed_up_for_a_device(&server, "alice@example.com").await;
            let alice = session.user.clone();
            posts::create(&server, &alice, "written on the web", &[])
                .await
                .expect("a post");
            communities::create(&server, &alice, "Baking")
                .await
                .expect("a community");

            let (device, local, _folder) = device_with_a_local_user().await;
            made_by(&device, &local).await;
            move_to_account(&device, session).await.expect("moved");
            let report = sync::run(&device, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(
                (report.pushed, report.rejected, report.conflicts),
                (8, 0, 0)
            );

            assert_eq!(
                names_of_communities(&server, &alice).await,
                ["Alice's Laptop", "Baking", "Garden", "Personal"]
            );
            let on_server = posts_of(&server, &alice).await;
            assert_eq!(on_server.len(), 3);
            assert_eq!(posts_of(&device, &alice).await, on_server);
            assert_eq!(
                names_of_communities(&device, &alice).await,
                names_of_communities(&server, &alice).await
            );
        })
        .await;
    }

    #[tokio::test]
    async fn local_users_move_to_account_is_only_for_a_local_user() {
        with_each_database(|database| async move {
            let (server, _storage) = server(database);
            let session =
                accounts::tests::signed_up_for_a_device(&server, "alice@example.com").await;
            let device = users::tests::device_of(&session.user, &session.token).await;
            posts::create(&device, &session.user, "hers already", &[])
                .await
                .expect("a post");

            let error = move_to_account(&device, session)
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
