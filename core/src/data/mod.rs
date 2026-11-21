//! Transaction-scoped record access, shared by application workflows.
//! Each context owns its private database adapters; workflows own commits.
//! Bulk lifecycle operations are explicit exceptions to per-resource writes,
//! documented in `docs/design/low-level-contexts.md`.

#[cfg(feature = "app-store")]
pub(crate) mod accounts_app_store_notifications;
#[cfg(feature = "stripe")]
pub(crate) mod accounts_checkouts;
pub mod accounts_credentials;
#[cfg(feature = "stripe")]
pub(crate) mod accounts_prices;
#[cfg(feature = "stripe")]
pub(crate) mod accounts_products;
#[cfg(feature = "stripe")]
pub(crate) mod accounts_purchases;
#[cfg(feature = "stripe")]
pub(crate) mod accounts_stripe_events;
#[cfg(feature = "stripe")]
pub(crate) mod accounts_subscriptions;
pub mod accounts_tokens;
pub mod accounts_users;
pub mod app_communities;
pub(crate) mod app_events;
pub mod app_files;
pub mod app_files_communities;
pub mod app_jobs;
pub mod app_posts;
pub mod app_posts_communities;
pub mod app_purges;
pub(crate) mod app_sync_backfills;
pub(crate) mod app_sync_state;
pub mod app_users;
pub mod app_users_communities;
pub mod device_settings;
pub(crate) mod local_users;
pub(crate) mod ops_staff;
pub(crate) mod trial_users;
