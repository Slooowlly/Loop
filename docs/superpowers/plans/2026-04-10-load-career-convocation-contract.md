# Load Career Convocation Contract Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restaurar o contrato de `load_career` para que a UI consiga reconstruir corretamente a `JanelaConvocacao` e o estado de aceite de oferta especial.

**Architecture:** Expandir o payload de `CareerData` no backend com o estado mínimo que a UI já consome (`season.fase`, `player.categoria_especial_ativa`, `player_team.classe`) e ajustar o store para derivar a oferta aceita a partir das ofertas pendentes e da categoria especial ativa, sem assumir que `player_team` já seja a equipe especial durante `JanelaConvocacao`.

**Tech Stack:** Rust + Tauri + Serde no backend, React + Zustand + Vitest no frontend.

---

### Task 1: Cobrir a regressão no backend

**Files:**
- Modify: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Write the failing test**
- [ ] **Step 2: Run the Rust test to verify it fails**
- [ ] **Step 3: Implement the minimal contract changes**
- [ ] **Step 4: Run the Rust test to verify it passes**

### Task 2: Cobrir a regressão no store

**Files:**
- Modify: `src/stores/useCareerStore.test.js`
- Modify: `src/stores/useCareerStore.js`

- [ ] **Step 1: Write the failing test**
- [ ] **Step 2: Run the frontend test to verify it fails**
- [ ] **Step 3: Implement the minimal store fix**
- [ ] **Step 4: Run the frontend test to verify it passes**

### Task 3: Verificação focada

**Files:**
- Modify: `src-tauri/src/commands/career_types.rs`

- [ ] **Step 1: Run the focused backend tests**
- [ ] **Step 2: Run the focused frontend tests**
- [ ] **Step 3: Confirm no remaining contract mismatch**
