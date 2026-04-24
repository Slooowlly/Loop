# Historical Career Draft Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** New careers generate a complete historical world from 2000-2024, then let the player enter the generated 2025 season as a rookie N2.

**Architecture:** Add an explicit draft lifecycle around save creation, isolate historical generation in a backend module, and keep playable-only side effects out of the historical path. The frontend wizard becomes a two-phase flow: identity plus world generation, then category/team selection from the generated 2025 database.

**Tech Stack:** Rust/Tauri backend, SQLite via rusqlite, React frontend, Zustand store, Vitest/jsdom-style frontend tests, Rust unit tests.

---

## Reference Spec

- `docs/superpowers/specs/2026-04-24-historical-career-draft-design.md`

## File Structure

- Create `src-tauri/src/commands/historical_draft.rs`
  - Owns draft creation, lookup, discard, finalization, and historical simulation orchestration.
- Modify `src-tauri/src/commands/mod.rs`
  - Exposes the new command module.
- Modify `src-tauri/src/commands/career_commands.rs`
  - Adds Tauri command wrappers for draft APIs.
- Modify `src-tauri/src/lib.rs`
  - Registers new Tauri commands.
- Modify `src-tauri/src/commands/career_types.rs`
  - Adds draft request/response types and lifecycle enums.
- Modify `src-tauri/src/config/app_config.rs`
  - Adds lifecycle metadata fields and filters draft saves from normal save listing.
- Modify `src-tauri/src/commands/career.rs`
  - Reuses/adjusts save helpers where needed; keeps current active-save creation intact until frontend migration.
- Modify `src-tauri/src/evolution/pipeline.rs`
  - Adds historical end-of-season options if playable side effects need to be skipped.
- Modify `src-tauri/src/evolution/season_transition.rs`
  - Adds team-season archive support if needed.
- Modify `src-tauri/src/db/migrations.rs`
  - Adds migration for team-season archive and lifecycle-related schema support if schema tables need it.
- Modify `src/pages/NewCareer.jsx`
  - Reworks wizard flow around draft generation and finalization.
- Modify `src/utils/constants.js`
  - Removes static team-preview dependency from post-generation team selection where possible.
- Modify `src/stores/useCareerStore.js`
  - Adds draft lifecycle helpers if shared state is useful.
- Add/modify tests in:
  - `src-tauri/src/commands/historical_draft.rs`
  - `src-tauri/src/config/app_config.rs`
  - `src-tauri/src/db/migrations.rs`
  - `src/pages/LoadSave.test.jsx`
  - `src/pages/NewCareer.test.jsx` if missing, create it.

## Chunk 1: Lifecycle Metadata And Save Listing

### Task 1: Add Lifecycle Types

**Files:**
- Modify: `src-tauri/src/commands/career_types.rs`
- Modify: `src-tauri/src/config/app_config.rs`

- [ ] **Step 1: Write tests for legacy active behavior**

Add or extend tests in `src-tauri/src/config/app_config.rs`:

```rust
#[test]
fn list_saves_treats_missing_lifecycle_as_active() {
    // Seed a meta.json without lifecycle_status.
    // Assert list_saves returns it.
}
```

- [ ] **Step 2: Write tests for draft filtering**

```rust
#[test]
fn list_saves_excludes_draft_and_failed_saves() {
    // Seed active, draft, and failed meta.json files.
    // Assert only active appears in AppConfig::list_saves().
}
```

- [ ] **Step 3: Run tests and confirm they fail**

Run:

```powershell
cd src-tauri
cargo test config::app_config::tests::list_saves_treats_missing_lifecycle_as_active config::app_config::tests::list_saves_excludes_draft_and_failed_saves
```

Expected: fail because lifecycle fields/filtering do not exist.

- [ ] **Step 4: Add lifecycle metadata**

Add in `career_types.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveLifecycleStatus {
    Draft,
    Failed,
    Active,
}

impl Default for SaveLifecycleStatus {
    fn default() -> Self {
        Self::Active
    }
}
```

