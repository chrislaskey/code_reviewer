# Multi-App Architecture Plan

## Overview

A shared Rust core library (`./core`) used by both a Tauri desktop app (`./tauri`)
and an Axum web server (`./axum`), with a common React frontend (`./frontend`).

The key design principle: **one set of UI components, one set of routes, one set of
business logic** — with thin platform-specific adapters at the edges.

## Directory Structure

```
.
├── Cargo.toml              # Cargo workspace root
├── package.json            # pnpm workspace root
├── pnpm-workspace.yaml
├── core/                   # Shared Rust library (business logic)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
├── axum/                   # Axum web server
│   ├── Cargo.toml
│   ├── src/
│   │   └── main.rs
│   └── web/                # Axum frontend entry point (thin)
│       ├── package.json
│       ├── index.html
│       ├── vite.config.ts
│       └── src/
│           └── main.tsx    # Provides HTTP adapter, mounts shared app
├── tauri/                  # Tauri desktop app (existing)
│   ├── package.json
│   ├── src/
│   │   └── main.tsx        # Provides Tauri adapter, mounts shared app
│   ├── src-tauri/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── main.rs
│   └── vite.config.ts
├── frontend/               # Shared React UI (the bulk of the frontend)
│   ├── package.json
│   └── src/
│       ├── App.tsx         # Root component with router
│       ├── api/
│       │   ├── types.ts    # Shared request/response types
│       │   ├── context.ts  # React context for API adapter
│       │   └── adapter.ts  # Adapter interface
│       ├── pages/          # Route page components
│       │   └── Home.tsx
│       └── components/     # Reusable UI components
└── docs/plans/
```

## Architecture Layers

### 1. Rust Core (`./core`)

Pure business logic. No framework dependencies. Defines:
- Domain types (with serde Serialize/Deserialize)
- Business logic functions
- Error types via `thiserror`

Both Axum and Tauri depend on this crate. Neither contains business logic.

### 2. Axum Server (`./axum`)

Thin HTTP wrapper around core:
- Exposes `/api/*` routes that call core functions
- Serves the built frontend from `./axum/web/dist/` as static files
- Uses `tower-http` for static file serving and CORS

### 3. Tauri App (`./tauri/src-tauri`)

Thin Tauri command wrapper around core:
- `#[tauri::command]` functions that call core functions
- Serves the Vite dev server / built frontend via Tauri's webview

### 4. Shared Frontend (`./frontend`)

All React UI lives here as an npm package (`@code-reviewer/ui`):
- `api/adapter.ts` — defines a `BackendAdapter` interface (e.g., `greet(name)`)
- `api/context.ts` — React context that provides the adapter to all components
- `pages/` and `components/` — use the adapter via context, never call
  `invoke()` or `fetch()` directly
- `App.tsx` — react-router with all shared routes

### 5. Platform Entry Points (thin)

Each app has a tiny entry point that:
1. Creates the platform-specific adapter (Tauri invoke or HTTP fetch)
2. Wraps `<App />` from `frontend/` with the adapter context provider
3. Renders

## Communication Pattern

```
┌─────────────────────────────────────────────┐
│  Shared React UI (frontend/)                │
│  Uses BackendAdapter interface via context   │
└──────────────┬──────────────────────────────┘
               │
       ┌───────┴───────┐
       │               │
┌──────▼──────┐ ┌──────▼──────┐
│ TauriAdapter│ │ HttpAdapter │
│ invoke()    │ │ fetch()     │
└──────┬──────┘ └──────┬──────┘
       │               │
┌──────▼──────┐ ┌──────▼──────┐
│ Tauri Cmds  │ │ Axum Routes │
│ (src-tauri) │ │ (axum/src)  │
└──────┬──────┘ └──────┬──────┘
       │               │
       └───────┬───────┘
               │
       ┌───────▼───────┐
       │  Core Library  │
       │  (core/)       │
       └───────────────┘
```

## Build Steps

### Phase 1: Rust Foundation
1. Create root `Cargo.toml` workspace with members: core, axum, tauri/src-tauri
2. Flesh out `core/src/lib.rs` with shared `greet()` and types
3. Wire `tauri/src-tauri` to depend on core
4. Create `axum/` crate with basic server + `/api/greet` route
5. Verify: `cargo build --workspace`

### Phase 2: Frontend Foundation
1. Create root pnpm workspace (`package.json` + `pnpm-workspace.yaml`)
2. Create `frontend/` package with adapter pattern, router, shared pages
3. Refactor `tauri/src/` to import from `frontend/` with Tauri adapter
4. Create `axum/web/` with HTTP adapter entry point
5. Verify: both apps render the same UI

### Phase 3: Iterate
Add real features through the established pattern:
core function → axum route + tauri command → frontend page using adapter
