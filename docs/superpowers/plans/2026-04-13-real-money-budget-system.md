# Real Money Budget System Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrar as decisoes financeiras das equipes de `budget` 0-100 para dinheiro real baseado em caixa, divida, receita projetada, custos comprometidos e poder de gasto.

**Architecture:** Criar um modulo central de planejamento financeiro em `src-tauri/src/finance/planning.rs` e migrar consumidores em fases. `budget` permanece no banco como campo legado/compatibilidade, mas novos calculos devem usar funcoes explicitas como `calculate_spending_power`, `calculate_salary_ceiling` e `derive_budget_index_from_money`.

**Tech Stack:** Rust, rusqlite, Vitest/React para UI, testes unitarios Rust no proprio modulo.

---

## Reference

Spec aprovada:

- `docs/superpowers/specs/2026-04-13-real-money-budget-system-design.md`

## Current Hotspots

Arquivos que hoje ainda dependem diretamente de `budget`:

- `src-tauri/src/finance/state.rs`
- `src-tauri/src/finance/cashflow.rs`
- `src-tauri/src/commands/race.rs`
- `src-tauri/src/market/car_build_strategy.rs`
- `src-tauri/src/market/pit_strategy.rs`
- `src-tauri/src/market/preseason.rs`
- `src-tauri/src/commands/career.rs`
- `src-tauri/src/market/team_ai.rs`
- `src-tauri/src/market/renewal.rs`
- `src-tauri/src/market/pipeline.rs`
- `src-tauri/src/models/team.rs`
- `src/pages/tabs/MyTeamTab.jsx`

## File Structure

### New Files

- `src-tauri/src/finance/planning.rs`: escalas financeiras por categoria, `spending_power`, credito, reserva, pressao de divida, indice legado derivado e buckets de gasto.
- `src-tauri/src/finance/salary.rs`: teto salarial, oferta salarial baseada em dinheiro e pressao de renovacao.
- `src/pages/tabs/MyTeamTab.test.jsx`: cobertura do painel financeiro visivel ao jogador.

### Modified Files

- `src-tauri/src/finance/mod.rs`: exportar `planning` e `salary`.
- `src-tauri/src/finance/state.rs`: recalcular saude financeira usando dinheiro real.
- `src-tauri/src/finance/cashflow.rs`: usar planejamento financeiro no impacto de offseason.
- `src-tauri/src/finance/events.rs`: juros e crise por escala/estado.
- `src-tauri/src/commands/race.rs`: receita/custo por rodada sem `team.budget`.
- `src-tauri/src/market/car_build_strategy.rs`: escolha de perfil por custo em dinheiro e `spending_power`.
- `src-tauri/src/market/preseason.rs`: custo de perfil em caixa, nao pontos de budget.
- `src-tauri/src/market/pit_strategy.rs`: pit crew e risco por dinheiro real/estado financeiro.
- `src-tauri/src/commands/career.rs`: ofertas ao jogador por teto salarial real.
- `src-tauri/src/market/team_ai.rs`: propostas de mercado por dinheiro real.
- `src-tauri/src/market/renewal.rs`: renovacoes por teto salarial real.
- `src-tauri/src/market/pipeline.rs`: transportar contexto financeiro quando necessario.
- `src-tauri/src/models/team.rs`: caixa inicial por categoria e `budget` inicial derivado.
- `src-tauri/src/commands/career_types.rs`: expor `spending_power`, `salary_ceiling` e `budget_index`.
- `src/pages/tabs/MyTeamTab.jsx`: trocar leitura de budget abstrato por dinheiro real/plano financeiro.

---

## Chunk 1: Finance Planning Core

### Task 1: Add category money scales and planning primitives

**Files:**
- Create: `src-tauri/src/finance/planning.rs`
- Modify: `src-tauri/src/finance/mod.rs`
- Test: `src-tauri/src/finance/planning.rs`

- [ ] **Step 1: Write failing tests for category scales**