Add to `SaveMeta` in `config/app_config.rs`:

```rust
#[serde(default)]
pub lifecycle_status: crate::commands::career_types::SaveLifecycleStatus,
#[serde(default)]
pub history_start_year: Option<u32>,
#[serde(default)]
pub history_end_year: Option<u32>,
#[serde(default)]
pub playable_start_year: Option<u32>,
#[serde(default)]
pub draft_progress_year: Option<u32>,
#[serde(default)]
pub draft_error: Option<String>,
#[serde(default)]
pub pending_player_nationality: Option<String>,
#[serde(default)]
pub pending_player_age: Option<i32>,
```

- [ ] **Step 5: Filter normal save listing**

In `AppConfig::list_saves`, only push metas with `lifecycle_status == SaveLifecycleStatus::Active`.

- [ ] **Step 6: Run focused tests**

Run:

```powershell
cd src-tauri
cargo test config::app_config
```

Expected: pass.

- [ ] **Step 7: Commit**

```powershell
git add src-tauri/src/commands/career_types.rs src-tauri/src/config/app_config.rs
git commit -m "feat: add save lifecycle metadata"
```

## Chunk 2: Draft Command Surface

### Task 2: Add Draft API Types And Command Wrappers

**Files:**
- Create: `src-tauri/src/commands/historical_draft.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/commands/career_commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/career_types.rs`

- [ ] **Step 1: Add API types**

