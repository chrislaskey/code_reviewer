//! Application resources and workflows, shared by server and device deployments.
//! Record contexts and shared infrastructure remain at the crate root.

pub mod avatars;
pub mod communities;
pub mod conflicts;
pub mod current_community;
pub mod events;
pub mod files;
pub mod files_communities;
pub mod jobs;
pub mod local_users;
pub mod memberships;
pub mod pending_changes;
pub mod posts;
pub mod posts_communities;
pub mod purge;
pub mod sync;
pub mod users;
