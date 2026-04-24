# Window Controls Navigation Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the widget panel into three global navigation shortcuts and make the drawer render across all app routes.

**Architecture:** Keep the behavior centered in `WindowControlsDrawer.jsx` by defining route-aware widget metadata and click handlers there. Move the drawer from `MainLayout` into `App.jsx` so every screen shares the same tray. Remove the dashboard-only menu button once the `🏠` widget assumes its behavior by calling `clearCareer()` before navigating to `/menu`.

**Tech Stack:** React Router, Zustand, React, Node test runner, Vite build

---

### Task 1: Add a failing contract test for global widget navigation

**Files:**
- Create: `scripts/tests/window-controls-navigation.test.mjs`
- Test: `src/components/layout/WindowControlsDrawer.jsx`
- Test: `src/App.jsx`
- Test: `src/components/layout/MainLayout.jsx`
- Test: `src/components/layout/Header.jsx`

- [ ] **Step 1: Write the failing test**
- [ ] **Step 2: Run `node --test scripts/tests/window-controls-navigation.test.mjs` and confirm it fails**
- [ ] **Step 3: Cover the three widget routes, global mounting in `App.jsx`, removal from `MainLayout.jsx`, and removal of the header menu button**

### Task 2: Implement route-aware widget actions

**Files:**
- Modify: `src/components/layout/WindowControlsDrawer.jsx`

- [ ] **Step 1: Replace the visual-only widget list with route metadata**
- [ ] **Step 2: Add navigation handlers using React Router**
- [ ] **Step 3: Make the `🏠` widget call `clearCareer()` before navigating to `/menu`**
- [ ] **Step 4: Add `title` plus subtle active-route styling**

### Task 3: Make the drawer global and remove redundancy

**Files:**
- Modify: `src/App.jsx`
- Modify: `src/components/layout/MainLayout.jsx`
- Modify: `src/components/layout/Header.jsx`

- [ ] **Step 1: Render `WindowControlsDrawer` once at the app shell level**
- [ ] **Step 2: Remove the duplicate drawer from `MainLayout.jsx`**
- [ ] **Step 3: Remove the dashboard header button that becomes redundant**

### Task 4: Verify the navigation tray stays healthy

**Files:**
- Verify the files above only

- [ ] **Step 1: Run `node --test scripts/tests/window-controls-navigation.test.mjs scripts/tests/window-controls-widget-panel.test.mjs scripts/tests/window-controls-contract.test.mjs scripts/tests/window-controls-sizing.test.mjs`**
- [ ] **Step 2: Run `npm run build`**
- [ ] **Step 3: Review the source diff to ensure the drawer is now global and the three widgets have real actions**
