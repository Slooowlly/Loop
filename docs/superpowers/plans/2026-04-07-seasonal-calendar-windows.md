# Seasonal Calendar Windows Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Alinhar o ano esportivo da carreira às novas janelas mensais: mercado normal de dezembro a fevereiro, corridas regulares de fevereiro a agosto, janela especial curta na virada agosto/setembro e corridas especiais de setembro a dezembro.

**Architecture:** A regra temporal deve ser centralizada no calendário, com helpers que traduzem fase sazonal em janelas reais de mês. `market/preseason.rs` e os fluxos de convocação passam a consumir essa regra sem duplicar ranges mágicos, enquanto os testes deixam de validar cortes fixos por semana e passam a validar pertencimento à janela mensal correta.

**Tech Stack:** Rust, chrono, rusqlite, Tauri, React, Vitest, cargo test.

---

## Chunk 1: Backend temporal base

### Task 1: Centralizar as janelas mensais do ano esportivo

**Files:**
- Modify: `src-tauri/src/calendar/mod.rs`
- Modify: `src-tauri/src/models/enums.rs`
- Test: `src-tauri/src/calendar/mod.rs`

- [ ] **Step 1: Write the failing tests for seasonal windows**

Adicionar testes que afirmem:
- corridas regulares geradas têm `display_date` entre fevereiro e agosto;
- corridas especiais geradas têm `display_date` entre setembro e dezembro;
- nenhuma asserção depende mais de `2..40` ou `41..50`.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test calendar::mod::tests`

Expected: falhas ligadas às expectativas antigas de faixa semanal.

- [ ] **Step 3: Implement minimal calendar window helpers**

Criar helpers centrais no calendário para:
- mapear `BlocoRegular` para a janela `fevereiro..agosto`;
- mapear `BlocoEspecial` para a janela `setembro..dezembro`;
- distribuir `rodada -> data visual` dentro da janela mensal correta;
- preservar `week_of_year` consistente com a nova data gerada.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test calendar::mod::tests`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/calendar/mod.rs src-tauri/src/models/enums.rs
git commit -m "feat: align seasonal calendar windows"
```

### Task 2: Ajustar queries e invariantes do calendário

**Files:**
- Modify: `src-tauri/src/db/queries/calendar.rs`
- Test: `src-tauri/src/db/queries/calendar.rs`

- [ ] **Step 1: Write the failing tests for query expectations**

Atualizar testes para validar:
- especiais não aparecem antes do início da janela de setembro;
- consultas por `target_week` continuam respeitando a separação entre regular e especial;
- comentários e mensagens deixam de citar `40/41/50`.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test db::queries::calendar::tests`

Expected: FAIL nas expectativas antigas.

- [ ] **Step 3: Implement minimal query/test updates**

Revisar:
- comentários;
- asserts que dependem de faixas antigas;
- qualquer helper que derive comportamento a partir de semanas hardcoded.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test db::queries::calendar::tests`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/queries/calendar.rs
git commit -m "test: update calendar query expectations for monthly windows"
```

## Chunk 2: Preseason and seasonal handoff

### Task 3: Reancorar a pré-temporada no eixo dezembro → fevereiro

**Files:**
- Modify: `src-tauri/src/market/preseason.rs`
- Modify: `src-tauri/src/commands/career.rs`
- Test: `src-tauri/src/market/preseason.rs`
- Test: `src-tauri/src/commands/career.rs`
- Test: `src/components/season/PreSeasonView.test.jsx`

- [ ] **Step 1: Write the failing tests for preseason dates**

Adicionar ou ajustar testes que afirmem:
- `current_display_date` sempre cai na janela dezembro → fevereiro;
- avançar semana move a data dentro dessa janela;
- a data ancorada continua coerente com a primeira corrida regular.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test preseason`
Run: `cargo test test_get_preseason_state_returns_initialized_state`
Run: `npm test -- PreSeasonView.test.jsx`

Expected: FAIL nas expectativas antigas de data.

- [ ] **Step 3: Implement minimal preseason date changes**

Atualizar o cálculo da pré-temporada para:
- usar a primeira corrida regular em fevereiro como âncora;
- distribuir as semanas da pré-temporada para trás dentro do intervalo dezembro → fevereiro;
- evitar depender do relógio do PC ou de semanas antigas fixas.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test preseason`
Run: `cargo test test_get_preseason_state_returns_initialized_state`
Run: `npm test -- PreSeasonView.test.jsx`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/market/preseason.rs src-tauri/src/commands/career.rs src/components/season/PreSeasonView.test.jsx
git commit -m "feat: align preseason dates to december through february"
```

### Task 4: Ajustar a semântica de transição do bloco especial

**Files:**
- Modify: `src-tauri/src/convocation/pipeline.rs`
- Modify: `src/stores/useCareerStore.js`
- Test: `src-tauri/src/convocation/pipeline.rs`

- [ ] **Step 1: Write the failing tests for transition semantics**

Adicionar ou atualizar testes/comentários para refletir:
- `JanelaConvocacao` como transição curta entre agosto e setembro;
- `BlocoEspecial` como janela de setembro a dezembro;
- ausência de referências antigas a “semanas 41–50”.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test convocation::pipeline::tests`

Expected: FAIL ou inconsistências textuais/semânticas detectáveis nos asserts.

- [ ] **Step 3: Implement minimal semantic updates**

Atualizar:
- comentários e mensagens de erro;
- qualquer helper que ainda descreva a janela antiga;
- textos de store/frontend que apresentem a fase para o usuário.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test convocation::pipeline::tests`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/convocation/pipeline.rs src/stores/useCareerStore.js
git commit -m "refactor: describe special window using monthly season phases"
```

## Chunk 3: Final verification

### Task 5: Executar verificação integrada

**Files:**
- Verify: `src-tauri/src/calendar/mod.rs`
- Verify: `src-tauri/src/db/queries/calendar.rs`
- Verify: `src-tauri/src/market/preseason.rs`
- Verify: `src-tauri/src/convocation/pipeline.rs`
- Verify: `src/components/season/PreSeasonView.test.jsx`

- [ ] **Step 1: Run focused backend tests**

Run: `cargo test calendar::mod::tests`
Run: `cargo test db::queries::calendar::tests`
Run: `cargo test convocation::pipeline::tests`
Run: `cargo test preseason`

Expected: PASS.

- [ ] **Step 2: Run focused frontend test**

Run: `npm test -- PreSeasonView.test.jsx`

Expected: PASS.

- [ ] **Step 3: Run a broader smoke verification**

Run: `cargo test test_create_career_full_flow`

Expected: PASS com calendário sazonal coerente.

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "test: verify seasonal calendar window rollout"
```
