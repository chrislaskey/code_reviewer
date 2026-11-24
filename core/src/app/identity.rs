//! Which app this is: its name, which people read, and its identifier,
//! for a device to tell a server of its own app from a server of another
//! app built from the template, whose synced tables would not match, see
//! `servers`. To change in a fork, with the `productName`, the window's
//! `title` and the `identifier` of `tauri/tauri.conf.json`, which a test
//! of the Tauri app keeps equal to them.

/// The app's name, in its emails, its window and its server's name.
pub const APP_NAME: &str = "Starter Pack";

/// The app's identifier, its bundle identifier on the phones.
pub const APP_IDENTIFIER: &str = "com.starterpack.app";