Add tests covering:

```rust
#[test]
fn category_scale_makes_gt3_more_expensive_than_rookie() {
    let rookie = category_finance_scale("mazda_rookie");
    let gt3 = category_finance_scale("gt3");

    assert!(gt3.expected_cash_midpoint() > rookie.expected_cash_midpoint());
    assert!(gt3.operating_cost_midpoint() > rookie.operating_cost_midpoint());
}

#[test]
fn unknown_category_gets_safe_mid_tier_scale() {
    let scale = category_finance_scale("unknown");

    assert!(scale.cash_min > 0.0);
    assert!(scale.operating_cost_min > 0.0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test finance::planning`

Expected: FAIL because `finance::planning` does not exist.

- [ ] **Step 3: Implement `planning.rs` primitives**

Create `CategoryFinanceScale`, `TeamFinancialPlan`, `category_finance_scale`, `expected_cash_midpoint` and `operating_cost_midpoint`.

Use these category ranges:

```text
mazda_rookie/toyota_rookie: cash 100k-700k, operating 120k-250k
mazda_amador/toyota_amador: cash 250k-1.5M, operating 250k-600k
bmw_m2/production_challenger: cash 750k-4M, operating 600k-1.6M
gt4: cash 2M-9M, operating 1.5M-4M
gt3: cash 6M-25M, operating 4M-12M
endurance: cash 12M-60M, operating 8M-25M
unknown: use bmw_m2/production scale
```

- [ ] **Step 4: Export module**

In `src-tauri/src/finance/mod.rs` add:

```rust
pub mod planning;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test finance::planning`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/finance/mod.rs src-tauri/src/finance/planning.rs
git commit -m "feat: add real money finance planning core"
```

### Task 2: Add spending power calculation

**Files:**
- Modify: `src-tauri/src/finance/planning.rs`
- Test: `src-tauri/src/finance/planning.rs`

- [ ] **Step 1: Write failing tests for spending power**

Add tests covering:

- debt and committed costs reduce `spending_power`;
- `budget_index` is derived from money and ignores raw `team.budget`;
- `spending_power` can be negative;
- richer category scale changes interpretation of the same cash amount.

- [ ] **Step 2: Implement state multipliers**

Add:

```rust
pub fn income_confidence_for_state(state: &str) -> f64
pub fn credit_aggressiveness_for_state(state: &str) -> f64
pub fn safety_reserve_multiplier_for_state(state: &str) -> f64
```

Use the spec values:

```text
income confidence: elite 0.90, healthy 0.80, stable 0.60, pressured 0.45, crisis 0.35, collapse 0.25
credit aggression: elite 0.10, healthy 0.20, stable 0.30, pressured 0.55, crisis 0.75, collapse 0.40
safety reserve: elite 1.50, healthy 1.20, stable 0.90, pressured 0.45, crisis 0.10, collapse 0.00
```

- [ ] **Step 3: Implement financial plan calculation**

Add:

```rust
pub fn calculate_projected_income(team: &Team) -> f64
pub fn calculate_committed_costs(team: &Team) -> f64
pub fn calculate_available_credit(team: &Team) -> f64
pub fn calculate_debt_pressure(team: &Team) -> f64
pub fn calculate_safety_reserve(team: &Team) -> f64
pub fn calculate_spending_power(team: &Team) -> f64
pub fn derive_budget_index_from_money(team: &Team) -> f64
pub fn calculate_financial_plan(team: &Team) -> TeamFinancialPlan
```

Rules:

- Use `category_finance_scale(&team.categoria)`.
- Do not read `team.budget` inside `derive_budget_index_from_money`.
- Clamp `budget_index` to `0.0..=100.0`.
- Allow `spending_power` to be negative.

- [ ] **Step 4: Run tests**

Run: `cargo test finance::planning`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/finance/planning.rs
git commit -m "feat: calculate team spending power from real money"
```

---

