# Next Race Briefing Context Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Entregar no payload da carreira um bloco `next_race_briefing` com historico enxuto por pista, rival principal e noticias da etapa, e fazer a `NextRaceTab` consumir esses dados com testes.

**Architecture:** O backend Rust continua sendo a fonte de verdade para narrativa e contexto competitivo. A montagem do bloco acontece durante `load_career`, com helpers pequenos para contrato, historico, rival e noticias; o frontend apenas renderiza e preserva fallbacks seguros.

**Tech Stack:** Rust + Tauri commands, rusqlite queries, React, Zustand, Vitest, Testing Library

---

## File Structure

- Modify: `src-tauri/src/commands/career_types.rs`
  Responsibility: definir o contrato serializavel do novo bloco `next_race_briefing`.
- Modify: `src-tauri/src/commands/career.rs`
  Responsibility: montar o resumo da previa durante `load_career`.
- Modify: `src-tauri/src/db/queries/race_history.rs`
  Responsibility: expor consulta enxuta de resultados do jogador por pista, se necessario.
- Modify: `src-tauri/src/db/queries/news.rs`
  Responsibility: expor consulta filtrada por temporada, categoria e rodada, se necessario.
- Modify: `src/stores/useCareerStore.js`
  Responsibility: persistir `nextRaceBriefing` no estado da carreira.
- Modify: `src/pages/tabs/NextRaceTab.jsx`
  Responsibility: consumir o bloco novo e renderizar historico, rival e noticias.
- Modify: `src/pages/tabs/NextRaceTab.test.jsx`
  Responsibility: cobrir renderizacao e fallbacks.
- Modify: `src/stores/useCareerStore.test.js`
  Responsibility: cobrir carregamento e limpeza do novo campo.

## Chunk 1: Backend Contract

### Task 1: Adicionar structs do payload da previa

**Files:**
- Modify: `src-tauri/src/commands/career_types.rs`
- Test: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Write the failing test**

Adicionar ou ajustar um teste de serializacao/contrato em `src-tauri/src/commands/career.rs` esperando `next_race_briefing` no JSON retornado por `load_career`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test commands::career::tests::test_load_career_includes_next_race_briefing --manifest-path src-tauri/Cargo.toml`
Expected: FAIL com campo ausente ou erro de compilacao por tipo inexistente.

- [ ] **Step 3: Write minimal implementation**

Criar:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextRaceBriefingSummary { ... }
```

e os structs filhos em `career_types.rs`, adicionando:

```rust
pub next_race_briefing: Option<NextRaceBriefingSummary>,
```

em `CareerData`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test commands::career::tests::test_load_career_includes_next_race_briefing --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/career_types.rs src-tauri/src/commands/career.rs
git commit -m "feat: add next race briefing contract"
```

## Chunk 2: Backend Data Builders

### Task 2: Implementar historico enxuto por pista

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/db/queries/race_history.rs`
- Test: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Write the failing test**

Criar teste com duas ou mais visitas do jogador a mesma pista e esperar:

```rust
assert_eq!(briefing.track_history.starts, 2);
assert_eq!(briefing.track_history.best_finish, Some(3));
assert_eq!(briefing.track_history.last_finish, Some(5));
assert_eq!(briefing.track_history.dnfs, 1);
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test commands::career::tests::test_next_race_briefing_summarizes_track_history --manifest-path src-tauri/Cargo.toml`
Expected: FAIL com valores ausentes ou incorretos.

- [ ] **Step 3: Write minimal implementation**

Adicionar helper dedicado, preferencialmente:

```rust
fn build_track_history_summary(
    conn: &Connection,
    player_id: &str,
    track_name: &str,
) -> Result<TrackHistorySummary, String> { ... }
```

com consulta usando `race_results` + `calendar.track_name`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test commands::career::tests::test_next_race_briefing_summarizes_track_history --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/career.rs src-tauri/src/db/queries/race_history.rs
git commit -m "feat: add track history summary to race briefing"
```

### Task 3: Implementar rival principal pronto no payload

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Test: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Write the failing test**

Cobrir dois cenarios:

```rust
assert_eq!(briefing.primary_rival.as_ref().unwrap().driver_name, "Piloto P2");
assert!(briefing.primary_rival.as_ref().unwrap().is_ahead);
```

e

```rust
assert_eq!(briefing.primary_rival.as_ref().unwrap().driver_name, "Piloto P2");
assert!(!briefing.primary_rival.as_ref().unwrap().is_ahead);
```

para lider e nao lider.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test commands::career::tests::test_next_race_briefing_exposes_primary_rival --manifest-path src-tauri/Cargo.toml`
Expected: FAIL

- [ ] **Step 3: Write minimal implementation**

Adicionar helper:

```rust
fn build_primary_rival_summary(...) -> Option<PrimaryRivalSummary> { ... }
```

