# PT-4 Pre-Temporada Frontend Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrar o fluxo completo de fim de temporada e pre-temporada semanal ao dashboard React.

**Architecture:** A store Zustand centraliza o estado transitório do fluxo sazonal e as views novas consomem esse estado sem replicar chamadas ao backend fora das acoes do store. `Dashboard` decide a view ativa e `NextRaceTab` vira o ponto de entrada do fluxo.

**Tech Stack:** React, Zustand, Tauri invoke, Tailwind CSS, componentes glass existentes.

---

### Task 1: Expandir a store sazonal

**Files:**
- Modify: `src/stores/useCareerStore.js`

- [ ] Adicionar estado de fim de temporada, pre-temporada, loading e propostas.
- [ ] Implementar `advanceSeason`, `enterPreseason`, `advanceMarketWeek`, `respondToProposal` e `finalizePreseason`.
- [ ] Reusar `loadCareer` para normalizar o retorno ao dashboard.

### Task 2: Criar as duas views sazonais

**Files:**
- Create: `src/components/season/EndOfSeasonView.jsx`
- Create: `src/components/season/PreSeasonView.jsx`
- Modify: `src/utils/formatters.js`

- [ ] Criar resumo de fim de temporada com secoes expansíveis.
- [ ] Criar pre-temporada com progresso, feed acumulado, propostas e toast.
- [ ] Adicionar helpers de formatacao usados pelas views.

### Task 3: Integrar dashboard e entrada do fluxo

**Files:**
- Modify: `src/pages/Dashboard.jsx`
- Modify: `src/pages/tabs/NextRaceTab.jsx`

- [ ] Fazer o dashboard escolher entre resultado, fim de temporada, pre-temporada e tabs normais.
- [ ] Ativar o CTA de avancar temporada e detectar pre-temporada em andamento.
- [ ] Mostrar loading coerente durante o processamento.

### Task 4: Verificacao

**Files:**
- Verify: `src/**`

- [ ] Rodar `npm run build`.
- [ ] Corrigir erros de tipagem/importacao/estado ate o build passar.
