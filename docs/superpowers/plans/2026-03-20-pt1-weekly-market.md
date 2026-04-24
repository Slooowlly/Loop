# PT-1 Weekly Market Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refatorar o mercado de transferências para uma pré-temporada semanal, persistida em JSON, com comandos Tauri dedicados.

**Architecture:** Um novo módulo `market/preseason.rs` passa a planejar e executar a pré-temporada. `evolution/pipeline.rs` deixa de resolver o mercado diretamente e passa apenas a inicializar o plano após criar a nova temporada. Os comandos Tauri operam em cima do JSON sidecar e do banco da carreira.

**Tech Stack:** Rust, Tauri, rusqlite, serde/serde_json, testes unitários/integrados com banco em memória.

---

## Chunk 1: Novo Orquestrador

### Task 1: Criar o esqueleto do módulo de pré-temporada

**Files:**
- Create: `src-tauri/src/market/preseason.rs`
- Modify: `src-tauri/src/market/mod.rs`
- Test: `src-tauri/src/market/preseason.rs`

- [ ] **Step 1: Escrever testes falhando para as structs e fluxo básico**
- [ ] **Step 2: Rodar os testes filtrados e confirmar falha**
- [ ] **Step 3: Implementar `PreSeasonState`, `PreSeasonPhase`, `WeekResult`, `MarketEvent`, `PendingAction`, `PreSeasonPlan`**
- [ ] **Step 4: Exportar `preseason` em `market/mod.rs`**
- [ ] **Step 5: Rodar os testes filtrados e confirmar verde**

### Task 2: Implementar planejamento e persistência do plano

**Files:**
- Modify: `src-tauri/src/market/preseason.rs`
- Test: `src-tauri/src/market/preseason.rs`

- [ ] **Step 1: Escrever testes falhando para `initialize_preseason()` e persistência JSON**
- [ ] **Step 2: Rodar os testes e confirmar falha**
- [ ] **Step 3: Implementar `initialize_preseason()`, `save_preseason_plan()`, `load_preseason_plan()`, `delete_preseason_plan()`**
- [ ] **Step 4: Rodar os testes filtrados e confirmar verde**

## Chunk 2: Execução Semanal

### Task 3: Implementar `advance_week()` com efeitos no banco

**Files:**
- Modify: `src-tauri/src/market/preseason.rs`
- Test: `src-tauri/src/market/preseason.rs`

- [ ] **Step 1: Escrever testes falhando para expiração, renovação, transferência, rookie e transição de fase**
- [ ] **Step 2: Rodar os testes e confirmar falha**
- [ ] **Step 3: Implementar `advance_week()` e helpers internos de execução**
- [ ] **Step 4: Rodar os testes filtrados e confirmar verde**

## Chunk 3: Integração no Pipeline e Comandos

### Task 4: Integrar a pré-temporada ao fim de temporada

**Files:**
- Modify: `src-tauri/src/evolution/pipeline.rs`
- Test: `src-tauri/src/evolution/pipeline.rs`

- [ ] **Step 1: Escrever/ajustar testes falhando para `run_end_of_season()` sem `market_report` e com `preseason_initialized`**
- [ ] **Step 2: Rodar os testes filtrados e confirmar falha**
- [ ] **Step 3: Remover chamada direta ao mercado e inicializar/salvar o plano semanal**
- [ ] **Step 4: Rodar os testes filtrados e confirmar verde**

### Task 5: Expor comandos Tauri novos

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/commands/career_commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Escrever testes falhando para `advance_market_week`, `get_preseason_state` e `finalize_preseason`**
- [ ] **Step 2: Rodar os testes filtrados e confirmar falha**
- [ ] **Step 3: Implementar helpers in-base-dir e comandos Tauri**
- [ ] **Step 4: Registrar os comandos em `career_commands.rs` e `lib.rs`**
- [ ] **Step 5: Rodar os testes filtrados e confirmar verde**

## Chunk 4: Verificação Final

### Task 6: Validar integração completa

**Files:**
- Modify: `src-tauri/src/market/preseason.rs`
- Modify: `src-tauri/src/evolution/pipeline.rs`
- Modify: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Rodar `cargo fmt --manifest-path src-tauri/Cargo.toml`**
- [ ] **Step 2: Rodar `cargo test --manifest-path src-tauri/Cargo.toml`**
- [ ] **Step 3: Rodar `cargo build --manifest-path src-tauri/Cargo.toml`**
- [ ] **Step 4: Corrigir falhas restantes e repetir até tudo ficar verde**
