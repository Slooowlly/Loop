# Window Controls Widget Panel Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a delayed secondary widget panel below the window controls drawer, keeping the glassmorphism style and using emoji-only visual slots.

**Architecture:** Keep the change isolated to `src/components/layout/WindowControlsDrawer.jsx`. Add one local widget list, one delayed-visibility state, and one timer lifecycle that opens the secondary panel after the main drawer has already slid in. Center the secondary panel beneath the main tray and preserve enough vertical space for the `iRacerApp` text block. Validate behavior with a source-level contract test and a frontend production build.

**Tech Stack:** React, Tauri frontend, Tailwind utility classes, Node test runner, Vite build

---

### Task 1: Add a failing contract test for the delayed widget panel

**Files:**
- Create: `scripts/tests/window-controls-widget-panel.test.mjs`
- Test: `src/components/layout/WindowControlsDrawer.jsx`

- [ ] **Step 1: Write the failing test**
- [ ] **Step 2: Run `node --test scripts/tests/window-controls-widget-panel.test.mjs` and confirm it fails because the widget panel does not exist yet**
- [ ] **Step 3: Keep the test focused on structure only: widget list, delayed state/timer, and secondary panel markup**

### Task 2: Implement the secondary panel in the drawer

**Files:**
- Modify: `src/components/layout/WindowControlsDrawer.jsx`

- [ ] **Step 1: Add local widget definitions using 5-7 emojis**
- [ ] **Step 2: Add a delayed panel state plus timer cleanup**
- [ ] **Step 3: Render the secondary glass panel below the main drawer**
- [ ] **Step 4: Style each widget slot as a compact glass capsule with subtle hover polish**
- [ ] **Step 5: Center the widget column under the top tray and reduce the delay to `500ms`**

### Task 3: Verify the drawer stays healthy

**Files:**
- Verify the files above only

- [ ] **Step 1: Run `node --test scripts/tests/window-controls-widget-panel.test.mjs scripts/tests/window-controls-contract.test.mjs scripts/tests/window-controls-sizing.test.mjs`**
- [ ] **Step 2: Run `npm run build`**
- [ ] **Step 3: Review the result to ensure the main drawer behavior remains intact and the secondary panel is now represented in code**
