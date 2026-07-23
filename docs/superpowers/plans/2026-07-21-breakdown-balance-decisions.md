# Breakdown Balance Decisions Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the approved climate, player-protection, sprint-DNF, and partial-enduro-service decisions with reproducible calibration evidence and consistent live/forecast behavior.

**Architecture:** Keep the existing breakdown engine as the single owner of live wear and severity, but introduce a small private balance-parameter seam so the diagnostic harness can sweep candidate values without duplicating the simulation. The iRacing monitor remains the authority for completed pit stops, the forecast uses the calendar's planned lap count, and the React card only renders the new preventive-service note when the backend marks it available.

**Tech Stack:** Rust, Tauri 2, deterministic Monte Carlo tests, iRacing SDK telemetry, React 18, Vitest, Testing Library, i18next.

**Design spec:** `docs/superpowers/specs/2026-07-21-breakdown-balance-decisions-design.md`

**Dirty-worktree rule:** Several target files already contain unrelated uncommitted work. Before every task, inspect `git diff -- <paths>`. Stage only the task's new hunks with `git add -p -- <paths>`, inspect `git diff --cached`, and never use `git add -A` or stage a whole overlapping file blindly.

---

## Chunk 1: Breakdown engine and calibration seam

### Task 1: Add a private balance-parameter seam without changing behavior

**Files:**
- Modify: `src-tauri/src/car/breakdown.rs:21-75`
- Modify: `src-tauri/src/car/breakdown.rs:680-880`
- Test: `src-tauri/src/car/breakdown.rs:1139-1980`

- [ ] **Step 1: Inspect and preserve the existing balance diff**

Run:

```powershell
git diff -- src-tauri/src/car/breakdown.rs
```

Expected: the already-applied risk-window, wall, condition-cap, severity-weight, forecast, and harness changes are visible. Save this output for comparison; do not revert it.

- [ ] **Step 2: Write the failing parameter-default test**

Add a test beside the existing calibration tests:

```rust
#[test]
fn parametros_padrao_reproduzem_os_dials_de_producao() {
    let p = BalanceParams::production();
    assert_eq!(p.conditions_max_mult, CONDITIONS_MAX_MULT);
    assert_eq!(p.sprint_dnf_scale, SPRINT_DNF_SCALE);
    assert_eq!(p.pit_service_relief, PIT_SERVICE_RELIEF);
}
```

- [ ] **Step 3: Run the test and confirm RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p loop parametros_padrao_reproduzem_os_dials_de_producao -- --nocapture
```

Expected: compilation fails because `BalanceParams`, `SPRINT_DNF_SCALE`, and `PIT_SERVICE_RELIEF` do not exist.

- [ ] **Step 4: Implement the minimal private parameter seam**

In `breakdown.rs`, add the initial production dials:

```rust
const SPRINT_DNF_SCALE: f64 = 0.60;
const PIT_SERVICE_RELIEF: f64 = 0.60;

#[derive(Debug, Clone, Copy)]
struct BalanceParams {
    conditions_max_mult: f64,
    sprint_dnf_scale: f64,
    pit_service_relief: f64,
}

impl BalanceParams {
    const fn production() -> Self {
        Self {
            conditions_max_mult: CONDITIONS_MAX_MULT,
            sprint_dnf_scale: SPRINT_DNF_SCALE,
            pit_service_relief: PIT_SERVICE_RELIEF,
        }
    }
}
```

Store `params: BalanceParams` in `LiveBreakdown`. Keep `LiveBreakdown::new(...)` public and behavior-compatible by delegating to a private `new_with_params(..., BalanceParams::production())`. Add a private `roll_race_breakdowns_cfg_with_params(...)`; the public wrappers pass production parameters, while the nested test module can pass candidates.

Change `conditions_mult` to accept the cap explicitly in the private path:

```rust
fn conditions_mult_with_cap(
    pt: PartType,
    track_pha: (f64, f64, f64),
    mean_align: f64,
    weather: Weather,
    cap: f64,
) -> f64 {
    (track_wear_mult(pt, track_pha, mean_align) * weather_wear_mult(pt, weather)).min(cap)
}
```

The production wrapper and `conditions_wear_mults` must continue using `CONDITIONS_MAX_MULT`. In `LiveBreakdown::advance_lap_at`, replace the production-only call with:

```rust
conditions_mult_with_cap(
    pt,
    self.track_pha,
    self.mean_align,
    weather,
    self.params.conditions_max_mult,
)
```

This connection is required: without it, `roll_race_breakdowns_cfg_with_params` would report identical live-risk results for every climate-cap candidate. Add a private `conditions_wear_mults_with_cap(...)` for the economic comparison in Task 7, while the public `conditions_wear_mults(...)` delegates with the production cap.

- [ ] **Step 5: Run focused and module tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p loop parametros_padrao_reproduzem_os_dials_de_producao -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml -p loop car::breakdown::tests --lib
```

Expected: the new test passes and all current breakdown tests remain green.

- [ ] **Step 6: Commit only the parameter-seam hunks**

```powershell
git add -p -- src-tauri/src/car/breakdown.rs
git diff --cached --check
git diff --cached
git commit -m "refactor: make breakdown balance dials testable"
```

### Task 2: Apply the global sprint-DNF retention filter and 6% player relief

**Files:**
- Modify: `src-tauri/src/car/breakdown.rs:529-625`
- Test: `src-tauri/src/car/breakdown.rs:1410-1469`
- Test: `src-tauri/src/car/breakdown.rs:1783-1830`

- [ ] **Step 1: Write failing severity-filter tests**

