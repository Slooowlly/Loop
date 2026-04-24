# FT-1 End Of Season Pipeline Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the backend end-of-season pipeline for season rollover, driver evolution, licenses, retirements, rookies, and the `advance_season` Tauri command.

**Architecture:** Build pure evolution modules in `src-tauri/src/evolution/` with deterministic tests first, then compose them in a database-backed pipeline that persists licenses, retirements, season resets, and next-season calendars. Integrate the pipeline into the career command surface without touching FT-2 market logic, FT-3 promotion logic, or frontend code.

**Tech Stack:** Rust, Tauri, rusqlite, serde, rand

---

## Chunk 1: Pure Evolution Modules

### Task 1: Add failing growth tests

**Files:**
- Modify: `src-tauri/src/evolution/growth.rs`

- [ ] Add `SeasonStats`, `GrowthReport`, `AttributeChange`, and failing tests for champion growth, last-place growth, youth bonus, diminishing returns, and low-tier bonus.
- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml growth::`
- [ ] Confirm the new tests fail before implementation.

### Task 2: Implement growth

**Files:**
- Modify: `src-tauri/src/evolution/growth.rs`

- [ ] Implement result-based base growth, attribute helpers, clamped `f64` mutation, and deterministic reports that serialize rounded values as `u8`.
- [ ] Re-run: `cargo test --manifest-path src-tauri/Cargo.toml growth::`
- [ ] Confirm the growth tests pass.

### Task 3: Add failing decline tests

**Files:**
- Modify: `src-tauri/src/evolution/decline.rs`

- [ ] Add failing tests for no decline under 33, higher decline with age, experience never declining, and fitness declining fastest.
- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml decline::`
- [ ] Confirm the new tests fail before implementation.

### Task 4: Implement decline

**Files:**
- Modify: `src-tauri/src/evolution/decline.rs`

- [ ] Implement age-triggered decline with attribute-specific rates and guaranteed experience gain.
- [ ] Re-run: `cargo test --manifest-path src-tauri/Cargo.toml decline::`
- [ ] Confirm the decline tests pass.

### Task 5: Add failing motivation tests

**Files:**
- Modify: `src-tauri/src/evolution/motivation.rs`

- [ ] Add failing tests for champion boost, stagnation penalty, and clamp to `0..=100`.
- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml motivation::`
- [ ] Confirm the new tests fail before implementation.

### Task 6: Implement motivation

**Files:**
- Modify: `src-tauri/src/evolution/motivation.rs`

- [ ] Implement end-of-season motivation deltas and reasons using current driver personality and season context.
- [ ] Re-run: `cargo test --manifest-path src-tauri/Cargo.toml motivation::`
- [ ] Confirm the motivation tests pass.

### Task 7: Add failing retirement and rookie tests

**Files:**
- Modify: `src-tauri/src/evolution/retirement.rs`
- Create: `src-tauri/src/evolution/rookies.rs`

- [ ] Add failing retirement tests for young drivers, guaranteed 47+ retirement, and low-motivation retirement.
- [ ] Add failing rookie tests for count, age range, and type distribution.
- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml retirement:: rookies::`
- [ ] Confirm the new tests fail before implementation.

### Task 8: Implement retirement and rookies

**Files:**
- Modify: `src-tauri/src/evolution/retirement.rs`
- Create: `src-tauri/src/evolution/rookies.rs`
- Modify: `src-tauri/src/evolution/mod.rs`

- [ ] Implement retirement checks/processors and rookie generation using existing identity generation and driver schema.
- [ ] Export the new modules from `mod.rs`.
- [ ] Re-run: `cargo test --manifest-path src-tauri/Cargo.toml retirement:: rookies::`
- [ ] Confirm the retirement and rookie tests pass.

## Chunk 2: Persistence And Pipeline

### Task 9: Add failing pipeline integration tests

**Files:**
- Create: `src-tauri/src/evolution/pipeline.rs`

- [ ] Add in-memory integration tests for incrementing year, creating the next season, and resetting season stats.
- [ ] Include fixture setup that creates a season, drivers, teams, contracts, and completed calendars.
- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml pipeline::`
- [ ] Confirm the new tests fail before implementation.

### Task 10: Implement pipeline orchestration

**Files:**
- Create: `src-tauri/src/evolution/pipeline.rs`
- Modify: `src-tauri/src/evolution/mod.rs`

- [ ] Implement standings calculation, license persistence, driver evolution, retirement persistence, rookie insertion, stat reset, next-season creation, next-calendar generation, and meta counter/year updates.
- [ ] Keep promotion/relegation and market steps explicitly skipped.
- [ ] Re-run: `cargo test --manifest-path src-tauri/Cargo.toml pipeline::`
- [ ] Confirm the pipeline tests pass.

### Task 11: Add minimal query/helpers required by the pipeline

**Files:**
- Modify: `src-tauri/src/db/queries/drivers.rs`
- Modify: `src-tauri/src/db/queries/teams.rs`
- Modify: `src-tauri/src/db/queries/seasons.rs`
- Modify: `src-tauri/src/db/queries/calendar.rs`
- Modify: `src-tauri/src/commands/career.rs`

- [ ] Add only the helpers required to persist/reset data cleanly from the pipeline.
- [ ] Re-run the affected pipeline tests after each helper addition.

## Chunk 3: Command Surface And Verification

### Task 12: Add failing command tests

**Files:**
- Modify: `src-tauri/src/commands/career.rs`

- [ ] Add a failing backend test that blocks season advance while pending races exist.
- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml advance_season`
- [ ] Confirm the new test fails before implementation.

### Task 13: Implement `advance_season`

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/commands/career_commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] Implement the command flow that opens the career database, validates no pending races, runs the end-of-season pipeline, updates `meta.json`, and returns the pipeline result.
- [ ] Register the Tauri command in `career_commands.rs` and `lib.rs`.
- [ ] Re-run: `cargo test --manifest-path src-tauri/Cargo.toml advance_season`
- [ ] Confirm the command tests pass.

### Task 14: Full verification

**Files:**
- Modify: `docs/superpowers/plans/2026-03-20-ft1-end-of-season-pipeline.md`

- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] Run: `cargo build --manifest-path src-tauri/Cargo.toml`
- [ ] Confirm both succeed before completion.
