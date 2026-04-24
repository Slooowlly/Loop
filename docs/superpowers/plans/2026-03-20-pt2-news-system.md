# PT-2 News System Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** adicionar persistencia e consulta de noticias para mercado semanal, fim de temporada e corridas.

**Architecture:** um modulo `news` gera `NewsItem` ricos no backend, enquanto `db/queries/news.rs` faz o mapeamento para a tabela enxuta existente usando envelope JSON em `texto`. Os fluxos de carreira e corrida passam a persistir noticias imediatamente apos cada evento relevante.

**Tech Stack:** Rust, Tauri, rusqlite, serde/serde_json, testes unitarios no backend.

---

### Task 1: Definir modelo de noticia

**Files:**
- Modify: `src-tauri/src/news/mod.rs`
- Create: `src-tauri/src/news/generator.rs`

- [ ] Escrever testes que falham para serializacao basica e icones por tipo.
- [ ] Implementar `NewsItem`, `NewsType`, `NewsImportance`.
- [ ] Exportar `generator` em `news/mod.rs`.

### Task 2: Geradores de noticias

**Files:**
- Modify: `src-tauri/src/news/generator.rs`
- Test: `src-tauri/src/news/generator.rs`

- [ ] Escrever testes que falham para mercado, fim de temporada e corrida.
- [ ] Implementar `generate_news_from_market_events`.
- [ ] Implementar `generate_news_from_end_of_season`.
- [ ] Implementar `generate_news_from_race`.

### Task 3: Persistencia e queries

**Files:**
- Modify: `src-tauri/src/db/queries/news.rs`
- Test: `src-tauri/src/db/queries/news.rs`

- [ ] Escrever testes que falham para insert/load/filters/trim.
- [ ] Implementar envelope JSON em `texto`.
- [ ] Implementar batch insert, filtros e trim FIFO.

### Task 4: Integracao no fluxo

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/commands/career_commands.rs`
- Modify: `src-tauri/src/commands/race.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] Escrever testes de integracao que falham para persistencia de noticias em fim de temporada, mercado semanal e corrida.
- [ ] Integrar geracao/persistencia em `advance_season`.
- [ ] Integrar geracao/persistencia em `advance_market_week`.
- [ ] Integrar geracao/persistencia em `simulate_race_weekend`.
- [ ] Adicionar comando `get_news`.

### Task 5: Verificacao

**Files:**
- Modify: arquivos acima apenas se necessario

- [ ] Rodar testes focados das novas areas.
- [ ] Rodar `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] Rodar `cargo build --manifest-path src-tauri/Cargo.toml`.