Extract the intended final operation into a pure private helper and test its boundaries:

```rust
#[test]
fn escala_de_dnf_do_sprint_rebaixa_so_dnf() {
    assert_eq!(scale_dnf(Severity::Dnf, 0.59, 0.60), Severity::Dnf);
    assert_eq!(scale_dnf(Severity::Dnf, 0.60, 0.60), Severity::Heavy);
    assert_eq!(scale_dnf(Severity::Heavy, 0.99, 0.00), Severity::Heavy);
    assert_eq!(scale_dnf(Severity::Light, 0.99, 0.00), Severity::Light);
}

#[test]
fn escala_de_dnf_do_sprint_nao_substitui_a_regra_de_enduro() {
    let count_enduro_dnf = |sprint_scale: f64| {
        (0..20_000)
            .filter(|k| {
                let r = (*k as f64 + 0.5) / 20_000.0;
                sample_severity_with_params(
                    PartType::Engine,
                    false,
                    r,
                    true,
                    BalanceParams {
                        sprint_dnf_scale: sprint_scale,
                        ..BalanceParams::production()
                    },
                ) == Severity::Dnf
            })
            .count()
    };
    let sem_reter_sprint = count_enduro_dnf(0.0);
    let reter_todo_sprint = count_enduro_dnf(1.0);
    assert!(sem_reter_sprint > 0, "motor estrutural ainda precisa poder dar DNF no enduro");
    assert_eq!(sem_reter_sprint, reter_todo_sprint);
}

#[test]
fn alivio_do_jogador_escala_de_zero_a_seis_porcento() {
    assert!((player_wear_relief(0.0) - 0.06).abs() < 1e-12);
    assert!((player_wear_relief(50.0) - 0.03).abs() < 1e-12);
    assert!(player_wear_relief(100.0).abs() < 1e-12);
}

#[test]
fn protecao_aprovada_reduz_quebra_relevante() {
    const N: u32 = 20_000;
    let raw = perfil("pobre_esticando");
    let protected = player_protected_car(&raw, 0.0);
    let relevant = |car: &Car| {
        (0..N)
            .filter(|i| {
                let seed = splitmix64(0x00C0_FFEE ^ splitmix64(*i as u64));
                roll_race_breakdowns_cfg_with_params(
                    car,
                    18,
                    seed,
                    0.0,
                    TRACK_NEUTRO,
                    WEATHER_NEUTRO,
                    &[],
                    false,
                    BalanceParams::production(),
                )
                .iter()
                .any(|e| matches!(e.severity, Severity::Heavy | Severity::Dnf))
            })
            .count() as f64
    };
    let raw_relevant = relevant(&raw);
    let protected_relevant = relevant(&protected);
    let reduction = 1.0 - protected_relevant / raw_relevant;
    assert!((0.20..=0.25).contains(&reduction), "redução relativa = {reduction:.3}");
}
```

- [ ] **Step 2: Run the tests and confirm RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p loop escala_de_dnf_do_sprint -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml -p loop alivio_do_jogador_escala -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml -p loop protecao_aprovada_reduz_quebra_relevante -- --nocapture
```

Expected: compilation fails because the helper and parameterized severity path do not exist.

- [ ] **Step 3: Implement deterministic DNF retention**

Implement:

```rust
fn scale_dnf(severity: Severity, keep_roll: f64, scale: f64) -> Severity {
    if severity == Severity::Dnf && keep_roll >= scale.clamp(0.0, 1.0) {
        Severity::Heavy
    } else {
        severity
    }
}
```

In `sample_severity_with_params`, keep the current base/forced calculation. If the result is DNF and `is_enduro == false`, derive a decorrelated roll with a dedicated salt and apply `params.sprint_dnf_scale`. If `is_enduro == true`, retain the existing structural/enduro filter and never apply the sprint scale.

Route production and calibration through that function inside `LiveBreakdown::advance_lap_at`:

```rust
let severity = sample_severity_with_params(
    pt,
    forced,
    roll(self.seed, i, lap, CH_SEVERITY),
    self.is_enduro,
    self.params,
);
```

Keep `sample_severity(...)` only as a production-parameter wrapper for existing direct unit tests.

Change `PLAYER_MAX_RELIEF` from `0.05` to `0.06`; do not change the weakness formula or give the protection to AI cars.

Update the final sprint assertion in `enduro_estrutural_mantem_so_a_fracao_de_dnf`, which currently assumes the full base DNF slice survives in sprint. Replace it with explicit scale boundaries:

```rust
let never_keep = BalanceParams { sprint_dnf_scale: 0.0, ..BalanceParams::production() };
let always_keep = BalanceParams { sprint_dnf_scale: 1.0, ..BalanceParams::production() };
assert_eq!(
    sample_severity_with_params(PartType::Engine, false, 0.95, false, never_keep),
    Severity::Heavy,
);
assert_eq!(
    sample_severity_with_params(PartType::Engine, false, 0.95, false, always_keep),
    Severity::Dnf,
);
```

- [ ] **Step 4: Run severity and player-protection tests**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p loop escala_de_dnf_do_sprint -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml -p loop protecao_aprovada_reduz_quebra_relevante -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml -p loop time_forte_nao_da_protecao_ao_jogador -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml -p loop car::breakdown::tests --lib
```

Expected: all pass. Remove or update any older overlapping protection assertion only when it measures the same `pobre_esticando`/pit-crew-0/`Heavy | Dnf` benchmark; do not encode the approved band against a different synthetic fleet.

