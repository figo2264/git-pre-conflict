# Issue #1 — Conflict Details & Resolution Guide

## Status: Complete

## Changes Made

### Step 1: Core — ConflictType, ConflictDetail, enriched parser
- Added `ConflictType` enum with 8 variants (Content, AddAdd, ModifyDelete, etc.)
- Added `ConflictDetail` struct with path, conflict_type, message
- Changed `ConflictReport.conflicted_files` from `Vec<String>` to `Vec<ConflictDetail>`
- Added `Deserialize` to `ConflictReport` (needed for Tauri IPC deserialization)
- Rewrote `parse_merge_tree()` to extract conflict types from CONFLICT lines
- Uses `BTreeMap` for dedup; CONFLICT lines take precedence over tab entries

### Step 2: Core — merge_base() and diff_file() git wrappers
- `merge_base()` — runs `git merge-base` to find common ancestor
- `diff_file()` — runs `git diff <base> <target> -- <file>` with allow_failure

### Step 3: Core — Resolution guide module (guide.rs)
- `ResolutionGuide`, `GuideStep`, `FileGuide` structs
- `generate_resolution_guide()` — step-by-step merge workflow
- `advice_for_type()` — human-readable per-type advice

### Step 4: Tauri — IPC commands
- `get_conflict_diff` — on-demand per-file diff via merge_base + diff_file
- `get_resolution_guide` — generates guide from ConflictReport
- Both registered in invoke_handler

### Step 5: CLI — Type badges, --detail flag, resolution guide
- Added `--detail` flag to Check subcommand
- Shows `[type]` badge before each file path
- Always prints resolution guide when conflicts exist
- `--detail` shows per-file advice + diff output (50 line limit, colored)

### Step 6: Frontend — Badges, expandable diff, resolution guide panel
- Conflict rows with type badges (color-coded by category)
- Expand/collapse button per file to load diff on demand
- Diff panel with syntax coloring (+green, -red, @@purple)
- Resolution guide panel with numbered steps and click-to-copy commands
- All new CSS styles for badges, diff panel, guide panel

### Step 7: Cleanup
- `cargo fmt --all` — formatted
- `cargo clippy --workspace` — fixed useless_format warnings in guide.rs
- Pre-existing clippy warnings in src-tauri (derivable_impls, single_match) not touched

## Design Decisions
- Diffs are on-demand (not in ConflictReport) to avoid overhead in watch mode
- Guide logic lives in core (guide.rs), shared between CLI and Tauri
- ConflictType uses snake_case serde for clean JS interop