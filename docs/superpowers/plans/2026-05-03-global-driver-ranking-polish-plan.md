# Global Driver Ranking Polish Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Polish the hidden global driver ranking with clear loading feedback, richer driver context, clickable focus changes, player emphasis, retirement details, and filtering of drivers with no competitive history.

**Architecture:** Extend the existing `GlobalDriverRankingRow` payload instead of adding another command. Keep ranking eligibility and derived career/retirement fields in the Rust backend, then render those fields in the existing React tab. The Dashboard and standings double-click flow already activate the hidden tab, so the visible feedback should live in `GlobalDriversTab` loading state.

**Tech Stack:** Rust/Tauri commands, rusqlite, serde payload structs, React 18, Vitest, Testing Library, Tailwind utility classes.

---

Spec: `docs/superpowers/specs/2026-05-03-global-driver-ranking-polish-design.md`

Recommended skills for execution:

- `modo-carreira-padrao`
- `superpowers:test-driven-development`
- `frontend-design`
- `superpowers:verification-before-completion`

## File Structure

- Modify `src-tauri/src/commands/career_types.rs`
  - Add optional salary, career-year, and retirement fields to `GlobalDriverRankingRow`.
- Modify `src-tauri/src/commands/global_driver_rankings.rs`
  - Load active season year, include salary and career fields, parse retirement year, calculate retirement age in years, and filter rows with no competitive history before rank assignment.
- Modify `src/pages/tabs/GlobalDriversTab.jsx`
  - Add a proper loading screen, local focused driver state, clickable rows, player emphasis, richer summary/table fields, combined team/category label, retired tooltip, and new formatters.
- Modify `src/pages/tabs/GlobalDriversTab.test.jsx`
  - Cover loading, richer row fields, retired tooltip, focus changes, player emphasis, and filtered backend rows as represented by payload expectations.
- Optionally modify `src/pages/Dashboard.test.jsx`
  - Only if existing tests do not prove the hidden tab appears immediately after double click/open callback.

---

## Chunk 1: Backend Payload And Ranking Eligibility

### Task 1: Extend Global Ranking Row Types

**Files:**
- Modify: `src-tauri/src/commands/career_types.rs`
- Test: `src-tauri/src/commands/global_driver_rankings.rs`

- [ ] **Step 1: Write the failing backend usage test**

In `src-tauri/src/commands/global_driver_rankings.rs`, add a test that reads the new row fields from the payload:

```rust
#[test]
fn payload_includes_salary_career_and_retirement_context() {
    let conn = setup_conn();
    conn.execute(
        "UPDATE seasons SET numero = 2, ano = 2026 WHERE status = 'EmAndamento'",
        [],
    )
    .expect("update active season");

    let mut active = driver_with_stats("D_ACTIVE", "Piloto Ativo", Some("gt4"), 3, 5, 0);
    active.ano_inicio_carreira = 2020;
    insert_driver(&conn, &active).expect("insert active");

    conn.execute(
        "INSERT INTO teams (id, nome, nome_curto, categoria, ativa, marca, classe, piloto_1_id, piloto_2_id)
         VALUES ('T_GT4', 'Equipe Azul', 'AZL', 'gt4', 1, 'Marca', 'GT4', 'D_ACTIVE', NULL)",
        [],
    )
    .expect("insert team");
    conn.execute(
        "INSERT INTO contracts (
            id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
            duracao_anos, temporada_fim, salario, salario_anual, papel, status, tipo, categoria, created_at
        ) VALUES (
            'C_ACTIVE', 'D_ACTIVE', 'Piloto Ativo', 'T_GT4', 'Equipe Azul', 2,
            1, 2, 250000, 250000, 'Numero1', 'Ativo', 'Regular', 'gt4', CURRENT_TIMESTAMP
        )",
        [],
    )
    .expect("insert contract");

    conn.execute(
        "INSERT INTO retired (piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            "D_RET",
            "Lenda Aposentada",
            "2024",
            "gt3",
            r#"{"vitorias": 7, "podios": 12, "titulos": 1, "corridas": 30, "pontos": 220, "ano_inicio_carreira": 2018}"#,
            "Aposentadoria"
        ],
    )
    .expect("insert retired");

    let payload = build_global_driver_rankings(&conn, None).expect("payload");
    let active = payload.rows.iter().find(|row| row.id == "D_ACTIVE").unwrap();
    let retired = payload.rows.iter().find(|row| row.id == "D_RET").unwrap();

    assert_eq!(active.salario_anual, Some(250000.0));
    assert_eq!(active.ano_inicio_carreira, Some(2020));
    assert_eq!(active.anos_carreira, Some(7));
    assert_eq!(retired.temporada_aposentadoria.as_deref(), Some("2024"));
    assert_eq!(retired.anos_aposentado, Some(2));
    assert_eq!(retired.anos_carreira, Some(7));
}
```

