# Dependency security status

## Decision: wait for the published SQLite fix

This application is in development and is not being deployed. The project owner chose to use published upstream dependencies and wait for a compatible SQLx release rather than maintain a vendored driver or local dependency patch.

The temporary `vendor/sqlx-sqlite` directory, root Cargo patch and workspace exclusion have been removed. The lockfile resolves the published SQLx SQLite driver again. The runtime-version test introduced specifically to enforce the local patch was removed with that patch. The stable opener plugin update to 2.7.3 is retained.

## Known SQLite advisory

The published SQLx 0.9.0 SQLite driver constrains `libsqlite3-sys` below the patched 0.39.x line. The resulting SQLite dependency is affected by RUSTSEC-2026-0459 (CVE-2026-119375). According to the advisory, opening a crafted database/WAL pair with mismatched page sizes can expose uninitialized memory. SQLite 3.54.0 fixes it.

Upstream widened the driver's constraint in [commit 10462fa4414c6c7f1ed1d39bb31678e3b726878c](https://github.com/launchbadge/sqlx/commit/10462fa4414c6c7f1ed1d39bb31678e3b726878c). Upgrade to a published compatible release when available, resolve the lockfile, and rerun the audit and workspace checks. This advisory remains visible and is not marked fixed or suppressed.

## Remaining upstream warnings

Five additional warnings remain visible, with no advisory ignores or target filters added:

| Crates | Warning | Dependency path / constraint |
| --- | --- | --- |
| `glib 0.18.5` | Unsound `VariantStrIter` (RUSTSEC-2024-0429) | Tauri's Linux GTK3/WebKit stack; the advisory's patched line is >=0.20, outside the current GTK dependencies |
| `proc-macro-error 1.0.4` | Unmaintained (RUSTSEC-2024-0370) | GLib/GTK3 procedural macros in the same Linux stack |
| `zbus_names 4.3.5`, `zvariant 5.15.1`, `zvariant_derive 5.15.1` | Unmaintained (RUSTSEC-2026-0371 / 0370 / 0372) | Stable opener 2.7.3 still uses zbus 5; these APIs moved into zbus 6 |

Their resolution needs compatible upstream changes. No alpha framework migration or local source backport is included.

## Verification after removing the local patch

- Formatting, workspace Clippy with warnings denied, and whitespace checks passed.
- Workspace tests passed: 210 core, 31 Tauri and 45 web tests; two HTTP integration tests remain ignored.
- The lockfile resolves registry packages: `sqlx-sqlite 0.9.0`, `libsqlite3-sys 0.37.0`, and opener 2.7.3. No vendor directory, Cargo patch or patch-specific runtime-version test remains.
- `cargo audit` reports the expected SQLite advisory and five existing upstream warnings. Its nonzero result is recorded as the known development-time dependency decision, not a resolved finding.
