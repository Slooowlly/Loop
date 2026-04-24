# Grid Expectation Copy Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Gerar frases mais inteligentes e contextuais para o bloco `Sobre o grid`.

**Architecture:** A mudanca fica concentrada em `NextRaceTab.jsx`, reaproveitando os dados de forma recente e campeonato ja carregados no briefing. Os testes validam o texto gerado para um grid conhecido e impedem regressao para frases fixas.

**Tech Stack:** React, Vitest, Testing Library

---

### Task 1: Cobrir a nova linguagem do grid

**Files:**
- Modify: `src/pages/tabs/NextRaceTab.test.jsx`

- [ ] **Step 1: Atualizar as expectativas de texto**
- [ ] **Step 2: Rodar o teste e confirmar a falha**

### Task 2: Implementar a logica contextual

**Files:**
- Modify: `src/pages/tabs/NextRaceTab.jsx`

- [ ] **Step 1: Derivar sinais da forma recente**
- [ ] **Step 2: Mapear os sinais para frases esportivas**
- [ ] **Step 3: Rodar testes e build**