- [ ] **Step 2: Run the backend test to verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml global_driver_rankings::tests::payload_includes_salary_career_and_retirement_context -- --nocapture
```

Expected: compile failure because the new fields do not exist.

- [ ] **Step 3: Add the payload fields**

In `GlobalDriverRankingRow`, add these fields after `categoria_atual`:

```rust
pub salario_anual: Option<f64>,
pub ano_inicio_carreira: Option<i32>,
pub anos_carreira: Option<i32>,
pub temporada_aposentadoria: Option<String>,
pub anos_aposentado: Option<i32>,
```

- [ ] **Step 4: Run the backend test again**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml global_driver_rankings::tests::payload_includes_salary_career_and_retirement_context -- --nocapture
```

Expected: compile errors in row construction sites that still need the new fields.

### Task 2: Populate Salary, Career Years, And Retirement Years

**Files:**
- Modify: `src-tauri/src/commands/global_driver_rankings.rs`
- Test: `src-tauri/src/commands/global_driver_rankings.rs`

- [ ] **Step 1: Import season queries**

Add `seasons` to backend imports:

```rust
use crate::db::queries::seasons as season_queries;
```

- [ ] **Step 2: Load active career year once**

At the start of `build_global_driver_rankings`, load the active year:

```rust
let current_year = season_queries::get_active_season(conn)
    .map_err(|e| format!("Falha ao carregar temporada ativa do ranking global: {e}"))?
    .map(|season| season.ano)
    .unwrap_or(2024);
```

Pass `current_year` into `build_current_driver_row` and `build_retired_driver_row`.

- [ ] **Step 3: Store retirement metadata in the snapshot helper**

Extend `RetiredDriverSnapshot`:

```rust
retirement_season: String,
career_start_year: Option<i32>,
career_years: Option<i32>,
```

Update the retired query to select `temporada_aposentadoria`:

```sql
SELECT piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas FROM retired
```

Parse these optional JSON keys from `estatisticas`:

```rust
let career_start_year = json_i32_option(&snapshot, "ano_inicio_carreira");
let career_years = json_i32_option(&snapshot, "anos_carreira");
```

Add a helper:

```rust
fn json_i32_option(value: &Value, key: &str) -> Option<i32> {
    value.get(key).and_then(Value::as_i64).map(|value| value as i32)
}
```

- [ ] **Step 4: Add date calculation helpers**

Add:

```rust
fn years_since(start_year: i32, current_year: i32) -> Option<i32> {
    if start_year <= 0 || current_year <= 0 || current_year < start_year {
        return None;
    }
    Some(current_year - start_year + 1)
}

fn parse_year(value: &str) -> Option<i32> {
    value.trim().parse::<i32>().ok().filter(|year| *year > 0)
}
```

- [ ] **Step 5: Populate active driver fields**

In `build_current_driver_row`, set:

```rust
salario_anual: contract.as_ref().map(|value| value.salario_anual),
ano_inicio_carreira: Some(driver.ano_inicio_carreira as i32),
anos_carreira: years_since(driver.ano_inicio_carreira as i32, current_year),
temporada_aposentadoria: None,
anos_aposentado: None,
```

