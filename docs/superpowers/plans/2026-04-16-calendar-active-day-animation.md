# Calendar Active Day Animation Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fazer a aba `Calendario` reagir visualmente ao `Avancar calendario`, usando a animacao aprovada de foco no dia ativo com trilha superior no mes corrente.

**Architecture:** `Dashboard` passa a aba ativa para `CalendarTab`, que combina `calendarDisplayDate` + `isCalendarAdvancing` para derivar um dia ativo temporario. O grid continua o mesmo, mas `MonthCard` e `DayCell` recebem props adicionais para renderizar trilha de progresso e destaque dourado no dia em foco.

**Tech Stack:** React, Zustand, Vitest, Testing Library, Tailwind utility classes

---

## Chunk 1: Frontend Wiring

### Task 1: Expor a aba ativa para a CalendarTab

**Files:**
- Modify: `src/pages/Dashboard.jsx`
- Modify: `src/pages/tabs/CalendarTab.jsx`

- [ ] **Step 1: Write the failing test**

Criar teste de `CalendarTab` validando que a aba ativa controla a renderizacao do estado animado.

- [ ] **Step 2: Run test to verify it fails**

Run: `npx.cmd vitest run src/pages/tabs/CalendarTab.test.jsx`
Expected: FAIL porque `CalendarTab` ainda nao recebe `activeTab`.

- [ ] **Step 3: Write minimal implementation**

Passar `activeTab={activeTab}` em `Dashboard` e aceitar essa prop em `CalendarTab`.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx.cmd vitest run src/pages/tabs/CalendarTab.test.jsx`
Expected: PASS no caso de wiring.

## Chunk 2: Animated Focus

### Task 2: Destacar o dia ativo com trilha superior

**Files:**
- Modify: `src/pages/tabs/CalendarTab.jsx`
- Test: `src/pages/tabs/CalendarTab.test.jsx`

- [ ] **Step 1: Write the failing test**

Cobrir que, com `activeTab="calendar"`, `isCalendarAdvancing=true` e `calendarDisplayDate` apontando para um dia com corrida, a celula ativa recebe marcador visual proprio e o mes recebe trilha de progresso.

- [ ] **Step 2: Run test to verify it fails**

Run: `npx.cmd vitest run src/pages/tabs/CalendarTab.test.jsx`
Expected: FAIL porque o destaque ainda nao existe.

- [ ] **Step 3: Write minimal implementation**

Adicionar helpers/props para:
- detectar dia ativo animado;
- decorar a celula com estado `isAnimatedCurrentDay`;
- desenhar a trilha superior no mes ativo;
- preservar os estilos atuais fora da animacao.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx.cmd vitest run src/pages/tabs/CalendarTab.test.jsx`
Expected: PASS

## Chunk 3: Verification

### Task 3: Verificacao focada

**Files:**
- Verify only: `src/pages/tabs/CalendarTab.jsx`
- Verify only: `src/pages/Dashboard.jsx`
- Verify only: `src/pages/tabs/CalendarTab.test.jsx`

- [ ] **Step 1: Focused test**

Run: `npx.cmd vitest run src/pages/tabs/CalendarTab.test.jsx`

- [ ] **Step 2: Relevant regression**

Run: `npx.cmd vitest run src/components/layout/Header.test.jsx`

- [ ] **Step 3: Build**

Run: `npm.cmd run build`