## Chunk 2: Financial State Uses Real Money

### Task 3: Migrate financial health and strategy away from raw budget

**Files:**
- Modify: `src-tauri/src/finance/state.rs`
- Test: `src-tauri/src/finance/state.rs`

- [ ] **Step 1: Write failing tests**

Add tests covering:

- a team with low raw `budget` but healthy cash still scores as financially healthy;
- a team with high raw `budget` but heavy debt can be forced into `survival`;
- stable/austerity decisions are based on `spending_power`, not `team.budget`.

- [ ] **Step 2: Replace budget support score**

In `financial_health_score`, import:

```rust
use crate::finance::planning::{calculate_financial_plan, category_finance_scale};
```

Derive financial score from:

```rust
let plan = calculate_financial_plan(team);
let scale = category_finance_scale(&team.categoria);
let cash_score = (team.cash_balance / scale.expected_cash_midpoint() * 65.0).clamp(-20.0, 100.0);
let spending_score = (plan.spending_power / scale.operating_cost_midpoint() * 55.0).clamp(-25.0, 100.0);
let debt_penalty = (plan.debt_pressure / scale.expected_cash_midpoint() * 80.0).clamp(0.0, 70.0);
let support_score = ((plan.budget_index + team.reputacao) / 2.0).clamp(0.0, 100.0);
```

Keep structure and momentum terms.

- [ ] **Step 3: Replace strategy budget checks**

Replace raw budget thresholds with thresholds based on:

```rust
let plan = calculate_financial_plan(team);
let scale = category_finance_scale(&team.categoria);
```

Examples:

```rust
if plan.spending_power < scale.operating_cost_midpoint() * 0.20 && team.car_performance < 6.0 {
    return "all_in";
}
```

and stable/austerity:

```rust
if plan.spending_power < scale.operating_cost_midpoint() * 0.50 {
    "austerity"
} else {
    "balanced"
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test finance::state`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/finance/state.rs
git commit -m "feat: derive financial state from real money"
```

---

## Chunk 3: Race Round Cashflow Uses Real Money Scale

### Task 4: Replace round income and costs based on raw budget

**Files:**
- Modify: `src-tauri/src/commands/race.rs`
- Modify: `src-tauri/src/finance/events.rs`
- Test: `src-tauri/src/commands/race.rs`
- Test: `src-tauri/src/finance/events.rs`

- [ ] **Step 1: Write failing finance event tests**

In `finance/events.rs`, add tests covering:

- `debt_service_for_state(1_000_000.0, "collapse")` is greater than healthy debt service;
- unknown state keeps safe default interest;
- zero/negative debt still produces zero service.

- [ ] **Step 2: Implement state-based debt service**

Add:

```rust
pub fn debt_interest_rate_for_state(state: &str) -> f64
pub fn debt_service_for_state(debt_balance: f64, state: &str) -> f64
```

Use:

```text
elite/healthy: 0.0075
stable: 0.0125
pressured: 0.020
crisis: 0.0325
collapse: 0.050
unknown: 0.015
```

- [ ] **Step 3: Write or update race finance tests**

Cover:

- rich GT4 and poor GT4 get different sponsorship from real money/reputation, not raw `budget`;
- setting `budget = 1.0` on a rich team does not zero out sponsorship;
- collapse debt produces higher debt service.

- [ ] **Step 4: Replace budget-based formulas in race cashflow**

In `commands/race.rs`, replace sponsorship and technical investment formulas that read `team.budget`.

Use:

```rust
let scale = category_finance_scale(&team.categoria);
let plan = calculate_financial_plan(team);
let round_operating_base = scale.operating_cost_midpoint() / rounds_in_season;
let sponsorship_income = (
    scale.expected_cash_midpoint() / rounds_in_season * 0.16
    + team.reputacao * round_operating_base * 0.004
    + plan.budget_index * round_operating_base * 0.002
) * income_modifier;
let technical_investment_cost = (
    round_operating_base * 0.16
    + team.car_performance.max(0.0) * round_operating_base * 0.015
) * cost_modifier;
let debt_service_cost = debt_service_for_state(team.debt_balance, &team.financial_state);
```

Tune constants only after scenario tests reveal absurd values.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test finance::events
cargo test commands::race -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/finance/events.rs src-tauri/src/commands/race.rs
git commit -m "feat: base race cashflow on real money"
```

