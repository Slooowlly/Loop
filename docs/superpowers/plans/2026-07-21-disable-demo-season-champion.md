# Disable Demo Season Champion Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Impedir que a tela de campeão com dados DEMO apareça nesta versão, sem apagar os arquivos visuais preparados para o backend futuro.

**Architecture:** Remover todos os pontos de entrada de produção (`App`, `Dashboard`, `Settings`) e o comando destrutivo de debug. Simplificar o estado dormente do store para abrir/fechar apenas o payload visual, sem navegação pós-overlay. Um teste estrutural impede reexposição acidental e testes comportamentais preservam o destino direto para Notícias no final.

**Tech Stack:** React 18, Zustand 5, Vitest 4, Node test runner, Vite.

---

## Chunk 1: Desativação completa

### Task 1: Criar a trava estrutural

**Files:**
- Create: `scripts/tests/season-champion-disabled.test.mjs`

- [ ] **Step 1: Escrever o teste estrutural RED**

Criar o arquivo completo:

```js
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

async function readSource(relativePath) {
  return readFile(resolve(projectRoot, relativePath), "utf8");
}

test("demo season champion screen stays unreachable in production", async () => {
  const [appSource, settingsSource, dashboardSource, storeSource] = await Promise.all([
    readSource("src/App.jsx"),
    readSource("src/pages/Settings.jsx"),
    readSource("src/pages/Dashboard.jsx"),
    readSource("src/stores/useCareerStore.js"),
  ]);

  assert.doesNotMatch(appSource, /SeasonChampionOverlay/, "App must not mount the demo host");
  assert.doesNotMatch(
    settingsSource,
    /debugForceSeasonEndChampion|Forçar fim de temporada/,
    "Settings must not expose the destructive demo action",
  );
  assert.doesNotMatch(
    dashboardSource,
    /showChampionOverlay|pendingDashboardTab|consumePendingDashboardTab/,
    "Dashboard must not trigger or consume the demo overlay",
  );
  assert.doesNotMatch(
    storeSource,
    /debugForceSeasonEndChampion|pendingDashboardTab|consumePendingDashboardTab|afterCloseTab/,
    "Store must not keep debug or post-overlay navigation",
  );
  assert.match(storeSource, /showChampionOverlay/, "Dormant open action must remain");
  assert.match(storeSource, /hideChampionOverlay/, "Dormant close action must remain");
});
```

- [ ] **Step 2: Confirmar RED**

Run: `node --test scripts/tests/season-champion-disabled.test.mjs`

Expected: FAIL porque host, gatilho automático, debug destrutivo e navegação pós-overlay ainda existem.

- [ ] **Step 3: Alterar o teste do Dashboard para o comportamento desativado**

Em `Dashboard.test.jsx`, trocar o caso “opens Home with the champion overlay” por um caso que parte de Calendário, fecha um resultado fresco final e espera `activeTab="news"` e uma única chamada a `dismissResult`. Remover do estado-base os mocks de overlay/pending e remover o teste de consumo pós-overlay. Manter os testes negativos de resultado antigo e corrida comum.

- [ ] **Step 4: Confirmar RED comportamental**

Run: `npx vitest run src/pages/Dashboard.test.jsx`

Expected: FAIL porque o branch atual tenta chamar `showChampionOverlay` ou seleciona Home.

### Task 2: Remover os pontos de entrada e simplificar o store

**Files:**
- Modify: `src/App.jsx`
- Modify: `src/pages/Settings.jsx`
- Modify: `src/pages/Dashboard.jsx`
- Modify: `src/stores/useCareerStore.js`

- [ ] **Step 1: Desmontar o host global**

Remover de `src/App.jsx` o import de `SeasonChampionOverlay` e `<SeasonChampionOverlay />`. Preservar os arquivos do componente e CSS.

- [ ] **Step 2: Remover o debug destrutivo das Configurações**

Remover de `Settings.jsx` o estado `seasonEndBusy`, a função `forceSeasonEndChampion` e todo o bloco visual “Fim de temporada → tela de campeão (debug)”. Não alterar os demais controles de debug.

- [ ] **Step 3: Restaurar o pós-corrida direto**

Em `Dashboard.jsx`:

- remover imports `HOME_TAB` e `NEWS_TAB` adicionados para o overlay;
- remover seletores `showChampionOverlay`, `pendingDashboardTab` e `consumePendingDashboardTab`;
- remover o efeito que consome `pendingDashboardTab`;
- remover o branch especial `resultIsFresh && lastRaceWasFinale` de `handleDismissResult`.

