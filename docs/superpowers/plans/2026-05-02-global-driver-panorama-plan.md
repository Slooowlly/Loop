# Global Driver Panorama Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a hidden global driver panorama opened by double-clicking a driver in standings, showing all active, free, and retired drivers with a balanced historical ranking.

**Architecture:** Add a focused Tauri backend command that returns a prepared ranking payload, including raw stats, weighted score, ranks, status, team/category context, and injuries. Add a hidden React tab rendered by `Dashboard`, and let `StandingsTab` signal single-click versus double-click so the existing driver modal remains intact.

**Tech Stack:** Rust/Tauri commands, rusqlite, serde payload structs, React 18, Vitest, Testing Library, Tailwind utility classes.

---

## File Structure

- Create `src-tauri/src/commands/global_driver_rankings.rs`
  - Owns category multipliers, historical score calculation, archived season aggregation, retired snapshot merge, rank assignment, and `get_global_driver_rankings_in_base_dir`.
- Modify `src-tauri/src/commands/mod.rs`
  - Exports the new backend module.
- Modify `src-tauri/src/commands/career_types.rs`
  - Adds serializable payload structs for the global panorama.
- Modify `src-tauri/src/commands/career_commands.rs`
  - Adds the public Tauri command wrapper.
- Modify `src-tauri/src/lib.rs`
  - Registers the new Tauri command in `generate_handler!`.
- Create `src/pages/tabs/GlobalDriversTab.jsx`
  - Hidden tab UI: selected-driver summary, metric leaders, sortable global table, return action.
- Create `src/pages/tabs/GlobalDriversTab.test.jsx`
  - Tests loading, highlighting, opacity for free/retired drivers, and sorting.
- Modify `src/pages/tabs/StandingsTab.jsx`
  - Adds delayed single-click handling and double-click callback for the hidden tab.
- Modify `src/pages/tabs/StandingsTab.test.jsx`
  - Tests single click opens modal and double click opens panorama callback without modal.
- Modify `src/pages/Dashboard.jsx`
  - Holds hidden tab state and renders `GlobalDriversTab` when activated.
- Modify `src/pages/Dashboard.test.jsx`
  - Tests hidden tab render and return to standings.

---

## Chunk 1: Backend Payload And Scoring

### Task 1: Add Payload Structs

**Files:**
- Modify: `src-tauri/src/commands/career_types.rs`

- [ ] **Step 1: Write the failing type-level usage test**

Add a compile-checked test in the new backend module in Task 2 that imports these names before they exist:

```rust
use crate::commands::career_types::{GlobalDriverRankingPayload, GlobalDriverRankingRow};
```

- [ ] **Step 2: Add payload structs**

