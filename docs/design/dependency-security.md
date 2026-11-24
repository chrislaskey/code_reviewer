# Dependency security status

## SQLite advisory, fixed by the published SQLx 0.9.1

The project uses published upstream dependencies only, with no vendored driver or local dependency patch, and waits for a compatible upstream release when an advisory needs one.

RUSTSEC-2026-0459 (CVE-2026-119375) affected `libsqlite3-sys` below 0.39: opening a crafted database/WAL pair with mismatched page sizes could expose uninitialized memory. SQLx 0.9.0 constrained the driver below the patched line; SQLx 0.9.1 (2026-11-18) allows `libsqlite3-sys` up to 0.40, and the lockfile resolves `sqlx-sqlite 0.9.1` with `libsqlite3-sys 0.40.0`. `core/Cargo.toml` still asks for `sqlx 0.9`, so only the lockfile changed.

## Remaining upstream warnings

Seven warnings remain visible, with no advisory ignores or target filters added:

| Crates | Warning | Dependency path / constraint |
| --- | --- | --- |
| `glib 0.18.5` | Unsound `VariantStrIter` (RUSTSEC-2024-0429) | Tauri's Linux GTK3/WebKit stack; the advisory's patched line is >=0.20, outside the current GTK dependencies |
| `proc-macro-error 1.0.4` | Unmaintained (RUSTSEC-2024-0370) | GLib/GTK3 procedural macros in the same Linux stack |
| `chrono 0.4.45` | Unmaintained (RUSTSEC-2026-0543) | Used by the application directly and by SQLx, object_store, Stripe and the App Store library |
| `humantime 2.4.0` | Unmaintained (RUSTSEC-2026-0542) | object_store 0.14 |
| `zbus_names 4.3.5`, `zvariant 5.15.1`, `zvariant_derive 5.15.1` | Unmaintained (RUSTSEC-2026-0371 / 0370 / 0372) | Stable opener 2.7.3 still uses zbus 5; these APIs moved into zbus 6 |

Their resolution needs compatible upstream changes. No alpha framework migration or local source backport is included.

## Verification after the SQLx 0.9.1 update

- Formatting and workspace Clippy with warnings denied passed.
- Workspace tests passed: 225 core, 31 Tauri and 45 web tests; four core tests remain ignored.
- `cargo audit` reports no vulnerabilities, only the seven warnings above.
