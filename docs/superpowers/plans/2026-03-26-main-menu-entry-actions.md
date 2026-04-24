# Main Menu Entry Actions Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tornar os dois primeiros botões do menu inicial explícitos e coerentes com suas ações.

**Architecture:** A mudança fica isolada em `src/pages/MainMenu.jsx`, removendo a lógica condicional do antigo botão de entrada e substituindo-a por navegações diretas. A validação será feita com build do frontend.

**Tech Stack:** React, React Router, Vite

---

## Chunk 1: Main menu action swap

### Task 1: Atualizar o menu inicial

**Files:**
- Modify: `src/pages/MainMenu.jsx`
- Verify: `package.json`

- [ ] **Step 1: Remover imports e estado que existem apenas para o fluxo antigo**

Excluir `useState` e `invoke`, já que o menu passará a usar somente navegação direta.

- [ ] **Step 2: Reordenar os dois primeiros botões**

Colocar `NOVA CARREIRA` como primeira ação visível e `CARREGAR SAVE` como segunda.

- [ ] **Step 3: Atualizar os handlers**

Fazer `NOVA CARREIRA` navegar para `/new-career` e `CARREGAR SAVE` navegar para `/load-save`.

- [ ] **Step 4: Rodar a verificação**

Run: `npm run build`
Expected: build concluído com sucesso.
