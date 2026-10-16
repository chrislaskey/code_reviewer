# Agent Instructions: Rust Production Application Architecture

## 0. Using This Document with LLM Agents

### Tool Calling Requirements
This project uses the `pi` agent harness which requires specific tool calling conventions:

* **Always use bash for file operations**: When reading, writing, or modifying files, use `bash` commands (e.g., `cat > file`, `echo >> file`, `mkdir -p dir`). Do not rely on file write tools that may fail silently.
* **Verify tool execution**: If a tool call shows no output or appears to hang, it may have failed. Try a different approach using bash commands.
* **Check for silent failures**: Tools like `write` or `read` may appear to succeed but not actually create/modify files. Always verify with `cat` or `ls` if unsure.
* **Debugging failed operations**: If a tool call fails, retry using explicit bash alternatives:
  * For writing files: `cat > path/to/file << 'EOF' ... content ... EOF`
  * For reading files: `cat path/to/file` or `grep pattern path/to/file`
  * For directory operations: `mkdir -p path/to/dir` then `cd path/to/dir`

### Agent Output Minimization
* Keep tool output to a minimum in responses
* Summarize at the end what was done and key decision points
* Only show full output when debugging or when explicitly requested

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