---

## Chunk 4: Car Build Strategy Uses Spending Power

### Task 5: Replace car profile affordability from budget points to money

**Files:**
- Modify: `src-tauri/src/market/car_build_strategy.rs`
- Modify: `src-tauri/src/simulation/car_build.rs`
- Test: `src-tauri/src/market/car_build_strategy.rs`
- Test: `src-tauri/src/simulation/car_build.rs`

- [ ] **Step 1: Add money cost tests for car profiles**

In `simulation/car_build.rs`, test that balanced profile costs more money than extreme profile.

- [ ] **Step 2: Implement money profile cost**

Add:

```rust
pub fn profile_money_cost_multiplier(profile: CarBuildProfile) -> f64
pub fn profile_money_cost(profile: CarBuildProfile, category_car_cost_unit: f64) -> f64
```

Use:

```text
Balanced: 1.25
Intermediate profiles: 1.05
Extreme profiles: 0.85
```

Keep `profile_budget_cost` for legacy callers until all are migrated.

- [ ] **Step 3: Write failing strategy tests**

Add tests covering:

- rich team with `budget = 1.0` still prefers balanced on mixed calendar;
- poor team with `budget = 99.0` avoids expensive balanced profile when cash/debt are bad;
- power calendar still pulls poor team toward power specialization.

- [ ] **Step 4: Replace `budget_bias`**

Change:

```rust
+ budget_bias(team.budget, profile)
```

to:

```rust
+ affordability_bias(team, profile)
```

Implementation rules:

- Use `calculate_financial_plan(team)`.
- Use `category_finance_scale(&team.categoria)`.
- Set `car_unit = scale.operating_cost_midpoint() * 0.30`.
- Calculate cost through `profile_money_cost(profile, car_unit)`.
- Penalize profiles that exceed spending power too hard.
- Reward balanced only when spending power is comfortably above car unit.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test simulation::car_build
cargo test market::car_build_strategy
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/simulation/car_build.rs src-tauri/src/market/car_build_strategy.rs
git commit -m "feat: choose car build profiles from spending power"
```

### Task 6: Apply profile cost to cash during preseason

**Files:**
- Modify: `src-tauri/src/market/preseason.rs`
- Test: `src-tauri/src/market/preseason.rs`

- [ ] **Step 1: Write failing preseason test**

Replace old expectation that `budget` decreases with:

```rust
assert!(updated_team.cash_balance < original_team.cash_balance);
assert_eq!(updated_team.budget, derive_budget_index_from_money(&updated_team));
```

- [ ] **Step 2: Replace budget deduction**

Replace:

```rust
updated_team.budget = (updated_team.budget - profile_budget_cost(updated_team.car_build_profile)).max(0.0);
```

with:

```rust
let scale = category_finance_scale(&updated_team.categoria);
let car_unit = scale.operating_cost_midpoint() * 0.30;
let profile_cost = profile_money_cost(updated_team.car_build_profile, car_unit);
updated_team.cash_balance -= profile_cost;
sync_legacy_budget_index(&mut updated_team);
```

If cash drops below the controlled negative cash limit, reuse existing financing behavior or add a helper in `finance/planning.rs`.

- [ ] **Step 3: Run tests**

Run:

```bash
cargo test initialize_preseason
cargo test market::car_build_strategy
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/market/preseason.rs
git commit -m "feat: charge car profile cost as real money"
```

---

## Chunk 5: Pit Operations Use Money

### Task 7: Replace pit crew quality and risk budget dependency

**Files:**
- Modify: `src-tauri/src/market/pit_strategy.rs`
- Modify: `src-tauri/src/models/team.rs`
- Test: `src-tauri/src/market/pit_strategy.rs`
- Test: `src-tauri/src/models/team.rs`

- [ ] **Step 1: Write failing pit tests**

Add tests covering:

- rich team with low legacy `budget` gets better pit crew than poor team with high legacy `budget`;
- poor/pressured team gets higher pit strategy risk;
- category cap still prevents rookie categories from elite pit quality.

- [ ] **Step 2: Replace `budget_strength` with derived money strength**

In `pit_strategy.rs`, import planning:

```rust
use crate::finance::planning::{calculate_financial_plan, derive_budget_index_from_money};
```

Replace direct `team.budget` usage in recalculation functions with:

```rust
let plan = calculate_financial_plan(team);
let money_strength = (plan.budget_index / 100.0).clamp(0.0, 1.0);
```

Keep current seed function signatures temporarily, but add:

```rust
pub fn seed_pit_crew_quality_from_team(team: &Team) -> f64
pub fn seed_pit_strategy_risk_from_team(team: &Team) -> f64
```

- [ ] **Step 3: Update initial team generation**

In `models/team.rs`, initialize `cash_balance` from category scale. Then derive:

```rust
team.budget = derive_budget_index_from_money(&team);
team.pit_strategy_risk = seed_pit_strategy_risk_from_team(&team);
team.pit_crew_quality = seed_pit_crew_quality_from_team(&team);
```

This may require building `Team` mutably before final return.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test market::pit_strategy
cargo test models::team
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/market/pit_strategy.rs src-tauri/src/models/team.rs
git commit -m "feat: base pit operations on real money"
```

