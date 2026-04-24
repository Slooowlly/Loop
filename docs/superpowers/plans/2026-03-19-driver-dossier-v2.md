# Driver Dossier V2 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand the current driver drawer into a modular dossier with profile, competitive snapshot, racing-centric performance stats, current form, basic career path, and prepared strategic blocks for future systems.

**Architecture:** Keep `get_driver_detail` as the single backend entry point, but migrate it from a flat payload toward section-oriented command DTOs assembled from existing driver, contract, season, standings, and race-history data. Update the right drawer to consume the new modular response and conditionally render only the sections that have meaningful content.

**Tech Stack:** Rust, Tauri commands, rusqlite, React 18, Vite, Tailwind CSS, node:test, cargo test

---

## File Structure

- `src-tauri/src/commands/career.rs`
  Command DTOs, derivation helpers, dossier assembly, and backend regression tests.
- `src-tauri/src/commands/race_history.rs`
  Existing history source to reuse for recent-form and derived finish metrics; modify only if a small helper is needed.
- `src/components/driver/DriverDetailModal.jsx`
  Drawer UI migration from flat sections to dossier-driven sections.
- `src/utils/formatters.js`
  UI helpers for license labels, form trends, stat labels, and compact display formatting if needed.
- `src/index.css`
  Any dossier-specific presentation polish not cleanly expressible via utility classes.
- `scripts/tests/driver-detail-modal.test.mjs`
  Focused frontend regression checks for the dossier contract and presentation decisions.
- `docs/superpowers/specs/2026-03-19-driver-dossier-v2-design.md`
  Approved design source of truth.

## Chunk 1: Lock The Backend Contract With Tests

### Task 1: Add failing backend tests for the modular dossier

**Files:**
- Modify: `src-tauri/src/commands/career.rs`

- [ ] Add a failing test for a contracted AI driver that expects dossier-style blocks for profile, competitive snapshot, performance, form, career path, and contract.
- [ ] Add a failing test for a free driver that expects optional strategic blocks to be absent or empty instead of populated with fake data.
- [ ] Add a failing test that proves points are no longer required by the drawer-facing performance section.
- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml test_get_driver_detail`
- [ ] Confirm the new tests fail for the current payload shape or missing derived fields.

### Task 2: Add failing frontend assertions for the redesigned drawer contract

**Files:**
- Modify: `scripts/tests/driver-detail-modal.test.mjs`

- [ ] Add assertions that the drawer source contains a license badge area near the name.
- [ ] Add assertions that personality, qualities, and defects render in a shared competitive section instead of three disconnected stacked sections.
- [ ] Add assertions that points are no longer the primary stats in season and career.
- [ ] Add assertions that a current-form section exists.
- [ ] Run: `node --test scripts/tests/driver-detail-modal.test.mjs`
- [ ] Confirm the new assertions fail before implementation.

## Chunk 2: Build Backend Dossier Blocks

### Task 3: Define dossier-oriented command structs

**Files:**
- Modify: `src-tauri/src/commands/career.rs`

- [ ] Introduce section DTOs for profile, competitive snapshot, performance, form, career path, contract/market, relationships, reputation, and health.
- [ ] Keep field names explicit and frontend-friendly so React does not need to reconstruct domain meaning.
- [ ] Prefer optional blocks over placeholder strings when data is not available.

### Task 4: Implement profile and competitive derivations

**Files:**
- Modify: `src-tauri/src/commands/career.rs`

- [ ] Add a license derivation helper based on category context and/or current experience rules.
- [ ] Add badge derivation helpers for player, rookie, champion, and championship-leader states where existing data supports them.
- [ ] Move personality and visible-tag assembly into the new competitive block.
- [ ] Keep the existing tag conversion logic, but map it into grouped qualities and defects for easier UI consumption.

### Task 5: Implement racing-centric performance derivations

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/commands/race_history.rs` only if a read helper is truly needed

- [ ] Derive season metrics for wins, podiums, top 10, outside top 10, poles, fastest laps, hat-tricks, races, and DNFs.
- [ ] Derive career metrics with the same shape where existing persisted totals allow it.
- [ ] If fastest laps or hat-tricks cannot be reconstructed historically yet, represent them honestly as `None` or `0` with a documented limitation instead of fabricating data.

