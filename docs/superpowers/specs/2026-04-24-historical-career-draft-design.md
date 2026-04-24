# Historical Career Draft Design

## Context

New saves currently start from a blank sporting universe: drivers have no past, teams have no meaningful record, and the first playable season feels like the first year of the world itself.

The new direction is to make every new career begin in a lived-in paddock. Before the player enters the career, the game should simulate a full historical era from 2000 through 2024. The playable career begins in 2025, with the player inserted only after that history exists.

This is intentionally a significant backend and persistence change. The design favors correctness, rich historical data, and future history screens over instant save creation. A creation time of up to a few minutes is acceptable.

## Goals

- Generate a complete historical motorsport world before the player's playable debut.
- Simulate seasons from 2000 through 2024 using the real race, evolution, retirement, rookie, promotion/relegation, and market systems as much as possible.
- Start the playable save in 2025.
- Insert the player only in 2025, with personal history and stats at zero.
- Preserve race-by-race historical results for future dossiers and history screens.
- Preserve retired drivers as consultable legends, not as active market candidates.
- Avoid generating or persisting historical news/editorial text in this first version.
- Keep incomplete generated worlds out of the normal load-save flow.

## Non-Goals

- Build the full history UI in this iteration.
- Generate narrative biographies for drivers or teams.
- Persist historical news articles.
- Add replay or season-review screens for every historical year.
- Support checkpoint resume from the last completed historical year after a generation failure.
- Simulate a pre-2025 career for the player.

## Chosen Direction

Use a real historical simulation rather than synthetic aggregates.

The backend creates a draft save, simulates the world from 2000 to the end of 2024, leaves 2025 active and pending, then lets the player choose a starting category and team from the actual 2025 world.

The player is created only when the draft is finalized. The player replaces the selected team's current N2 driver and receives a new active contract starting in 2025.

## User Flow

The new career wizard is split into two phases.

### Phase 1: Player Identity And World Generation

The user chooses only the inputs needed before generation:

- difficulty;
- player name;
- nationality;
- age.

The primary action becomes `Generate world`. When triggered, the frontend calls a backend command that creates a draft save and runs the historical simulation.

During generation, the UI shows simple progress such as:

- creating historical world;
- simulating season 2000;
- simulating season 2017;
- preparing playable season 2025.

The UI does not need to show a historical summary after generation.

### Phase 2: Entry Into The 2025 World

Once the draft is complete, the wizard continues using the generated world:

- choose starting category;
- choose a team from that category's real 2025 grid;
- confirm career.

Only teams from the selected category are shown. If the user goes back between category, team, and confirmation, the generated draft is preserved. The simulation is not rerun.

If the user changes an input that affects world generation, especially difficulty, the app should warn that the existing draft must be discarded and regenerated.

## Draft Save Lifecycle

Save creation now has lifecycle states.

### `draft`

A draft is a generated historical world that is not yet playable. It may contain the full 2000-2024 history and the active 2025 season, but the player has not yet been inserted.

Draft saves must not appear in the normal load-save list.

### `failed`

A failed draft represents a generation attempt that did not complete cleanly. The first version should support retry by discarding the failed draft and starting over from zero.

### `active`

An active save is a playable career. The player has been inserted into a 2025 team, the save appears in the normal load list, and normal game flows apply.

Legacy saves that do not have a lifecycle field should be treated as `active`.

## Resume And Retry Behavior

When the user opens the new career flow and a complete draft exists, the UI offers:

- resume generated world;
- discard and generate another.

If generation fails, `Retry` discards the incomplete or failed draft and restarts generation from the beginning. Annual checkpoints can be considered later, but are not part of the first version.

## Backend Commands

The command surface should be split so generation, draft inspection, cleanup, and finalization are explicit.

Recommended commands:

- `create_historical_career_draft`
- `get_career_draft`
- `discard_career_draft`
- `finalize_career_draft`

### `create_historical_career_draft`

Responsibilities:

- validate player identity inputs;
- create a new save directory and SQLite database in `draft` lifecycle state;
- generate the base world at historical year 2000 without a player;
- simulate seasons through the end of 2024;
- prepare the active 2025 season with pending races;
- persist enough progress/error state for the UI to report failure clearly.

### `get_career_draft`

Responsibilities:

- return an existing complete draft, failed draft, or no draft;
- expose the available 2025 categories and teams once generation is complete;
- exclude draft saves from normal `list_saves` behavior.

### `discard_career_draft`

Responsibilities:

- remove the draft directory and all sidecar files;
- be safe to call for complete or failed drafts;
- never delete active saves.

### `finalize_career_draft`

Responsibilities:

- receive the selected category and team;
- insert the player into the 2025 world;
- replace the selected team's current N2 driver;
- transition the save from `draft` to `active`.

Finalization must be transactional.

## Metadata

`meta.json` should gain lifecycle and historical generation fields.

Recommended fields:

- `lifecycle_status`: `draft`, `failed`, or `active`;
- `history_start_year`: `2000`;
- `history_end_year`: `2024`;
- `playable_start_year`: `2025`;
- `current_year`: `2025` after successful generation;
- `current_season`: season 26 if 2000 is season 1;
- `player_name`;
- `player_nationality`;
- `player_age`;
- `difficulty`;
- `draft_progress_year`;
- `draft_error`;
- `team_name`, only after finalization;
- `category`, only after finalization or when meaningful for the active save.

