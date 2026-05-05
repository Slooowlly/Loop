# Complete Historical World Contract Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every new career start from a complete historical world whose data fully supports driver dossiers, team dossiers, and the global driver ranking.

**Architecture:** Keep the existing historical draft flow as the canonical new-career path, add an explicit world-integrity audit, add team-season snapshots, and remove normal UI usage of the legacy quick `create_career` path. Preserve backwards compatibility for old saves while making new drafts fail fast if required history is missing.

**Review additions:** This plan includes explicit minimal audit seed data, `meta` ID sync validation, failed-draft cleanup, a named race-results sufficiency rule, idempotent team archiving, legacy fallback posture for saves without `team_season_archive`, real generation progress, identity-change draft discard, and the decision that audit is an inline backend gate implemented as a separate testable module.

**Tech Stack:** Rust/Tauri, SQLite via rusqlite, React/Vitest, existing career generation/evolution pipelines.

---

## File Structure

- Modify: `src-tauri/src/commands/historical_draft.rs`
  - Run integrity audit after historical generation.
  - Expose only audited categories/teams.
  - Block finalization for invalid drafts.
  - Mark invalid drafts as failed and clean generated DB/sidecars while preserving `meta.json` error state.
- Create: `src-tauri/src/world/mod.rs`
  - New module boundary for world-level validation/archive helpers.
- Create: `src-tauri/src/world/integrity.rs`
  - Structured audit of draft completeness.
  - Named constants for minimum historical data sufficiency.
- Create: `src-tauri/src/world/team_archive.rs`
  - Team-season snapshot persistence and queries.
- Modify: `src-tauri/src/lib.rs`
  - Register `world` module if needed.
- Modify: `src-tauri/src/db/migrations.rs`
  - Add `team_season_archive`.
- Modify: `src-tauri/src/evolution/season_transition.rs`
  - Archive team season snapshots alongside driver snapshots.
- Modify: `src-tauri/src/evolution/pipeline.rs`
  - Call team archive in regular and historical end-of-season paths.
- Modify: `src-tauri/src/commands/career.rs`
  - Prefer `team_season_archive` where useful in team dossier.
  - Keep `create_career_in_base_dir` available for tests/legacy only.
- Modify: `src/pages/NewCareer.jsx`
  - Ensure only historical draft flow is exposed.
  - Remove or hide quick create path.
  - Show real draft progress and discard stale draft when identity changes.
- Test: `src-tauri/src/commands/historical_draft.rs`
- Test: `src-tauri/src/world/integrity.rs`
- Test: `src-tauri/src/world/team_archive.rs`
- Test: `src-tauri/src/evolution/season_transition.rs`
- Test: `src/pages/NewCareer.test.jsx`

## Chunk 1: Integrity Audit Contract

### Task 1: Create World Integrity Types

**Files:**
- Create: `src-tauri/src/world/mod.rs`
- Create: `src-tauri/src/world/integrity.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing test for a valid generated draft audit**

Add a test in `src-tauri/src/world/integrity.rs`:

```rust
#[test]
fn audit_accepts_complete_historical_draft_shape() {
    let conn = setup_integrity_conn();
    seed_complete_minimal_world(&conn);

    let report = audit_historical_world(&conn, 2025).expect("audit should run");

    assert!(report.is_valid(), "{report:?}");
    assert!(report.errors.is_empty());
}
```

- [ ] **Step 1a: Define the `seed_complete_minimal_world` contract before implementing it**

The test helper must use real migrations and seed a complete minimum shape:

- `meta` rows for current season/year and all next-ID counters;
- one completed historical season before `playable_year`;
- one active playable season;
- pending calendar for the playable season;
- one active regular team with `piloto_1_id` and `piloto_2_id`;
- two active drivers with category/license-compatible data;
- regular active contracts linking both drivers to the team;
- historical `race_results` before `playable_year`;
- `driver_season_archive` for any veteran with results;
- valid `retired` rows only in tests that need retired coverage.

Do not let the helper silently omit a domain that the audit is supposed to protect.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml world::integrity::tests::audit_accepts_complete_historical_draft_shape -- --nocapture`

Expected: FAIL because module/function does not exist.

- [ ] **Step 3: Implement minimal audit structs**