- [ ] **Step 5: Commit only these balance hunks**

```powershell
git add -p -- src-tauri/src/car/breakdown.rs
git diff --cached --check
git diff --cached
git commit -m "feat: soften sprint DNFs and protect weak players"
```

### Task 3: Replace full pit replacement with partial enduro service

**Files:**
- Modify: `src-tauri/src/car/breakdown.rs:42-75`
- Modify: `src-tauri/src/car/breakdown.rs:736-760`
- Modify: `src-tauri/src/car/breakdown.rs:1000-1085`
- Test: `src-tauri/src/car/breakdown.rs:1831-1947`

- [ ] **Step 1: Write exact failing service tests**

Add tests that inspect `LiveBreakdown::wear` from the nested module:

```rust
#[test]
fn pit_enduro_remove_so_desgaste_adquirido_na_corrida() {
    let mut car = Car::uniform(3);
    car.set_wear(PartType::Engine, 0.40);
    let mut live = LiveBreakdown::new(&car, 1, PIT_NEUTRO, TRACK_NEUTRO).with_enduro(true);
    let i = PartType::ALL.iter().position(|&p| p == PartType::Engine).unwrap();
    live.wear[i] = 0.90;
    live.service_pit();
    assert!((live.wear[i] - 0.60).abs() < 1e-9);
}

#[test]
fn pit_enduro_nunca_apaga_desgaste_de_largada() {
    let mut car = Car::uniform(3);
    car.set_wear(PartType::Engine, 0.80);
    let mut live = LiveBreakdown::new(&car, 1, PIT_NEUTRO, TRACK_NEUTRO).with_enduro(true);
    let i = PartType::ALL.iter().position(|&p| p == PartType::Engine).unwrap();
    live.wear[i] = 1.00;
    live.service_pit();
    assert!((live.wear[i] - 0.88).abs() < 1e-9);
}

#[test]
fn pit_de_sprint_e_peca_abaixo_do_piso_nao_mudam() {
    let mut car = Car::uniform(3);
    car.set_wear(PartType::Engine, 0.40);
    let i = PartType::ALL.iter().position(|&p| p == PartType::Engine).unwrap();

    let mut sprint = LiveBreakdown::new(&car, 1, PIT_NEUTRO, TRACK_NEUTRO);
    sprint.wear[i] = 0.90;
    sprint.service_pit();
    assert!((sprint.wear[i] - 0.90).abs() < 1e-9);

    let mut below_floor = LiveBreakdown::new(&car, 1, PIT_NEUTRO, TRACK_NEUTRO).with_enduro(true);
    below_floor.wear[i] = 0.59;
    below_floor.service_pit();
    assert!((below_floor.wear[i] - 0.59).abs() < 1e-9);
}

#[test]
fn paradas_repetidas_nunca_cruzam_o_desgaste_de_largada() {
    let mut car = Car::uniform(3);
    car.set_wear(PartType::Engine, 0.40);
    let mut live = LiveBreakdown::new(&car, 1, PIT_NEUTRO, TRACK_NEUTRO).with_enduro(true);
    let i = PartType::ALL.iter().position(|&p| p == PartType::Engine).unwrap();
    live.wear[i] = 0.90;
    live.service_pit(); // 0.90 - (0.90 - 0.40) * 0.60 = 0.60
    live.service_pit(); // 0.60 - (0.60 - 0.40) * 0.60 = 0.48
    assert!((live.wear[i] - 0.48).abs() < 1e-9);
    assert!(live.wear[i] >= live.entered[i]);
}

#[test]
fn pit_nao_mexe_em_peca_quebrada_nem_carro_em_dnf() {
    let mut car = Car::uniform(3);
    car.set_wear(PartType::Engine, 0.40);
    let i = PartType::ALL.iter().position(|&p| p == PartType::Engine).unwrap();

    let mut broken = LiveBreakdown::new(&car, 1, PIT_NEUTRO, TRACK_NEUTRO).with_enduro(true);
    broken.wear[i] = 0.90;
    broken.broken[i] = true;
    broken.service_pit();
    assert!((broken.wear[i] - 0.90).abs() < 1e-9);

    let mut out = LiveBreakdown::new(&car, 1, PIT_NEUTRO, TRACK_NEUTRO).with_enduro(true);
    out.wear[i] = 0.90;
    out.out = true;
    out.service_pit();
    assert!((out.wear[i] - 0.90).abs() < 1e-9);
}
```

- [ ] **Step 2: Run and confirm RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p loop pit_enduro_ -- --nocapture
```

Expected: the old implementation resets eligible wear to zero, so the numeric assertions fail.

- [ ] **Step 3: Implement the approved formula and local enduro gate**

Replace `service_pit` with:

```rust
pub fn service_pit(&mut self) {
    if !self.is_enduro || self.out {
        return;
    }
    for i in 0..self.wear.len() {
        if self.broken[i] || self.wear[i] < SERVICE_WEAR_FLOOR {
            continue;
        }
        let gained = (self.wear[i] - self.entered[i]).max(0.0);
        self.wear[i] = (self.wear[i] - gained * self.params.pit_service_relief)
            .max(self.entered[i]);
    }
}
```

Add `BreakdownDirector::service_car(car_number) -> bool`, returning `false` only when the car number is not mapped; it delegates the sprint/enduro decision to `LiveBreakdown`.

Update the existing `parada_de_box_reduz_quebras_no_enduro` regression to call `roll_race_breakdowns_cfg(..., true)` (or the parameterized equivalent) for both serviced and unserviced runs. The current `roll_race_breakdowns(...)` call is a sprint path and must become a no-op after the local enduro gate is added.

- [ ] **Step 4: Add and run director-routing tests**

```rust
#[test]
fn diretor_encaminha_servico_so_para_carro_mapeado() {
    let mut dir = BreakdownDirector::new();
    let mut car = Car::uniform(3);
    car.set_wear(PartType::Engine, 0.40);
    let mut live = LiveBreakdown::new(&car, 1, PIT_NEUTRO, TRACK_NEUTRO).with_enduro(true);
    let i = PartType::ALL.iter().position(|&p| p == PartType::Engine).unwrap();
    live.wear[i] = 0.90;
    dir.add_car(7, live, vec![]);
    assert!(dir.service_car(7));
    assert!(!dir.service_car(99));
    let serviced = &dir.cars.get(&7).unwrap().live;
    assert!((serviced.wear[i] - 0.60).abs() < 1e-9);
}
```

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p loop pit_enduro_ -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml -p loop diretor_encaminha_servico -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml -p loop parada_de_box_reduz_quebras_no_enduro -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml -p loop car::breakdown::tests --lib
```

