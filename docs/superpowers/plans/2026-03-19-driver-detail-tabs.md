# Driver Detail Tabs Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reorganizar a ficha de piloto em abas internas, abrindo por padrão na visão atual da temporada e escondendo carreira/forma/mercado atrás de navegação local.

**Architecture:** A mudança fica concentrada em `DriverDetailModal.jsx`, sem alterar o comando backend principal. O componente passa a controlar uma aba ativa local e renderiza subconjuntos do mesmo payload `detail`.

**Tech Stack:** React, Tauri frontend, Node test runner, Vite.

---

## Chunk 1: Testes da navegação por abas

### Task 1: Travar o contrato novo da ficha

**Files:**
- Modify: `scripts/tests/driver-detail-modal.test.mjs`

- [ ] **Step 1: Write the failing test**

Adicionar assertions para:
- presença das tabs `Atual`, `Forma`, `Carreira`, `Mercado`
- estado local de aba ativa iniciando em `Atual`
- ausência do bloco `Carreira` como conteúdo inicial padrão

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test scripts/tests/driver-detail-modal.test.mjs`
Expected: FAIL porque a ficha ainda renderiza tudo em sequência.

- [ ] **Step 3: Write minimal implementation**

Implementar apenas o necessário no modal para satisfazer os novos matchers.

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test scripts/tests/driver-detail-modal.test.mjs`
Expected: PASS

## Chunk 2: Tabs no modal

### Task 2: Criar o estado e a barra de tabs

**Files:**
- Modify: `src/components/driver/DriverDetailModal.jsx`

- [ ] **Step 1: Introduce tab state**

Adicionar estado local com default `Atual`.

- [ ] **Step 2: Add tab navigation UI**

Criar barra de tabs glass/pill logo abaixo do header, com variantes visuais para ativa/inativa.

- [ ] **Step 3: Scope content by tab**

Mapear conteúdo:
- `Atual`: competitivo, temporada, resumo de momento, contrato resumido
- `Forma`: bloco de forma
- `Carreira`: carreira + trajetória
- `Mercado`: contrato completo + blocos opcionais

- [ ] **Step 4: Keep drawer behavior intact**

Garantir que portal, animações de abertura/fechamento e backdrop permaneçam iguais.

## Chunk 3: Verificação

### Task 3: Validar frontend

**Files:**
- Verify: `scripts/tests/driver-detail-modal.test.mjs`
- Verify: `scripts/tests/result-badge-fastest-lap.test.mjs`
- Verify: frontend build

- [ ] **Step 1: Run source tests**

Run: `node --test scripts/tests/driver-detail-modal.test.mjs scripts/tests/result-badge-fastest-lap.test.mjs`
Expected: PASS

- [ ] **Step 2: Run build**

Run: `npm run build`
Expected: PASS