---

## Chunk 6: Salary And Market Use Money

### Task 8: Add salary ceiling helpers

**Files:**
- Create: `src-tauri/src/finance/salary.rs`
- Modify: `src-tauri/src/finance/mod.rs`
- Test: `src-tauri/src/finance/salary.rs`

- [ ] **Step 1: Write failing salary tests**

Add tests covering:

- rich team salary ceiling is higher than indebted team salary ceiling;
- offer salary ignores legacy `budget`;
- salary floor never falls below `5_000.0`;
- category tier still matters.

- [ ] **Step 2: Implement salary helpers**

Create:

```rust
pub fn calculate_salary_ceiling(team: &Team) -> f64
pub fn calculate_offer_salary_from_money(team: &Team, driver_skill: f64) -> f64
pub fn calculate_renewal_pressure_from_money(team: &Team, current_salary: f64) -> f64
```

Rules:

- Base category salary from category tier remains.
- Multiply by spending power relative to category operating cost.
- Clamp lower bound to `5_000.0`.
- Do not read `team.budget`.

- [ ] **Step 3: Export module**

In `finance/mod.rs`:

```rust
pub mod salary;
```

- [ ] **Step 4: Run tests**

Run: `cargo test finance::salary`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/finance/mod.rs src-tauri/src/finance/salary.rs
git commit -m "feat: add salary ceiling from real money"
```

### Task 9: Convert market salary call sites

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/market/team_ai.rs`
- Modify: `src-tauri/src/market/renewal.rs`
- Modify: `src-tauri/src/market/pipeline.rs`
- Test: `src-tauri/src/commands/career.rs`
- Test: `src-tauri/src/market/team_ai.rs`
- Test: `src-tauri/src/market/renewal.rs`

- [ ] **Step 1: Write or update tests**

Cover:

- player offer salary scales with cash/debt, not `budget`;
- Team AI proposal salary scales with real money;
- renewal does not treat high legacy `budget` as affordability if cash is bad.

- [ ] **Step 2: Replace `calculate_offer_salary_for_team`**

In `commands/career.rs`, replace:

```rust
let budget_modifier = (team.budget / 70.0).clamp(0.6, 1.5);
(tier_base * skill_modifier * budget_modifier).max(5_000.0)
```

with:

```rust
calculate_offer_salary_from_money(team, player.atributos.skill)
```

- [ ] **Step 3: Replace Team AI offer budget modifier**

In `market/team_ai.rs`, remove salary dependency on `vacancy.budget`.

If `Vacancy` does not carry full team finance, update vacancy creation in `market/pipeline.rs` to carry:

```rust
cash_balance: f64,
debt_balance: f64,
financial_state: String,
category: String,
```

Prefer passing a `Team` reference into salary calculation where possible.

- [ ] **Step 4: Replace renewal affordability**

In `market/renewal.rs`, replace:

```rust
let effective_budget = (team_budget.max(1.0) * 5_000.0).max(25_000.0);
```

with salary ceiling from real money.

If the function currently only receives `team_budget`, change the signature to receive `&Team` or a compact finance context.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test commands::career -- --test-threads=1
cargo test market::team_ai
cargo test market::renewal
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/career.rs src-tauri/src/market/team_ai.rs src-tauri/src/market/renewal.rs src-tauri/src/market/pipeline.rs
git commit -m "feat: base market salaries on real money"
```

---

## Chunk 7: UI And Compatibility

### Task 10: Expose derived money readouts to career payload

**Files:**
- Modify: `src-tauri/src/commands/career_types.rs`
- Modify: `src-tauri/src/commands/career.rs`
- Test: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Add payload fields**

Add to the team payload:

```rust
pub spending_power: f64,
pub salary_ceiling: f64,
pub budget_index: f64,
```

Keep old `budget` for compatibility.

- [ ] **Step 2: Populate fields**

In the mapper that builds the frontend team DTO:

```rust
let plan = calculate_financial_plan(team);
let salary_ceiling = calculate_salary_ceiling(team);
```

Map:

```rust
spending_power: plan.spending_power,
salary_ceiling,
budget_index: plan.budget_index,
budget: plan.budget_index,
```

- [ ] **Step 3: Update tests**

Assert:

```rust
assert!(player_team.spending_power.is_finite());
assert!(player_team.salary_ceiling > 0.0);
assert!((0.0..=100.0).contains(&player_team.budget_index));
```

- [ ] **Step 4: Run tests**

Run: `cargo test commands::career -- --test-threads=1`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/career_types.rs src-tauri/src/commands/career.rs
git commit -m "feat: expose real money finance plan to UI"
```

### Task 11: Update My Team finance panel

**Files:**
- Modify: `src/pages/tabs/MyTeamTab.jsx`
- Create: `src/pages/tabs/MyTeamTab.test.jsx`
- Test: `src/pages/tabs/MyTeamTab.test.jsx`

- [ ] **Step 1: Write failing UI test**

Render a mocked player team with:

```js
cash_balance: 6500000,
debt_balance: 1250000,
spending_power: 2800000,
salary_ceiling: 420000,
budget_index: 72,
financial_state: "healthy",
season_strategy: "balanced",
```

Assert:

```js
expect(screen.getByText(/Caixa/i)).toBeInTheDocument();
expect(screen.getByText(/Poder de gasto/i)).toBeInTheDocument();
expect(screen.getByText(/Teto salarial/i)).toBeInTheDocument();
expect(screen.queryByText(/^Budget$/i)).not.toBeInTheDocument();
```

- [ ] **Step 2: Replace budget stat visually**

In `MyTeamTab.jsx`, replace old `Budget` bar with:

- `Caixa`
- `Divida`
- `Poder de gasto`
- `Teto salarial`
- optional `Indice financeiro` if useful for debugging

Use existing `formatMoney`.

- [ ] **Step 3: Run UI test**

Run: `npx.cmd vitest run src/pages/tabs/MyTeamTab.test.jsx`

Expected: PASS.

- [ ] **Step 4: Run broader frontend test**

