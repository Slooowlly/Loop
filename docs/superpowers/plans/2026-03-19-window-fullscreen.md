# Window Fullscreen Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Tauri app open in native fullscreen so the Windows taskbar no longer blocks the app viewport.

**Architecture:** Keep the change focused on the existing single-window Tauri setup. Drive the fullscreen behavior from `src-tauri/tauri.conf.json` and update the React window controls so the UI no longer advertises maximize/restore in a permanently fullscreen app.

**Tech Stack:** Tauri v2, React, Vite, JSON configuration, built-in app build verification

---

### Task 1: Add a failing fullscreen contract test

**Files:**
- Create: `scripts/tests/window-fullscreen-config.test.mjs`
- Verify: `src-tauri/tauri.conf.json`

- [ ] Write a Node test that reads `src-tauri/tauri.conf.json` and expects the main window to use `fullscreen: true` and `maximized: false`.
- [ ] Run `node --test scripts/tests/window-fullscreen-config.test.mjs` and confirm it fails against the current config.

### Task 2: Switch the main window to fullscreen

**Files:**
- Modify: `src-tauri/tauri.conf.json`

- [ ] Update the main window config to enable native fullscreen.
- [ ] Keep the rest of the window configuration unchanged unless required for fullscreen behavior.

### Task 3: Align the custom window controls

**Files:**
- Modify: `src/components/layout/WindowControlsDrawer.jsx`

- [ ] Remove the maximize state sync and maximize button from the drawer.
- [ ] Keep minimize and close working through the existing Tauri commands.

### Task 4: Verify the change stays healthy

**Files:**
- Verify the files above only

- [ ] Run `node --test scripts/tests/window-fullscreen-config.test.mjs`.
- [ ] Run `npm run build`.
- [ ] Review the diff to confirm the app now launches in fullscreen and the UI no longer exposes maximize.