O fluxo restante já chama `resolvePostRaceLanding(..., lastRaceWasFinale)`, que devolve Notícias para finais sem iniciar avaliação de leitura.

- [ ] **Step 4: Simplificar o contrato dormente do store**

Em `useCareerStore.js`:

- remover `pendingDashboardTab` do estado inicial;
- manter `isSwitchingCareer` e a limpeza de `championOverlay` entre saves, mas remover `pendingDashboardTab` do objeto condicional (`{ championOverlay: null }`);
- manter `championOverlay: null`;
- reduzir as ações para:

```js
showChampionOverlay: (data = null) => set({
  championOverlay: data ?? { demo: true },
}),
hideChampionOverlay: () => set({ championOverlay: null }),
```

- remover `consumePendingDashboardTab` e `debugForceSeasonEndChampion`.

- [ ] **Step 5: Confirmar GREEN estrutural**

Run: `node --test scripts/tests/season-champion-disabled.test.mjs`

Expected: PASS.

### Task 3: Atualizar testes comportamentais

**Files:**
- Modify: `src/pages/Dashboard.test.jsx`
- Modify: `src/stores/useCareerStore.test.js`
- Modify: `src/components/season/SeasonChampionOverlay.test.jsx`

- [ ] **Step 1: Remover testes do contrato excluído e preservar show/hide básico**

Em `useCareerStore.test.js`, remover testes de `pendingDashboardTab` e `debugForceSeasonEndChampion`. Ajustar os testes de carregamento para continuar garantindo que trocar de save limpa `championOverlay` e recarregar o mesmo ID preserva o payload, retirando somente as expectativas de navegação removida. Manter ou adicionar um teste pequeno verificando que `showChampionOverlay({ demo: true })` define o payload e `hideChampionOverlay()` volta para `null`.

Usar o store real:

```js
describe("useCareerStore dormant champion overlay", () => {
  beforeEach(() => {
    useCareerStore.setState({ championOverlay: null });
  });

  it("keeps basic open and close actions for the future backend", () => {
    useCareerStore.getState().showChampionOverlay({ demo: true });
    expect(useCareerStore.getState().championOverlay).toEqual({ demo: true });

    useCareerStore.getState().hideChampionOverlay();
    expect(useCareerStore.getState().championOverlay).toBeNull();
  });
});
```

- [ ] **Step 2: Ajustar testes isolados do componente**

Em `SeasonChampionOverlay.test.jsx`, manter os quatro mecanismos de fechamento e a precedência sobre `PauseMenu`, mas substituir as expectativas de `pendingDashboardTab` por `championOverlay === null`. Remover consumo de destino no teste de integração.

O helper final deve ser:

```js
function expectClosed(container) {
  expect(container.querySelector(".champ-ov")).not.toBeInTheDocument();
  expect(useCareerStore.getState().championOverlay).toBeNull();
}
```

- [ ] **Step 3: Confirmar GREEN comportamental e focado**

Run: `npx vitest run src/pages/Dashboard.test.jsx src/stores/useCareerStore.test.js src/components/season/SeasonChampionOverlay.test.jsx src/utils/postRaceLanding.test.js`

Expected: PASS.

### Task 4: Verificação

**Files:**
- Verify only.

- [ ] **Step 1: Rodar a trava estrutural e os testes focados juntos**

Run: `node --test scripts/tests/season-champion-disabled.test.mjs && npx vitest run src/pages/Dashboard.test.jsx src/stores/useCareerStore.test.js src/components/season/SeasonChampionOverlay.test.jsx src/utils/postRaceLanding.test.js`

Expected: PASS.

- [ ] **Step 2: Gerar build**

Run: `npm run build`

Expected: exit code 0.

- [ ] **Step 3: Auditar o diff**

Run: `git diff --check -- src/App.jsx src/pages/Settings.jsx src/pages/Dashboard.jsx src/pages/Dashboard.test.jsx src/stores/useCareerStore.js src/stores/useCareerStore.test.js src/components/season/SeasonChampionOverlay.test.jsx scripts/tests/season-champion-disabled.test.mjs`

Expected: nenhuma mensagem além de avisos LF/CRLF.

- [ ] **Step 4: Não criar commit de código automaticamente**

Os arquivos já possuem alterações locais anteriores. Preservar o working tree e entregar os paths alterados sem staging para não capturar trabalho alheio.
