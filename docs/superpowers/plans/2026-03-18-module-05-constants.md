# Module 05 Constants Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align the Rust constants layer with the Module 05 design document without touching `driver.rs`, `migrations.rs`, or the frontend.

**Architecture:** Keep `src-tauri/src/constants/` as the canonical home for static data, update `models/enums.rs` to match the approved spec, add `skill_ranges.rs`, and preserve existing helper APIs where useful while adding the required public functions. Tests will drive the rename from `production` to `production_challenger`, enum alignment, and utility completeness.

**Tech Stack:** Rust, Tauri backend, static slices/arrays, unit tests with `cargo test`

---

### Task 1: Add failing Module 05 tests

**Files:**
- Modify: `src-tauri/src/constants/categories.rs`
- Modify: `src-tauri/src/constants/scoring.rs`
- Modify: `src-tauri/src/constants/tracks.rs`
- Modify: `src-tauri/src/constants/teams.rs`
- Create: `src-tauri/src/constants/skill_ranges.rs`
- Modify: `src-tauri/src/constants/mod.rs`

- [ ] Add unit tests for categories, points, tracks, team counts, weather penalties, difficulty lookup, and skill ranges.
- [ ] Run `cargo test` from `src-tauri` and confirm the new tests fail for the expected missing behavior or mismatched values.

### Task 2: Align enums and static data

**Files:**
- Modify: `src-tauri/src/models/enums.rs`
- Modify: `src-tauri/src/constants/categories.rs`
- Modify: `src-tauri/src/constants/cars.rs`
- Modify: `src-tauri/src/constants/tracks.rs`
- Modify: `src-tauri/src/constants/scoring.rs`
- Modify: `src-tauri/src/constants/teams.rs`
- Modify: `src-tauri/src/constants/mod.rs`
- Create: `src-tauri/src/constants/skill_ranges.rs`

- [ ] Rename and extend enums to match the approved spec.
- [ ] Rename `production` to `production_challenger` across the constants layer.
- [ ] Add or rename the required helper functions while keeping useful existing helpers.
- [ ] Preserve existing static datasets and extend them instead of replacing them wholesale.

### Task 3: Verify everything stays green

**Files:**
- Verify the files above only

- [ ] Run `cargo test` from `src-tauri`.
- [ ] Review failures, fix remaining issues, and rerun until green.
- [ ] Summarize the remaining known inconsistency with `driver.rs` without modifying it.
