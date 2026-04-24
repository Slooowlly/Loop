# FT-3 Promotion/Relegation Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implementar promoção/rebaixamento de equipes com efeitos em pilotos e integração no pipeline de fim de temporada antes do mercado.

**Architecture:** O FT-3 será isolado em `src-tauri/src/promotion/`, com standings de construtores alimentando três blocos independentes. O pipeline principal passará a rodar promoção antes do mercado, mas continuará criando a nova temporada antes do mercado para preservar a persistência de `market` e `market_proposals`.

**Tech Stack:** Rust, Tauri backend, rusqlite, serde, rand, testes unitários com `cargo test`

---

## Chunk 1: Base de Promotion

### Task 1: Criar standings e tipos compartilhados

**Files:**
- Create: `src-tauri/src/promotion/mod.rs`
- Create: `src-tauri/src/promotion/standings.rs`

- [ ] **Step 1: Write the failing tests**
- [ ] **Step 2: Run the standings tests to verify they fail**
- [ ] **Step 3: Implement constructor standings por categoria e por classe**
- [ ] **Step 4: Run the standings tests to verify they pass**

### Task 2: Implementar os três blocos de movimentação

**Files:**
- Create: `src-tauri/src/promotion/block1.rs`
- Create: `src-tauri/src/promotion/block2.rs`
- Create: `src-tauri/src/promotion/block3.rs`
- Test: `src-tauri/src/promotion/block1.rs`
- Test: `src-tauri/src/promotion/block2.rs`
- Test: `src-tauri/src/promotion/block3.rs`

- [ ] **Step 1: Write the failing tests para block1**
- [ ] **Step 2: Run os testes de block1 para confirmar RED**
- [ ] **Step 3: Implement minimal code de block1**
- [ ] **Step 4: Run os testes de block1 para confirmar GREEN**
- [ ] **Step 5: Repetir o ciclo para block2**
- [ ] **Step 6: Repetir o ciclo para block3**

## Chunk 2: Efeitos e Pilotos

### Task 3: Implementar efeitos de atributos nas equipes

**Files:**
- Create: `src-tauri/src/promotion/effects.rs`

- [ ] **Step 1: Write the failing tests para promotion/relegation effects**
- [ ] **Step 2: Run os testes para confirmar RED**
- [ ] **Step 3: Implement deltas e aplicação com clamp**
- [ ] **Step 4: Run os testes para confirmar GREEN**

### Task 4: Resolver pilotos afetados pelas movimentações

**Files:**
- Create: `src-tauri/src/promotion/pilots.rs`

- [ ] **Step 1: Write the failing tests para licença e liberação de pilotos**
- [ ] **Step 2: Run os testes para confirmar RED**
- [ ] **Step 3: Implement checagem de licença e efeitos em pilotos**
- [ ] **Step 4: Run os testes para confirmar GREEN**

## Chunk 3: Pipeline e Integração

### Task 5: Orquestrar FT-3

**Files:**
- Create: `src-tauri/src/promotion/pipeline.rs`

- [ ] **Step 1: Write the failing pipeline tests para temporada 1, invariantes e ordem**
- [ ] **Step 2: Run os testes para confirmar RED**
- [ ] **Step 3: Implement `run_promotion_relegation` e helpers internos**
- [ ] **Step 4: Run os testes para confirmar GREEN**

### Task 6: Integrar promoção antes do mercado no fim de temporada

**Files:**
- Modify: `src-tauri/src/evolution/pipeline.rs`
- Possibly modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write/adjust failing integration tests for end-of-season order**
- [ ] **Step 2: Run os testes para confirmar RED**
- [ ] **Step 3: Reestruturar o pipeline para: standings -> licenças -> evolução -> aposentadoria -> promoção -> nova temporada -> mercado -> reset -> calendários**
- [ ] **Step 4: Run os testes do pipeline para confirmar GREEN**

## Chunk 4: Verificação final

### Task 7: Validar o pacote inteiro

**Files:**
- Modify: `src-tauri/src/market/pipeline.rs` only if compatibility fixes are strictly needed

- [ ] **Step 1: Run `cargo fmt --manifest-path src-tauri/Cargo.toml`**
- [ ] **Step 2: Run `cargo test --manifest-path src-tauri/Cargo.toml`**
- [ ] **Step 3: Run `cargo build --manifest-path src-tauri/Cargo.toml`**
- [ ] **Step 4: Summarize any non-blocking warnings and changed files**
