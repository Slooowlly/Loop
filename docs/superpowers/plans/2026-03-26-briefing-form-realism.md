# Briefing Form Realism Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tornar a forma recente do briefing resiliente ao reabrir a carreira e recalibrar a narrativa para refletir chances reais de resultado.

**Architecture:** O backend de `get_drivers_by_category` monta `results` com fallback para o historico persistido em `ultimos_resultados`. O frontend do briefing calcula um contexto de expectativa esportiva a partir de forma recente, favoritismo, pontos e etapas restantes para gerar textos mais honestos.

**Tech Stack:** Rust, React, Vitest, Cargo tests, Testing Library

---

### Task 1: Cobrir o fallback de forma recente no backend

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Test: `src-tauri/src/commands/career.rs`

- [ ] Escrever teste que prova que `results` nao fica vazio quando `race_results.json` estiver ausente, mas o driver tiver `ultimos_resultados`.
- [ ] Implementar o fallback no `get_drivers_by_category_in_base_dir`.

### Task 2: Cobrir a expectativa realista do briefing no frontend

**Files:**
- Modify: `src/pages/tabs/NextRaceTab.jsx`
- Modify: `src/pages/tabs/NextRaceTab.test.jsx`

- [ ] Escrever teste para um piloto com forma ruim, grande gap e poucas corridas restantes.
- [ ] Ajustar headline, paragrafos, metas, cenario e call to action com base no contexto realista.

### Task 3: Verificar

**Files:**
- Test: `src/pages/tabs/NextRaceTab.test.jsx`
- Test: `src-tauri/src/commands/career.rs`

- [ ] Rodar `npm test -- src/pages/tabs/NextRaceTab.test.jsx`
- [ ] Rodar `cargo test test_get_drivers_by_category --manifest-path src-tauri/Cargo.toml`
- [ ] Rodar `npm run build`
