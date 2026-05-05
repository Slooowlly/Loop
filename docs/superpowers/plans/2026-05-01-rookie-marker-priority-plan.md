# Rookie Marker Priority Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure life-debut rookies show only `🌱` in driver standings, while category-debut-only drivers show `⭐`.

**Architecture:** Keep backend rookie flags unchanged so the standings API still exposes both life-debut and category-debut context. Apply the display priority in the frontend marker renderer so `🌱` suppresses the redundant `⭐` only when both flags are true.

**Tech Stack:** React frontend, Vitest/jsdom tests.

---

## File Structure

- Modify `src/pages/tabs/StandingsTab.jsx`
  - Update marker rendering priority so `is_estreante_da_vida` hides the category star.
- Modify `src/pages/tabs/StandingsTab.test.jsx`
  - Lock the new display rule with a focused UI test.
- Modify `docs/superpowers/specs/2026-04-29-rookie-star-standings-design.md`
  - Sync the documented marker semantics with the new priority.

## Chunk 1: Marker Priority

### Task 1: Hide Category Star For Life Debuts

**Files:**
- Modify: `src/pages/tabs/StandingsTab.test.jsx`
- Modify: `src/pages/tabs/StandingsTab.jsx`

- [ ] **Step 1: Write the failing test**

Update the existing driver marker test so a driver with both `is_estreante_da_vida` and `is_estreante` renders `🌱` but not `⭐`.

- [ ] **Step 2: Run the focused test and confirm it fails**

Run:

```powershell
npm test -- StandingsTab -t "distinguishes career debut from category debut"
```

Expected: fail because the UI still renders both markers.

- [ ] **Step 3: Implement the minimal UI change**

Render `⭐` only when `driver.is_estreante` is true and `driver.is_estreante_da_vida` is false.

- [ ] **Step 4: Run focused UI verification**

Run:

```powershell
npm test -- StandingsTab
```

Expected: pass.

## Chunk 2: Docs Sync

### Task 2: Align Spec Text

**Files:**
- Modify: `docs/superpowers/specs/2026-04-29-rookie-star-standings-design.md`

- [ ] **Step 1: Update the marker semantics**

Document:
- `🌱` = estreia na vida, sem `⭐` redundante.
- `⭐` = estreia na categoria apenas quando o piloto não estiver em estreia da vida.
- `🩸` and `🚑` unchanged.
