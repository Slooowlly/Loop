# Race Briefing Layout Refine Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reorganizar o briefing pre-corrida para separar narrativa e contexto competitivo, removendo informacoes redundantes e tirando o botao Voltar de dentro do card temporal do header.

**Architecture:** O `Header` passa a renderizar o bloco temporal e o botao `Voltar` como elementos irmaos. O `NextRaceTab` substitui os pills superiores por um card-resumo com linhas, concentra o conteudo editorial na coluna esquerda e preserva grid/contexto na direita.

**Tech Stack:** React, Zustand, Vitest, Testing Library, Tailwind utility classes

---

### Task 1: Ajustar testes do layout

**Files:**
- Modify: `src/components/layout/Header.test.jsx`
- Modify: `src/pages/tabs/NextRaceTab.test.jsx`

- [ ] Atualizar assercoes do header para refletir o `Voltar` fora do card temporal.
- [ ] Atualizar assercoes do briefing para o novo card-resumo e a remocao de `Momento da etapa`.

### Task 2: Reorganizar o header

**Files:**
- Modify: `src/components/layout/Header.jsx`

- [ ] Renderizar o botao `Voltar` abaixo do bloco temporal, fora do card.
- [ ] Manter o botao visivel apenas com `showRaceBriefing`.

### Task 3: Reorganizar o briefing

**Files:**
- Modify: `src/pages/tabs/NextRaceTab.jsx`

- [ ] Remover `Momento da etapa` do resumo.
- [ ] Trocar os pills por um card-resumo com linhas no estilo de `Condicoes`.
- [ ] Concentrar a coluna esquerda no conteudo editorial e manter a direita com grid e campeonato.

### Task 4: Verificar

**Files:**
- Test: `src/components/layout/Header.test.jsx`
- Test: `src/pages/tabs/NextRaceTab.test.jsx`

- [ ] Rodar `npm test -- src/components/layout/Header.test.jsx src/pages/tabs/NextRaceTab.test.jsx`
- [ ] Rodar `npm run build`
