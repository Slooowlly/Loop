# Entry Visual Unification Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the boot logo screen, splash screen, and main menu feel like one coherent branded entry flow anchored by the new logo.

**Architecture:** Introduce shared entry-surface classes in `src/index.css`, then restyle `BootLogoScreen`, `SplashScreen`, and `MainMenu` to reuse them. Keep routing unchanged and use the same logo asset across the entry flow where it strengthens continuity.

**Tech Stack:** React, Tailwind utility classes, shared CSS in `src/index.css`, Node test runner, Vite build

---

### Task 1: Add a failing contract test for the unified entry flow

**Files:**
- Create: `scripts/tests/entry-visual-unification.test.mjs`
- Test: `src/pages/BootLogoScreen.jsx`
- Test: `src/pages/SplashScreen.jsx`
- Test: `src/pages/MainMenu.jsx`
- Test: `src/index.css`

- [ ] **Step 1: Write the failing test**
- [ ] **Step 2: Run `node --test scripts/tests/entry-visual-unification.test.mjs` and confirm it fails**
- [ ] **Step 3: Cover shared entry classes, logo reuse, and upgraded menu/splash structure**

### Task 2: Add shared entry styling

**Files:**
- Modify: `src/index.css`

- [ ] **Step 1: Add shared classes for the entry background, glow layers, and content shell**
- [ ] **Step 2: Add reusable glass/button treatment that fits the new logo palette**

### Task 3: Restyle the boot, splash, and menu screens

**Files:**
- Modify: `src/pages/BootLogoScreen.jsx`
- Modify: `src/pages/SplashScreen.jsx`
- Modify: `src/pages/MainMenu.jsx`

- [ ] **Step 1: Keep boot minimal but align it to the shared visual system**
- [ ] **Step 2: Bring the logo and CTA hierarchy into the splash**
- [ ] **Step 3: Turn the main menu into a branded glass hub**

### Task 4: Verify the redesigned entry flow

**Files:**
- Verify the files above only

- [ ] **Step 1: Run `node --test scripts/tests/entry-visual-unification.test.mjs scripts/tests/boot-logo-screen.test.mjs scripts/tests/window-controls-navigation.test.mjs scripts/tests/window-controls-widget-panel.test.mjs scripts/tests/window-controls-contract.test.mjs scripts/tests/window-controls-sizing.test.mjs scripts/tests/window-controls-hover-zone.test.mjs`**
- [ ] **Step 2: Run `npm run build`**
- [ ] **Step 3: Review the resulting entry flow sources for visual consistency**
