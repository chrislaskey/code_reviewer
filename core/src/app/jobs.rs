//! The app's own recurring jobs, run after the template's, see `jobs`:
//! a job the app adds implements `jobs::RecurringJob` and is listed here.

use crate::jobs::RecurringJob;

/// The app's jobs, in the order `jobs::run_due` runs them.
pub fn list() -> &'static [&'static dyn RecurringJob] {
    &[]
}
