# Driver Detail Modal Stability Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the driver detail drawer loading bug when identifiers are missing and reduce modal-file sprawl with a moderate extraction.

**Architecture:** Keep `DriverDetailModal.jsx` as the feature container responsible for portal mounting, transition timing, and adjacent-driver navigation. Move the heavier dossier sections and shared value-formatting helpers into a companion module so the container focuses on state and shell rendering while preserving the existing props contract.

**Tech Stack:** React 18, Tauri API invoke, Node test runner, Tailwind CSS

---

## Chunk 1: Lock The Bug With Tests

### Task 1: Add failing source-contract tests

**Files:**
- Modify: `scripts/tests/driver-detail-modal.test.mjs`

- [ ] Add a test that requires `DriverDetailModal.jsx` to explicitly stop loading when `careerId` or `driverId` is missing.
- [ ] Add a test that requires the dossier sections to be extracted into a companion module imported by `DriverDetailModal.jsx`.
- [ ] Run: `node scripts/tests/driver-detail-modal.test.mjs`
- [ ] Confirm the new assertions fail before implementation.

## Chunk 2: Fix And Refactor

### Task 2: Extract dossier sections

**Files:**
- Create: `src/components/driver/DriverDetailModalSections.jsx`
- Modify: `src/components/driver/DriverDetailModal.jsx`

- [ ] Move the current-moment, form, career, and market tab sections plus their local helper renderers into `DriverDetailModalSections.jsx`.
- [ ] Keep `Section`, `MotivationBar`, edge-navigation helpers, and the container state in `DriverDetailModal.jsx`.
- [ ] Preserve the existing prop contract and visual output.

### Task 3: Fix missing-id loading behavior

**Files:**
- Modify: `src/components/driver/DriverDetailModal.jsx`

- [ ] Add a guard inside the fetch flow that clears loading and avoids invoking Tauri when `careerId` or `driverId` is missing.
- [ ] Keep error/detail reset behavior predictable for the guarded path.
- [ ] Remove any now-unused dead markup left behind by the extraction.

## Chunk 3: Verify

### Task 4: Run targeted verification

**Files:**
- Modify: `docs/superpowers/plans/2026-03-20-driver-detail-modal-stability.md`

- [ ] Run: `node scripts/tests/driver-detail-modal.test.mjs`
- [ ] Run: `npm run build`
- [ ] Record any failures honestly and fix them before completion.
