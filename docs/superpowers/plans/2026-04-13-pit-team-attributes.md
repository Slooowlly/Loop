# Pit Team Attributes Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persistir e recalcular `pit_strategy_risk` e `pit_crew_quality` como atributos sazonais da equipe, com caps por categoria, momentum por resultado e exposicao no payload/UI.

**Architecture:** A implementacao fica em quatro frentes pequenas. Primeiro adicionamos os campos ao modelo/schema e cobrimos persistencia. Depois criamos um modulo pequeno e puro para calculo dos atributos de pit. Em seguida conectamos o recalculo na preseason junto do `car_build_profile`, usando contexto da nova temporada e resultado da anterior. Por fim, expomos os campos no payload de carreira e na aba `My Team`.

**Tech Stack:** Rust, Tauri, rusqlite, serde, rand, React

---

## Chunk 1: Model And Persistence

### Task 1: Adicionar campos de pit na equipe

**Files:**
- Modify: `src-tauri/src/models/team.rs`
- Modify: `src-tauri/src/db/queries/teams.rs`
- Modify: `src-tauri/src/db/migrations.rs`
- Test: `src-tauri/src/models/team.rs`
- Test: `src-tauri/src/db/queries/teams.rs`

- [ ] Adicionar testes falhando para valores default e round-trip de persistencia.
- [ ] Rodar `cargo test team` para ver a falha inicial.
- [ ] Adicionar `pit_strategy_risk` e `pit_crew_quality` em `Team`.
- [ ] Criar migration com defaults seguros e leitura legada robusta.
- [ ] Rodar `cargo test team` ate passar.
- [ ] Commitar com mensagem focada em schema/persistencia.

## Chunk 2: Calculation Helpers

### Task 2: Criar o modulo canonico dos atributos de pit

**Files:**
- Create: `src-tauri/src/market/pit_strategy.rs`
- Modify: `src-tauri/src/market/mod.rs`
- Test: `src-tauri/src/market/pit_strategy.rs`

- [ ] Adicionar testes falhando para:
  - cap por categoria
  - equipe rica gerar `pit_crew_quality` maior
  - equipe fraca/pressionada gerar `pit_strategy_risk` maior
  - suavizacao entre temporadas
- [ ] Rodar `cargo test pit_strategy` para ver a falha inicial.
- [ ] Implementar helpers puros para:
  - `category_pit_crew_cap`
  - `category_risk_modifier`
  - target e smoothing de `pit_crew_quality`
  - target e smoothing de `pit_strategy_risk`
- [ ] Rodar `cargo test pit_strategy` ate passar.
- [ ] Commitar o modulo de calculo.

## Chunk 3: Offseason Recalculation

### Task 3: Recalcular os atributos na preseason

**Files:**
- Modify: `src-tauri/src/market/preseason.rs`
- Test: `src-tauri/src/market/preseason.rs`

- [ ] Adicionar teste falhando cobrindo recalculo dos novos atributos junto do `car_build_profile`.
- [ ] Rodar `cargo test initialize_preseason` para ver a falha inicial.
- [ ] Integrar o modulo de pit ao fluxo de preseason.
- [ ] Considerar temporada anterior para momentum quando houver historico.
- [ ] Rodar `cargo test initialize_preseason` ate passar.
- [ ] Commitar o recalculo sazonal.

## Chunk 4: Payload And UI

### Task 4: Expor no payload e na aba My Team

**Files:**
- Modify: `src-tauri/src/commands/career_types.rs`
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src/pages/tabs/MyTeamTab.jsx`

- [ ] Adicionar os campos ao `TeamSummary`.
- [ ] Popular os campos em `build_team_summary`.
- [ ] Mostrar `pit_strategy_risk` e `pit_crew_quality` na `MyTeamTab`.
- [ ] Rodar os testes backend relevantes e validar a UI localmente se possivel.
- [ ] Commitar a exposicao final.

## Final Verification

- [ ] Rodar `cargo test pit_strategy`
- [ ] Rodar `cargo test initialize_preseason`
- [ ] Rodar `cargo test team`
- [ ] Rodar `cargo test commands::career`
- [ ] Se `vitest` estiver disponivel, rodar `npm test`