Append near the driver detail structs:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalDriverRankingPayload {
    pub selected_driver_id: Option<String>,
    pub rows: Vec<GlobalDriverRankingRow>,
    pub leaders: GlobalDriverRankingLeaders,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalDriverRankingLeaders {
    pub historical_index_driver_id: Option<String>,
    pub wins_driver_id: Option<String>,
    pub titles_driver_id: Option<String>,
    pub injuries_driver_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalDriverRankingRow {
    pub id: String,
    pub nome: String,
    pub nacionalidade: String,
    pub idade: i32,
    pub status: String,
    pub status_tone: String,
    pub is_jogador: bool,
    pub equipe_nome: Option<String>,
    pub equipe_cor_primaria: Option<String>,
    pub categoria_atual: Option<String>,
    pub historical_index: f64,
    pub historical_rank: i32,
    pub wins_rank: i32,
    pub titles_rank: i32,
    pub podiums_rank: i32,
    pub injuries_rank: i32,
    pub corridas: i32,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
    pub poles: i32,
    pub titulos: i32,
    pub dnfs: i32,
    pub lesoes: i32,
    pub lesoes_leves: i32,
    pub lesoes_moderadas: i32,
    pub lesoes_graves: i32,
}
```

- [ ] **Step 3: Run backend check**

Run: `cargo test --manifest-path src-tauri/Cargo.toml global_driver_rankings -- --nocapture`

Expected before Task 2 implementation: compile failure or missing module.

### Task 2: Implement Balanced Ranking Backend

**Files:**
- Create: `src-tauri/src/commands/global_driver_rankings.rs`
- Modify: `src-tauri/src/commands/mod.rs`

- [ ] **Step 1: Write backend tests first**

In `global_driver_rankings.rs`, add tests that create an in-memory migrated DB and assert:

```rust
#[test]
fn balanced_index_weights_higher_categories_without_erasing_lower_category_dominance() {
    // GT3 driver with 2 wins should beat rookie driver with 2 wins.
    // Rookie driver with many wins should still be competitive.
}

#[test]
fn payload_includes_active_free_and_retired_drivers_with_dimmed_statuses() {
    // Active contracted -> "Ativo"; active no contract -> "Livre"; retired snapshot -> "Aposentado".
}

#[test]
fn injuries_are_reported_but_do_not_reduce_historical_index() {
    // Same stats with/without injuries have same index; injury count differs.
}
```

- [ ] **Step 2: Run tests to verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml global_driver_rankings -- --nocapture`

Expected: tests fail because the command/score functions do not exist.

- [ ] **Step 3: Implement module skeleton**

Create:

```rust
use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::commands::career::open_career_db;
use crate::commands::career_types::{
    GlobalDriverRankingLeaders, GlobalDriverRankingPayload, GlobalDriverRankingRow,
};
use crate::constants::categories;
use crate::db::queries::{contracts as contract_queries, drivers as driver_queries, injuries as injury_queries};
use crate::models::driver::Driver;
use crate::models::enums::DriverStatus;

pub(crate) fn get_global_driver_rankings_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    selected_driver_id: Option<&str>,
) -> Result<GlobalDriverRankingPayload, String> {
    let db = open_career_db(base_dir, career_id)?;
    build_global_driver_rankings(&db.conn, selected_driver_id)
}
```

If `open_career_db` is not public enough, follow the existing pattern used by `get_driver_detail_in_base_dir`.

- [ ] **Step 4: Implement category weighting**

Use helpers:

```rust
fn category_multiplier(category: &str) -> f64 {
    match category {
        "mazda_rookie" | "toyota_rookie" => 0.75,
        "mazda_amador" | "toyota_amador" => 0.85,
        "bmw_m2" => 0.95,
        "gt4" => 1.08,
        "production_challenger" => 1.12,
        "gt3" => 1.22,
        "endurance" => 1.25,
        _ => 1.0,
    }
}

fn balanced_score(
    category: &str,
    titles: i32,
    wins: i32,
    podiums: i32,
    poles: i32,
    points: f64,
    races: i32,
    dnfs: i32,
) -> f64 {
    let normalized_points = points.max(0.0).sqrt() * 3.0;
    let race_bonus = (races.max(0) as f64).sqrt() * 2.0;
    let base = titles as f64 * 140.0
        + wins as f64 * 34.0
        + podiums as f64 * 13.0
        + poles as f64 * 9.0
        + normalized_points * 0.9
        + race_bonus
        - dnfs.max(0) as f64 * 1.5;
    (base.max(0.0) * category_multiplier(category) * 10.0).round() / 10.0
}
```

- [ ] **Step 5: Aggregate archived seasons by category**

Query `driver_season_archive`:

```sql
SELECT categoria, pontos, snapshot_json
FROM driver_season_archive
WHERE piloto_id = ?1
```

Read `snapshot_json` keys: `pontos`, `vitorias`, `podios`, `poles`, `corridas`, `dnfs`, `titulos` when present. When archive rows do not exist, use the driver's current `stats_carreira` under `driver.categoria_atual.unwrap_or("unknown")`.

- [ ] **Step 6: Include retired snapshots**

Query `retired` if the table exists:

```sql
SELECT piloto_id, nome, categoria_final, estatisticas FROM retired
```

Only add a retired row if its `piloto_id` was not already present in `drivers`. Parse `estatisticas` JSON using the same stat keys.

- [ ] **Step 7: Add rank assignment**

Sort rows by:

- `historical_index` desc, then `titulos` desc, `vitorias` desc, `podios` desc, `nome` asc.
- assign `historical_rank`.
- assign metric ranks independently for wins, titles, podiums, injuries.

- [ ] **Step 8: Export module**

Add to `src-tauri/src/commands/mod.rs`:

```rust
pub mod global_driver_rankings;
```

- [ ] **Step 9: Verify backend tests pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml global_driver_rankings -- --nocapture`

Expected: all new backend tests pass.

### Task 3: Register Tauri Command

**Files:**
- Modify: `src-tauri/src/commands/career_commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add wrapper**

Import `get_global_driver_rankings_in_base_dir` and payload type, then add:

```rust
#[tauri::command]
pub async fn get_global_driver_rankings(
    app: AppHandle,
    career_id: String,
    selected_driver_id: Option<String>,
) -> Result<GlobalDriverRankingPayload, String> {
    let base_dir = app_data_dir(&app)?;
    get_global_driver_rankings_in_base_dir(&base_dir, &career_id, selected_driver_id.as_deref())
}
```

- [ ] **Step 2: Register handler**

Add to `src-tauri/src/lib.rs` `generate_handler!`:

```rust
commands::career_commands::get_global_driver_rankings,
```

- [ ] **Step 3: Verify command compiles**

Run: `cargo test --manifest-path src-tauri/Cargo.toml global_driver_rankings -- --nocapture`

Expected: pass.

---

## Chunk 2: Hidden Global Drivers UI

### Task 4: Build GlobalDriversTab With Tests

**Files:**
- Create: `src/pages/tabs/GlobalDriversTab.jsx`
- Create: `src/pages/tabs/GlobalDriversTab.test.jsx`

- [ ] **Step 1: Write failing UI test**

Mock `invoke("get_global_driver_rankings")` with rows for:

- selected active driver;
- free driver;
- retired driver.

Assert:

```jsx
expect(await screen.findByText(/Panorama global de pilotos/i)).toBeInTheDocument();
expect(screen.getByText(/#2 no Indice Historico/i)).toBeInTheDocument();
expect(screen.getByText("Piloto Livre").closest("tr")).toHaveClass("opacity-60");
expect(screen.getByText("Lenda Aposentada").closest("tr")).toHaveClass("opacity-50");
```

- [ ] **Step 2: Run RED**

Run: `npm run test:ui -- src/pages/tabs/GlobalDriversTab.test.jsx`

Expected: fail because component does not exist.

- [ ] **Step 3: Implement component**

Props:

```jsx
function GlobalDriversTab({ selectedDriverId, onBack }) {}
```

Behavior:

- read `careerId` from `useCareerStore`;
- call `invoke("get_global_driver_rankings", { careerId, selectedDriverId })`;
- keep `rows`, `leaders`, `sort`, `error`, `loading`;
- default sort by `historical_index` descending;
- render button `Voltar para Classificacao`.

- [ ] **Step 4: Render summary and leaders**

Top layout:

- selected driver summary if found;
- cards for `leaders.historical_index_driver_id`, `wins_driver_id`, `titles_driver_id`, `injuries_driver_id`;
- avoid explanatory helper copy.

- [ ] **Step 5: Render table**

Columns:

`#`, `Piloto`, `Status`, `Equipe/Categoria`, `Indice`, `Titulos`, `Vit.`, `Pod.`, `Poles`, `Pts`, `Corr.`, `DNFs`, `Lesoes`.

Row classes:

```jsx
row.id === selectedDriverId ? "bg-accent-primary/12 ring-1 ring-accent-primary/40" : ""
row.status === "Livre" ? "opacity-60" : ""
row.status === "Aposentado" ? "opacity-50" : ""
```

- [ ] **Step 6: Add sorting test**

Click `Vit.` header and assert the row with most wins moves to the top.

- [ ] **Step 7: Verify UI tests**

Run: `npm run test:ui -- src/pages/tabs/GlobalDriversTab.test.jsx`

Expected: pass.

---

## Chunk 3: Dashboard Routing And Click Semantics

### Task 5: Wire Hidden Tab In Dashboard

**Files:**
- Modify: `src/pages/Dashboard.jsx`
- Modify: `src/pages/Dashboard.test.jsx`

- [ ] **Step 1: Write failing Dashboard test**

Mock `StandingsTab` so it calls `onOpenGlobalDrivers("D001")`. Mock `GlobalDriversTab` to show selected id and expose back button.

Assert:

```jsx
fireEvent.click(screen.getByText("Abrir panorama"));
expect(screen.getByText("Panorama D001")).toBeInTheDocument();
expect(screen.getByTestId("main-layout")).toHaveAttribute("data-active-tab", "global-drivers");
fireEvent.click(screen.getByText("Voltar"));
expect(screen.getByTestId("main-layout")).toHaveAttribute("data-active-tab", "standings");
```

- [ ] **Step 2: Run RED**

Run: `npm run test:ui -- src/pages/Dashboard.test.jsx`

Expected: fail because Dashboard does not pass callback/render hidden tab.

- [ ] **Step 3: Implement hidden state**

In `Dashboard.jsx`:

```jsx
const [activeTab, setActiveTab] = useState("standings");
const [globalDriversSelectedId, setGlobalDriversSelectedId] = useState(null);

function openGlobalDrivers(driverId) {
  setGlobalDriversSelectedId(driverId);
  setActiveTab("global-drivers");
}
```

Render:

```jsx
case "global-drivers":
  return (
    <GlobalDriversTab
      selectedDriverId={globalDriversSelectedId}
      onBack={() => setActiveTab("standings")}
    />
  );
case "standings":
  return <StandingsTab onOpenGlobalDrivers={openGlobalDrivers} />;
```

- [ ] **Step 4: Verify Dashboard test**

Run: `npm run test:ui -- src/pages/Dashboard.test.jsx`

Expected: pass.

### Task 6: Single Click Opens Modal, Double Click Opens Hidden Tab

**Files:**
- Modify: `src/pages/tabs/StandingsTab.jsx`
- Modify: `src/pages/tabs/StandingsTab.test.jsx`

- [ ] **Step 1: Write failing tests**

Add tests using fake timers:

```jsx
vi.useFakeTimers();
fireEvent.click(screen.getByText("Alex Stone"));
await vi.advanceTimersByTimeAsync(221);
expect(screen.getByRole("dialog", { name: /Alex Stone/i })).toBeInTheDocument();
```

For double click:

```jsx
const onOpenGlobalDrivers = vi.fn();
render(<StandingsTab onOpenGlobalDrivers={onOpenGlobalDrivers} />);
fireEvent.doubleClick(screen.getByText("Alex Stone"));
await vi.advanceTimersByTimeAsync(221);
expect(onOpenGlobalDrivers).toHaveBeenCalledWith("D001");
expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
```

- [ ] **Step 2: Run RED**

Run: `npm run test:ui -- src/pages/tabs/StandingsTab.test.jsx`

Expected: double-click test fails and/or modal opens immediately.

- [ ] **Step 3: Implement click timer**

At top of `StandingsTab`:

```jsx
const DRIVER_CLICK_DELAY_MS = 220;
```

Inside component:

```jsx
const driverClickTimeoutRef = useRef(null);

function clearDriverClickTimeout() {
  if (driverClickTimeoutRef.current) {
    clearTimeout(driverClickTimeoutRef.current);
    driverClickTimeoutRef.current = null;
  }
}

function handleDriverClick(driverId) {
  clearDriverClickTimeout();
  driverClickTimeoutRef.current = setTimeout(() => {
    setSelectedDriverId((prev) => (prev === driverId ? null : driverId));
    driverClickTimeoutRef.current = null;
  }, DRIVER_CLICK_DELAY_MS);
}

function handleDriverDoubleClick(driverId) {
  clearDriverClickTimeout();
  setSelectedDriverId(null);
  onOpenGlobalDrivers?.(driverId);
}
```

Clean up timeout in `useEffect(() => clearDriverClickTimeout, [])`.

- [ ] **Step 4: Wire row events**

Replace direct `onClick` on driver rows with `handleDriverClick(driver.id)`, and add:

```jsx
onDoubleClick={() => handleDriverDoubleClick(driver.id)}
```

Keyboard Enter/Space should keep opening the modal immediately or through `handleDriverClick`; do not open panorama from keyboard unless a future explicit shortcut is requested.

- [ ] **Step 5: Verify standings tests**

Run: `npm run test:ui -- src/pages/tabs/StandingsTab.test.jsx`

Expected: pass.

---

## Chunk 4: Full Verification

### Task 7: Run Complete Checks

**Files:**
- No code changes expected.

- [ ] **Step 1: Run frontend tests**

Run: `npm run test:ui`

Expected: all Vitest tests pass.

- [ ] **Step 2: Run backend tests relevant to command**

Run: `cargo test --manifest-path src-tauri/Cargo.toml global_driver_rankings -- --nocapture`

Expected: all global ranking tests pass.

- [ ] **Step 3: Run full build**

Run: `npm run build`

Expected: Vite build exits 0. Existing chunk-size warnings are acceptable if unrelated.

- [ ] **Step 4: Manual smoke test**

Start app/dev server through the repo's usual workflow. In dashboard:

- single-click a driver name: ficha opens;
- close ficha;
- double-click a driver name: hidden panorama opens;
- selected driver row is highlighted;
- free/retired rows are dimmed when present;
- click `Voltar para Classificacao`: standings returns.

- [ ] **Step 5: Commit implementation**

Stage only files touched for this feature:

```bash
git add src-tauri/src/commands/global_driver_rankings.rs \
  src-tauri/src/commands/mod.rs \
  src-tauri/src/commands/career_types.rs \
  src-tauri/src/commands/career_commands.rs \
  src-tauri/src/lib.rs \
  src/pages/tabs/GlobalDriversTab.jsx \
  src/pages/tabs/GlobalDriversTab.test.jsx \
  src/pages/tabs/StandingsTab.jsx \
  src/pages/tabs/StandingsTab.test.jsx \
  src/pages/Dashboard.jsx \
  src/pages/Dashboard.test.jsx
git commit -m "feat: add hidden global driver panorama"
```