Expected: all pass.

- [ ] **Step 5: Commit the partial-service engine**

```powershell
git add -p -- src-tauri/src/car/breakdown.rs
git diff --cached --check
git diff --cached
git commit -m "feat: partially service worn parts in enduros"
```

---

## Chunk 2: Live pit wiring and forecast contract

### Task 4: Route completed four-second stops into the live breakdown director

**Files:**
- Modify: `src-tauri/src/car/breakdown.rs:35-75`
- Modify: `src-tauri/src/iracing_sdk/race_monitor.rs:103-108`
- Modify: `src-tauri/src/iracing_sdk/race_monitor.rs:1753-1810`
- Modify: `src-tauri/src/commands/race.rs:1948-1956`
- Test: `src-tauri/src/iracing_sdk/race_monitor.rs:3524-3900`

- [ ] **Step 1: Centralize the domain threshold with a failing test**

Add `pub const GENUINE_SERVICE_PIT_MIN_SECS: f64 = 4.0` to `car::breakdown` and a monitor helper:

```rust
fn grants_mechanical_service(stationary_secs: f64) -> bool {
    stationary_secs >= crate::car::breakdown::GENUINE_SERVICE_PIT_MIN_SECS
}
```

Write first:

```rust
#[test]
fn servico_mecanico_exige_quatro_segundos() {
    assert!(!grants_mechanical_service(2.5));
    assert!(!grants_mechanical_service(3.999));
    assert!(grants_mechanical_service(4.0));
}
```

- [ ] **Step 2: Run and confirm RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p loop servico_mecanico_exige_quatro_segundos -- --nocapture
```

Expected: compilation fails until the helper and shared constant exist.

- [ ] **Step 3: Wire the completed-stop transition exactly once**

Split completed-stop detection from history consumption:

```rust
fn capture_completed_pit_stops(&mut self, t: &IracingTelemetry, now: f64) -> usize
```

owns the existing `InPitStall → outside` state machine, queues accepted `PitStop` values in a new `pending_completed_pits: Vec<PitStop>`, routes mechanical service, and returns the number of mapped mechanical-service calls during that tick. The existing `capture_tire_strategy` keeps weather capture and drains `pending_completed_pits` into `history.pit_stops`; it no longer detects the transition a second time.

Before touching `pit_in_stall`, `capture_completed_pit_stops` must preserve the old `record_history` eligibility rules: return with state reset when the frame is qualification, no attempt is active, the active attempt number differs from `history.attempt_number`, the session is not Racing, the telemetry is replaying, `hist_session_num` differs from `t.session_num`, or the subsession changed. The first frame of a new attempt/session is therefore only settled by the later `record_history`; it cannot close a stale stop or service a car. If `record_history` finds no active attempt after `process_player`, clear `pending_completed_pits` before returning so a completed event cannot leak into another attempt.

Inside `capture_completed_pit_stops`, after closing the transition and computing `dwell`:

1. retain the existing `MIN_PIT_STALL_DWELL_SECS = 2.5` gate for `history.pit_stops` and tire inference;
2. when `grants_mechanical_service(dwell)` is true, read signed `car_number[i]`, require `number > 0`, convert only then to `u32`, and call `service_car(number as u32)` once;
3. do not add an enduro flag to the monitor — sprint no-op belongs to `LiveBreakdown`;
4. increment the returned count only when `service_car` reports a mapped car;
5. do not replay service by scanning `history.pit_stops` later.

Call `capture_completed_pit_stops(t, now)` in `observe` immediately before `process_player(t)`. Keep the weather/history consumer `capture_tire_strategy(t, now)` and `capture_player_sectors(t)` inside `record_history`. This ordering is mandatory: `process_player` calls `tick_breakdown_player`, and `tick_breakdown_grid` runs later in the same tick, so the completed pit service is applied before either lap-advance path while the stop is committed to history only after the existing history reset/gates have run.

- [ ] **Step 4: Add transition regression tests**

Extend the monitor tests to cover:

```rust
fn monitor_with_mapped_breakdown() -> RaceMonitor {
    let mut monitor = RaceMonitor::new();
    monitor.attempts.push(active_attempt());
    monitor.record_history(&race_frame(2, 1)); // settle history/session gates
    monitor.car_number[0] = 7;
    let live = crate::car::breakdown::LiveBreakdown::new(
        &crate::car::Car::uniform(3),
        1,
        50.0,
        (1.0, 1.0, 1.0),
    )
    .with_enduro(true);
    let mut director = crate::car::breakdown::BreakdownDirector::new();
    director.add_car(7, live, vec![]);
    monitor.breakdown = Some(director);
    monitor
}