Create:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldAuditSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldAuditIssue {
    pub code: String,
    pub message: String,
    pub severity: WorldAuditSeverity,
}

#[derive(Debug, Clone, Default)]
pub struct WorldAuditReport {
    pub errors: Vec<WorldAuditIssue>,
    pub warnings: Vec<WorldAuditIssue>,
}

impl WorldAuditReport {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}
```

Add `pub fn audit_historical_world(conn: &Connection, playable_year: i32) -> Result<WorldAuditReport, String>`.

Add named sufficiency constants:

```rust
pub const MIN_HISTORICAL_RESULT_SEASONS: i64 = 1;
pub const MIN_HISTORICAL_RESULTS_PER_EXISTING_CATEGORY: i64 = 1;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml world::integrity::tests::audit_accepts_complete_historical_draft_shape -- --nocapture`

Expected: PASS.

### Task 2: Add Blocking Integrity Rules

**Files:**
- Modify: `src-tauri/src/world/integrity.rs`

- [ ] **Step 1: Write failing tests for blocking gaps**

Add tests:

```rust
#[test]
fn audit_rejects_player_before_finalization() { /* seed is_jogador = 1 */ }

#[test]
fn audit_rejects_missing_historical_race_results() { /* delete race_results */ }

#[test]
fn audit_rejects_veteran_without_driver_archive() { /* delete driver_season_archive */ }

#[test]
fn audit_rejects_active_team_without_two_drivers() { /* null piloto_2_id */ }

#[test]
fn audit_rejects_retired_snapshot_without_retirement_year() { /* retired temporada_aposentadoria = '' */ }

#[test]
fn audit_rejects_stale_meta_next_ids() { /* set next_driver_id below max P id */ }

