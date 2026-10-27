# Code Reviewer

A code review tool built with Rust and React, shipping as both a desktop app (Tauri) and a web app (Axum).

## Architecture

```
frontend/          Shared React UI (@code-reviewer/ui)
  └── api/         BackendAdapter interface + types
tauri/             Tauri desktop app (provides invoke-based adapter)
  └── src-tauri/   Tauri Rust crate
axum/              Axum web server (provides HTTP routes)
  └── web/         Axum frontend entry point (provides fetch-based adapter)
core/              Shared Rust library (all business logic)
```

Both apps share the same React UI from `frontend/`. Each provides a thin platform adapter so components never call `invoke()` or `fetch()` directly.

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [pnpm](https://pnpm.io/)
- [Tauri prerequisites](https://tauri.app/start/prerequisites/) (for desktop app)

## Quick Start

Install JS dependencies:

```sh
pnpm install
```

### Run the Tauri desktop app

```sh
bin/dev tauri
```

Or manually:

```sh
cd tauri && pnpm tauri dev
```

### Run the Axum web app

```sh
bin/dev axum
```

Or manually, start the Axum server and Vite dev server in separate terminals:

```sh
cargo run -p code_reviewer_axum
cd axum/web && pnpm dev
```

Then open http://localhost:5173 (Vite proxies `/api` to the Axum server on port 3000).

### Run all checks

```sh
bin/check
```

This runs formatting, linting, tests, TypeScript checks, and security audit.

## Adding Features

1. Add the function and types to `core/src/lib.rs`
2. Add an Axum route in `axum/src/main.rs` that calls the core function
3. Add a Tauri command in `tauri/src-tauri/src/lib.rs` that calls the core function
4. Add the method to `BackendAdapter` in `frontend/src/api/adapter.ts`
5. Use `useBackend()` in shared page components under `frontend/src/pages/`
