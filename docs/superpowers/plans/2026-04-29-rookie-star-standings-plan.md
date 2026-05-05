# Driver Status Markers Standings Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show `⭐`, `🩸`, and `🚑` status markers beside drivers in the driver standings table.

**Architecture:** The backend owns driver status decisions by exposing `is_estreante` and active injury type on `DriverSummary`. The frontend stays presentational and maps those fields to compact emoji markers.

**Tech Stack:** Rust/Tauri backend, React frontend, Vitest/jsdom frontend tests, Rust unit tests.

---

## File Structure

- Modify `src-tauri/src/commands/career_types.rs`
  - Add `is_estreante: bool` and `lesao_ativa_tipo: Option<String>` to `DriverSummary`.
- Modify `src-tauri/src/db/queries/injuries.rs`
  - Add a lookup for active injury type by driver IDs.
- Modify `src-tauri/src/commands/career.rs`
  - Populate `is_estreante` from `driver.stats_carreira.temporadas == 0` in normal and special driver standings.
  - Populate `lesao_ativa_tipo` from active injuries.
- Modify `src/pages/tabs/StandingsTab.jsx`
  - Render status marker spans beside the driver name.
- Modify `src/pages/tabs/StandingsTab.test.jsx`
  - Cover first-season star, blood drop, and ambulance markers.

## Chunk 1: Backend Flag

### Task 1: Expose Rookie And Injury Status In Driver Standings

**Files:**
- Modify: `src-tauri/src/commands/career_types.rs`
- Modify: `src-tauri/src/db/queries/injuries.rs`
- Modify: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Write the failing backend test**

Add or extend a focused `get_drivers_by_category` test so one driver has `stats_carreira.temporadas = 0`, another has `stats_carreira.temporadas = 1`, and active injuries exist for two drivers. Assert the first-season driver returns `is_estreante = true`, the veteran returns `false`, and active injury types are exposed.

- [ ] **Step 2: Run the focused backend test**

Run:

```powershell
cd src-tauri
cargo test commands::career::tests::test_get_drivers_by_category_marks_rookies
```

Expected: fail because the backend does not expose the new status shape yet.

- [ ] **Step 3: Implement the minimal backend change**

Add fields to `DriverSummary` and populate them in both normal and special standings with:

```rust
is_estreante: driver.stats_carreira.temporadas == 0,
lesao_ativa_tipo: active_injuries_by_driver.get(&driver.id).cloned(),
```

- [ ] **Step 4: Run the backend test again**

Run:

```powershell
cd src-tauri
cargo test commands::career::tests::test_get_drivers_by_category_marks_rookies
```

Expected: pass.

## Chunk 2: Frontend Marker

### Task 2: Render Status Markers In Driver Standings

**Files:**
- Modify: `src/pages/tabs/StandingsTab.jsx`
- Modify: `src/pages/tabs/StandingsTab.test.jsx`

- [ ] **Step 1: Write the failing frontend test**

Add tests in `StandingsTab.test.jsx` that assert:
- `⭐` appears for `is_estreante`.
- `🩸` appears for light/moderate active injuries.
- `🚑` appears for grave/critical active injuries.

- [ ] **Step 2: Run the focused frontend test**

Run:

```powershell
npm test -- StandingsTab -t "shows rookie star beside rookie drivers"
```

Expected: fail because the new marker mapping is not rendered yet.

- [ ] **Step 3: Implement the minimal frontend change**

In the driver-name cell, render marker spans after the driver name:

```jsx
<DriverStatusMarkers driver={driver} />
```

- [ ] **Step 4: Run the frontend test again**

Run:

```powershell
npm test -- StandingsTab -t "shows rookie star beside rookie drivers"
```

Expected: pass.

## Chunk 3: Verification

### Task 3: Run Focused Checks

**Files:**
- Modify only if verification finds issues.

- [ ] **Step 1: Run backend focused checks**

```powershell
cd src-tauri
cargo test commands::career::tests::test_get_drivers_by_category_marks_rookies
```

- [ ] **Step 2: Run frontend focused checks**

```powershell
npm test -- StandingsTab
```

- [ ] **Step 3: Review diff**

```powershell
git diff -- src-tauri/src/commands/career_types.rs src-tauri/src/commands/career.rs src/pages/tabs/StandingsTab.jsx src/pages/tabs/StandingsTab.test.jsx
```
