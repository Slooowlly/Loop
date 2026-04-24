# Boot Logo Screen Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show a cinematic logo boot screen before the existing splash screen.

**Architecture:** Add a dedicated boot page rendered at `/`, move the current splash to `/splash`, and navigate automatically after `2s`. Keep the logo as a frontend asset and hide the global window-controls drawer on the boot flow routes so the opening remains clean.

**Tech Stack:** React, React Router, Vite asset handling, Node test runner, Vite build

---

### Task 1: Add a failing contract test for the boot flow

**Files:**
- Create: `scripts/tests/boot-logo-screen.test.mjs`
- Test: `src/App.jsx`
- Test: `src/pages/SplashScreen.jsx`
- Test: `src/pages/BootLogoScreen.jsx`
- Test: `src/components/layout/WindowControlsDrawer.jsx`

- [ ] **Step 1: Write the failing test**
- [ ] **Step 2: Run `node --test scripts/tests/boot-logo-screen.test.mjs` and confirm it fails**
- [ ] **Step 3: Cover the `/` boot route, `/splash` route, 2-second auto navigation, logo asset usage, and drawer hiding on boot routes**

### Task 2: Implement the boot logo screen

**Files:**
- Create: `src/pages/BootLogoScreen.jsx`
- Modify: `src/App.jsx`
- Modify: `src/pages/SplashScreen.jsx`
- Create/Copy asset for frontend logo use

- [ ] **Step 1: Add a frontend-safe logo asset**
- [ ] **Step 2: Create the boot page with centered logo and cinematic animation**
- [ ] **Step 3: Move the existing splash screen to `/splash`**
- [ ] **Step 4: Auto-navigate from boot to splash after `2000ms`**

### Task 3: Keep the global drawer out of the boot flow

**Files:**
- Modify: `src/components/layout/WindowControlsDrawer.jsx`

- [ ] **Step 1: Detect boot-flow routes with React Router location**
- [ ] **Step 2: Return `null` on `/` and `/splash`**

### Task 4: Verify the new startup flow

**Files:**
- Verify the files above only

- [ ] **Step 1: Run `node --test scripts/tests/boot-logo-screen.test.mjs scripts/tests/window-controls-navigation.test.mjs scripts/tests/window-controls-widget-panel.test.mjs scripts/tests/window-controls-contract.test.mjs scripts/tests/window-controls-sizing.test.mjs`**
- [ ] **Step 2: Run `npm run build`**
- [ ] **Step 3: Review the resulting startup flow sources for route and timing correctness**
