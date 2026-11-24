//! The app's high-level contexts, shared by server and device deployments.
//! Their records are under `data`.

#[cfg(feature = "host")]
pub mod analysis;
pub mod avatars;
pub mod communities;
pub mod current_community;
pub mod features;
pub mod files;
pub mod files_communities;
#[cfg(feature = "host")]
pub mod groups;
pub mod identity;
pub mod jobs;
pub mod local_users;
pub mod locales;
pub mod memberships;
pub mod posts;
pub mod posts_communities;
pub mod reviews;
pub mod server_state;
pub mod server_tasks;
pub mod synced_tables;
pub mod traces;
pub mod users;
