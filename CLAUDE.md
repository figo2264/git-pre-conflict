# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**git-pre-conflict** detects git merge conflicts before they happen. It runs `git merge-tree --write-tree` to simulate a merge and reports conflicting files without modifying the working tree. Two frontends: a Tauri v2 desktop GUI and a CLI binary.

## Build & Run Commands

```bash
# Build all workspace crates
cargo build

# Build/run CLI only
cargo build -p git-pre-conflict-cli
cargo run -p git-pre-conflict-cli -- check <target-branch>

# Tauri desktop app (requires npm deps installed)
npm install
npm run tauri:dev     # Dev mode with hot reload
npm run tauri:build   # Production bundle

# Lint & format (no custom config, uses defaults)
cargo fmt --all
cargo clippy --workspace

# No tests exist yet. When added:
cargo test --workspace
```

## Architecture

Three-layer workspace with shared core:

```
crates/core  →  Core library (git-pre-conflict-core)
src-cli/     →  CLI binary (git-pre-conflict-cli), uses clap
src-tauri/   →  Tauri v2 desktop app (git-pre-conflict-app)
src/         →  Frontend UI (vanilla HTML/CSS/JS, no framework/bundler)
```

**Core library** (`crates/core`): All git logic lives here. Shells out to the `git` binary (no libgit2). Key modules:
- `git.rs` — subprocess wrappers: `run_git()`, `find_git_dir()`, `current_branch()`, `fetch_origin()`, `resolve_target_ref()`, `merge_tree()`
- `conflict.rs` — `ConflictReport` struct and `parse_merge_tree()` parser (two strategies: CONFLICT lines + tab-delimited stage entries)
- `error.rs` — `AppError` enum with `serde::Serialize` for Tauri IPC compatibility

**CLI** (`src-cli`): `git-pre-conflict check <target> [--no-fetch]`. Exit codes: 0=clean, 1=conflicts, 2=error.

**Tauri app** (`src-tauri`): Exposes 5 IPC commands (`check_conflicts`, `get_current_branch`, `start_watch`, `stop_watch`, `get_watch_status`). Background watch loop uses Tokio + `tokio::sync::watch` channel for stop signaling, stores state in `Arc<Mutex<WatchState>>`.

**Frontend** (`src/`): Calls Tauri via `window.__TAURI__.core.invoke(...)`. Polls `get_watch_status` every 3 seconds during watch mode. Dark theme UI.

## Key Patterns

- All git operations go through `run_git()` in `crates/core/src/git.rs` which captures stdout/stderr/exit code
- `AppError` implements `serde::Serialize` (serializes to error string) so it can cross the Tauri IPC boundary
- Tauri state is managed via `tauri::State<Arc<Mutex<WatchState>>>` — always lock briefly and release
- The merge-tree parser handles two git output formats for robustness
- Rust edition 2021, Tauri v2, Tokio for async in the Tauri crate only
