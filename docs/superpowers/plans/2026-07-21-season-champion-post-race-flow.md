# Season Champion Post-Race Flow Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Exibir o overlay de campeão sobre a Home ao concluir o debriefing da última corrida e navegar uma única vez para Notícias quando o overlay for fechado.

**Architecture:** O `Dashboard` decide quando o debriefing final abre a celebração e continua proprietário da aba ativa. O store mantém o overlay global e uma solicitação transitória `pendingDashboardTab`; o fechamento do overlay publica a solicitação e o `Dashboard` a consome. O listener de Esc do overlay roda em captura e interrompe a propagação para não abrir o menu de pausa.

**Tech Stack:** React 18, Zustand 5, Vitest 4, Testing Library, Tauri/Vite.

---

## Mapa de arquivos

- `src/stores/useCareerStore.js`: contrato transitório do overlay e da solicitação de aba.
- `src/stores/useCareerStore.test.js`: regressões das ações globais do overlay.
- `src/components/season/SeasonChampionOverlay.jsx`: caminho único de fechamento e precedência do Esc.
- `src/components/season/SeasonChampionOverlay.test.jsx`: interação dos quatro mecanismos de fechamento e bloqueio de propagação.
- `src/pages/Dashboard.jsx`: orquestração `debriefing final → Home + overlay → Notícias`.
- `src/pages/Dashboard.test.jsx`: testes do gatilho final e do consumo da navegação.

## Chunk 1: Estado global e fechamento do overlay

### Task 1: Implementar o contrato transitório no store

**Files:**
- Modify: `src/stores/useCareerStore.test.js`
- Modify: `src/stores/useCareerStore.js`

- [ ] **Step 1: Escrever testes que descrevem publicação, consumo e ausência de destino**

Adicionar ao final de `src/stores/useCareerStore.test.js`:

```js
describe("useCareerStore champion overlay navigation", () => {
  beforeEach(() => {
    useCareerStore.setState({
      championOverlay: null,
      pendingDashboardTab: null,
    });
  });

  it("publishes and consumes the dashboard tab requested by the closed overlay", () => {
    useCareerStore.getState().showChampionOverlay({
      demo: true,
      afterCloseTab: "news",
    });

    useCareerStore.getState().hideChampionOverlay();

    expect(useCareerStore.getState().championOverlay).toBeNull();
    expect(useCareerStore.getState().pendingDashboardTab).toBe("news");
    expect(useCareerStore.getState().consumePendingDashboardTab()).toBe("news");
    expect(useCareerStore.getState().pendingDashboardTab).toBeNull();
    expect(useCareerStore.getState().consumePendingDashboardTab()).toBeNull();
  });

  it("does not request navigation when a debug preview closes", () => {
    useCareerStore.getState().showChampionOverlay({ demo: true });

    useCareerStore.getState().hideChampionOverlay();

    expect(useCareerStore.getState().championOverlay).toBeNull();
    expect(useCareerStore.getState().pendingDashboardTab).toBeNull();
  });

  it("keeps an unread request when hide is called again after the overlay closed", () => {
    useCareerStore.getState().showChampionOverlay({
      demo: true,
      afterCloseTab: "news",
    });
    useCareerStore.getState().hideChampionOverlay();

    useCareerStore.getState().hideChampionOverlay();

    expect(useCareerStore.getState().pendingDashboardTab).toBe("news");
  });

  it("clears a stale dashboard request when a new overlay opens", () => {
    useCareerStore.getState().showChampionOverlay({
      demo: true,
      afterCloseTab: "news",
    });
    useCareerStore.getState().hideChampionOverlay();

    useCareerStore.getState().showChampionOverlay({ demo: true });

    expect(useCareerStore.getState().pendingDashboardTab).toBeNull();
  });

  it("clears overlay navigation state when the career is cleared", () => {
    useCareerStore.getState().showChampionOverlay({
      demo: true,
      afterCloseTab: "news",
    });
    useCareerStore.getState().hideChampionOverlay();

    useCareerStore.getState().clearCareer();

    expect(useCareerStore.getState().championOverlay).toBeNull();
    expect(useCareerStore.getState().pendingDashboardTab).toBeNull();
  });
});
```

