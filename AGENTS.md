# Agent Instructions: Rust Production Application Architecture

## Architecture Overview

This is a Cargo workspace with three Rust crates: `core/` (`app_core`, the shared application logic), `axum/` (`app_web`, the HTTP server wrapping core), and `tauri/` (`app_tauri`, the desktop and mobile app wrapping core). The shared Svelte frontend lives in `svelte/`; its pages call `svelte/src/lib/core/`, which picks the Tauri commands inside the app and the HTTP API in a browser, so pages never call `invoke()` or `fetch()` directly. The web server carries other apps beside the application (`public`, `accounts`, `support`, `forum`, `ops`), each with its name on its tables, core module, API path and page folder, and only `app_` tables sync: the design is `docs/design/multi-app.md`, read it before adding a table or a route. The public website is plain HTML pages in `svelte/src/public/` (`docs/design/public-pages.md`). App resource workflows live in `core/src/app/`, declared by `core/src/app.rs` and re-exported at the crate root for existing callers; account/ops workflows and shared infrastructure retain their existing locations. To add an app feature: add the function under `core/src/app/`, expose it as an Axum route and a Tauri command, add it to both files under `svelte/src/lib/core/`, then use it in the shared pages. Public application operations take a `Deployment` (the database and its role, server or device) and own their transactions; internal record contexts under `core/src/data/` borrow the transaction and own private SQLite/PostgreSQL adapters; synced business writes are events (`core/src/app/events.rs`) so the apps' databases sync with the web server, the system of record: the design is `docs/design/offline-sync.md`, read it before touching a write path or a migration. A person may use the app before signing up, as a local user on a device, who never syncs, or a trial user on the web, deleted after two days unless they sign up; the pages tell them apart by the current user's `kind`: the design is `docs/design/accounts-local-and-trial-users.md`.

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
* **Naming** Always use concrete naming instead of jargon. Better to have an explicit name that tells what happens when it is called instead of a shorter name with jargon and ambiguity. Prefer concrete terms like list/get/create/update/upsert/delete on Backend and index/show/new/edit/delete for UI/UX.
* **Resource first, then the action**, in every layer, so names sort and read by resource. In Rust the resource is the module and the action the function, called through the module as the standard library does (`posts::list`, never `list_posts`, and never a bare `list` imported on its own). Where one namespace spans every resource the two join as `posts_list`: Tauri command names (`pub async fn posts_list`, which is its name on the wire too), test names (`posts_get_missing`), migration names (`posts_create`). In JavaScript they join as `postsList`, `postsShowUrl`; Svelte pages are `PostsShowPage`. HTTP routes are resource first by their paths (`/api/posts/:id`).
* **Scope** Better to build through many small, well-defined iterative steps than try to build everything at once. Do not add product features or functionality that hasn't been discussed and planned first - instead write suggestions for additional functionality in `docs/suggestions/*.md` files.
* **Rust vs JavaScript** Keep the JavaScript frontend a very thin presentation layer. Prefer writing application logic in Rust whenever possible, minimal logic in JS just related to presentation layer.
* **Record access:** Application workflows call the contexts under `core/src/data/` rather than issuing SQL. Keep atomic bulk lifecycle and maintenance operations explicit there; their exceptions and hook limits are documented in `docs/design/low-level-contexts.md`. Database connection/migration setup and test fixtures are separate.
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