Reutilizar standings da categoria e, se houver dados de rivalidade ligados ao mesmo piloto, preencher `rivalry_label`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test commands::career::tests::test_next_race_briefing_exposes_primary_rival --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/career.rs
git commit -m "feat: add primary rival summary to race briefing"
```

### Task 4: Implementar noticias da etapa

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/db/queries/news.rs`
- Test: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Write the failing test**

Criar noticias de multiplas rodadas/categorias e validar:

```rust
assert_eq!(briefing.weekend_stories.len(), 3);
assert!(briefing.weekend_stories.iter().all(|item| item.title.len() > 0));
```

com filtro respeitando categoria + rodada da proxima corrida.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test commands::career::tests::test_next_race_briefing_filters_weekend_stories --manifest-path src-tauri/Cargo.toml`
Expected: FAIL

- [ ] **Step 3: Write minimal implementation**

Adicionar consulta/helper para selecionar noticias:

```rust
fn build_weekend_story_summaries(...) -> Vec<BriefingStorySummary> { ... }
```

Ordenar por importancia, prioridade editorial e timestamp.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test commands::career::tests::test_next_race_briefing_filters_weekend_stories --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/career.rs src-tauri/src/db/queries/news.rs
git commit -m "feat: add weekend stories to race briefing"
```

## Chunk 3: Store and UI Integration

### Task 5: Expor o campo novo no store

**Files:**
- Modify: `src/stores/useCareerStore.js`
- Test: `src/stores/useCareerStore.test.js`

- [ ] **Step 1: Write the failing test**

Adicionar expectativa para:

```js
expect(state.nextRaceBriefing).toEqual(mockCareerData.next_race_briefing);
```

e para limpeza em fluxos que zeram `nextRace`.

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test -- src/stores/useCareerStore.test.js`
Expected: FAIL

- [ ] **Step 3: Write minimal implementation**

Adicionar `nextRaceBriefing` ao `initialState`, `applyCareerData` e fluxos de limpeza.

- [ ] **Step 4: Run test to verify it passes**

Run: `npm test -- src/stores/useCareerStore.test.js`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/stores/useCareerStore.js src/stores/useCareerStore.test.js
git commit -m "feat: store next race briefing context"
```

### Task 6: Renderizar historico, rival e noticias na NextRaceTab

**Files:**
- Modify: `src/pages/tabs/NextRaceTab.jsx`
- Test: `src/pages/tabs/NextRaceTab.test.jsx`

- [ ] **Step 1: Write the failing test**

Adicionar cenarios para:

```js
expect(screen.getByText(/melhor resultado na pista/i)).toBeInTheDocument();
expect(screen.getByText(/rival principal/i)).toBeInTheDocument();
expect(screen.getByText(/no paddock/i)).toBeInTheDocument();
```

e fallback quando `nextRaceBriefing` nao existir.

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test -- src/pages/tabs/NextRaceTab.test.jsx`
Expected: FAIL

- [ ] **Step 3: Write minimal implementation**

Consumir `nextRaceBriefing` do store e:

- trocar o resumo de historico para leitura da pista
- plugar `primary_rival` nas copys contextuais
- renderizar um card/lista curta de noticias da etapa

- [ ] **Step 4: Run test to verify it passes**

Run: `npm test -- src/pages/tabs/NextRaceTab.test.jsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/pages/tabs/NextRaceTab.jsx src/pages/tabs/NextRaceTab.test.jsx
git commit -m "feat: show track history rival and stories in race briefing"
```

## Chunk 4: Verification

### Task 7: Rodar verificacao final direcionada

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/commands/career_types.rs`
- Modify: `src-tauri/src/db/queries/news.rs`
- Modify: `src-tauri/src/db/queries/race_history.rs`
- Modify: `src/stores/useCareerStore.js`
- Modify: `src/pages/tabs/NextRaceTab.jsx`
- Test: `src/stores/useCareerStore.test.js`
- Test: `src/pages/tabs/NextRaceTab.test.jsx`

- [ ] **Step 1: Run backend tests**

Run: `cargo test next_race_briefing --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 2: Run frontend tests**

Run: `npm test -- src/pages/tabs/NextRaceTab.test.jsx src/stores/useCareerStore.test.js`
Expected: PASS

- [ ] **Step 3: Run any targeted formatting or lint command used by the repo**

Run: `npm run test -- src/pages/tabs/NextRaceTab.test.jsx`
Expected: PASS or same as test runner expectation if aliased

- [ ] **Step 4: Review changed files for fallback behavior**

Verificar manualmente:

- sem `next_race`
- sem `next_race_briefing`
- sem noticias da etapa
- sem historico na pista

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/career.rs src-tauri/src/commands/career_types.rs src-tauri/src/db/queries/news.rs src-tauri/src/db/queries/race_history.rs src/stores/useCareerStore.js src/pages/tabs/NextRaceTab.jsx src/pages/tabs/NextRaceTab.test.jsx src/stores/useCareerStore.test.js
git commit -m "feat: enrich next race briefing context"
```
