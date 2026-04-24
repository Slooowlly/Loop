# Calendar Convocation Week Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mostrar a janela de convocação como uma semana real dentro do grid do calendário e exibir as corridas futuras do bloco especial nas datas corretas.

**Architecture:** A `CalendarTab` passa a carregar o calendário regular e, quando existir, o calendário da categoria especial aceita. O grid recebe dois novos estados visuais: `dia da convocação` e `corrida especial`, enquanto o card intermediário de convocação é removido.

**Tech Stack:** React, Zustand, Tauri, Vitest.

---

### Task 1: Cobrir o novo comportamento com testes

**Files:**
- Modify: `src/pages/tabs/CalendarTab.test.jsx`

- [ ] **Step 1: Write the failing tests**
- [ ] **Step 2: Run `npx.cmd vitest run src/pages/tabs/CalendarTab.test.jsx` and verify fail**
- [ ] **Step 3: Implement only the minimum behavior to satisfy the new assertions**
- [ ] **Step 4: Re-run the same test file and verify pass**

### Task 2: Integrar a convocação ao grid

**Files:**
- Modify: `src/pages/tabs/CalendarTab.jsx`

- [ ] **Step 1: Buscar o calendário especial quando houver `acceptedSpecialOffer.special_category`**
- [ ] **Step 2: Derivar a semana de convocação a partir da primeira corrida especial**
- [ ] **Step 3: Remover o card `Janela de Convocação`**
- [ ] **Step 4: Adicionar novos estados visuais no grid e na legenda**

### Task 3: Verificação final

**Files:**
- Verify: `src/pages/tabs/CalendarTab.jsx`
- Verify: `src/pages/tabs/CalendarTab.test.jsx`

- [ ] **Step 1: Run `npx.cmd vitest run src/pages/tabs/CalendarTab.test.jsx`**
- [ ] **Step 2: Run `npx.cmd vitest run src/pages/Dashboard.test.jsx src/components/layout/Header.test.jsx`**
- [ ] **Step 3: Run `npm.cmd run build`**