#[test]
fn parada_de_tres_segundos_conta_para_pneu_mas_nao_para_mecanica() {
    let mut monitor = monitor_with_mapped_breakdown();
    let mut enter = race_frame(2, 1);
    enter.cars[0].track_surface = SURFACE_IN_PIT_STALL;
    assert_eq!(monitor.capture_completed_pit_stops(&enter, 100.0), 0);

    let mut leave = race_frame(2, 1);
    leave.cars[0].track_surface = SURFACE_ON_TRACK;
    assert_eq!(monitor.capture_completed_pit_stops(&leave, 103.0), 0);
    monitor.record_history(&leave); // drains the queued stop into tire history
    assert_eq!(monitor.history.pit_stops.len(), 1);
    assert!((monitor.history.pit_stops[0].stationary_secs - 3.0).abs() < 1e-9);
}

#[test]
fn saida_de_parada_de_quatro_segundos_rota_um_unico_servico() {
    let mut monitor = monitor_with_mapped_breakdown();
    let mut enter = race_frame(2, 1);
    enter.cars[0].track_surface = SURFACE_IN_PIT_STALL;
    assert_eq!(monitor.capture_completed_pit_stops(&enter, 100.0), 0);

    let mut leave = race_frame(2, 1);
    leave.cars[0].track_surface = SURFACE_ON_TRACK;
    assert_eq!(monitor.capture_completed_pit_stops(&leave, 104.0), 1);
    monitor.record_history(&leave);
    assert_eq!(monitor.history.pit_stops.len(), 1);

    assert_eq!(monitor.capture_completed_pit_stops(&leave, 105.0), 0);
    assert_eq!(monitor.history.pit_stops.len(), 1);
}

#[test]
fn quali_replay_e_primeiro_tick_de_sessao_nao_servem_pecas() {
    let mut monitor = monitor_with_mapped_breakdown();
    monitor.pit_in_stall[0] = true;
    monitor.pit_stall_enter_time[0] = 100.0;

    let mut qualy = race_frame(1, 2);
    qualy.cars[0].track_surface = SURFACE_ON_TRACK;
    monitor.qualy_session_num = 1;
    assert_eq!(monitor.capture_completed_pit_stops(&qualy, 104.0), 0);

    monitor.pit_in_stall[0] = true;
    monitor.pit_stall_enter_time[0] = 100.0;
    let mut replay = race_frame(2, 2);
    replay.is_replay_playing = true;
    assert_eq!(monitor.capture_completed_pit_stops(&replay, 104.0), 0);

    monitor.pit_in_stall[0] = true;
    monitor.pit_stall_enter_time[0] = 100.0;
    let changed_session = race_frame(3, 2);
    assert_eq!(monitor.capture_completed_pit_stops(&changed_session, 104.0), 0);
}

#[test]
fn nova_tentativa_na_mesma_sessao_nao_fecha_pit_antigo() {
    let mut monitor = monitor_with_mapped_breakdown();
    monitor.pit_in_stall[0] = true;
    monitor.pit_stall_enter_time[0] = 100.0;
    let mut restarted = active_attempt();
    restarted.number = 2;
    monitor.attempts.push(restarted);

    let exit = race_frame(2, 2);
    assert_eq!(monitor.capture_completed_pit_stops(&exit, 104.0), 0);
    assert!(monitor.pending_completed_pits.is_empty());
}

#[test]
fn observe_fecha_pit_antes_de_avancar_quebra() {
    let mut car = crate::car::Car::uniform(3);
    car.set_wear(crate::car::PartType::Engine, 0.85);

    // Find a deterministic seed where service→lap and lap→service put the engine on opposite
    // sides of RISK_OPEN. This makes the test observe ordering through the existing public
    // `parts_in_danger` contract, without adding test-only getters or setters.
    let (live_after_lap_1, expected_danger) = (0..10_000u64)
        .find_map(|seed| {
            let mut live = crate::car::breakdown::LiveBreakdown::new(
                &car,
                seed,
                50.0,
                (1.0, 1.0, 1.0),
            )
            .with_enduro(true);
            live.advance_lap_at(1, crate::car::breakdown::Weather::NEUTRAL, 0.10);
            if live.is_out() {
                return None;
            }
            let mut service_first = live.clone();
            service_first.service_pit();
            service_first.advance_lap_at(2, crate::car::breakdown::Weather::NEUTRAL, 0.20);
            let mut lap_first = live.clone();
            lap_first.advance_lap_at(2, crate::car::breakdown::Weather::NEUTRAL, 0.20);
            lap_first.service_pit();
            let danger = |state: &crate::car::breakdown::LiveBreakdown| {
                state.parts_in_danger().iter().any(|(_, part, _)| *part == crate::car::PartType::Engine)
            };
            (danger(&service_first) != danger(&lap_first)).then(|| (live, danger(&service_first)))
        })
        .expect("fixture must distinguish service-before-lap from lap-before-service");

    let mut monitor = RaceMonitor::new();
    monitor.attempts.push(active_attempt());
    monitor.record_history(&race_frame(2, 1));
    monitor.car_number[0] = 7;
    monitor.history.player_car_idx = 0;
    monitor.pit_in_stall[0] = true;
    monitor.pit_stall_enter_time[0] = 100.0;
    let mut director = crate::car::breakdown::BreakdownDirector::new();
    director.add_car(7, live_after_lap_1, vec![]);
    director.prime_lap(7, 1);
    monitor.breakdown = Some(director);

    let mut exit = race_frame(2, 1);
    exit.session_time = 104.0;
    exit.cars[0].track_surface = SURFACE_ON_TRACK;
    exit.cars[0].lap_completed = 2;
    exit.lap_completed = 2;
    monitor.observe(&exit);

    let actual_danger = monitor
        .breakdown
        .as_ref()
        .unwrap()
        .car_parts_in_danger(7)
        .iter()
        .any(|(_, part, _)| *part == crate::car::PartType::Engine);
    assert_eq!(actual_danger, expected_danger);
}
```

The ordering test must fail against the current `observe` order and pass only after pit capture moves before both player and grid advancement. Do not add test-only methods to production classes; use `parts_in_danger` as shown.

- [ ] **Step 5: Reuse the same constant in post-race economy**

Delete the local `GENUINE_PIT_MIN_SECS` in `commands/race.rs` and compare `stationary_secs` against `crate::car::breakdown::GENUINE_SERVICE_PIT_MIN_SECS`. Do not change the 2.5-second tire-history threshold.

- [ ] **Step 6: Run affected Rust tests**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p loop iracing_sdk::race_monitor::tests --lib
cargo test --manifest-path src-tauri/Cargo.toml -p loop commands::race::tests --lib
cargo test --manifest-path src-tauri/Cargo.toml -p loop car::breakdown::tests --lib
```