- [ ] **Step 6: Populate retired driver fields**

In `build_retired_driver_row`, calculate:

```rust
let retirement_year = parse_year(&retired.retirement_season);
let career_years = retired
    .career_years
    .or_else(|| retired.career_start_year.and_then(|start| retirement_year.and_then(|end| years_since(start, end))));
let years_retired = retirement_year
    .map(|year| (current_year - year).max(0));
```

Set:

```rust
salario_anual: None,
ano_inicio_carreira: retired.career_start_year,
anos_carreira: career_years,
temporada_aposentadoria: Some(retired.retirement_season),
anos_aposentado: years_retired,
```

- [ ] **Step 7: Verify the metadata test passes**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml global_driver_rankings::tests::payload_includes_salary_career_and_retirement_context -- --nocapture
```

Expected: test passes.

### Task 3: Filter Drivers With No Competitive History

**Files:**
- Modify: `src-tauri/src/commands/global_driver_rankings.rs`
- Test: `src-tauri/src/commands/global_driver_rankings.rs`

- [ ] **Step 1: Write the failing filter test**

Add:

```rust
#[test]
fn payload_excludes_drivers_without_competitive_history() {
    let conn = setup_conn();
    let empty = driver_with_stats("D_EMPTY", "Sem Historico", Some("mazda_rookie"), 0, 0, 0);
    let scorer = driver_with_stats("D_SCORE", "Com Pontos", Some("mazda_rookie"), 0, 0, 0);
    insert_driver(&conn, &empty).expect("insert empty");
    insert_driver(&conn, &scorer).expect("insert scorer");
    conn.execute(
        "UPDATE drivers SET stats_pontos = 12.0, stats_corridas = 2 WHERE id = 'D_SCORE'",
        [],
    )
    .expect("update scorer stats");

    let payload = build_global_driver_rankings(&conn, None).expect("payload");

    assert!(payload.rows.iter().all(|row| row.id != "D_EMPTY"));
    assert!(payload.rows.iter().any(|row| row.id == "D_SCORE"));
}
```

Adjust column names if the local schema uses different persisted names for career points/races; inspect `src-tauri/src/db/queries/drivers.rs` before implementing.

- [ ] **Step 2: Run the filter test to verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml global_driver_rankings::tests::payload_excludes_drivers_without_competitive_history -- --nocapture
```

Expected: `D_EMPTY` is still present.

- [ ] **Step 3: Add the ranking eligibility helper**

Add:

```rust
fn has_competitive_history(row: &GlobalDriverRankingRow) -> bool {
    row.historical_index > 0.0
        || row.corridas > 0
        || row.pontos > 0
        || row.titulos > 0
        || row.vitorias > 0
        || row.podios > 0
        || row.poles > 0
        || row.dnfs > 0
}
```

- [ ] **Step 4: Filter before ranks are assigned**

In `build_global_driver_rankings`, after adding current and retired rows and before `assign_ranks`:

```rust
rows.retain(has_competitive_history);
```