In `career_types.rs`, add:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateHistoricalDraftInput {
    pub player_name: String,
    pub player_nationality: String,
    pub player_age: Option<i32>,
    pub difficulty: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FinalizeHistoricalDraftInput {
    pub career_id: String,
    pub category: String,
    pub team_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DraftTeamOption {
    pub id: String,
    pub nome: String,
    pub nome_curto: String,
    pub categoria: String,
    pub cor_primaria: String,
    pub cor_secundaria: String,
    pub car_performance: f64,
    pub reputacao: f64,
    pub n1_nome: Option<String>,
    pub n2_nome: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CareerDraftState {
    pub exists: bool,
    pub career_id: Option<String>,
    pub lifecycle_status: SaveLifecycleStatus,
    pub progress_year: Option<u32>,
    pub error: Option<String>,
    pub categories: Vec<String>,
    pub teams: Vec<DraftTeamOption>,
}
```

- [ ] **Step 2: Stub backend functions**

Create `historical_draft.rs` with stubs:

```rust
pub(crate) fn create_historical_career_draft_in_base_dir(
    base_dir: &std::path::Path,
    input: CreateHistoricalDraftInput,
) -> Result<CareerDraftState, String> {
    todo!("implemented in later tasks")
}
```

Add matching stubs for get, discard, and finalize.

- [ ] **Step 3: Add Tauri wrappers**

In `career_commands.rs`, add async/sync wrappers using `app_data_dir`.

- [ ] **Step 4: Register module and commands**

Update `commands/mod.rs` and `lib.rs`.

- [ ] **Step 5: Run compile check**

Run:

```powershell
cd src-tauri
cargo check
```

Expected: pass after stubs compile.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/commands src-tauri/src/lib.rs
git commit -m "feat: add historical draft command surface"
```

## Chunk 3: Historical World Generation Without Player

### Task 3: Generate A Playerless Historical Draft

**Files:**
- Modify: `src-tauri/src/generators/world.rs`
- Modify: `src-tauri/src/commands/historical_draft.rs`
- Test: `src-tauri/src/commands/historical_draft.rs`

- [ ] **Step 1: Write failing test for draft base world**

In `historical_draft.rs` tests:

```rust
#[test]
fn create_draft_base_world_has_no_player_and_starts_in_2000() {
    let base_dir = unique_test_dir("draft_base_world");
    let input = sample_draft_input();
    let state = create_historical_career_draft_base_for_test(&base_dir, input)
        .expect("draft base should be created");

    assert_eq!(state.lifecycle_status, SaveLifecycleStatus::Draft);
    let db = open_draft_db(&base_dir, state.career_id.as_deref().unwrap());
    assert!(driver_queries::get_player_driver(&db.conn).is_err());
    let season = season_queries::get_active_season(&db.conn).unwrap().unwrap();
    assert_eq!(season.ano, 2000);
}
```

- [ ] **Step 2: Run test and confirm it fails**

Run:

```powershell
cd src-tauri
cargo test commands::historical_draft::tests::create_draft_base_world_has_no_player_and_starts_in_2000
```

- [ ] **Step 3: Add playerless world generator**

Refactor `generate_world_with_rng` so player creation is optional, or add a separate function:

```rust
pub fn generate_historical_world(
    difficulty: &str,
    start_year: i32,
) -> Result<WorldData, String> {
    // No player in drivers.
    // No player_team_id/player_contract requirement.
}
```

If `WorldData` is too player-centric, introduce `GeneratedWorld` with optional player fields while keeping `generate_world` compatible.

- [ ] **Step 4: Persist draft meta**

Create meta with:

- `lifecycle_status = draft`;
- `history_start_year = 2000`;
- `history_end_year = 2024`;
- `playable_start_year = 2025`;
- pending player identity fields;
- current season/year initially `1`/`2000`.

- [ ] **Step 5: Run focused tests**

Run:

```powershell
cd src-tauri
cargo test commands::historical_draft::tests::create_draft_base_world_has_no_player_and_starts_in_2000 generators::world
```

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/generators/world.rs src-tauri/src/commands/historical_draft.rs
git commit -m "feat: create playerless historical draft world"
```

## Chunk 4: Silent Historical Simulation

### Task 4: Simulate 2000-2024 Into A 2025 Draft

**Files:**
- Modify: `src-tauri/src/commands/historical_draft.rs`
- Modify: `src-tauri/src/evolution/pipeline.rs`
- Modify: `src-tauri/src/evolution/season_transition.rs`
- Modify: `src-tauri/src/db/migrations.rs` if team archive is added now.

- [ ] **Step 1: Write reduced-range test first**

Use a test-only helper to simulate a shorter range, such as 2000-2001, to keep tests fast:

```rust
#[test]
fn historical_simulation_reaches_playable_year_with_results_and_no_news() {
    let base_dir = unique_test_dir("historical_short");
    let state = create_historical_career_draft_for_range_for_test(&base_dir, sample_draft_input(), 2000, 2001, 2002)
        .expect("historical generation should finish");

    assert_eq!(state.lifecycle_status, SaveLifecycleStatus::Draft);
    let db = open_draft_db(&base_dir, state.career_id.as_deref().unwrap());
    let season = season_queries::get_active_season(&db.conn).unwrap().unwrap();
    assert_eq!(season.ano, 2002);

    let result_count: i64 = db.conn.query_row("SELECT COUNT(*) FROM race_results", [], |row| row.get(0)).unwrap();
    assert!(result_count > 0);

    let news_count: i64 = db.conn.query_row("SELECT COUNT(*) FROM news", [], |row| row.get(0)).unwrap();
    assert_eq!(news_count, 0);
}
```

- [ ] **Step 2: Run test and confirm it fails**

Run:

```powershell
cd src-tauri
cargo test commands::historical_draft::tests::historical_simulation_reaches_playable_year_with_results_and_no_news
```

- [ ] **Step 3: Implement historical season loop**

In `historical_draft.rs`, add an internal function:

```rust
fn simulate_historical_range(
    db: &mut Database,
    career_dir: &Path,
    start_year: i32,
    end_year: i32,
    playable_year: i32,
) -> Result<(), String> {
    for year in start_year..=end_year {
        simulate_current_historical_season(db, career_dir, year)?;
        advance_historical_season(db, career_dir)?;
        update_draft_progress(career_dir, year as u32)?;
    }
    assert_active_year(db, playable_year)
}
```

- [ ] **Step 4: Simulate races without news**

Use `crate::commands::race::simulate_category_race(&mut db, &race, false)` for each pending race. Append to `race_results.json` only if current UI needs it.

- [ ] **Step 5: Handle special phases**

Use the same phase transitions as `skip_all_pending_races_in_base_dir`, but without player-specific branches.

- [ ] **Step 6: Add historical end-of-season option if needed**

If `run_end_of_season` creates playable-only side effects, add:

```rust
pub struct EndOfSeasonOptions {
    pub initialize_preseason: bool,
    pub write_resume_context: bool,
}
```

Keep existing `run_end_of_season` behavior by delegating to an options version with playable defaults.

- [ ] **Step 7: Ensure no historical backups/news**

Do not call `advance_season_in_base_dir` for historical years because it creates backups and resume context.

- [ ] **Step 8: Run focused backend tests**

Run:

```powershell
cd src-tauri
cargo test commands::historical_draft evolution::pipeline commands::race::tests::test_simulate_race_weekend_updates_state
```

- [ ] **Step 9: Commit**

```powershell
git add src-tauri/src/commands/historical_draft.rs src-tauri/src/evolution src-tauri/src/db/migrations.rs
git commit -m "feat: simulate historical seasons for draft careers"
```

## Chunk 5: Finalize Draft And Insert Player

### Task 5: Replace Selected Team N2 With Player

**Files:**
- Modify: `src-tauri/src/commands/historical_draft.rs`
- Modify: `src-tauri/src/models/driver.rs` only if a fixed-year player constructor is needed.
- Modify: `src-tauri/src/db/queries/contracts.rs` if a helper for active-contract rescind is missing.
- Modify: `src-tauri/src/db/queries/teams.rs` if a helper for replacing N2 is missing.

- [ ] **Step 1: Write failing finalization test**

```rust
#[test]
fn finalize_draft_inserts_player_as_n2_and_displaces_existing_n2() {
    let base_dir = unique_test_dir("finalize_draft");
    let state = create_historical_career_draft_for_range_for_test(&base_dir, sample_draft_input(), 2000, 2000, 2001)
        .expect("draft");
    let option = first_rookie_team_option(&base_dir, state.career_id.as_deref().unwrap());
    let displaced_n2 = option.n2_id.clone().expect("team should have n2");

    let result = finalize_career_draft_in_base_dir(
        &base_dir,
        FinalizeHistoricalDraftInput {
            career_id: state.career_id.unwrap(),
            category: option.categoria,
            team_id: option.id,
        },
    ).expect("finalize");

    assert!(result.success);
    // Assert player exists, stats zero, team slot points to player.
    // Assert displaced N2 has no active regular contract.
}
```

- [ ] **Step 2: Run test and confirm it fails**

Run:

```powershell
cd src-tauri
cargo test commands::historical_draft::tests::finalize_draft_inserts_player_as_n2_and_displaces_existing_n2
```

- [ ] **Step 3: Implement finalization transaction**

Inside a database transaction:

- load draft meta;
- validate lifecycle is `draft`;
- load selected team and ensure category matches;
- create player with `ano_inicio_carreira = 2025`;
- insert player;
- grant license;
- find current N2;
- rescind N2 active regular contract;
- replace `piloto_2_id` and `hierarquia_n2_id`;
- set `is_player_team`;
- insert player contract as `Numero2`;
- update meta lifecycle to `active`.

- [ ] **Step 4: Keep player stats zero**

Assert/ensure:

```rust
player.stats_temporada == DriverSeasonStats::default();
player.stats_carreira == DriverCareerStats::default();
```

- [ ] **Step 5: Run finalization tests**

Run:

```powershell
cd src-tauri
cargo test commands::historical_draft::tests::finalize_draft
```

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/commands/historical_draft.rs src-tauri/src/models src-tauri/src/db/queries
git commit -m "feat: finalize historical draft with player insertion"
```

## Chunk 6: Frontend Wizard Flow

### Task 6: Split New Career Into Draft Generation And Finalization

**Files:**
- Modify: `src/pages/NewCareer.jsx`
- Modify: `src/utils/constants.js`
- Modify: `src/stores/useCareerStore.js` if needed.
- Test: `src/pages/NewCareer.test.jsx`

- [ ] **Step 1: Write UI test for generation-first flow**

```jsx
it("generates the world before showing category and team selection", async () => {
  // Render NewCareer.
  // Fill identity.
  // Click Generate world.
  // Mock get_career_draft returning categories/teams.
  // Assert category choices appear only after generation.
});
```

- [ ] **Step 2: Write UI test for back navigation preserving draft**

```jsx
it("does not regenerate the draft when navigating back after generation", async () => {
  // Generate once.
  // Go to team, back to category, choose again.
  // Assert create_historical_career_draft was called once.
});
```

- [ ] **Step 3: Run tests and confirm they fail**

Run:

```powershell
npm test -- NewCareer
```

If no matching script exists, use the existing project test command and document it in the implementation notes.

- [ ] **Step 4: Update wizard steps**

Target step sequence:

1. difficulty;
2. player identity;
3. generate world/progress;
4. category selection from draft;
5. team selection from draft;
6. confirmation/finalization.

- [ ] **Step 5: Wire commands**

Use `invoke` for:

- `get_career_draft` on mount;
- `create_historical_career_draft` on generate;
- `discard_career_draft` on discard;
- `finalize_career_draft` on final confirmation.

- [ ] **Step 6: Load active career after finalization**

After finalization:

```js
await loadCareer(result.career_id);
navigate("/dashboard");
```

- [ ] **Step 7: Run frontend tests**

Run:

```powershell
npm test -- NewCareer
npm test -- LoadSave
```

- [ ] **Step 8: Commit**

```powershell
git add src/pages/NewCareer.jsx src/pages/NewCareer.test.jsx src/utils/constants.js src/stores/useCareerStore.js
git commit -m "feat: update new career wizard for historical drafts"
```

## Chunk 7: Full Verification And Cleanup

### Task 7: End-To-End Backend And Frontend Verification

**Files:**
- Modify only files needed to fix issues found by verification.

- [ ] **Step 1: Run Rust focused tests**

Run:

```powershell
cd src-tauri
cargo test commands::historical_draft config::app_config evolution::pipeline
```

Expected: all pass.

- [ ] **Step 2: Run broader Rust tests around save/race/career**

Run:

```powershell
cd src-tauri
cargo test commands::career commands::race commands::save
```

Expected: all pass.

- [ ] **Step 3: Run frontend focused tests**

Run:

```powershell
npm test -- NewCareer LoadSave
```

Expected: all pass.

- [ ] **Step 4: Run project lint/build if available**

Inspect `package.json` scripts, then run the relevant commands. Common candidates:

```powershell
npm run lint
npm run build
```

Expected: no errors.

- [ ] **Step 5: Manual smoke test**

Run the app dev server, create a historical draft, finalize into a team, and load dashboard.

Expected:

- draft generation reaches 2025;
- normal load screen does not show draft before finalization;
- player appears as N2 after finalization;
- displaced N2 is not on the selected team;
- first race remains pending.

- [ ] **Step 6: Commit final fixes**

```powershell
git add .
git commit -m "test: verify historical career draft flow"
```

## Open Implementation Notes

- The exact historical runtime should be measured early. If 25 full years is much slower than expected, optimize the historical orchestration path before changing design scope.
- If `race_results.json` becomes too large or too slow, prefer moving UI reads to SQLite instead of dropping historical facts.
- If the current market preseason cannot run without a player, keep a historical AI-only vacancy filling path in `historical_draft.rs` and document the limitation.
- Do not generate historical news as a shortcut for future history UI. Preserve facts first.

