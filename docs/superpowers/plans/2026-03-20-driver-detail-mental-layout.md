# Driver Detail Mental Layout Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reorganize the driver detail drawer so the mental/competitive area wastes less space and the current-moment cards communicate form and contract status more clearly.

**Architecture:** Keep the existing data contract intact and refactor only the presentation layer inside `DriverDetailModal.jsx`. The mental block will become a two-column layout with motivation promoted to the top, while the current-moment block will be rewritten into clearer labeled cards without changing how the drawer fetches data.

**Tech Stack:** React 18, JSX, Tailwind utility classes, Node `node:test`

---

## Chunk 1: Verification First

### Task 1: Extend the drawer contract test

**Files:**
- Modify: `scripts/tests/driver-detail-modal.test.mjs`
- Test: `scripts/tests/driver-detail-modal.test.mjs`

- [ ] **Step 1: Add failing assertions for the new mental and current-moment copy**
- [ ] **Step 2: Run `node --test scripts/tests/driver-detail-modal.test.mjs` and confirm the new assertions fail for the expected missing strings**

## Chunk 2: Mental Section

### Task 2: Refactor the competitive block into a mental layout

**Files:**
- Modify: `src/components/driver/DriverDetailModal.jsx`
- Test: `scripts/tests/driver-detail-modal.test.mjs`

- [ ] **Step 1: Move motivation to the top of the section and rename the section to `Mental`**
- [ ] **Step 2: Replace the three-column layout with two columns: personality on the left and a combined prós/contras panel on the right**
- [ ] **Step 3: Keep current backend fields and empty states intact**

## Chunk 3: Current Moment

### Task 3: Rebuild the current moment cards

**Files:**
- Modify: `src/components/driver/DriverDetailModal.jsx`
- Test: `scripts/tests/driver-detail-modal.test.mjs`

- [ ] **Step 1: Rename the left card to `Forma recente` and place the average plus trend together**
- [ ] **Step 2: Show the form status as a labeled row instead of leaving it floating in the corner**
- [ ] **Step 3: Rename the right card to `Situacao contratual` and convert content to label/value rows**
- [ ] **Step 4: Replace the ambiguous duration emphasis with `Expira em` based on `anos_restantes`**

## Chunk 4: Verification

### Task 4: Validate the drawer behavior

**Files:**
- Modify: `src/components/driver/DriverDetailModal.jsx`
- Test: `scripts/tests/driver-detail-modal.test.mjs`

- [ ] **Step 1: Re-run `node --test scripts/tests/driver-detail-modal.test.mjs`**
- [ ] **Step 2: Run `npm run build`**