Expected: all pass.

- [ ] **Step 7: Commit only the pit-routing hunks**

```powershell
git add -p -- src-tauri/src/car/breakdown.rs src-tauri/src/iracing_sdk/race_monitor.rs src-tauri/src/commands/race.rs
git diff --cached --check
git diff --cached
git commit -m "feat: service enduro wear after genuine pit stops"
```

### Task 5: Make the forecast use planned laps and expose preventive-service context

**Files:**
- Modify: `src-tauri/src/commands/iracing.rs:2739-2940`
- Modify: `src-tauri/src/commands/iracing.rs:2970-3000`

- [ ] **Step 1: Write failing pure contract tests**

Add private forecast-routing helpers and a nested test module near the forecast code:

```rust
fn planned_breakdown_laps(calendar_laps: i32) -> u32 {
    calendar_laps.max(1) as u32
}

fn forecast_view_context(ctx: Option<&RaceBreakdownCtx>) -> (bool, u32) {
    ctx.map(|c| (c.is_enduro, c.laps)).unwrap_or((false, 0))
}

fn forecast_for_ctx(
    car: &crate::car::Car,
    ctx: &RaceBreakdownCtx,
    seed: u64,
    pit_crew_quality: f64,
    samples: u32,
) -> crate::car::breakdown::BreakdownForecast {
    crate::car::breakdown::forecast_breakdown_risk(
        car,
        ctx.laps,
        seed,
        pit_crew_quality,
        ctx.track_pha,
        ctx.weather,
        &[],
        samples,
        ctx.is_enduro,
    )
}

#[cfg(test)]
mod breakdown_forecast_tests {
    use super::*;

    #[test]
    fn previsao_usa_distancia_planejada_com_piso_de_uma_volta() {
        assert_eq!(planned_breakdown_laps(40), 40);
        assert_eq!(planned_breakdown_laps(0), 1);
    }

    fn ctx(laps: u32, is_enduro: bool) -> RaceBreakdownCtx {
        RaceBreakdownCtx {
            player_team_id: "team".into(),
            categoria: "gt3".into(),
            weather: crate::car::breakdown::Weather::NEUTRAL,
            track_pha: (1.0, 1.0, 1.0),
            ev_seed: 42,
            is_enduro,
            laps,
        }
    }

    #[test]
    fn contexto_da_view_distingue_indisponivel_sprint_e_enduro() {
        assert_eq!(forecast_view_context(None), (false, 0));
        assert_eq!(forecast_view_context(Some(&ctx(18, false))), (false, 18));
        assert_eq!(forecast_view_context(Some(&ctx(40, true))), (true, 40));
    }

    #[test]
    fn helper_de_previsao_encaminha_laps_e_enduro_do_contexto() {
        let car = crate::car::Car::uniform(3);
        let race = ctx(40, true);
        let actual = forecast_for_ctx(&car, &race, 42, 50.0, 50);
        let expected = crate::car::breakdown::forecast_breakdown_risk(
            &car,
            40,
            42,
            50.0,
            race.track_pha,
            race.weather,
            &[],
            50,
            true,
        );
        assert_eq!(actual, expected);
    }
}
```

Write the test before the helper.

- [ ] **Step 2: Run and confirm RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p loop breakdown_forecast_tests -- --nocapture
```

Expected: compilation fails until the helper exists.

- [ ] **Step 3: Extend the backend data contract**

Add to `BreakdownForecastView`:

```rust
pub preventive_service_available: bool,
pub forecast_laps: u32,
```

Add `laps: u32` to `RaceBreakdownCtx`, filled with `planned_breakdown_laps(race.voltas)`. Make both `get_breakdown_forecast` and `get_grid_breakdown_risk` call `forecast_for_ctx`; no direct `forecast_breakdown_risk` call may remain in either command body.

Make unavailable and available DTO construction use `forecast_view_context(None)` and `forecast_view_context(Some(&ctx))`, respectively. The unavailable DTO must therefore use `preventive_service_available: false` and `forecast_laps: 0`; the available DTO uses `preventive_service_available: ctx.is_enduro` and `forecast_laps: ctx.laps`. Continue passing `service_laps = &[]` inside `forecast_for_ctx` so the result explicitly means “no future preventive service.”

- [ ] **Step 4: Run focused tests and compile the library**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p loop breakdown_forecast_tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml -p loop --lib --no-run
rg -n "forecast_breakdown_risk\(" src-tauri/src/commands/iracing.rs
```

