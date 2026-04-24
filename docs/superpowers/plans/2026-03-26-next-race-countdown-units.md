# Next Race Countdown Units Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Atualizar o texto `Proxima corrida` para usar meses, semanas e dias conforme a distancia para a etapa.

**Architecture:** A mudanca fica concentrada no formatter do frontend, que ja recebe `days_until_next_event` do store. Os testes cobrem tanto a funcao de formatacao quanto a leitura do header para evitar regressao visual.

**Tech Stack:** React, Vitest, Testing Library

---

### Task 1: Cobrir a nova regua temporal

**Files:**
- Create: `src/utils/formatters.test.js`
- Test: `src/components/layout/Header.test.jsx`

- [ ] **Step 1: Write the failing test**
- [ ] **Step 2: Run test to verify it fails**
- [ ] **Step 3: Implement the minimal formatter change**
- [ ] **Step 4: Run tests and build to verify the behavior**

### Task 2: Validar integracao no header

**Files:**
- Modify: `src/components/layout/Header.test.jsx`
- Modify: `src/utils/formatters.js`

- [ ] **Step 1: Cover one month-range case in the header**
- [ ] **Step 2: Confirm the header renders the updated label**
- [ ] **Step 3: Run the focused suite and production build**