- [ ] **Step 2: Rodar o teste e confirmar RED**

Run: `npx vitest run src/stores/useCareerStore.test.js`

Expected: FAIL porque `pendingDashboardTab` e `consumePendingDashboardTab` ainda não existem e o fechamento atual não publica o destino.

- [ ] **Step 3: Implementar o mínimo no store**

Em `initialState`, adicionar:

```js
pendingDashboardTab: null,
```

Substituir as ações do overlay por:

```js
showChampionOverlay: (data = null) => set({
  championOverlay: data ?? { demo: true },
  pendingDashboardTab: null,
}),

hideChampionOverlay: () => {
  const overlay = get().championOverlay;
  if (!overlay) return;
  set({
    championOverlay: null,
    pendingDashboardTab:
      typeof overlay?.afterCloseTab === "string" ? overlay.afterCloseTab : null,
  });
},

consumePendingDashboardTab: () => {
  const target = get().pendingDashboardTab;
  if (target) set({ pendingDashboardTab: null });
  return target ?? null;
},
```

`clearCareer()` já espalha `initialState`, portanto deve limpar os dois campos sem tratamento adicional.

- [ ] **Step 4: Rodar o teste e confirmar GREEN**

Run: `npx vitest run src/stores/useCareerStore.test.js`

Expected: PASS em todos os testes do arquivo.

- [ ] **Step 5: Revisar o diff sem incluir alterações alheias**

Run: `git diff --check -- src/stores/useCareerStore.js src/stores/useCareerStore.test.js`

Expected: nenhuma mensagem. Não usar staging amplo porque `useCareerStore.js` já contém trabalho local do overlay.

### Task 2: Unificar o fechamento visual e dar precedência ao Esc

**Files:**
- Create: `src/components/season/SeasonChampionOverlay.test.jsx`
- Modify: `src/components/season/SeasonChampionOverlay.jsx`

- [ ] **Step 1: Escrever testes para os quatro mecanismos de fechamento**

Criar `src/components/season/SeasonChampionOverlay.test.jsx` usando o store real. Em cada caso, inicializar `championOverlay` com `{ demo: true, afterCloseTab: "news" }`, renderizar o componente e acionar separadamente:

```jsx
fireEvent.click(screen.getByRole("button", { name: /Continuar/i }));
fireEvent.click(screen.getByRole("button", { name: /Fechar/i }));
fireEvent.click(container.querySelector(".champ-ov"));
fireEvent.keyDown(window, { key: "Escape" });
```

Para cada mecanismo, verificar:

```js
expect(useCareerStore.getState().championOverlay).toBeNull();
expect(useCareerStore.getState().pendingDashboardTab).toBe("news");
```

No teste de Esc, registrar também um listener de bolha depois da renderização:

```js
const pauseListener = vi.fn();
window.addEventListener("keydown", pauseListener);
fireEvent.keyDown(window, { key: "Escape" });
expect(pauseListener).not.toHaveBeenCalled();
window.removeEventListener("keydown", pauseListener);
```

Disparar um `KeyboardEvent` cancelável e também verificar `expect(event.defaultPrevented).toBe(true)`.

Adicionar um teste de integração que renderiza irmãos reais dentro de `MemoryRouter`:

```jsx
render(
  <MemoryRouter>
    <PauseMenu />
    <SeasonChampionOverlay />
  </MemoryRouter>,
);

fireEvent.keyDown(window, { key: "Escape" });

expect(document.querySelector(".champ-ov")).not.toBeInTheDocument();
expect(document.querySelector(".glass-strong")).not.toBeInTheDocument();
expect(useCareerStore.getState().pendingDashboardTab).toBe("news");
```