Expected: all three tests pass, all DTO construction sites compile, and `rg` finds one call inside `forecast_for_ctx` rather than separate hard-coded command calls.

- [ ] **Step 5: Commit only forecast-contract hunks**

```powershell
git add -p -- src-tauri/src/commands/iracing.rs
git diff --cached --check
git diff --cached
git commit -m "feat: forecast breakdowns across the planned distance"
```

### Task 6: Show the translated preventive-service note in the expanded risk card

**Files:**
- Create: `src/components/race/BreakdownRiskButton.test.jsx`
- Establish then modify: `src/components/race/BreakdownRiskButton.jsx:1-190` (currently untracked breakdown-feature work)
- Modify: `src/i18n/locales/pt-BR/common.json:911-925`
- Modify: `src/i18n/locales/en-US/common.json:911-925`

- [ ] **Step 1: Establish the existing untracked breakdown card as an in-scope baseline**

The component is currently `?? src/components/race/BreakdownRiskButton.jsx`, so `git add -p` has no tracked baseline. Review the whole file and confirm it contains only the breakdown-risk card already referenced by the existing `NextRaceTab` work, then commit that file alone before adding the new note:

```powershell
git status --short -- src/components/race/BreakdownRiskButton.jsx
Get-Content -Encoding utf8 -LiteralPath src/components/race/BreakdownRiskButton.jsx
git add -- src/components/race/BreakdownRiskButton.jsx
git diff --cached --check
git diff --cached
git commit -m "feat: add breakdown risk card"
```

Expected: status initially shows exactly `??` for this path; the cached diff contains one new breakdown-specific component and no other file. This explicit baseline commit is the exception to hunk staging for this untracked in-scope file. If the file is no longer untracked at execution time, skip this baseline commit and use hunk staging normally.

- [ ] **Step 2: Write the failing component tests**

Create a focused test with a minimal available forecast:

```jsx
import { fireEvent, render, screen } from "@testing-library/react";
import BreakdownRiskButton from "./BreakdownRiskButton";

const forecast = {
  available: true,
  dnf_prob: 0.04,
  overall_level: "baixo",
  forecast_laps: 40,
  preventive_service_available: true,
  parts: [],
};

it("shows the preventive-service note for an enduro forecast", () => {
  render(<BreakdownRiskButton forecast={forecast} />);
  fireEvent.click(screen.getByRole("button", { name: /risco de quebra/i }));
  expect(screen.getByText(/previsão sem manutenção futura/i)).toBeInTheDocument();
});

it("omits the preventive-service note for a sprint forecast", () => {
  render(
    <BreakdownRiskButton forecast={{ ...forecast, preventive_service_available: false }} />,
  );
  fireEvent.click(screen.getByRole("button", { name: /risco de quebra/i }));
  expect(screen.queryByText(/previsão sem manutenção futura/i)).not.toBeInTheDocument();
});
```

- [ ] **Step 3: Run and confirm RED**

```powershell
npm run test:ui -- src/components/race/BreakdownRiskButton.test.jsx
```

Expected: the note is absent.

- [ ] **Step 4: Add translations and render the conditional note**

Add under `nextRaceTab.labels`:

```json
"breakdownPreventiveServiceNote": "Previsão sem manutenção futura; uma parada preventiva pode reduzir o risco."
```

and the English equivalent:

```json
"breakdownPreventiveServiceNote": "Forecast without future servicing; a preventive stop may reduce the risk."
```

Below the parts list in the expanded modal, render a subdued note only when `forecast.preventive_service_available` is true. Do not show it in the compact card or for unavailable/sprint forecasts.

- [ ] **Step 5: Run UI and locale tests**

```powershell
npm run test:ui -- src/components/race/BreakdownRiskButton.test.jsx
node --test scripts/tests/text-encoding-sanity.test.mjs scripts/tests/portuguese-copy-accents.test.mjs
```

Expected: all pass.

- [ ] **Step 6: Commit only UI-note hunks**

```powershell
git add -- src/components/race/BreakdownRiskButton.test.jsx
git add -p -- src/components/race/BreakdownRiskButton.jsx src/i18n/locales/pt-BR/common.json src/i18n/locales/en-US/common.json
git diff --cached --check
git diff --cached
git commit -m "feat: explain preventive service in enduro forecasts"
```

---

## Chunk 3: Reproducible calibration and final verification

### Task 7: Build and run the approved calibration matrix

**Files:**
- Modify: `src-tauri/src/car/breakdown.rs:1139-1280`
- Modify: `src-tauri/src/car/breakdown.rs:1790-1980`
- Create: `docs/superpowers/validation/2026-07-21-breakdown-balance-results.md`

- [ ] **Step 1: Extend the ignored harness to accept `BalanceParams`**

Keep the existing profile formulas and fixed seed. Add a `medir_com_params(...)` wrapper that calls `roll_race_breakdowns_cfg_with_params`. Record three booleans per race: any event, at least one `Heavy | Dnf`, and at least one DNF.

Add helpers that:

