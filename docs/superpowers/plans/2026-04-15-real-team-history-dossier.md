# Real Team History Dossier Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fazer a ficha esportiva de qualquer equipe usar apenas fatos reais persistidos no backend.

**Architecture:** O backend expõe um comando `get_team_history_dossier` calculado de `race_results`, `calendar`, `seasons`, `standings` e `teams`. O frontend deixa de estimar a aba `Esportivo` e renderiza o payload real para a equipe selecionada, com estado vazio quando não houver histórico registrado.

**Tech Stack:** React, Tauri, Rust, rusqlite, Vitest, testes unitários Rust

---

## Chunk 1: Backend Real

### Task 1: Criar payload histórico real de equipe

**Files:**
- Modify: `src-tauri/src/commands/career_types.rs`
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/commands/career_commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Write failing backend test**

Criar fixtures com resultados reais em `race_results` para duas equipes da mesma categoria e validar que `get_team_history_dossier_in_base_dir` retorna corridas, vitórias, pódios, taxas, sequências e timeline sem usar pontos estimados.

- [ ] **Step 2: Run backend test**

Run: `cargo test get_team_history_dossier`
Expected: FAIL porque o comando ainda não existe.

- [ ] **Step 3: Implement backend command**

Adicionar tipos serializáveis e função que:
- agrupa categorias equivalentes por carro/grupo;
- agrega resultados por `equipe_id`;
- calcula corridas, vitórias, pódios, taxa de pódio, taxa de vitória;
- calcula sequência atual de temporadas e melhor sequência de pódios/pontos;
- retorna timeline real ou estado vazio.

- [ ] **Step 4: Run backend test**

Run: `cargo test get_team_history_dossier`
Expected: PASS

## Chunk 2: Frontend Real

### Task 2: Consumir dossiê real em todas as fichas

**Files:**
- Modify: `src/pages/tabs/MyTeamTab.jsx`
- Test: `src/pages/tabs/MyTeamTab.test.jsx`

- [ ] **Step 1: Write failing frontend test**

Validar que abrir a ficha da primeira equipe chama `get_team_history_dossier`, navegar para outra equipe chama o mesmo comando com outro `teamId`, e a aba `Esportivo` mostra os números do payload real.

- [ ] **Step 2: Run frontend test**

Run: `npx vitest run src/pages/tabs/MyTeamTab.test.jsx`
Expected: FAIL porque a tela ainda calcula localmente.

- [ ] **Step 3: Implement frontend integration**

Buscar o dossiê no drawer por `careerId`, `team.id` e `playerTeam.categoria`. Renderizar loading/erro/vazio e passar `historyDossier` para `buildTeamHistoryDossier`, usando backend para `records`, `sport`, `timeline`, `titleCategories` e `categoryPath`.

- [ ] **Step 4: Run frontend test**

Run: `npx vitest run src/pages/tabs/MyTeamTab.test.jsx`
Expected: PASS

## Chunk 3: Verification

### Task 3: Verificação focada

**Files:**
- Verify only: `src-tauri/src/commands/career.rs`
- Verify only: `src/pages/tabs/MyTeamTab.jsx`

- [ ] **Step 1: Backend focused**

Run: `cargo test get_team_history_dossier`

- [ ] **Step 2: Frontend focused**

Run: `npx vitest run src/pages/tabs/MyTeamTab.test.jsx`

- [ ] **Step 3: Build**

Run: `npm run build`