### Task 6: Implement current-form and basic career-path derivations

**Files:**
- Modify: `src-tauri/src/commands/career.rs`

- [ ] Build a recent-form block from the driver's last five known results.
- [ ] Calculate recent average finish and a simple trend indicator.
- [ ] Build a basic career-path block with debut season, debut team when inferable, current category tenure, and any reconstructable milestones.
- [ ] Keep market, relationships, reputation, and health blocks structurally present but optional when no real data exists.

### Task 7: Verify backend after the first green pass

**Files:**
- Modify: `src-tauri/src/commands/career.rs` if fixes are needed

- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml test_get_driver_detail`
- [ ] Confirm the dossier-specific backend tests pass.
- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] Confirm the full Rust suite still passes.

## Chunk 3: Migrate The Drawer UI

### Task 8: Update the drawer header and profile presentation

**Files:**
- Modify: `src/components/driver/DriverDetailModal.jsx`
- Modify: `src/utils/formatters.js` if display helpers are needed

- [ ] Replace the current header layout with a stronger profile strip that highlights the flag and places the license badge beside the name.
- [ ] Keep player and status badges readable without overcrowding the header.
- [ ] Preserve close behavior, portal usage, and right-drawer shell.

### Task 9: Rebuild the competitive snapshot section

**Files:**
- Modify: `src/components/driver/DriverDetailModal.jsx`
- Modify: `src/index.css` only if a reusable divider or dossier layout helper is needed

- [ ] Replace the separate stacked personality, qualities, and defects sections with one combined competitive band.
- [ ] Add visual separators between personality, qualities, and defects.
- [ ] Keep the section responsive so it stacks cleanly on smaller widths.

### Task 10: Replace the old stats grids with dossier performance blocks

**Files:**
- Modify: `src/components/driver/DriverDetailModal.jsx`

- [ ] Remove points from the primary season and career summaries.
- [ ] Add grouped stat clusters for trophies/results, qualifying highlights, and reliability.
- [ ] Render fastest laps and hat-tricks only when the backend provides meaningful values.

### Task 11: Add current form and career path sections

**Files:**
- Modify: `src/components/driver/DriverDetailModal.jsx`

- [ ] Add a current-form section with last-five results, average finish, and trend indicator.
- [ ] Add a basic trajectory section that can render milestone items only when present.
- [ ] Keep contract as a strategic section and prepare optional slots for future market data without showing empty placeholders.

### Task 12: Render optional strategic blocks safely

**Files:**
- Modify: `src/components/driver/DriverDetailModal.jsx`

- [ ] Add conditional rendering helpers so market, relationships, reputation, and health only appear when populated.
- [ ] Ensure empty optional blocks do not leave gaps or section headings behind.

## Chunk 4: Frontend Verification And Polish

### Task 13: Verify the frontend dossier contract

**Files:**
- Modify: `scripts/tests/driver-detail-modal.test.mjs` if assertions need refinement

- [ ] Run: `node --test scripts/tests/driver-detail-modal.test.mjs`
- [ ] Confirm the focused frontend tests pass.

### Task 14: Build the app and review real limitations

**Files:**
- Modify: `docs/superpowers/plans/2026-03-19-driver-dossier-v2.md` only if documenting a limitation is necessary

- [ ] Run: `npm run build`
- [ ] Confirm the production build succeeds.
- [ ] Run: `cargo build --manifest-path src-tauri/Cargo.toml`
- [ ] Confirm the Rust app still builds.
- [ ] Document any intentionally deferred data gaps such as market value, relationships, or health remaining optional.

## Chunk 5: Execution Notes

### Task 15: Keep implementation honest and incremental

**Files:**
- Reference: `docs/superpowers/specs/2026-03-19-driver-dossier-v2-design.md`

- [ ] Prefer minimal derivation helpers over broad refactors.
- [ ] Do not fabricate gameplay data to make a section look full.
- [ ] Keep the drawer usable after each task so the dossier can ship in slices if needed.
- [ ] Re-run the focused test immediately after each meaningful UI or command contract change.
