# Career Types Extraction Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract `career.rs` request/response DTOs into a dedicated module without changing behavior.

**Architecture:** Keep `career.rs` as the logic and command entrypoint, and move only data-structure declarations into `career_types.rs`. The main file imports the types from the sibling module, reducing file size while preserving signatures and serialization.

**Tech Stack:** Rust, Tauri, serde, Node test runner

---

## Chunk 1: Lock The Extraction

### Task 1: Add failing structural test

**Files:**
- Create: `scripts/tests/career-command-structure.test.mjs`

- [ ] Add assertions requiring a dedicated `career_types.rs` module.
- [ ] Add assertions requiring `career.rs` to import those types instead of defining key structs inline.
- [ ] Run: `node scripts/tests/career-command-structure.test.mjs`
- [ ] Confirm it fails before implementation.

## Chunk 2: Extract Types

### Task 2: Move DTOs into a sibling module

**Files:**
- Create: `src-tauri/src/commands/career_types.rs`
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/commands/mod.rs`

- [ ] Move the request/response and block structs out of `career.rs`.
- [ ] Wire `commands::career_types` in `mod.rs`.
- [ ] Import the moved types back into `career.rs`.
- [ ] Keep command signatures and serde shape unchanged.

## Chunk 3: Verify

### Task 3: Run verification

**Files:**
- Modify: `docs/superpowers/plans/2026-03-20-career-types-extraction.md`

- [ ] Run: `node scripts/tests/career-command-structure.test.mjs`
- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] Confirm both pass before completion.
