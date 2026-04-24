# Next Race Editorial Variation Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refatorar a editorial da `NextRaceTab` para variar a previa pela combinacao entre momento do campeonato e contexto esportivo da etapa, preservando o payload atual.

**Architecture:** Extrair a camada editorial para um modulo dedicado que classifica `championship axis` e `weekend axis`, depois resolve os blocos de copy a partir dessa matriz. `NextRaceTab.jsx` passa a consumir um contexto pronto, mantendo a UI enxuta e os testes focados em comportamento editorial.

**Tech Stack:** React, Vitest, Testing Library, JavaScript modular, TDD

---

## File Structure

- Modify: `src/pages/tabs/NextRaceTab.jsx`
  manter renderizacao e integracao visual; remover decisoes editoriais pesadas do arquivo
- Create: `src/pages/tabs/nextRaceEditorial.js`
  concentrar classificadores editoriais, pools de copy e builder do contexto textual
- Create: `src/pages/tabs/nextRaceEditorial.test.js`
  validar classificacao por eixo, fallback e resolucao das copys
- Modify: `src/pages/tabs/NextRaceTab.test.jsx`
  ajustar asserts para a nova editorial contextual

## Chunk 1: Extract Editorial Engine

### Task 1: Add failing unit tests for editorial axes

**Files:**
- Create: `src/pages/tabs/nextRaceEditorial.test.js`

- [ ] **Step 1: Write the failing test**

```js
import { describe, expect, it } from "vitest";
import { classifyChampionshipState, classifyWeekendState } from "./nextRaceEditorial";

describe("classifyChampionshipState", () => {
  it("marks a close title chase as chase", () => {
    expect(
      classifyChampionshipState({
        playerStanding: { posicao_campeonato: 2, pontos: 88 },
        leader: { pontos: 94 },
        remainingRounds: 5,
        outlook: { titleFight: "contender" },
        gapBehind: 10,
      }),
    ).toBe("chase");
  });
});

describe("classifyWeekendState", () => {
  it("marks a heated weekend when stories and rival are active", () => {
    expect(
      classifyWeekendState({
        trackHistory: { has_data: true, best_finish: 2, dnfs: 0 },
        briefingRival: { driver_name: "M. Costa" },
        nextRace: { clima: "Wet" },
        weekendStories: [{ importanceLabel: "Alta" }, { importanceLabel: "Media" }],
      }),
    ).toBe("weekend_hot");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/pages/tabs/nextRaceEditorial.test.js`
Expected: FAIL with missing module or missing exports

- [ ] **Step 3: Write minimal implementation**

```js
export function classifyChampionshipState() {
  return "chase";
}

export function classifyWeekendState() {
  return "weekend_hot";
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/pages/tabs/nextRaceEditorial.test.js`
Expected: PASS

### Task 2: Expand unit tests for fallback and opposite states

**Files:**
- Modify: `src/pages/tabs/nextRaceEditorial.test.js`

- [ ] **Step 1: Write failing tests for leader, outsider, neutral weekend, negative history**

```js
it("marks championship leader correctly", () => {
  expect(
    classifyChampionshipState({
      playerStanding: { posicao_campeonato: 1, pontos: 100 },
      leader: { pontos: 100 },
      remainingRounds: 2,
      outlook: { titleFight: "leader" },
      gapBehind: 3,
    }),
  ).toBe("leader");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/pages/tabs/nextRaceEditorial.test.js`
Expected: FAIL on new expectations

- [ ] **Step 3: Implement classification rules**

```js
if (playerStanding?.posicao_campeonato === 1) return "leader";
if (outlook?.titleFight === "longshot") return "outsider";
if (gapToLeader <= 12) return "chase";
if (gapBehind != null && gapBehind <= 4) return "pressure";
return "survival";
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/pages/tabs/nextRaceEditorial.test.js`
Expected: PASS

## Chunk 2: Build Editorial Copy Matrix

### Task 3: Add failing tests for contextual text resolution

**Files:**
- Modify: `src/pages/tabs/nextRaceEditorial.test.js`

- [ ] **Step 1: Write failing tests for resolved copy**

```js
import { buildEditorialCopy } from "./nextRaceEditorial";

it("uses title-chase language when championship is alive", () => {
  const copy = buildEditorialCopy({
    championshipState: "chase",
    weekendState: "rival_spotlight",
    playerStanding: { posicao_campeonato: 2, pontos: 88 },
    leader: { nome: "M. Costa", pontos: 94 },
    rival: { nome: "M. Costa" },
    nextRace: { track_name: "Interlagos" },
    gapToLeader: 6,
    remainingRounds: 5,
  });

  expect(copy.headline).toMatch(/encurtar|pressionar|aproximar/i);
  expect(copy.rivalSummary).toMatch(/M\\. Costa/i);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/pages/tabs/nextRaceEditorial.test.js`
