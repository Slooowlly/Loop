# Preseason Displaced Drivers Modal Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Group end-of-preseason displaced drivers by category, show their category explicitly, and enlarge the confirmation modal.

**Architecture:** Keep the change inside `src/components/season/PreSeasonView.jsx` by deriving a modal-specific grouped structure from existing free-agent data. Validate behavior through focused component tests that open the modal from the existing preseason-complete flow.

**Tech Stack:** React, Vitest, Testing Library, Tauri invoke mock, Zustand store mock

---

## Chunk 1: Test Coverage

### Task 1: Add failing modal grouping test

**Files:**
- Modify: `src/components/season/PreSeasonView.test.jsx`

- [ ] **Step 1: Write the failing test**

Add a test that renders a completed preseason with displaced veterans across at least two categories, opens the modal through the existing CTA, and expects grouped category labels plus driver names.

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test -- src/components/season/PreSeasonView.test.jsx`
Expected: FAIL because the current modal renders a flat list without category grouping details.

## Chunk 2: UI Update

### Task 2: Implement grouped displaced-driver modal

**Files:**
- Modify: `src/components/season/PreSeasonView.jsx`

- [ ] **Step 1: Add grouped derived data**

Create a `useMemo` that groups `displacedVeterans` by `categoria` and sorts categories with the existing free-agent order.

- [ ] **Step 2: Update modal rendering**

Render category sections, show a category header with count, display the category on each driver row, and increase modal width/scroll area height.

- [ ] **Step 3: Keep fallbacks safe**

Continue supporting missing category values by using `outras`.

## Chunk 3: Verification

### Task 3: Verify behavior and project conventions

**Files:**
- Modify: `src/components/season/PreSeasonView.jsx`
- Modify: `src/components/season/PreSeasonView.test.jsx`

- [ ] **Step 1: Run focused tests**

Run: `npm test -- src/components/season/PreSeasonView.test.jsx`
Expected: PASS

- [ ] **Step 2: Run project audit**

Run: `python scripts/auditar_padrao.py --root . --paths src/components/season/PreSeasonView.jsx src/components/season/PreSeasonView.test.jsx`
Expected: audit output without critical violations for the touched files

- [ ] **Step 3: Review dirty worktree context**

Confirm only intended changes were made in the touched files and do not revert unrelated user edits.

Plan complete and saved to `docs/superpowers/plans/2026-04-08-preseason-displaced-drivers-modal.md`. Ready to execute.
