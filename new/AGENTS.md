# Agent Instructions: Rust Production Application Architecture

## Architecture Overview

This is a Cargo workspace with two Rust crates: `core/` (the analysis library) and `tauri/src-tauri/` (the desktop app wrapping core). The frontend in `tauri/src/` is a Svelte app with one HTML entry per page; it calls the Rust side only through `tauri/src/lib/backend.js`, so no page or component calls `invoke()` directly. To add a feature: add the function to `core/`, expose it as a Tauri command (listed in `build.rs` and allowed in `capabilities/default.json`), add a function to `backend.js`, then use it in a page. See `docs/decisions.md` for the decisions behind the layout.

## 1. Project Context
* **Language:** Rust (Stable)
* **Application Type:** Cargo workspace: a library crate and a Tauri desktop app with a Svelte frontend
* **Core Philosophy:** Strictly idiomatic, zero-cost abstractions, robust type-driven safety, and compile-first accuracy.

## 2. Verification Toolchain Commands
AI Agents MUST run these commands to test, lint, and verify code before declaring a task complete. Never skip these steps.

* **Format Check:** `cargo fmt --all -- --check`
* **Linter (Fail on Warnings):** `cargo clippy --workspace --all-targets -- -D warnings`
* **Test Suite:** `cargo test --workspace --all-targets`
* **Security Audit:** `cargo audit`
* **Frontend:** `cd tauri && pnpm check && pnpm build`
* **All of them:** `bin/check`

## 3. Strict Architectural Rules
* **Naming** Always use concrete naming instead of jargon. Better to have an explicit name that tells what happens when it is called instead of a shorter name with jargon and ambiguity. Prefer concrete terms like list/get/create/update/upsert/delete on Backend and index/show/new/edit/delete for UI/UX.
* **Scope** Better to build through many small, well-defined iterative steps than try to build everything at once. Do not add product features or functionality that hasn't been discussed and planned first - instead write suggestions for additional functionality in `docs/suggestions/*.md` files.
* **Rust vs JavaScript** Keep the JavaScript frontend a very thin presentation layer. Prefer writing application logic in Rust whenever possible, minimal logic in JS just related to presentation layer.
* **Module Layout:** Follow standard Rust layouts. Keep `main.rs` or `lib.rs` thin. House core domain logic in decoupled modules (e.g., `src/domain/`, `src/api/`, `src/storage/`).
* **Workspaces:** If adding features spanning multiple domains, separate them into dedicated crates within a Cargo workspace.
* **State Management:** Web/API handlers must pass shared state via thread-safe atomic wrappers (`std::sync::Arc`) or Layered Extractors. Avoid global mutable state (`lazy_static`/`once_cell` for mutable variables).

## 4. Code Style & Idioms
* **Naming:** Variables/Functions/Modules = `snake_case`; Structs/Traits/Enums = `PascalCase`; Statics/Constants = `SCREAMING_SNAKE_CASE`.
* **Clarity over Cleverness:** Prefer explicit, readable imperative or clear functional styles (`Iterator` chains). Avoid over-engineering hyper-complex lifetime definitions unless performance profiles demand it.
* **Clarity over Performance:** Prefer clear well structured code over optimizing for raw execution speed.
* **Clippy Alignment:** Strictly conform to standard clippy settings. Fix all suggestions.
* **Code Comments** prefer self documenting using good naming conventions for modules, functions, and variables. Only use comments to explain when absolutely critical. Always keep comments very short and concise.

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
