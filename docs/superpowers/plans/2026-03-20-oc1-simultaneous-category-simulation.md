# OC-1 Simultaneous Category Simulation Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Simular automaticamente as outras categorias quando o jogador corre, mantendo o calendario global sincronizado sem mudar o significado de `season.rodada_atual`.

**Architecture:** Um novo modulo `simulation/batch.rs` concentra o calculo proporcional e a simulacao automatica das outras categorias, reaproveitando a mesma persistencia de stats, calendario, historico e noticias usada pela corrida do jogador. `commands/race.rs` passa a montar um `RaceWeekendResult` com a corrida do jogador e um resumo do resto do grid.

**Tech Stack:** Rust, Tauri commands, SQLite via queries existentes, JSON sidecar de `race_results`, React/Zustand para o resultado expandido.

---

### Task 1: Contrato e calculo proporcional

**Files:**
- Create: `src-tauri/src/simulation/batch.rs`
- Modify: `src-tauri/src/simulation/mod.rs` or `src-tauri/src/lib.rs` module export if needed
- Test: `src-tauri/src/simulation/batch.rs`

- [ ] Escrever teste falhando para `races_should_be_completed` com proporcao intermediaria.
- [ ] Escrever teste falhando para ultima corrida forcando conclusao total.
- [ ] Implementar `races_should_be_completed`.

### Task 2: Query de corridas pendentes por categoria

**Files:**
- Modify: `src-tauri/src/db/queries/calendar.rs`
- Test: `src-tauri/src/db/queries/calendar.rs`

- [ ] Escrever teste falhando para `get_pending_races_for_category`.
- [ ] Implementar query ordenada por rodada.

### Task 3: Pipeline compartilhado de simulacao

**Files:**
- Create: `src-tauri/src/simulation/batch.rs`
- Modify: `src-tauri/src/commands/race.rs`
- Test: `src-tauri/src/simulation/batch.rs`

- [ ] Escrever teste falhando para simular categoria sem o jogador.
- [ ] Extrair helper comum de montagem de grid e persistencia.
- [ ] Implementar `simulate_category_race`.

### Task 4: Simulacao das outras categorias

**Files:**
- Create: `src-tauri/src/simulation/batch.rs`
- Modify: `src-tauri/src/commands/race.rs`
- Test: `src-tauri/src/simulation/batch.rs`
- Test: `src-tauri/src/commands/race.rs`

- [ ] Escrever teste falhando para pular a categoria do jogador.
- [ ] Escrever teste falhando para contagem correta de corridas automaticas.
- [ ] Escrever teste falhando para completar todas as pendencias na ultima corrida do jogador.
- [ ] Implementar `simulate_other_categories` e structs de retorno.

### Task 5: Integracao no comando e frontend

**Files:**
- Modify: `src-tauri/src/commands/race.rs`
- Modify: `src/stores/useCareerStore.js`
- Modify: `src/components/race/RaceResultView.jsx`

- [ ] Escrever teste falhando para `simulate_race_weekend` retornar o payload expandido.
- [ ] Integrar highlights/noticias e historico das corridas automaticas.
- [ ] Atualizar store para separar `player_race` e `other_categories`.
- [ ] Adicionar secao "Outras Categorias" na view de resultado.

### Task 6: Verificacao final

**Files:**
- Verify: `src-tauri/**`
- Verify: `src/**`

- [ ] Rodar `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] Rodar `cargo build --manifest-path src-tauri/Cargo.toml`.
- [ ] Rodar `npm run build`.
