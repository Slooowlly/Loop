# Tauri Command Surface Cleanup Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove unused diagnostic commands from the production Tauri command surface without deleting the backend helpers.

**Architecture:** Keep the diagnostic functions in `career.rs`, but stop registering them in the app's `invoke_handler`. Add a lightweight contract test so the production command list stays intentionally small.

**Tech Stack:** Rust, Tauri, Node test runner

---

## Chunk 1: Lock The Contract

### Task 1: Add failing command-surface test

**Files:**
- Modify: `scripts/tests/window-controls-contract.test.mjs`

- [ ] Add assertions that `verify_database`, `test_create_driver`, and `test_list_drivers` are not present in `src-tauri/src/lib.rs`.
- [ ] Run: `node scripts/tests/window-controls-contract.test.mjs`
- [ ] Confirm the new assertions fail before implementation.

## Chunk 2: Remove Exposure

### Task 2: Clean the production invoke handler

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/career.rs`

- [ ] Remove the three diagnostic helpers from the `generate_handler!` list.
- [ ] Mark the helper functions as internal diagnostics rather than public Tauri commands.

## Chunk 3: Verify

### Task 3: Run verification

**Files:**
- Modify: `docs/superpowers/plans/2026-03-20-tauri-command-surface.md`

- [ ] Run: `node scripts/tests/window-controls-contract.test.mjs`
- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] Confirm both pass before completion.