- [ ] **Step 5: Verify backend ranking tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml global_driver_rankings -- --nocapture
```

Expected: all global ranking tests pass.

- [ ] **Step 6: Commit backend payload work**

Stage only the backend files touched in this chunk:

```bash
git add src-tauri/src/commands/career_types.rs src-tauri/src/commands/global_driver_rankings.rs
git commit -m "feat: enrich global driver ranking payload"
```

---

## Chunk 2: Global Ranking UI Polish

### Task 4: Add Loading Screen

**Files:**
- Modify: `src/pages/tabs/GlobalDriversTab.jsx`
- Test: `src/pages/tabs/GlobalDriversTab.test.jsx`

- [ ] **Step 1: Write the failing loading test**

Use a deferred promise so the component stays loading:

```jsx
it("shows a dedicated loading screen while the global ranking is loading", () => {
  let resolvePayload;
  invoke.mockReturnValue(new Promise((resolve) => {
    resolvePayload = resolve;
  }));

  render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

  expect(screen.getByText(/Ranking mundial de pilotos/i)).toBeInTheDocument();
  expect(screen.getByText(/Calculando historico global/i)).toBeInTheDocument();

  resolvePayload({ selected_driver_id: "D001", rows, leaders: {} });
});
```

- [ ] **Step 2: Run the UI test to verify RED**

Run:

```bash
npm run test:ui -- src/pages/tabs/GlobalDriversTab.test.jsx
```

Expected: loading text is missing or current empty table is rendered instead.

- [ ] **Step 3: Add a loading branch**

In `GlobalDriversTab`, keep the top back button available and return a focused loading panel when `loading` is true:

```jsx
if (loading) {
  return (
    <div className="space-y-5">
      <GlassCard hover={false} className="rounded-[30px]">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">Ranking mundial</p>
            <h2 className="mt-2 text-3xl font-semibold text-text-primary">Ranking mundial de pilotos</h2>
          </div>
          <button type="button" onClick={onBack} className="rounded-2xl border border-white/10 bg-white/[0.04] px-4 py-2 text-xs font-semibold uppercase tracking-[0.16em] text-text-secondary transition-glass hover:text-text-primary">
            Voltar para Classificacao
          </button>
        </div>
        <div className="mt-8 rounded-[24px] border border-accent-primary/25 bg-accent-primary/10 p-6">
          <p className="text-[10px] uppercase tracking-[0.2em] text-accent-primary">Calculando historico global</p>
          <div className="mt-4 h-2 overflow-hidden rounded-full bg-white/10">
            <div className="h-full w-1/3 animate-pulse rounded-full bg-accent-primary" />
          </div>
        </div>
      </GlassCard>
    </div>
  );
}
```

Extract repeated header/back markup to a small local component if it keeps JSX cleaner.

- [ ] **Step 4: Verify loading test passes**

Run:

```bash
npm run test:ui -- src/pages/tabs/GlobalDriversTab.test.jsx
```

Expected: test passes or only later tests fail because new fields are not rendered yet.

### Task 5: Make Rows Clickable And Separate Focus From Player Emphasis

**Files:**
- Modify: `src/pages/tabs/GlobalDriversTab.jsx`
- Test: `src/pages/tabs/GlobalDriversTab.test.jsx`

- [ ] **Step 1: Write the failing focus test**

Add a fourth row where `is_jogador: true`, then assert focus can move independently:

```jsx
it("changes the focused driver when a ranking row is clicked and keeps player emphasis", async () => {
  render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

  await screen.findByText(/Piloto em foco: Piloto Selecionado/i);
  fireEvent.click(screen.getByText("Piloto Livre"));

  expect(screen.getByText(/Piloto em foco: Piloto Livre/i)).toBeInTheDocument();
  expect(screen.getByText("Piloto Usuario").closest("tr")).toHaveClass("border-accent-primary/50");
  expect(screen.getByText(/Voce/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the UI test to verify RED**

Run:

```bash
npm run test:ui -- src/pages/tabs/GlobalDriversTab.test.jsx
```

Expected: click does not change the focus card.

- [ ] **Step 3: Add local focused driver state**

In `GlobalDriversTab`:

```jsx
const [focusedDriverId, setFocusedDriverId] = useState(selectedDriverId ?? null);

useEffect(() => {
  setFocusedDriverId(selectedDriverId ?? null);
}, [selectedDriverId]);
```

Resolve selected driver with `focusedDriverId`:

```jsx
const focusedDriver = rows.find((row) => row.id === focusedDriverId) ?? rows.find((row) => row.id === selectedDriverId) ?? rows[0] ?? null;
```

Use `focusedDriver` in the summary.

- [ ] **Step 4: Add row click and classes**

On each `<tr>`:

```jsx
onClick={() => setFocusedDriverId(row.id)}
className={[
  "cursor-pointer border-b border-white/6 last:border-0 transition-glass hover:bg-white/[0.04]",
  row.id === focusedDriver?.id ? "bg-accent-primary/12 ring-1 ring-accent-primary/40" : "",
  row.is_jogador ? "border-l-2 border-l-accent-primary/70" : "",
  row.status === "Livre" ? "opacity-60" : "",
  row.status === "Aposentado" ? "opacity-50" : "",
].join(" ")}
```

Add a small `Voce` marker next to the player name.

- [ ] **Step 5: Verify focus tests pass**

Run:

```bash
npm run test:ui -- src/pages/tabs/GlobalDriversTab.test.jsx
```

Expected: focus and player emphasis tests pass.

### Task 6: Render Team/Category, Age, Career, Salary, And Retired Tooltip

**Files:**
- Modify: `src/pages/tabs/GlobalDriversTab.jsx`
- Test: `src/pages/tabs/GlobalDriversTab.test.jsx`

- [ ] **Step 1: Expand test rows**

Update mocked rows with fields:

```jsx
salario_anual: 250000,
ano_inicio_carreira: 2020,
anos_carreira: 7,
temporada_aposentadoria: null,
anos_aposentado: null,
```

For the retired row:

```jsx
salario_anual: null,
ano_inicio_carreira: 2018,
anos_carreira: 7,
temporada_aposentadoria: "2024",
anos_aposentado: 2,
```

- [ ] **Step 2: Write the failing display test**

Add:

```jsx
it("renders team/category, age, career years, salary, and retired tooltip", async () => {
  render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

  const table = await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });

  expect(within(table).getByText(/Equipe Azul \/ GT4/i)).toBeInTheDocument();
  expect(within(table).getByText("28")).toBeInTheDocument();
  expect(within(table).getByText(/7 anos/i)).toBeInTheDocument();
  expect(within(table).getByText(/\$250k|250/i)).toBeInTheDocument();

  const retiredStatus = within(table).getByText(/Aposentado ha 2 anos/i);
  expect(retiredStatus).toHaveAttribute("title", "Aposentado em 2024");
});
```

- [ ] **Step 3: Run the UI test to verify RED**

Run:

```bash
npm run test:ui -- src/pages/tabs/GlobalDriversTab.test.jsx
```

Expected: new labels/columns are missing.

- [ ] **Step 4: Add columns and sorters**

Add sorters:

```jsx
idade: (row) => row.idade ?? 0,
anos_carreira: (row) => row.anos_carreira ?? 0,
salario_anual: (row) => row.salario_anual ?? 0,
```

Add headers near `Equipe/Categoria`:

```jsx
<SortableHeader label="Idade" sortKey="idade" sort={sort} onSort={handleSort} />
<SortableHeader label="Carreira" sortKey="anos_carreira" sort={sort} onSort={handleSort} />
<SortableHeader label="Salario" sortKey="salario_anual" sort={sort} onSort={handleSort} />
```

- [ ] **Step 5: Add display helpers**

```jsx
function teamCategoryLabel(row) {
  const category = categoryLabel(row.categoria_atual);
  if (row.equipe_nome) return `${row.equipe_nome} / ${category}`;
  if (row.status === "Aposentado") return `Aposentado / ${category}`;
  if (row.status === "Livre" && category !== "-") return `Livre / ${category}`;
  return row.status === "Livre" ? "Livre" : category;
}

function formatYears(value) {
  return value == null || value <= 0 ? "-" : `${value} anos`;
}

function formatMoney(value) {
  if (value == null || value <= 0) return "-";
  if (value >= 1000000) return `$${(value / 1000000).toLocaleString("pt-BR", { maximumFractionDigits: 1 })}M`;
  return `$${Math.round(value / 1000).toLocaleString("pt-BR")}k`;
}

function statusLabel(row) {
  if (row.status === "Aposentado" && row.anos_aposentado != null) {
    return `Aposentado ha ${row.anos_aposentado} anos`;
  }
  return row.status;
}

function statusTitle(row) {
  if (row.status === "Aposentado" && row.temporada_aposentadoria) {
    return `Aposentado em ${row.temporada_aposentadoria}`;
  }
  return undefined;
}
```

- [ ] **Step 6: Render the new cells**

Replace the team/category cell text with:

```jsx
{teamCategoryLabel(row)}
```

Render status as:

```jsx
<span className={statusClass(row.status)} title={statusTitle(row)}>
  {statusLabel(row)}
</span>
```

Add cells:

```jsx
<MetricCell value={row.idade || "-"} />
<td className="px-4 py-3 font-mono text-text-primary">{formatYears(row.anos_carreira)}</td>
<td className="px-4 py-3 font-mono text-text-primary">{formatMoney(row.salario_anual)}</td>
```

- [ ] **Step 7: Verify UI tests**

Run:

```bash
npm run test:ui -- src/pages/tabs/GlobalDriversTab.test.jsx
```

Expected: all `GlobalDriversTab` tests pass.

- [ ] **Step 8: Commit UI polish**

Stage only UI files touched:

```bash
git add src/pages/tabs/GlobalDriversTab.jsx src/pages/tabs/GlobalDriversTab.test.jsx
git commit -m "feat: polish global driver ranking UI"
```

---

## Chunk 3: Integration And Regression Verification

### Task 7: Check Hidden Tab Open Feedback

**Files:**
- Optional modify: `src/pages/Dashboard.test.jsx`

- [ ] **Step 1: Inspect existing Dashboard hidden tab test**

Run:

```bash
npm run test:ui -- src/pages/Dashboard.test.jsx
```

Expected: existing hidden tab tests pass.

- [ ] **Step 2: Add Dashboard test only if coverage is missing**

If there is no test proving `openGlobalDrivers` renders `GlobalDriversTab` immediately, add one with the existing mock pattern:

```jsx
expect(screen.getByText("Abrir panorama")).toBeInTheDocument();
fireEvent.click(screen.getByText("Abrir panorama"));
expect(screen.getByText(/Panorama D001|Ranking mundial/i)).toBeInTheDocument();
```

Use the existing local mock names in `Dashboard.test.jsx`.

- [ ] **Step 3: Verify Dashboard tests**

Run:

```bash
npm run test:ui -- src/pages/Dashboard.test.jsx
```

Expected: pass.

### Task 8: Full Verification

**Files:**
- No code changes expected.

- [ ] **Step 1: Run focused frontend tests**

Run:

```bash
npm run test:ui -- src/pages/tabs/GlobalDriversTab.test.jsx src/pages/Dashboard.test.jsx
```

Expected: pass.

- [ ] **Step 2: Run focused backend tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml global_driver_rankings -- --nocapture
```

Expected: pass.

- [ ] **Step 3: Run full frontend tests**

Run:

```bash
npm run test:ui
```

Expected: pass.

- [ ] **Step 4: Run build**

Run:

```bash
npm run build
```

Expected: Vite build exits 0. Existing chunk-size warnings are acceptable if unrelated.

- [ ] **Step 5: Manual smoke test**

Start the app through the repo's usual workflow. In the dashboard:

- double-click a standings driver and confirm the loading screen appears immediately;
- confirm the global table loads after the backend returns;
- confirm `Equipe / Categoria` appears for active contracted drivers;
- confirm retired rows show `Aposentado ha X anos`;
- hover a retired status and confirm the browser tooltip says `Aposentado em YYYY`;
- click another driver and confirm the top focus card changes;
- confirm the player row remains easy to locate with its own emphasis;
- confirm empty-history drivers no longer appear.

- [ ] **Step 6: Commit any integration-only test changes**

If `Dashboard.test.jsx` was changed:

```bash
git add src/pages/Dashboard.test.jsx
git commit -m "test: cover global ranking loading entry"
```

---

## Execution Notes

- Do not stage unrelated dirty files. This worktree already contains many modified files from earlier work.
- Keep accents consistent with the existing file style. Some source files currently use ASCII copy such as `Indice`, `Classificacao`, and `Lesoes`; match nearby UI text unless doing a dedicated copy pass.
- Do not change standings double-click semantics unless a regression appears. The hidden tab callback already exists.
