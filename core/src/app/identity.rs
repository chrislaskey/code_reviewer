//! Which app this is, for a device to tell a server of its own app from
//! a server of another app built from the template, whose synced tables
//! would not match, see `servers`. To change in a fork, with the
//! `identifier` of `tauri/tauri.conf.json`, which a test of the Tauri
//! app keeps equal to it.

/// The app's identifier, its bundle identifier on the phones.
pub const APP_IDENTIFIER: &str = "com.starterpack.app";
