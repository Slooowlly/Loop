# Team Finance Economy Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace abstract team budget with a persistent cash-and-debt economy that updates each round, drives AI strategy, and materially affects team development and competitiveness.

**Architecture:** Extend `Team` persistence with financial fields, introduce a dedicated backend finance module for cashflow/state/events, then connect it to round resolution, preseason planning, promotion/relegation, and read-only frontend surfaces. Keep `budget` as a derived compatibility value during migration instead of the source of truth.

**Tech Stack:** Rust (`src-tauri` backend), SQLite migrations/queries, React frontend, Vitest for UI, `cargo test` for backend.

---

## File Map

**Create**

- `src-tauri/src/finance/mod.rs`
- `src-tauri/src/finance/economy.rs`
- `src-tauri/src/finance/cashflow.rs`
- `src-tauri/src/finance/state.rs`
- `src-tauri/src/finance/events.rs`

**Modify**

- `src-tauri/src/models/team.rs`
- `src-tauri/src/db/migrations.rs`
- `src-tauri/src/db/queries/teams.rs`
- `src-tauri/src/commands/career_types.rs`
- `src-tauri/src/commands/career.rs`
- `src-tauri/src/commands/race.rs`
- `src-tauri/src/market/preseason.rs`
- `src-tauri/src/promotion/effects.rs`
- `src-tauri/src/promotion/pipeline.rs`
- `src/pages/tabs/MyTeamTab.jsx`

**Likely Tests**

- inline unit tests inside `src-tauri/src/finance/*.rs`
- inline unit tests in `src-tauri/src/models/team.rs`
- inline query/migration tests in `src-tauri/src/db/migrations.rs` and `src-tauri/src/db/queries/teams.rs`
- optional UI assertions in `src/pages/tabs/MyTeamTab.test.jsx` if that file exists or is added

---

## Chunk 1: Persistence And Finance Domain Skeleton

### Task 1: Add failing persistence coverage for financial fields

**Files:**
- Modify: `src-tauri/src/db/queries/teams.rs`
- Modify: `src-tauri/src/db/migrations.rs`
- Modify: `src-tauri/src/models/team.rs`

- [ ] **Step 1: Write failing backend tests for the new fields**

Add assertions for:

```rust
assert_eq!(loaded.cash_balance, team.cash_balance);
assert_eq!(loaded.debt_balance, team.debt_balance);
assert_eq!(loaded.financial_state, team.financial_state);
assert_eq!(loaded.season_strategy, team.season_strategy);
assert_eq!(loaded.last_round_income, team.last_round_income);
assert_eq!(loaded.last_round_expenses, team.last_round_expenses);
assert_eq!(loaded.last_round_net, team.last_round_net);
assert_eq!(
    loaded.parachute_payment_remaining,
    team.parachute_payment_remaining
);
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test db::queries::teams`

Expected: FAIL because the new fields do not exist in the schema/query mapping yet.

- [ ] **Step 3: Extend `Team` with the financial fields and safe defaults**

Add fields plus enums/strings for:

```rust
pub cash_balance: f64,
pub debt_balance: f64,
pub financial_state: String,
pub season_strategy: String,
pub last_round_income: f64,
pub last_round_expenses: f64,
pub last_round_net: f64,
pub parachute_payment_remaining: f64,
```

Initialize defaults in constructors/test helpers.

- [ ] **Step 4: Add migration support and query round-trip**

Update:

- `src-tauri/src/db/migrations.rs`
- `src-tauri/src/db/queries/teams.rs`

So the columns are created, backfilled, inserted, updated, and loaded.

- [ ] **Step 5: Run the focused tests again**

Run: `cargo test db::queries::teams`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/models/team.rs src-tauri/src/db/migrations.rs src-tauri/src/db/queries/teams.rs
git commit -m "feat: persist team finance fields"
```

### Task 2: Create the finance module skeleton with failing tests first

**Files:**
- Create: `src-tauri/src/finance/mod.rs`
- Create: `src-tauri/src/finance/economy.rs`
- Create: `src-tauri/src/finance/cashflow.rs`
- Create: `src-tauri/src/finance/state.rs`
- Create: `src-tauri/src/finance/events.rs`

- [ ] **Step 1: Write failing unit tests for the core finance contracts**

Add tests covering:

```rust
assert!(round_income > 0.0);
assert!(round_expenses > 0.0);
assert_eq!(derive_financial_state(90.0), "elite");
assert_eq!(derive_financial_state(10.0), "collapse");
assert!(debt_service(100_000.0) > 0.0);
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test finance::`

Expected: FAIL because the new module/functions are missing.

- [ ] **Step 3: Add minimal implementations**

Create:

- `economy.rs` for macro modifiers
- `cashflow.rs` for income/expense structs and round settlement
- `state.rs` for health score + state derivation
- `events.rs` for crisis-event decision helpers
- `mod.rs` to expose the module

- [ ] **Step 4: Wire the module into the backend crate**

Expose `pub mod finance;` from the appropriate root module if needed.

- [ ] **Step 5: Run the focused tests again**

Run: `cargo test finance::`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/finance
git commit -m "feat: add finance domain module"
```

