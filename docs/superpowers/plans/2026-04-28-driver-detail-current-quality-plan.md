# Driver Detail Current Quality Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:executing-plans in this Codex session. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rework the driver detail drawer into a six-tab current-quality dossier centered on whether the driver is good right now.

**Architecture:** Extend the existing `get_driver_detail` payload with ranking, context and rivalry blocks, then render those blocks in the existing drawer/section structure. Keep backend derivations in `career_detail.rs`; keep UI tab content in `DriverDetailModalSections.jsx`.

**Tech Stack:** Rust/Tauri, SQLite via rusqlite, React, existing Node structure tests and cargo tests.

---

## File Structure

- Modify: `src-tauri/src/commands/career_types.rs`
  - Add serializable blocks for current summary, career ranks, performance context and rivals.
- Modify: `src-tauri/src/commands/career_detail.rs`
  - Build the new blocks from existing drivers, contracts, teams and rivalries.
- Modify: `src/components/driver/DriverDetailModal.jsx`
  - Change tab model to `resumo`, `qualidade`, `leitura`, `historico`, `rivais`, `mercado`.
- Modify: `src/components/driver/DriverDetailModalSections.jsx`
  - Render the new six-tab content.
- Modify: `scripts/tests/driver-detail-modal.test.mjs`
  - Assert the new dossier tab names and key UI anchors.

## Chunk 1: Backend Detail Payload

### Task 1: Add Driver Detail Blocks

- [ ] **Step 1: Write failing backend tests**
  - Add/extend tests in `src-tauri/src/commands/career.rs` or `career_detail.rs` to assert career ranks include podiums and ordinals can be derived from payload values.
- [ ] **Step 2: Run focused Rust test**
  - Run `cd src-tauri; cargo test commands::career::tests::test_get_driver_detail`
  - Expected: fail before implementation if assertions target missing fields.
- [ ] **Step 3: Add data structs**
  - Add structs for `DriverCurrentSummaryBlock`, `DriverCareerRankBlock`, `DriverPerformanceContextBlock`, `DriverRivalriesBlock`.
- [ ] **Step 4: Build payload blocks**
  - Compute current summary from season stats/form.
  - Compute global ranks by sorting all drivers by career stats.
  - Compute context from team car performance, teammate and category position when available.
  - Load rivalries via `get_pilot_rivalries`.
- [ ] **Step 5: Run Rust tests**

## Chunk 2: Frontend Dossier Tabs

### Task 2: Render Six Tabs

- [ ] **Step 1: Write failing structure test**
  - Update `scripts/tests/driver-detail-modal.test.mjs` to require `Resumo`, `Qualidade`, `Leitura de desempenho`, `Historico`, `Rivais`, `Mercado`.
- [ ] **Step 2: Run structure test**
  - Run `npm run test:structure -- driver-detail-modal`
  - Expected: fail before UI implementation.
- [ ] **Step 3: Update tab navigation**
  - Replace current four-tab list with six tabs and default `resumo`.
- [ ] **Step 4: Render sections**
  - Move current season/form into `Resumo`.
  - Add `Qualidade`, `Leitura de desempenho`, `Historico`, `Rivais`, `Mercado` section renderers.
  - Color team name in `Mercado` using team primary color.
- [ ] **Step 5: Run structure test**

## Chunk 3: Verification

- [ ] Run `npm run test:structure -- driver-detail-modal`.
- [ ] Run `npm run test:ui -- StandingsTab`.
- [ ] Run focused Rust tests for driver detail.
- [ ] Run `npm run build` if time allows after focused tests are clean.
