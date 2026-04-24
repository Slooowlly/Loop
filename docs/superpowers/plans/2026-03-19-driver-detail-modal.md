# Driver Detail Modal Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a detailed driver modal opened from standings rows, backed by a new Tauri command that aggregates driver, team, contract, personality, tag, and stats data.

**Architecture:** Build a single backend `get_driver_detail` command in `career.rs` that opens the save DB, resolves related entities, and returns a frontend-ready payload. The React side adds a dedicated modal component that fetches this payload on demand and integrates into `StandingsTab` without changing unrelated pages or core models.

**Tech Stack:** Rust, Tauri commands, rusqlite, React 18, Zustand, Tailwind CSS

---

## Chunk 1: Backend Driver Detail

### Task 1: Add failing backend tests

**Files:**
- Modify: `src-tauri/src/commands/career.rs`

- [ ] Add tests for a contracted driver detail payload and a free driver payload.
- [ ] Run: `cargo test get_driver_detail --manifest-path src-tauri/Cargo.toml`
- [ ] Confirm the tests fail because `get_driver_detail_in_base_dir` or related helpers do not exist yet.

### Task 2: Implement backend payload and command

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] Add `DriverDetail`, `PersonalityInfo`, `TagInfo`, `StatsBlock`, and `ContractDetail`.
- [ ] Add tag/personality/status/helper conversion functions.
- [ ] Implement `get_driver_detail` plus an internal `get_driver_detail_in_base_dir`.
- [ ] Register the command in `src-tauri/src/lib.rs`.
- [ ] Run: `cargo test get_driver_detail --manifest-path src-tauri/Cargo.toml`
- [ ] Confirm the new tests pass.

## Chunk 2: Frontend Modal

### Task 3: Build modal component

**Files:**
- Create: `src/components/driver/DriverDetailModal.jsx`
- Modify: `src/utils/formatters.js`
- Modify: `src/index.css`

- [ ] Create the modal with loading, error, ESC close, click-outside close, sections, motivation bar, stats grid, and contract rendering.
- [ ] Add `formatSalary` to the formatter utilities.
- [ ] Add `fade-in` and `scale-in` animations to `src/index.css`.

### Task 4: Integrate standings click flow

**Files:**
- Modify: `src/pages/tabs/StandingsTab.jsx`

- [ ] Add `selectedDriverId` state.
- [ ] Make standings rows keyboard/click accessible enough for the current pattern and open the modal on row click.
- [ ] Render `DriverDetailModal` at the page level.

## Chunk 3: Verification

### Task 5: Run end-to-end verification

**Files:**
- Modify: `docs/superpowers/plans/2026-03-19-driver-detail-modal.md`

- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] Run: `cargo build --manifest-path src-tauri/Cargo.toml`
- [ ] Run: `npm run build`
- [ ] Record any failures honestly and fix before completion.