---

## Chunk 2: Round Cashflow And Financial State Updates

### Task 3: Calculate and apply round cashflow after race progression

**Files:**
- Modify: `src-tauri/src/commands/race.rs`
- Modify: `src-tauri/src/finance/cashflow.rs`
- Modify: `src-tauri/src/db/queries/teams.rs`

- [ ] **Step 1: Write a failing test for round cashflow application**

Create/extend a backend test asserting:

```rust
let before = team.cash_balance;
apply_round_cashflow(&mut team, &round_context);
assert_ne!(team.cash_balance, before);
assert_eq!(team.last_round_net, team.last_round_income - team.last_round_expenses);
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run: `cargo test commands::race`

Expected: FAIL because round closure does not update finance yet.

- [ ] **Step 3: Implement round income/expense calculation**

Add helpers for:

- sponsorship income
- result bonus
- partial prize income
- salary expense per round
- event operations cost
- structural maintenance
- technical investment
- debt service

- [ ] **Step 4: Update race closure to persist the new finance values**

At the end of round resolution:

- compute income/expenses
- update cash
- update debt when thresholds are crossed
- persist the changed team rows

- [ ] **Step 5: Re-run the tests**

Run:

- `cargo test commands::race`
- `cargo test finance::cashflow`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/race.rs src-tauri/src/finance/cashflow.rs src-tauri/src/db/queries/teams.rs
git commit -m "feat: apply team cashflow after each round"
```

### Task 4: Derive financial state and team strategy from finance context

**Files:**
- Modify: `src-tauri/src/finance/state.rs`
- Modify: `src-tauri/src/market/preseason.rs`
- Modify: `src-tauri/src/commands/race.rs`

- [ ] **Step 1: Write failing tests for state/strategy transitions**

Cover cases like:

```rust
assert_eq!(derive_financial_state(score_for_elite()), "elite");
assert_eq!(derive_financial_state(score_for_crisis()), "crisis");
assert_eq!(pick_season_strategy(&elite_team), "balanced");
assert_eq!(pick_season_strategy(&collapse_team), "survival");
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test finance::state`

Expected: FAIL because state transitions are not fully implemented yet.

- [ ] **Step 3: Implement the health score and state mapping**

Use:

- cash position
- debt pressure
- revenue stability
- structure strength
- recent results

- [ ] **Step 4: Implement strategy selection**

Map team context to:

- `expansion`
- `balanced`
- `austerity`
- `all_in`
- `survival`

- [ ] **Step 5: Recalculate state in both round closure and preseason**

Round closure updates immediate state.
Preseason updates the season-level strategy.

- [ ] **Step 6: Re-run tests**

Run:

- `cargo test finance::state`
- `cargo test initialize_preseason`

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/finance/state.rs src-tauri/src/market/preseason.rs src-tauri/src/commands/race.rs
git commit -m "feat: derive team financial states and strategies"
```

---

## Chunk 3: Convert Finance Into Competitive Impact

### Task 5: Apply finance-driven modifiers to car, reliability, and structure

**Files:**
- Modify: `src-tauri/src/finance/cashflow.rs`
- Modify: `src-tauri/src/market/preseason.rs`
- Modify: `src-tauri/src/market/pit_strategy.rs`
- Modify: `src-tauri/src/models/team.rs`

- [ ] **Step 1: Write failing tests for finance impact**

Cover behaviors like:

```rust
assert!(rich_team_reliability >= poor_team_reliability);
assert!(crisis_team_reliability <= baseline_reliability);
assert!(all_in_team_project_delta != balanced_team_project_delta);
```

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run:

- `cargo test market::pit_strategy`
- `cargo test models::team`

Expected: FAIL because finance is not shaping those values yet.

- [ ] **Step 3: Implement short-term and long-term finance impact helpers**

Short term:

- maintenance -> reliability
- crisis drag -> operational penalties

Long term:

- offseason project quality
- engineering/facilities drift
- car performance delta

- [ ] **Step 4: Integrate with the seasonal car-build logic**

Ensure:

- finance posture influences project ambition
- expensive balanced builds remain premium
- low-cash teams skew toward cheaper profiles

- [ ] **Step 5: Re-run targeted tests**

Run:

- `cargo test market::pit_strategy`
- `cargo test initialize_preseason`
- `cargo test models::team`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/finance/cashflow.rs src-tauri/src/market/preseason.rs src-tauri/src/market/pit_strategy.rs src-tauri/src/models/team.rs
git commit -m "feat: connect finance to team competitiveness"
```

