# Agent Instructions: Rust Production Application Architecture

## 1. Project Context
* **Language:** Rust (Stable)
* **Application Type:** [e.g., Modular Workspace, Axum HTTP API, Ratatui CLI/TUI]
* **Core Philosophy:** Strictly idiomatic, zero-cost abstractions, robust type-driven safety, and compile-first accuracy.

## 2. Verification Toolchain Commands
AI Agents MUST run these commands to test, lint, and verify code before declaring a task complete. Never skip these steps.

* **Format Check:** `cargo fmt --all -- --check`
* **Linter (Fail on Warnings):** `cargo clippy --workspace --all-targets -- -D warnings`
* **Test Suite:** `cargo test --workspace --all-targets`
* **Security Audit:** `cargo audit`

## 3. Strict Architectural Rules
* **Module Layout:** Follow standard Rust layouts. Keep `main.rs` or `lib.rs` thin. House core domain logic in decoupled modules (e.g., `src/domain/`, `src/api/`, `src/storage/`).
* **Workspaces:** If adding features spanning multiple domains, separate them into dedicated crates within a Cargo workspace.
* **State Management:** Web/API handlers must pass shared state via thread-safe atomic wrappers (`std::sync::Arc`) or Layered Extractors. Avoid global mutable state (`lazy_static`/`once_cell` for mutable variables).

## 4. Code Style & Idioms
* **Naming:** Variables/Functions/Modules = `snake_case`; Structs/Traits/Enums = `PascalCase`; Statics/Constants = `SCREAMING_SNAKE_CASE`.
* **Clarity over Cleverness:** Prefer explicit, readable imperative or clear functional styles (`Iterator` chains). Avoid over-engineering hyper-complex lifetime definitions unless performance profiles demand it.
* **Clippy Alignment:** Strictly conform to standard clippy settings. Fix all suggestions.

## 5. Explicit Type & Error Handling
* **No Swallowing Errors:** Production code MUST NOT use `.unwrap()`, `.expect()`, or `panic!` unless writing test suites or mathematically proven unreachability.
* **Crate Stack for Errors:** 
  * Use `thiserror` for domain-specific, strongly-typed library errors.
  * Use `anyhow` with descriptive `.context()` additions only at the application execution/CLI boundary layer.
* **Propagations:** Prefer the `?` operator over exhaustive macro-matching unless matching yields explicit domain logic.

## 6. Async & Concurrency Guardrails
* **Tokio Runtimes:** Never hold a standard `std::sync::MutexGuard` across an `.await` boundary. This freezes thread executors. ALWAYS substitute with `tokio::sync::Mutex` if a lock must be held across a yield point.
* **Blocking Tasks:** Do not execute heavy synchronous I/O or intensive CPU tasks directly inside async runtime threads. Intercept and wrap them via `tokio::task::spawn_blocking`.

## 7. Operational Boundaries (Three-Tier Enforcement)
* **ALWAYS DO:** Document public structs and functions using triple-slash (`///`) doc comments. Document safety guarantees with an explicit `// SAFETY:` block whenever `unsafe` is utilized.
* **ASK FIRST:** Prefer to use common libraries instead of writing our own, ask the user before writing our own version. Ask before altering database schemas / core migrations / deployment / git actions.
* **NEVER DO:** Never commit hardcoded API secrets, credentials, or development `.env` configurations. Use the `secrecy` crate or environment mapping wrappers.