Run: `npm test`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/pages/tabs/MyTeamTab.jsx src/pages/tabs/MyTeamTab.test.jsx
git commit -m "feat: show real money finance plan in team tab"
```

---

## Chunk 8: Legacy Budget Hardening

### Task 12: Keep legacy budget synchronized as derived compatibility

**Files:**
- Modify: `src-tauri/src/finance/planning.rs`
- Modify: `src-tauri/src/db/queries/teams.rs`
- Modify: `src-tauri/src/market/preseason.rs`
- Modify: `src-tauri/src/promotion/effects.rs`
- Test: `src-tauri/src/db/queries/teams.rs`
- Test: `src-tauri/src/promotion/effects.rs`

- [ ] **Step 1: Identify remaining production `team.budget` reads**

Run:

```bash
rg -n "\.budget|budget\b" src-tauri/src
```

Classify each occurrence:

- persisted legacy field;
- DTO compatibility;
- test fixture;
- production decision that still needs migration.

- [ ] **Step 2: Add synchronization helper**

In `finance/planning.rs`, add:

```rust
pub fn sync_legacy_budget_index(team: &mut Team) {
    team.budget = derive_budget_index_from_money(team);
}
```

- [ ] **Step 3: Replace promotion/relegation direct budget deltas**

In `promotion/effects.rs`, stop applying `budget_delta` directly as source of truth.

Preferred:

```rust
team.cash_balance += promotion_cash_delta_or_relegation_loss(...);
sync_legacy_budget_index(team);
```

Keep `budget_delta` in event structs only if UI/history still needs it.

- [ ] **Step 4: Ensure save/load preserves compatibility**

`db/queries/teams.rs` can keep loading/saving `budget`, but business logic should call `sync_legacy_budget_index` after mutating financial state.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test db::queries::teams
cargo test promotion::effects
cargo test finance::
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/finance/planning.rs src-tauri/src/db/queries/teams.rs src-tauri/src/promotion/effects.rs src-tauri/src/market/preseason.rs
git commit -m "refactor: keep legacy budget derived from money"
```

---

## Chunk 9: Final Verification

### Task 13: Full regression suite

**Files:**
- No new files expected.

- [ ] **Step 1: Format Rust**

Run:

```bash
cargo fmt
```

Expected: no formatting errors.

- [ ] **Step 2: Backend targeted tests**

Run:

```bash
cargo test finance::
cargo test market::car_build_strategy
cargo test market::pit_strategy
cargo test commands::race -- --test-threads=1
cargo test commands::career -- --test-threads=1
cargo test initialize_preseason
```

Expected: PASS.

- [ ] **Step 3: Frontend tests**

Run:

```bash
npm test
npm run test:structure
```

Expected: PASS.

- [ ] **Step 4: Final budget audit**

Run:

```bash
rg -n "\.budget|budget\b" src-tauri/src src
```

Expected:

- Remaining occurrences are DTO compatibility, DB persistence, test fixtures, or explicitly documented legacy sync.
- No new production decision should use raw `team.budget`.

- [ ] **Step 5: Commit final audit edits if needed**

If a short migration note is useful, update:

- `docs/superpowers/specs/2026-04-13-real-money-budget-system-design.md`

Otherwise skip.

- [ ] **Step 6: Final commit if there were final edits**

```bash
git add <changed-files>
git commit -m "test: verify real money budget migration"
```

## Execution Notes

- Do not remove the `budget` database column in this implementation.
- Do not migrate all UI screens at once; start with `MyTeamTab`.
- Do not tune numbers by feel after one test. Add scenario tests first, then tune.
- Keep negative `spending_power` valid. Negative power is how the IA knows a team is in danger.
- Keep cash/debt values category-relative. A value that is rich in Mazda can be weak in GT3.
- If any chunk creates too many conflicts, stop after the previous commit and split that chunk.
- Preserve existing save compatibility. This feature changes decision logic, not save format removal.
