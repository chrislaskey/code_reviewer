//! The `dynamic_form` Elixir library's forms, for apps without the BEAM.
//!
//! The Phoenix app defines a form as the library's JSON and renders it
//! with the library. This crate reads the same JSON and does what the
//! library does, the same way: `changeset` validates params by its rules,
//! on the `ecto_changeset` crate so the errors are Ecto's; `view` turns
//! the form into what it shows, each question as the props of the field
//! component the Phoenix app's `CoreComponents.input/1` renders and
//! mirrors into Svelte; and `DynamicForm` runs the form's lifecycle, as
//! the library's LiveComponent does.
//!
//! Parity is the goal: where the library behaves oddly, this does too,
//! and `phoenix/docs/design/dynamic-forms.md` lists those. A definition
//! the library would crash on, or one with a `file` question, fails to
//! load here instead.

pub mod carry_forward;
pub mod changeset;
pub mod definition;
pub mod elixir;
pub mod expression;
pub mod form;
pub mod nested;
pub mod view;

pub use changeset::{FormChangeset, changeset};
pub use definition::{Choice, Container, Definition, Element, Question, Validator};
pub use form::{DynamicForm, FormState, Shown};
pub use view::{ElementView, FormTarget, FormView, view};