#[test]
fn audit_uses_named_historical_result_sufficiency_rule() { /* remove enough race_results to cross const threshold */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml world::integrity -- --nocapture`

Expected: FAIL with missing validations.

- [ ] **Step 3: Implement focused SQL checks**

Audit must check:

- active season year equals playable year;
- no `drivers.is_jogador = 1`;
- pending calendar exists for active season;
- historical `race_results` exist before playable year;
- historical `race_results` satisfy the named sufficiency constants, without inline magic numbers;
- historical `driver_season_archive` exists;
- active regular contracts point to existing teams/drivers;
- active regular contracted teams have N1/N2;
- active regular drivers have license/category;
- `retired` rows have `temporada_aposentadoria`, `categoria_final`, `estatisticas`;
- no retired driver has active regular contract.
- `meta` next-ID counters are greater than every canonical ID already used for drivers, teams, seasons, races/calendar, and contracts.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml world::integrity -- --nocapture`

Expected: all integrity tests PASS.

## Chunk 2: Team Season Archive

### Task 3: Add Schema

**Files:**
- Modify: `src-tauri/src/db/migrations.rs`

- [ ] **Step 1: Write failing migration test**

Add a test asserting `team_season_archive` exists after migrations with required columns.

- [ ] **Step 2: Run migration test**

Run: `cargo test --manifest-path src-tauri/Cargo.toml db::migrations::tests::test_team_season_archive_schema -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Add table and indexes**

Add:

```sql
CREATE TABLE IF NOT EXISTS team_season_archive (
    team_id TEXT NOT NULL,
    season_number INTEGER NOT NULL,
    ano INTEGER NOT NULL,
    categoria TEXT NOT NULL,
    classe TEXT,
    posicao_campeonato INTEGER,
    pontos REAL NOT NULL DEFAULT 0.0,
    vitorias INTEGER NOT NULL DEFAULT 0,
    podios INTEGER NOT NULL DEFAULT 0,
    poles INTEGER NOT NULL DEFAULT 0,
    corridas INTEGER NOT NULL DEFAULT 0,
    titulos_construtores INTEGER NOT NULL DEFAULT 0,
    piloto_1_id TEXT,
    piloto_2_id TEXT,
    snapshot_json TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (team_id, season_number, categoria)
);
CREATE INDEX IF NOT EXISTS idx_team_season_archive_team
    ON team_season_archive(team_id);
CREATE INDEX IF NOT EXISTS idx_team_season_archive_season
    ON team_season_archive(season_number, categoria);
```

- [ ] **Step 4: Run migration test**

Run: `cargo test --manifest-path src-tauri/Cargo.toml db::migrations::tests::test_team_season_archive_schema -- --nocapture`

Expected: PASS.

### Task 4: Persist Team Season Snapshots

**Files:**
- Create: `src-tauri/src/world/team_archive.rs`
- Modify: `src-tauri/src/evolution/season_transition.rs`
- Modify: `src-tauri/src/evolution/pipeline.rs`

- [ ] **Step 1: Write failing tests for team archive persistence and idempotency**

Create a test that seeds teams, standings, and race results, calls `archive_team_season`, then asserts rows exist with points/wins/podiums/title.

Add a second test:

```rust
#[test]
fn archive_team_season_is_idempotent_for_same_team_season_category() {
    let conn = setup_team_archive_conn();
    let season = seed_completed_team_archive_world(&conn);

    archive_team_season(&conn, &season).expect("first archive");
    archive_team_season(&conn, &season).expect("second archive");

    assert_eq!(count_team_archive_rows(&conn), 1);
    assert_eq!(read_team_archive_points(&conn, "T001", season.numero, "mazda_rookie"), 100.0);
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test --manifest-path src-tauri/Cargo.toml world::team_archive -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Implement `archive_team_season`**

Function signature:

```rust
pub(crate) fn archive_team_season(
    conn: &Connection,
    season: &Season,
) -> Result<(), String>
```

It should aggregate from `race_results` joined to `calendar` and `standings`, then upsert snapshots by `(team_id, season_number, categoria)`. Re-running the archive for the same season must replace the snapshot, not duplicate rows or add stats twice.

- [ ] **Step 4: Call archive from season pipeline**

Call before team season stats reset and after standings are final.

- [ ] **Step 5: Run tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml world::team_archive -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml evolution::pipeline -- --nocapture
```

Expected: PASS.

## Chunk 3: Historical Draft As The Only New Career Path

### Task 5: Audit Draft Before Exposure And Finalization

**Files:**
- Modify: `src-tauri/src/commands/historical_draft.rs`

- [ ] **Step 1: Write failing test for audited draft**

Add test:

```rust
#[test]
fn historical_draft_runs_world_integrity_before_returning_choices() {
    // create a short-range draft, corrupt required data, assert state is failed or returns error
}

#[test]
fn failed_historical_draft_cleans_generated_data_but_preserves_error_meta() {
    // force audit failure, assert meta.json remains with failed/error and career.db/sidecars are removed
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test --manifest-path src-tauri/Cargo.toml historical_draft -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Call audit after `simulate_historical_range`**

If `report.is_valid()` is false:

- write `lifecycle_status = "failed"`;
- write `draft_error` with concise error summary;
- remove generated DB, backups, and generated sidecars for the failed draft while preserving `meta.json`;
- return error.

- [ ] **Step 4: Implement a single failed-draft helper**

Add a helper such as `mark_historical_draft_failed(career_dir, message)` so simulation errors and audit errors use the same cleanup path. It must be safe to call more than once.

- [ ] **Step 5: Block finalization when audit fails**

Run audit inside `finalize_career_draft` before player insertion.

- [ ] **Step 6: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml historical_draft -- --nocapture`

Expected: PASS.

### Task 6: Historical-Only UI Flow, Progress, And Draft Identity

**Files:**
- Modify: `src/pages/NewCareer.jsx`
- Modify: `src/pages/NewCareer.test.jsx`

- [ ] **Step 1: Write failing frontend tests**

Assert no UI path calls `create_career`:

```js
it("uses only historical draft flow for new careers", async () => {
  render(<NewCareer />);
  fireEvent.click(screen.getByRole("button", { name: /gerar hist.rico/i }));
  expect(invoke).toHaveBeenCalledWith("create_historical_career_draft", expect.anything());
  expect(invoke).not.toHaveBeenCalledWith("create_career", expect.anything());
});
```

Add tests:

```js
it("polls draft progress while historical generation is running", async () => {
  // keep create_historical_career_draft pending, make get_career_draft return progress_year updates,
  // assert the UI renders the latest year.
});

it("discards generated draft when pending player identity changes", async () => {
  // resume or generate a draft, change name/nationality/age/difficulty,
  // assert discard_career_draft is called before another generation.
});
```

- [ ] **Step 2: Run failing frontend tests**

Run: `npm.cmd run test:ui -- src/pages/NewCareer.test.jsx`

Expected: FAIL if quick flow is still reachable, progress is only cosmetic, or stale drafts survive identity changes.

- [ ] **Step 3: Remove quick create branch from normal UI**

Keep category/team selection only after draft has teams.

- [ ] **Step 4: Add real progress polling**

While generation is loading, poll `get_career_draft` and update the existing progress display from `draft_progress_year`. If the current backend command blocks useful polling, split generation into a start/poll/finalize command shape before marking this task complete.

- [ ] **Step 5: Discard stale draft on identity changes**

When a generated draft exists and the user changes name, nationality, age, or difficulty, call `discard_career_draft`, reset draft-only selections, and require a fresh historical generation.

- [ ] **Step 6: Run frontend tests**

Run: `npm.cmd run test:ui -- src/pages/NewCareer.test.jsx`

Expected: PASS.

## Chunk 4: Consumer Coverage

### Task 7: Verify Driver Detail, Team Dossier, And Global Ranking Against Finalized Historical Save

**Files:**
- Modify: `src-tauri/src/commands/historical_draft.rs`
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/commands/global_driver_rankings.rs`

- [ ] **Step 1: Write integration test using short historical range**

Use `create_historical_career_draft_for_range_for_test` with a small range, finalize the draft, then call:

- `get_driver_detail_in_base_dir`
- `get_team_history_dossier_in_base_dir`
- `get_global_driver_rankings_in_base_dir`

Assert:

- non-player veteran has `trajetoria.historico.presenca.corridas > 0`;
- team dossier has `has_history = true`;
- global ranking has non-empty rows with `historical_index > 0`;
- player exists and has zero career stats.

Add a legacy compatibility test:

```rust
#[test]
fn legacy_save_without_team_season_archive_still_builds_team_dossier_from_race_results() {
    // seed old-style race_results without team_season_archive rows,
    // assert get_team_history_dossier_in_base_dir returns has_history = true.
}
```

- [ ] **Step 2: Run failing integration test**

Run: `cargo test --manifest-path src-tauri/Cargo.toml historical_draft::tests::finalized_historical_save_feeds_dossiers_and_global_ranking -- --nocapture`

Expected: FAIL until all contracts are wired.

- [ ] **Step 3: Fix missing data producers only**

Do not add frontend fallbacks. Fix generation/archive/audit producers so consumers receive real data.

For legacy saves, keep fallback logic in backend readers only: prefer `team_season_archive` when present, then fall back to `race_results` for existing active saves that predate the archive. New historical drafts must not rely on this fallback to pass audit.

- [ ] **Step 4: Run backend consumer tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml historical_draft -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml global_driver_rankings -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml commands::career::tests::test_get_driver_detail -- --nocapture
```

Expected: PASS.

## Chunk 5: Final Verification

### Task 8: Full Focused Verification

**Files:**
- No code changes.

- [ ] **Step 1: Run Rust focused suites**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml historical_draft -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml world::integrity -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml world::team_archive -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml global_driver_rankings -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml evolution::pipeline -- --nocapture
```

- [ ] **Step 2: Run frontend focused suites**

Run:

```powershell
npm.cmd run test:ui -- src/pages/NewCareer.test.jsx
npm.cmd run test:ui -- src/pages/tabs/GlobalDriversTab.test.jsx
npm.cmd run test:ui -- src/components/driver/DriverDetailModalSections.test.jsx
npm.cmd run test:ui -- src/pages/tabs/MyTeamTab.test.jsx
```

- [ ] **Step 3: Run build**

Run: `npm.cmd run build`

Expected: build succeeds. Existing chunk-size warning is acceptable unless this work makes it materially worse.

- [ ] **Step 4: Manual smoke test**

Create a new career from UI:

- confirm no quick career option exists;
- generate historical world;
- select category/team from generated 2025 world;
- open driver dossier for an AI veteran;
- open team dossier;
- open global ranking;
- confirm no obvious empty/redundant historical fields.
