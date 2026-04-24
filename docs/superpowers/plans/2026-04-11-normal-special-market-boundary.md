# Normal vs Special Market Boundary Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Impedir que `Production` e `Endurance` vazem para o mercado normal, preservando o contrato regular como fonte de verdade e tratando a convocacao especial como um vinculo temporario paralelo.

**Architecture:** A implementacao fica dividida em tres frentes pequenas. Primeiro blindamos as queries do backend para que historico e agentes livres da preseason usem apenas contratos regulares. Depois removemos categorias especiais da `PreSeasonView` e do fluxo visual do mercado normal. Por fim, validamos a fronteira entre `normal` e `especial` com testes de integracao, preservando os comportamentos ja corretos do pipeline de convocacao.

**Tech Stack:** React, Zustand, Rust, Tauri, rusqlite, Vitest, testes unitarios do backend

---

## Chunk 1: Blindagem do Backend Regular

### Task 1: Cobrir o historico regular dos agentes livres

**Files:**
- Modify: `src-tauri/src/db/queries/contracts.rs`
- Test: `src-tauri/src/db/queries/contracts.rs`

- [ ] **Step 1: Write the failing test**

Adicionar teste que cria um piloto com:
- contrato regular expirado por uma equipe regular;
- contrato especial expirado mais recente em `production_challenger` ou `endurance`.

O teste deve validar que `get_free_agents_for_preseason` usa:
- a categoria regular;
- a equipe regular;
- a continuidade regular;
e ignora o contrato especial ao montar o preview do mercado normal.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test get_free_agents_for_preseason`
Expected: FAIL porque a query atual usa qualquer contrato expirado/rescindido como ultima referencia.

- [ ] **Step 3: Write minimal implementation**

Restringir os subselects de `get_free_agents_for_preseason` para considerar apenas contratos `Regular` ao derivar:
- `categoria`;
- `prev_team_name`;
- `prev_team_color`;
- `seasons_at_team`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test get_free_agents_for_preseason`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/2026-04-11-normal-special-market-boundary.md src-tauri/src/db/queries/contracts.rs
git commit -m "fix: ignore special contracts in preseason free agent history"
```

### Task 2: Garantir que o payload da preseason continue regular

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Test: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Write the failing test**

Adicionar teste em `career.rs` que monta um save com historico especial e valida que `get_preseason_free_agents_in_base_dir` nao devolve:
- categoria especial;
- ultimo time especial;
- cor de time especial
como referencia do mercado normal.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test get_preseason_free_agents_in_base_dir`
Expected: FAIL enquanto o payload ainda refletir historico especial.

- [ ] **Step 3: Write minimal implementation**

Se necessario, ajustar o mapeamento do comando para preservar a leitura regular normalizada pela query e nao reintroduzir metadados especiais.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test get_preseason_free_agents_in_base_dir`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/career.rs
git commit -m "test: keep preseason free agent payload on regular axis"
```

## Chunk 2: Limite Visual do Mercado Normal

### Task 3: Remover categorias especiais da PreSeasonView

**Files:**
- Modify: `src/components/season/PreSeasonView.jsx`
- Test: `src/components/season/PreSeasonView.test.jsx`

- [ ] **Step 1: Write the failing test**

Adicionar testes cobrindo que a `PreSeasonView`:
- nao renderiza filtros de `Production` e `Endurance`;
- nao consulta standings dessas categorias ao entrar no mercado normal;
- nao mostra secoes especiais no mapeamento das equipes.

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/components/season/PreSeasonView.test.jsx`
Expected: FAIL porque a tela ainda inclui categorias especiais no conjunto `CATEGORIES`.

- [ ] **Step 3: Write minimal implementation**

Ajustar a `PreSeasonView` para operar apenas com categorias regulares no mercado normal:
- remover `production` e `endurance` dos filtros;
- manter agrupamento e badges apenas para o eixo regular;
- garantir que o grid nao faca fetch de categorias especiais.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/components/season/PreSeasonView.test.jsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/components/season/PreSeasonView.jsx src/components/season/PreSeasonView.test.jsx
git commit -m "fix: hide special categories from normal preseason market"
```

### Task 4: Revisar o estado visual entre preseason e convocation

**Files:**
- Modify: `src/stores/useCareerStore.js`
- Test: `src/stores/useCareerStore.test.js`

- [ ] **Step 1: Write the failing test**

Adicionar teste cobrindo a fronteira de UI:
- a preseason nao popula estado especial;
- a convocation continua carregando apenas ofertas especiais;
- voltar ao mercado normal nao reusa dados especiais em `playerProposals` ou `preseasonFreeAgents`.

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/stores/useCareerStore.test.js`
Expected: FAIL se houver vazamento de estado entre `showPreseason` e `showConvocation`.

- [ ] **Step 3: Write minimal implementation**

Limpar ou isolar campos de store para que:
- preseason use apenas estado regular;
- convocation use apenas estado especial;
- uma tela nao reutilize payload da outra.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/stores/useCareerStore.test.js`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/stores/useCareerStore.js src/stores/useCareerStore.test.js
git commit -m "fix: isolate preseason and convocation ui state"
```

## Chunk 3: Integracao e Invariantes de Dominio

### Task 5: Preservar explicitamente a fronteira entre regular e especial

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Test: `src-tauri/src/commands/career.rs`
- Reference: `src-tauri/src/convocation/pipeline.rs`

- [ ] **Step 1: Write the failing test**

Adicionar teste de integracao no carregamento da carreira que valide:
- fora de `BlocoEspecial`, `player_team` continua sendo resolvido pelo contrato regular;
- ao entrar no mercado normal, a carreira nao expoe `Production`/`Endurance` como eixo ativo;
- contratos especiais expirados nao alteram a apresentacao do time regular do jogador.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test load_career`
Expected: FAIL se a carga da carreira ainda misturar o contexto especial na entrada da preseason.

- [ ] **Step 3: Write minimal implementation**

Reforcar no `career.rs` a preferencia pelo contrato regular fora de `BlocoEspecial` e evitar fallback generico a lineups especiais quando a fase ativa for normal.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test load_career`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/career.rs
git commit -m "test: keep regular team authoritative outside special block"
```

### Task 6: Rodar verificacao final do boundary

**Files:**
- Verify only: `src-tauri/src/db/queries/contracts.rs`
- Verify only: `src-tauri/src/commands/career.rs`
- Verify only: `src/components/season/PreSeasonView.jsx`
- Verify only: `src/stores/useCareerStore.js`

- [ ] **Step 1: Run focused backend tests**

Run: `cargo test preseason`
Expected: PASS nas suites relevantes de preseason e carreira.

- [ ] **Step 2: Run focused special-flow tests**

Run: `cargo test convocation`
Expected: PASS, preservando os testes existentes que ja cobrem:
- separacao entre ofertas especiais e propostas de mercado;
- ausencia de contrato especial antes do aceite;
- manutencao da categoria regular dos pilotos convocados.

- [ ] **Step 3: Run focused frontend tests**

Run: `npx vitest run src/components/season/PreSeasonView.test.jsx src/stores/useCareerStore.test.js`
Expected: PASS

- [ ] **Step 4: Review residual risks**

Confirmar manualmente que nenhum helper generico restante consulta contratos especiais ao montar:
- listas do mercado normal;
- historico visual de free agents;
- grids da preseason.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/queries/contracts.rs src-tauri/src/commands/career.rs src/components/season/PreSeasonView.jsx src/stores/useCareerStore.js
git commit -m "fix: separate normal preseason market from special convocation axis"
```