1. sweep sprint scale candidates `[0.50, 0.55, 0.60, 0.65, 0.70]` at cap `1.5`;
2. choose the valid candidate closest to `0.60`, lower on a tie;
3. sweep cap candidates `[1.60, 1.65, 1.70, 1.75, 1.80]` using the selected sprint scale;
4. compute the equal-weight four-profile brutal-grid DNF;
5. compute the mean of the 11 economic multipliers against the cap-`1.5` baseline;
6. sweep pit relief from `0.00` through `1.00` in `0.05` steps for 40 laps with service at lap 20;
7. compare protected/unprotected `pobre_esticando` at pit crew 0 and verify pit crew 100 is unchanged.

Use `N = 20_000` and `0x00C0_FFEE` exactly as the spec requires.

- [ ] **Step 2: Add a failing production-dials assertion**

At the end of the ignored calibration test, assert that a valid candidate exists for every sweep and that `BalanceParams::production()` equals the selected values. Also assert all acceptance bands from spec section 7.

- [ ] **Step 3: Run the matrix and capture the first result**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p loop analise_taxa_quebra -- --ignored --nocapture 2>&1 | Tee-Object -FilePath "$env:TEMP\iracer-breakdown-calibration.txt"
```

Expected: the report prints every candidate. The test may fail because production still uses cap `1.5` or because `PIT_SERVICE_RELIEF = 0.60` is not the selected valid value. A failure saying no candidate passes is a product blocker: stop rather than widening ranges or changing hazard/weights.

- [ ] **Step 4: Set only the selected production constants**

Update `SPRINT_DNF_SCALE`, `CONDITIONS_MAX_MULT`, and `PIT_SERVICE_RELIEF` to the candidates selected by the fixed matrix. Do not change `RISK_OPEN`, `HARD_WALL`, hazard, severity weights, or `ENDURO_DNF_SCALE`.

- [ ] **Step 5: Re-run until GREEN and preserve complete evidence**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p loop analise_taxa_quebra -- --ignored --nocapture 2>&1 | Tee-Object -FilePath "$env:TEMP\iracer-breakdown-calibration-final.txt"
```

Expected: PASS with all candidates and selected values printed.

Create `docs/superpowers/validation/2026-07-21-breakdown-balance-results.md` containing:

- exact command and commit SHA;
- selected three constants;
- full candidate tables for sprint DNF, climate/economy, player protection, and pit relief;
- the final acceptance table with pass/fail marks;
- warnings from the run identified as pre-existing where applicable.

- [ ] **Step 6: Run focused regression after selecting constants**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p loop car::breakdown::tests --lib
cargo test --manifest-path src-tauri/Cargo.toml -p loop iracing_sdk::race_monitor::tests --lib
```

Expected: all pass.

- [ ] **Step 7: Commit calibration and evidence**

```powershell
git add -p -- src-tauri/src/car/breakdown.rs
git add -- docs/superpowers/validation/2026-07-21-breakdown-balance-results.md
git diff --cached --check
git diff --cached
git commit -m "test: calibrate breakdown balance decisions"
```

### Task 8: Update stale documentation and run the full verification gate

**Files:**
- Modify: `docs/superpowers/specs/2026-07-18-car-breakdown-system.md:1-15`
- Verify: `src-tauri/src/car/breakdown.rs`
- Verify: `src-tauri/src/iracing_sdk/race_monitor.rs`
- Verify: `src-tauri/src/commands/race.rs`
- Verify: `src-tauri/src/commands/iracing.rs`
- Verify: `src/components/race/BreakdownRiskButton.jsx`
- Verify: `src/components/race/BreakdownRiskButton.test.jsx`

- [ ] **Step 1: Mark the older calibration section as superseded**

Add a short note at the top of `2026-07-18-car-breakdown-system.md` pointing to the approved 2026-07-21 design and validation result. Do not rewrite the old document as though its historical numbers had never existed.

- [ ] **Step 2: Correct stale source comments**

In touched code only, replace references to the old 95–105% risk window, full pit replacement, fixed 18-lap enduro forecast, and old DNF weights. Keep comments factual and point to the new design where useful.

- [ ] **Step 3: Run Rust formatting and diff checks**

```powershell
cd src-tauri
cargo fmt --all -- --check
cd ..
git diff --check
```

Expected: both commands exit 0. If formatting is needed, run `cargo fmt --all`, inspect the diff, and retain only formatting in touched Rust files.

- [ ] **Step 4: Run the complete Rust suite**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p loop
```

Expected: all non-ignored tests pass; only documented pre-existing warnings remain.

- [ ] **Step 5: Run the complete frontend and structure suites**

```powershell
cd ..
npm run test:all
npm run build
```

Expected: Vitest, structure tests, and production build all pass.

- [ ] **Step 6: Verify no unrelated work was staged**

```powershell
git status --short
git diff --cached --name-only
git diff --cached
```

Expected: unrelated pre-existing changes remain unstaged and untouched.

- [ ] **Step 7: Commit documentation corrections if present**

```powershell
git add -p -- docs/superpowers/specs/2026-07-18-car-breakdown-system.md src-tauri/src/car/breakdown.rs src-tauri/src/iracing_sdk/race_monitor.rs src-tauri/src/commands/iracing.rs src-tauri/src/commands/race.rs
git diff --cached --check
git diff --cached
git commit -m "docs: align breakdown references with final balance"
```

- [ ] **Step 8: Record the final handoff evidence**

Append the final Rust test, frontend test, structure test, and build summaries to `docs/superpowers/validation/2026-07-21-breakdown-balance-results.md`. If that changes the evidence file, commit it separately:

```powershell
git add -- docs/superpowers/validation/2026-07-21-breakdown-balance-results.md
git commit -m "docs: record breakdown verification results"
```
