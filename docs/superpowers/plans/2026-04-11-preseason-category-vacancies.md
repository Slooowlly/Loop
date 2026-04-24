# Preseason Category Vacancies Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Exibir a soma de vagas abertas por categoria no cabeçalho do mapeamento das equipes do mercado de transferências.

**Architecture:** A mudança fica isolada em `PreSeasonView`, sem alterar a lógica do mercado. O total de vagas será derivado do grid já carregado, somando slots vazios (`piloto_1_nome` e `piloto_2_nome`) das equipes agrupadas por categoria, e exibido no cabeçalho da seção alinhado à direita.

**Tech Stack:** React, Vitest, Testing Library

---

## Chunk 1: Teste e Implementação Local da UI

### Task 1: Cobrir a contagem de vagas por categoria

**Files:**
- Modify: `src/components/season/PreSeasonView.test.jsx`
- Modify: `src/components/season/PreSeasonView.jsx`

- [ ] **Step 1: Write the failing test**

Adicionar um teste que monte uma categoria com equipes contendo slots vazios e valide a presença de `N vaga`/`N vagas` no cabeçalho da categoria.

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/components/season/PreSeasonView.test.jsx`
Expected: FAIL porque o cabeçalho ainda não mostra a contagem de vagas.

- [ ] **Step 3: Write minimal implementation**

Calcular as vagas por categoria a partir de `groupedTeams[teamClass]` e renderizar um badge no lado direito do cabeçalho de cada categoria.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/components/season/PreSeasonView.test.jsx`
Expected: PASS

- [ ] **Step 5: Run focused project validation**

Run: `python scripts/auditar_padrao.py --root . --paths src/components/season/PreSeasonView.jsx src/components/season/PreSeasonView.test.jsx`
Expected: sem violações relevantes para os arquivos alterados.
