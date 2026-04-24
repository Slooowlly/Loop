# Window Drawer Exit Confirmation Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deixar a bandeja lateral apenas com `Home` e exigir confirmacao sempre ao sair pelo `Home` ou pelo `X`.

**Architecture:** A mudanca fica concentrada em `WindowControlsDrawer.jsx`, reaproveitando o modal atual com nova copia e removendo a dependencia de `isDirty` para abrir a confirmacao. Os testes cobrem a reducao da bandeja e os dois pontos de entrada do modal.

**Tech Stack:** React, React Router, Vitest, Testing Library

---

### Task 1: Cobrir o novo fluxo de bandeja

**Files:**
- Modify: `src/components/layout/WindowControlsDrawer.test.jsx`

- [ ] **Step 1: Write tests para drawer com apenas Home**
- [ ] **Step 2: Write tests para confirmacao no Home e no X**
- [ ] **Step 3: Run test to verify it fails**

### Task 2: Implementar a simplificacao da bandeja

**Files:**
- Modify: `src/components/layout/WindowControlsDrawer.jsx`

- [ ] **Step 1: Remover widgets extras**
- [ ] **Step 2: Atualizar o texto do modal**
- [ ] **Step 3: Abrir confirmacao sempre para Home e X**
- [ ] **Step 4: Run tests and build**
