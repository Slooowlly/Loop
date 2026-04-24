# Driver Detail Drawer Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the centered driver modal with a fixed right-side drawer that keeps the standings visible while open.

**Architecture:** Keep the existing detail-fetching component and backend contract, but change the frontend shell from centered modal to right drawer. Adjust the standings layout to reserve horizontal space on larger screens and add tests for the new drawer contract before implementation.

**Tech Stack:** React 18, Vite, Tailwind CSS, node:test

---

## Chunk 1: Test The New Drawer Contract

### Task 1: Write failing frontend assertions

**Files:**
- Modify: `scripts/tests/driver-detail-modal.test.mjs`

- [ ] Add assertions for right drawer classes and standings reserved space when a driver is selected.
- [ ] Run: `node --test scripts/tests/driver-detail-modal.test.mjs`
- [ ] Confirm the test fails for the current centered modal implementation.

## Chunk 2: Implement The Drawer

### Task 2: Convert the detail shell to a right drawer

**Files:**
- Modify: `src/components/driver/DriverDetailModal.jsx`
- Modify: `src/index.css`

- [ ] Replace centered modal positioning with fixed right drawer positioning.
- [ ] Add slide-in / slide-out oriented animation classes.
- [ ] Keep `ESC`, click-outside, loading, error, and close button behavior.

### Task 3: Keep standings readable while open

**Files:**
- Modify: `src/pages/tabs/StandingsTab.jsx`

- [ ] Reserve right-side space on large screens when the drawer is open.
- [ ] Add clearer selected-row styling.
- [ ] Keep the current row click behavior.

## Chunk 3: Verification

### Task 4: Run focused and full verification

**Files:**
- Modify: `docs/superpowers/plans/2026-03-19-driver-detail-drawer.md`

- [ ] Run: `node --test scripts/tests/driver-detail-modal.test.mjs`
- [ ] Run: `npm run build`
- [ ] Report any remaining UX limitations honestly.
