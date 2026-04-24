# News Dates Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fazer o rótulo temporal das notícias usar a data da carreira junto com o contexto esportivo.

**Architecture:** O backend continua responsável por montar `NewsTabStory`, mas troca o `relative_time_label` por um resolvedor de data editorial da carreira. Esse resolvedor usa `rodada`, `semana_pretemporada`, `display_date` do calendário e um fallback estável de temporada. O frontend continua consumindo apenas `time_label`.

**Tech Stack:** Rust, Tauri, React, Vitest

---

## Chunk 1: Backend editorial dates

### Task 1: Cobrir o novo `time_label` com testes

**Files:**
- Modify: `src-tauri/src/commands/news_tab.rs`

- [ ] **Step 1: Escrever teste falhando para notícia de rodada**

Adicionar um teste que confirme `Rodada X · DD mmm AAAA` em uma notícia ligada a uma corrida.

- [ ] **Step 2: Escrever teste falhando para notícia de pré-temporada**

Adicionar um teste que confirme `Pre-temporada Semana X · DD mmm AAAA`.

- [ ] **Step 3: Rodar o teste e confirmar falha**

Run: `cargo test news_tab_time_label --manifest-path src-tauri/Cargo.toml`
Expected: FAIL porque o backend ainda usa rótulo relativo.

### Task 2: Resolver datas da carreira no backend

**Files:**
- Modify: `src-tauri/src/commands/news_tab.rs`

- [ ] **Step 4: Adicionar contexto de datas editoriais**

Guardar no contexto o `display_date` por `categoria + rodada` e o maior `semana_pretemporada` por temporada.

- [ ] **Step 5: Implementar resolvedor para notícias de rodada**

Trocar o rótulo relativo por `Rodada X · DD mmm AAAA`.

- [ ] **Step 6: Implementar resolvedor para pré-temporada**

Derivar a data a partir da rodada 1 da categoria e montar `Pre-temporada Semana X · DD mmm AAAA`.

- [ ] **Step 7: Implementar fallback estável**

Quando não houver data editorial resolvível, usar `Temporada X · AAAA`.

### Task 3: Documentação e verificação

**Files:**
- Modify: `docs/superpowers/specs/2026-03-26-news-dates-design.md`
- Modify: `docs/superpowers/plans/2026-03-26-news-dates.md`

- [ ] **Step 8: Rodar os testes do backend**

Run: `cargo test news_tab --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 9: Rodar o build do frontend**

Run: `npm run build`
Expected: PASS
