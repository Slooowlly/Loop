# Preseason Contract Expiry Entry Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Garantir que pilotos com contrato encerrado na temporada anterior entrem na pré-temporada já sem contrato ativo.

**Architecture:** A correção fica concentrada no pipeline de inicialização da pré-temporada. Primeiro cobrimos a regressão com testes em `market/preseason.rs`; depois expiramos os contratos vencidos no banco principal ao iniciar a janela e ajustamos o plano para não adiar esse desligamento para a semana 2.

**Tech Stack:** Rust, rusqlite, testes unitários do backend Tauri

---

### Task 1: Cobrir a regressão

**Files:**
- Modify: `src-tauri/src/market/preseason.rs`
- Test: `src-tauri/src/market/preseason.rs`

- [ ] **Step 1: Write the failing test**

Adicionar teste que inicializa a pré-temporada e verifica:
1. o contrato vencido do jogador deixa de estar `Ativo` imediatamente;
2. o jogador não mantém contrato regular ativo ao entrar na janela;
3. não existe `PendingAction::ExpireContract` agendado para a semana 2.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_initialize_preseason_expires_ending_contracts_immediately`
Expected: FAIL, mostrando que o contrato ainda está ativo ou que ainda há expiração planejada para a semana 2.

- [ ] **Step 3: Write minimal implementation**

Expirar contratos vencidos no banco real dentro de `initialize_preseason` e alinhar o plano de eventos para que a expiração aconteça toda na entrada da janela.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_initialize_preseason_expires_ending_contracts_immediately`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/2026-04-08-preseason-contract-expiry-entry.md src-tauri/src/market/preseason.rs
git commit -m "fix: expire ending contracts at preseason start"
```

### Task 2: Verificar que o fluxo principal continua saudável

**Files:**
- Modify: `src-tauri/src/market/preseason.rs`
- Test: `src-tauri/src/market/preseason.rs`

- [ ] **Step 1: Run focused preseason tests**

Run: `cargo test test_contract_expiry_week`
Expected: PASS ou ajuste do teste caso a semântica mude para refletir a nova regra.

- [ ] **Step 2: Run broader preseason coverage**

Run: `cargo test preseason`
Expected: PASS para a suíte relevante de pré-temporada.

- [ ] **Step 3: Review for unintended side effects**

Confirmar que renovações, propostas do jogador e transferências continuam usando o plano planejado sem reativar contrato vencido.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/market/preseason.rs
git commit -m "test: cover preseason contract expiry entry flow"
```
