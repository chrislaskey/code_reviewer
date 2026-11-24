//! The features a server turns on or off, the `features` of its
//! settings, see `servers::settings`: one field per key, named for the
//! app whose code it turns on (`accounts_sign_up`), each with the value
//! a server has when its settings leave it out. A key never changes
//! type; a new meaning is a new key. Every app edits this list to add
//! its own.

use serde::{Deserialize, Serialize};

/// The features a server offers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Features {
    /// Whether the server takes new sign-ups, trial users included.
    pub accounts_sign_up: bool,
    /// Whether a person may sign in with an emailed code; off, by
    /// password alone, and the server sends no sign-in emails.
    pub accounts_sign_in_code: bool,
    /// Whether the server sells plans and products; off, it answers
    /// none of the purchasing routes and the pages offer nothing to buy.
    pub accounts_purchasing: bool,
}

impl Default for Features {
    fn default() -> Features {
        Features {
            accounts_sign_up: true,
            accounts_sign_in_code: true,
            accounts_purchasing: true,
        }
    }
}
