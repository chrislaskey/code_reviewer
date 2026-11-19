//! Transaction-scoped record access, shared by application workflows.
//! Each context owns its private database adapters; workflows own commits.
//! Bulk lifecycle operations are explicit exceptions to per-resource writes,
//! documented in `docs/design/low-level-contexts.md`.

#[cfg(feature = "app-store")]
pub(crate) mod accounts_app_store_notifications;
#[cfg(feature = "stripe")]
pub(crate) mod accounts_checkouts;
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
pub(crate) mod accounts_users;
pub(crate) mod communities;
pub(crate) mod credentials;
pub(crate) mod device_settings;
pub(crate) mod events;
pub(crate) mod files;
pub(crate) mod files_communities;
pub(crate) mod jobs;
pub(crate) mod local_users;
pub(crate) mod memberships;
pub(crate) mod ops_staff;
pub(crate) mod posts;
pub(crate) mod posts_communities;
pub(crate) mod purges;
pub(crate) mod sync_backfills;
pub(crate) mod sync_state;
pub(crate) mod tokens;
pub(crate) mod trial_users;
pub(crate) mod users;
