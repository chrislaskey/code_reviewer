//! Clients for the services the application talks to. Each is an enum
//! of the real client and a memory one that tests and development use.
//! An app lists its own here beside the template's.

#[cfg(feature = "app-store")]
pub mod app_store;
pub mod mailer;
#[cfg(feature = "object-storage")]
pub mod storage;
#[cfg(feature = "stripe")]
pub mod stripe;
pub mod web_server;