### Task 6: Add crisis and rescue event hooks

**Files:**
- Modify: `src-tauri/src/finance/events.rs`
- Modify: `src-tauri/src/commands/race.rs`
- Modify: `src-tauri/src/promotion/effects.rs`

- [ ] **Step 1: Write failing tests for crisis/rescue events**

Cover:

```rust
assert!(collapse_team_can_trigger_emergency_loan());
assert!(relegated_team_gets_parachute_payment());
assert!(technical_breakthrough_requires_good_engineering());
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test finance::events`

Expected: FAIL because event hooks are not wired yet.

- [ ] **Step 3: Implement event decision helpers**

Add support for:

- emergency loan
- investor rescue
- upgrade freeze
- staff loss
- technical breakthrough
- parachute payment initialization

- [ ] **Step 4: Trigger events at the appropriate checkpoints**

- round closure for crisis escalation
- promotion/relegation flow for parachute aid

- [ ] **Step 5: Re-run tests**

Run:

- `cargo test finance::events`
- `cargo test commands::race`
- `cargo test promotion::effects`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/finance/events.rs src-tauri/src/commands/race.rs src-tauri/src/promotion/effects.rs
git commit -m "feat: add finance crisis and rescue events"
```

---

## Chunk 4: Global Economy And UI Readout

### Task 7: Add macro economy modifiers

**Files:**
- Modify: `src-tauri/src/finance/economy.rs`
- Modify: `src-tauri/src/market/preseason.rs`
- Modify: `src-tauri/src/commands/race.rs`

- [ ] **Step 1: Write failing tests for macro economy effects**

Cover:

```rust
assert!(boom_income > neutral_income);
assert!(recession_income < neutral_income);
assert!(recession_cost < neutral_cost);
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test finance::economy`

Expected: FAIL because the macro state is missing or inert.

- [ ] **Step 3: Implement the macro economy model**

Start with:

- `boom`
- `neutral`
- `recession`

Apply modifiers to revenue and cost calculations without breaking relative competition.

- [ ] **Step 4: Re-run the focused tests**

Run: `cargo test finance::economy`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/finance/economy.rs src-tauri/src/market/preseason.rs src-tauri/src/commands/race.rs
git commit -m "feat: add global sport economy modifiers"
```

### Task 8: Expose finance data to the frontend team view

**Files:**
- Modify: `src-tauri/src/commands/career_types.rs`
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src/pages/tabs/MyTeamTab.jsx`
- Test: `src/pages/tabs/MyTeamTab.test.jsx`

- [ ] **Step 1: Write the failing frontend/backend readout tests**

Cover:

```jsx
expect(screen.getByText(/cash|caixa/i)).toBeInTheDocument();
expect(screen.getByText(/debt|divida/i)).toBeInTheDocument();
expect(screen.getByText(/financial state|saude financeira/i)).toBeInTheDocument();
```

And a backend assertion that `player_team` carries the new fields.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run:

- `cargo test commands::career`
- `npm test -- MyTeamTab`

Expected: FAIL because the payload/UI do not show the new fields yet.

- [ ] **Step 3: Extend the career payload**

Expose:

- cash balance
- debt balance
- financial state
- season strategy
- last round finance summary
- parachute payment remaining when applicable

- [ ] **Step 4: Render the finance summary in the team tab**

Add read-only cards for:

- caixa
- dívida
- saldo da última rodada
- estado financeiro
- estratégia da temporada

- [ ] **Step 5: Re-run tests**

Run:

- `cargo test commands::career`
- `npm test -- MyTeamTab`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/career_types.rs src-tauri/src/commands/career.rs src/pages/tabs/MyTeamTab.jsx src/pages/tabs/MyTeamTab.test.jsx
git commit -m "feat: expose team finance readouts in career ui"
```

---

## Final Verification

- [ ] **Step 1: Run backend finance verification**

Run:

```bash
cargo test finance::
cargo test initialize_preseason
cargo test commands::race
cargo test commands::career
cargo test db::queries::teams
```

Expected: all PASS

- [ ] **Step 2: Run frontend verification**

Run:

```bash
npm test
npm run test:structure
```

Expected: all PASS

- [ ] **Step 3: Commit the final integrated batch**

```bash
git add .
git commit -m "feat: add team finance economy system"
```