O seletor `.glass-strong` representa o painel real aberto do `PauseMenu`; após o Esc deve continuar ausente. Importar `MemoryRouter` e `PauseMenu` reais. Usar `afterEach(cleanup)` e remover listeners em `finally` para não vazar estado entre testes.

- [ ] **Step 2: Rodar o novo teste e confirmar RED**

Run: `npx vitest run src/components/season/SeasonChampionOverlay.test.jsx`

Expected: os fechamentos por clique passam a publicar o destino após Task 1, mas o caso de Esc falha porque o listener de bolha ainda recebe o evento.

- [ ] **Step 3: Implementar um único fechamento e Esc em captura**

Em `SeasonChampionOverlay.jsx`, criar uma função local `closeOverlay` que chama `hideChampionOverlay`. Fazer backdrop, botão Fechar e botão Continuar chamarem essa função.

No efeito de teclado:

```js
const onKey = (event) => {
  if (event.key !== "Escape") return;
  event.preventDefault();
  event.stopImmediatePropagation();
  closeOverlay();
};
window.addEventListener("keydown", onKey, true);
return () => window.removeEventListener("keydown", onKey, true);
```

Estabilizar `closeOverlay` com `useCallback` para manter a dependência do efeito explícita.

- [ ] **Step 4: Rodar o teste e confirmar GREEN**

Run: `npx vitest run src/components/season/SeasonChampionOverlay.test.jsx`

Expected: PASS para Continuar, Fechar, backdrop e Esc; o listener que representa o `PauseMenu` não é chamado.

- [ ] **Step 5: Rodar os testes combinados do chunk**

Run: `npx vitest run src/stores/useCareerStore.test.js src/components/season/SeasonChampionOverlay.test.jsx`

Expected: PASS em ambos os arquivos.

## Chunk 2: Orquestração no Dashboard e verificação

### Task 3: Encadear debriefing final, Home, overlay e Notícias

**Files:**
- Modify: `src/pages/Dashboard.test.jsx`
- Modify: `src/pages/Dashboard.jsx`

- [ ] **Step 1: Preparar os mocks do teste do Dashboard**

Fazer o mock de `RaceResultViewV2` expor o callback:

```jsx
default: ({ onDismiss }) => (
  <div>
    <div>Classificação final</div>
    <button type="button" onClick={onDismiss}>Continuar debriefing</button>
  </div>
),
```

Mockar `NewsMagazineTab` com `<div>Revista de notícias</div>` e acrescentar ao estado-base:

```js
lastRaceWasFinale: false,
resultIsFresh: false,
season: { numero: 1, ano: 2026 },
careerId: "career-1",
showChampionOverlay: vi.fn(),
pendingDashboardTab: null,
consumePendingDashboardTab: vi.fn(() => null),
```

- [ ] **Step 2: Escrever o teste RED do fechamento do debriefing final**

Começar na aba Calendário usando o botão já exposto pelo mock de `MainLayout`, abrir um resultado fresco final e clicar em “Continuar debriefing”. Verificar:

```js
expect(screen.getByTestId("main-layout")).toHaveAttribute("data-active-tab", "standings");
expect(mockState.showChampionOverlay).toHaveBeenCalledWith({
  demo: true,
  afterCloseTab: "news",
});
expect(mockState.dismissResult).toHaveBeenCalledTimes(1);
```

Nesse teste unitário, “Home sob o overlay” é comprovado pela combinação `activeTab === "standings"`, chamada de `showChampionOverlay` e chamada de `dismissResult`; o host global do overlay é coberto separadamente pelo teste do componente.

Adicionar dois testes negativos do gatilho composto:

1. Resultado não fresco, mesmo marcado como final: começar em Calendário, clicar no debriefing e verificar que `showChampionOverlay` não foi chamado e que a aba continua `calendar`.
2. Resultado fresco de corrida comum: limpar `localStorage`, começar em Calendário, clicar no debriefing e verificar que `showChampionOverlay` não foi chamado e que a política existente selecionou `news`.

