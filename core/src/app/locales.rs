//! The languages the app is in, which every app edits to add one:
//! English, the language its text is written in, first; then each with a
//! `<locale>.po` in `svelte/translations/`, `svelte/translations/public/`
//! and `core/translations/`, and its emails' catalog compiled, that
//! folder's `<locale>.mo`, below. Tests keep this list and the folders in
//! step, see `helpers::locales` and `docs/design/locales.md`.

/// Every language a person may choose, English first.
pub const SUPPORTED: &[&str] = &["en", "es"];

/// The emails' compiled catalog of each language but English, which
/// `pnpm translations:update` writes, see `helpers::translations`.
pub const CATALOGS: &[(&str, &[u8])] = &[("es", include_bytes!("../../translations/es.mo"))];
