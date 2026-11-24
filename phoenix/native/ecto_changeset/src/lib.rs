//! `Ecto.Changeset` outside the BEAM.
//!
//! A changeset function marked `@universal_changeset true` in the Phoenix
//! app is written as Rust next to its schema by `mix app.rust`, as a chain
//! of calls on `Changeset`, which casts and validates params with Ecto's
//! own semantics, messages and error options. The Phoenix app runs that
//! Rust too, so a form reaches the same rules with the same result in
//! every app. `form` turns a submitted form's entries into params the way
//! Plug does, so the step before the cast is shared as well. See
//! phoenix/priv/rust/README.md.

pub mod changeset;
pub mod form;

pub use changeset::{Changeset, Constraint, Count, Error, FieldType, Length, NumberCheck, Params};