Os dois casos também devem verificar uma única chamada a `dismissResult`. Assim fica provado que `resultIsFresh && lastRaceWasFinale` é necessário, sem alterar reabertura histórica nem o fluxo adaptativo comum.

- [ ] **Step 3: Escrever o teste RED do pedido pós-overlay**

Renderizar o Dashboard com `pendingDashboardTab: "news"` e `consumePendingDashboardTab` retornando `"news"`. Verificar que a aba muda para Notícias e o consumo acontece uma vez:

```js
expect(screen.getByText("Revista de notícias")).toBeInTheDocument();
expect(mockState.consumePendingDashboardTab).toHaveBeenCalledTimes(1);
```

- [ ] **Step 4: Rodar o teste e confirmar RED**

Run: `npx vitest run src/pages/Dashboard.test.jsx`

Expected: FAIL porque o Dashboard ainda envia o final diretamente a Notícias e não observa `pendingDashboardTab`.

- [ ] **Step 5: Implementar o fluxo mínimo no Dashboard**

Importar `HOME_TAB` e `NEWS_TAB` de `postRaceLanding`. Assinar `showChampionOverlay`, `pendingDashboardTab` e `consumePendingDashboardTab` no store.

Adicionar o consumo único:

```js
useEffect(() => {
  if (!pendingDashboardTab) return;
  const target = consumePendingDashboardTab();
  if (target) setActiveTab(target);
}, [pendingDashboardTab, consumePendingDashboardTab]);
```

No início de `handleDismissResult`, tratar o final fresco antes da política adaptativa:

```js
if (resultIsFresh && lastRaceWasFinale) {
  cancelNewsReadEval();
  setActiveTab(HOME_TAB);
  showChampionOverlay({ demo: true, afterCloseTab: NEWS_TAB });
  dismissResult();
  return;
}
```

Manter intacto o caminho atual para corrida fresca comum e o caminho de resultado antigo.

- [ ] **Step 6: Rodar o teste e confirmar GREEN**

Run: `npx vitest run src/pages/Dashboard.test.jsx`

Expected: PASS em todos os testes do arquivo.

- [ ] **Step 7: Rodar a regressão pós-corrida existente**

Run: `npx vitest run src/utils/postRaceLanding.test.js src/pages/Dashboard.test.jsx src/stores/useCareerStore.test.js src/components/season/SeasonChampionOverlay.test.jsx`

Expected: PASS em todos os arquivos; a política adaptativa das corridas comuns permanece verde.

### Task 4: Verificação completa e handoff

**Files:**
- Verify only; no production edits expected.

- [ ] **Step 1: Rodar toda a suíte de UI**

Run: `npm run test:ui`

Expected: todos os testes passam.

- [ ] **Step 2: Rodar testes estruturais**

Run: `npm run test:structure`

Expected: todos os testes passam.

- [ ] **Step 3: Gerar o build web**

Run: `npm run build`

Expected: exit code 0 e bundle Vite gerado sem erros.

- [ ] **Step 4: Auditar o diff final**

Run: `git diff --check -- src/stores/useCareerStore.js src/stores/useCareerStore.test.js src/components/season/SeasonChampionOverlay.jsx src/components/season/SeasonChampionOverlay.test.jsx src/pages/Dashboard.jsx src/pages/Dashboard.test.jsx`

Expected: nenhuma mensagem. Confirmar por `git diff --stat` que nenhum arquivo fora do escopo foi alterado durante a implementação.

- [ ] **Step 5: Commit somente se for possível isolar com segurança os hunks desta correção**

Como `useCareerStore.js` e `SeasonChampionOverlay.jsx` já contêm alterações locais anteriores, não usar `git add` por arquivo de forma automática. Se os hunks puderem ser isolados sem incluir trabalho alheio, usar staging interativo e criar `fix: show season champion after final debrief`; caso contrário, preservar o working tree e informar que a correção ficou sem commit para não capturar alterações preexistentes.