Expected: FAIL with missing `buildEditorialCopy`

- [ ] **Step 3: Implement copy pools and resolver**

```js
const HEADLINES = {
  chase: {
    rival_spotlight: ({ nextRace, leader, remainingRounds }) =>
      `Voce chega a ${nextRace.track_name} tentando encurtar a distancia para ${leader.nome}. ${remainingRounds} etapas seguem abertas depois desta corrida.`,
  },
};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/pages/tabs/nextRaceEditorial.test.js`
Expected: PASS

### Task 4: Cover track-history and weekend-story triggers

**Files:**
- Modify: `src/pages/tabs/nextRaceEditorial.test.js`

- [ ] **Step 1: Write failing tests for history-positive, history-negative, and no-story fallback**

```js
it("uses positive track-history language when the player has strong prior results", () => {
  const copy = buildEditorialCopy({
    championshipState: "survival",
    weekendState: "history_positive",
    trackHistory: { has_data: true, starts: 4, best_finish: 1, dnfs: 0 },
    nextRace: { track_name: "Interlagos" },
  });

  expect(copy.historyMeta).toMatch(/melhor resultado|pista/i);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/pages/tabs/nextRaceEditorial.test.js`
Expected: FAIL on new cases

- [ ] **Step 3: Implement fallback-aware history and weekend text helpers**

```js
if (!weekendStories.length) {
  return "O paddock ainda nao produziu manchetes fortes para esta etapa, entao a leitura segue focada na pista.";
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/pages/tabs/nextRaceEditorial.test.js`
Expected: PASS

## Chunk 3: Integrate Editorial Engine Into NextRaceTab

### Task 5: Add failing integration assertions in the tab test

**Files:**
- Modify: `src/pages/tabs/NextRaceTab.test.jsx`

- [ ] **Step 1: Write failing assertions for contextual variation**

```jsx
expect(screen.getByText(/rival principal/i)).toBeInTheDocument();
expect(screen.getByText(/m\. costa/i)).toBeInTheDocument();
expect(screen.getByText(/duelo|pressao|encurtar/i)).toBeInTheDocument();
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/pages/tabs/NextRaceTab.test.jsx`
Expected: FAIL because the current tab still derives copy locally

- [ ] **Step 3: Wire `NextRaceTab.jsx` to the editorial module**

```js
const editorialCopy = buildEditorialCopy({
  championshipState,
  weekendState,
  playerStanding,
  leader,
  rival,
  trackHistory,
  weekendStories,
  nextRace,
  gapToLeader,
  gapBehind,
  remainingRounds,
  playerTeam,
  season,
  outlook,
});
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/pages/tabs/NextRaceTab.test.jsx`
Expected: PASS

### Task 6: Remove duplicated editorial logic from the tab

**Files:**
- Modify: `src/pages/tabs/NextRaceTab.jsx`

- [ ] **Step 1: Move headline/paragraph/scenario/rival/history copy decisions into `nextRaceEditorial.js`**

```js
import {
  buildEditorialCopy,
  classifyChampionshipState,
  classifyWeekendState,
} from "./nextRaceEditorial";
```

- [ ] **Step 2: Run focused tests**

Run: `npx vitest run src/pages/tabs/nextRaceEditorial.test.js src/pages/tabs/NextRaceTab.test.jsx`
Expected: PASS

- [ ] **Step 3: Clean dead helpers from `NextRaceTab.jsx`**

```js
// remove old inline editorial helpers once no longer referenced
```

- [ ] **Step 4: Re-run focused tests**

Run: `npx vitest run src/pages/tabs/nextRaceEditorial.test.js src/pages/tabs/NextRaceTab.test.jsx`
Expected: PASS

## Chunk 4: Verification

### Task 7: Run final verification set

**Files:**
- Test: `src/pages/tabs/nextRaceEditorial.test.js`
- Test: `src/pages/tabs/NextRaceTab.test.jsx`
- Test: `src/stores/useCareerStore.test.js`

- [ ] **Step 1: Run editorial unit tests**

Run: `npx vitest run src/pages/tabs/nextRaceEditorial.test.js`
Expected: PASS

- [ ] **Step 2: Run tab integration tests**

Run: `npx vitest run src/pages/tabs/NextRaceTab.test.jsx`
Expected: PASS

- [ ] **Step 3: Run store regression test**

Run: `npx vitest run src/stores/useCareerStore.test.js`
Expected: PASS

- [ ] **Step 4: Review diff for scope control**

Run: `git diff -- src/pages/tabs/NextRaceTab.jsx src/pages/tabs/NextRaceTab.test.jsx src/pages/tabs/nextRaceEditorial.js src/pages/tabs/nextRaceEditorial.test.js`
Expected: only editorial-layer and test changes