Backward compatibility:

- missing `lifecycle_status` means `active`;
- existing active saves should not be forced through historical generation.

## Historical Simulation

The simulation should run season by season from 2000 through 2024.

For each historical season:

1. Simulate all regular pending races for all relevant categories.
2. Execute special windows and special blocks when the current season flow requires them.
3. Persist every race result in SQLite `race_results`.
4. Persist the auxiliary race history file if existing UI still depends on it.
5. Update driver and team season stats.
6. Close the season using the real end-of-season pipeline where possible.
7. Archive annual driver snapshots.
8. Preserve retired drivers.
9. Run promotion/relegation.
10. Run AI market and vacancy filling.
11. Create the next season and calendar.

The historical path should be quiet:

- no historical news;
- no resume context;
- no mandatory annual backup snapshots;
- no player proposal flow;
- no UI-only side effects.

If the existing end-of-season command performs playable-only side effects, the implementation should introduce an internal historical orchestration path that reuses the same domain modules without producing those side effects.

## 2025 Playable Start State

After historical generation completes:

- the active season is 2025;
- all 2025 races are pending;
- no player exists yet;
- active drivers have aged, evolved, moved teams, gained licenses, and accumulated stats;
- retired drivers remain persisted as inactive/retired;
- teams are in the categories produced by historical promotion/relegation;
- teams and drivers carry historical records;
- historical news is absent.

The wizard then reads categories and teams from this generated 2025 state.

## Player Insertion

When finalizing the draft, the player is inserted as a true 2025 rookie.

Rules:

- create the player with `is_jogador = true`;
- use the selected name, nationality, and age;
- set `categoria_atual` to the selected category;
- keep all player season and career stats at zero;
- grant the initial required license for the category;
- insert the player as `Numero2`;
- set the selected team as the player team;
- mark the save as `active`.

The selected team's current N2 is displaced:

- rescind that driver's active regular contract;
- remove the driver from `piloto_2_id` and hierarchy N2 slots;
- keep the driver active in the database;
- leave the driver as a free agent for future market/vacancy systems.

The first version should not reshuffle other teams just to place the displaced N2.

## Historical Data To Preserve

The design should preserve sporting facts now so future history screens can be built later.

Required:

- `race_results` rows from 2000 through 2024;
- enough calendar data to identify season, year, round, track, category, and date;
- final standings by season/category;
- `driver_season_archive` rows for annual driver snapshots;
- retired driver state, including retirement year/reason where current systems support it;
- driver career totals;
- team current and historical aggregate stats;
- contracts or snapshots sufficient to reconstruct team movement where current data supports it.

Recommended addition:

- a team-season archive table or equivalent snapshot, mirroring the usefulness of `driver_season_archive` for future team history screens.

## Historical Data Not Required In Phase 1

- news articles;
- editorial summaries;
- generated biographies;
- per-season narrative reports;
- historical UI screens;
- failed-generation annual checkpoint recovery.

## Storage Expectations

Current category configuration produces roughly:

- 58 regular races per season;
- around 1,192 regular race-result rows per season;
- around 29,800 regular result rows for 25 historical seasons.

If special categories are persisted as full historical results too, the upper estimate is roughly:

- 74 races per season;
- around 1,696 race-result rows per season;
- around 42,400 result rows for 25 historical seasons.

This is acceptable for SQLite. Expected storage impact is likely in the tens of MB, with conservative room for indexes, calendars, standings, archives, and sidecars.

## Error Handling

Generation failures should not create broken playable saves.

Rules:

- incomplete drafts do not appear in normal load-save UI;
- finalization cannot run unless the draft reached 2025 successfully;
- retry removes the failed draft and starts from scratch;
- finalization is transactional;
- a failed player insertion should leave the draft unfinalized;
- active saves are never deleted by draft cleanup commands.

## Testing Strategy

Backend tests should validate:

- historical draft generation reaches 2025;
- 2025 is active and has pending calendar entries;
- no player exists during the historical simulation;
- race results exist for historical years;
- historical news is not created;
- retired drivers remain persisted but inactive;
- active drivers and teams have non-zero historical records after generation;
- available category/team choices come from the generated 2025 database;
- finalizing creates a player with zero personal history;
- finalizing replaces the selected team's N2;
- the displaced N2 remains active without an active regular contract;
- draft saves are excluded from normal save listing;
- complete drafts can be resumed;
- failed drafts can be discarded/retried;
- legacy saves without lifecycle status still load as active.

Frontend tests should validate:

- the wizard separates identity/generation from category/team selection;
- progress is shown during generation;
- draft resume/discard choices appear when appropriate;
- changing generation-affecting inputs warns before discarding a draft;
- category and team screens read from the generated draft;
- back navigation after generation does not rerun the simulation;
- final confirmation calls the draft finalization command.

## Rollout Plan

### Phase 1

Introduce lifecycle metadata, draft commands, historical generation orchestration, player finalization, and focused tests.

### Phase 2

Update the new career wizard to use the draft flow and dynamic 2025 category/team data.

### Phase 3

Add richer historical archives where gaps remain, especially team-season snapshots.

### Phase 4

Build future history UI using the facts already preserved by this design.

