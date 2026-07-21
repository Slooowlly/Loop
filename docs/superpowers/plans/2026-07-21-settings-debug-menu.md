# Settings Debug Menu Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ocultar todas as ferramentas técnicas e de teste da tela de Configurações atrás de um toggle local `Menu Debug`, sempre fechado ao entrar.

**Architecture:** A alteração fica em `Settings.jsx`: um `useState(false)` controla uma linha acessível com `role="switch"` e um bloco condicional que agrupa as ferramentas existentes. Um teste de componente mocka as fronteiras Tauri, o canvas decorativo e o painel de rivalidades para verificar o comportamento real de renderização e remontagem dos controles da tela.

**Tech Stack:** React 18, React Router, Testing Library, Vitest, Tauri invoke mocks, Tailwind CSS.

---

## Chunk 1: Toggle e cobertura de regressão

### Task 1: Especificar o comportamento no teste de componente

**Files:**
- Create: `src/pages/Settings.test.jsx`
- Reference: `src/test/setup.js`
- Reference: `src/pages/LoadSave.test.jsx`

- [ ] **Step 1: Registrar o baseline local antes da edição**

Run: `git status --short -- src/pages/Settings.jsx src/pages/Settings.test.jsx`

Run: `git diff -- src/pages/Settings.jsx`

Expected: `Settings.jsx` já aparece modificado. Guardar o patch exibido como referência e não usar checkout, reset, staging amplo ou sobrescrita do arquivo.

Run: `npm run test:ui`

Expected baseline atual: duas falhas preexistentes (`src/i18n/localeParity.test.js` e `src/pages/Dashboard.test.jsx`). Registrar a contagem; esta tarefa não deve criar falhas adicionais.

- [ ] **Step 2: Criar o harness mínimo da tela**

Criar o arquivo completo abaixo. Ele mocka o canvas animado, as fronteiras Tauri e o painel de rivalidades, mantendo os controles reais de `Settings`.

```jsx
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

import Settings from "./Settings";

const mockInvoke = vi.fn();
let radioDemoEnabled = false;
let captureStatus = { active: false, frames: 0, dir: "" };

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args) => mockInvoke(...args),
}));

vi.mock("../components/ui/ParticleBackdrop", () => ({
  default: () => <div data-testid="particle-backdrop" />,
}));

vi.mock("../components/iracing/RivalryPerceptionPanel", () => ({
  default: () => <div>Rivalidades percebidas (debug)</div>,
}));

const config = {
  language: "pt-BR",
  autosave_enabled: true,
  version: "1.0.0",
};

function installInvokeMock() {
  mockInvoke.mockImplementation(async (command) => {
    if (command === "get_config") return config;
    if (command === "iracing_yellow_macro_status") {
      return { app_ini_found: true, installed: true, slot: 1 };
    }
    if (command === "iracing_auto_yellow_enabled") return false;
    if (command === "overlay_demo_enabled") return radioDemoEnabled;
    if (command === "race_capture_status") return captureStatus;
    return null;
  });
}

function renderPage() {
  return render(
    <MemoryRouter>
      <Settings />
    </MemoryRouter>,
  );
}

const debugLabels = [
  "Detalhes técnicos",
  "Comando de chat (teste)",
  "Quebra ao vivo (teste)",
  "Testar overlay de rádio",
  "Gravar corrida (debug)",
  "Fim de temporada → tela de campeão (debug)",
  "Rivalidades percebidas (debug)",
];

function expectDebugHidden() {
  for (const label of debugLabels) {
    expect(screen.queryByText(label)).not.toBeInTheDocument();
  }
}

function expectNormalSettingsVisible() {
  expect(screen.getByText("Idioma")).toBeInTheDocument();
  expect(screen.getByText("Salvamento automático")).toBeInTheDocument();
  expect(screen.getByText("Bandeira amarela automática")).toBeInTheDocument();
}

describe("Settings debug menu", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    radioDemoEnabled = false;
    captureStatus = { active: false, frames: 0, dir: "" };
    installInvokeMock();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // Os dois testes entram nos passos seguintes.
});
```

- [ ] **Step 3: Escrever o teste de visibilidade e remontagem**

Adicionar ao `describe`:

```jsx
it("starts closed on every mount and toggles all debug groups", async () => {
  const view = renderPage();
  let debugSwitch = await screen.findByRole("switch", { name: "Menu Debug" });

  expectNormalSettingsVisible();
  expect(debugSwitch).toHaveAttribute("aria-checked", "false");
  expectDebugHidden();

  fireEvent.click(debugSwitch);
  expect(debugSwitch).toHaveAttribute("aria-checked", "true");
  for (const label of debugLabels) expect(screen.getByText(label)).toBeInTheDocument();
  expectNormalSettingsVisible();

  fireEvent.click(debugSwitch);
  expect(debugSwitch).toHaveAttribute("aria-checked", "false");
  expectDebugHidden();
  expectNormalSettingsVisible();

  view.unmount();
  renderPage();
  debugSwitch = await screen.findByRole("switch", { name: "Menu Debug" });
  expect(debugSwitch).toHaveAttribute("aria-checked", "false");
  expectDebugHidden();
  expectNormalSettingsVisible();
});
```

- [ ] **Step 4: Escrever o teste de preservação de ações ativas**

Adicionar ao mesmo `describe`:

```jsx
it("hides active debug controls without stopping their backend actions", async () => {
  radioDemoEnabled = true;
  captureStatus = { active: true, frames: 42, dir: "C:/captures" };

  renderPage();
  const debugSwitch = await screen.findByRole("switch", { name: "Menu Debug" });
  await waitFor(() => {
    expect(mockInvoke).toHaveBeenCalledWith("overlay_demo_enabled");
    expect(mockInvoke).toHaveBeenCalledWith("race_capture_status");
  });

  fireEvent.click(debugSwitch);
  await waitFor(() => {
    const switches = screen.getAllByRole("switch");
    expect(switches).toHaveLength(2);
    expect(switches[1]).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("button", { name: "Parar (42 frames)" })).toBeInTheDocument();
  });

  fireEvent.click(debugSwitch);
  expect(screen.getAllByRole("switch")).toHaveLength(1);
  expect(mockInvoke).not.toHaveBeenCalledWith("overlay_set_demo", expect.anything());
  expect(mockInvoke).not.toHaveBeenCalledWith("race_capture_stop");

  fireEvent.click(debugSwitch);
  await waitFor(() => {
    const switches = screen.getAllByRole("switch");
    expect(switches[1]).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("button", { name: "Parar (42 frames)" })).toBeInTheDocument();
  });
});
```

- [ ] **Step 5: Executar o teste e observar a falha correta**

Run: `npm run test:ui -- src/pages/Settings.test.jsx`

Expected: FAIL porque ainda não existe um switch acessível chamado `Menu Debug` e os controles debug continuam renderizados inicialmente.

### Task 2: Implementar o toggle local e o agrupamento condicional

**Files:**
- Modify: `src/pages/Settings.jsx`
- Test: `src/pages/Settings.test.jsx`

- [ ] **Step 1: Adicionar estado local fechado por padrão**

Próximo dos demais estados exclusivamente visuais, adicionar:

```jsx
const [debugMenuOpen, setDebugMenuOpen] = useState(false);
```

Não persistir esse valor e não conectá-lo a Tauri ou Zustand.

- [ ] **Step 2: Renderizar a linha acessível `Menu Debug`**

Depois de `yellowMsg`, adicionar a linha completa abaixo:

```jsx
<div className="flex items-center justify-between gap-4 border-t border-white/10 px-5 py-3.5">
  <div className="min-w-0">
    <p className="text-[13px] font-medium text-text-primary">Menu Debug</p>
    <p className="text-[11px] text-text-secondary">
      Mostra ferramentas de teste e diagnóstico.
    </p>
  </div>
  <button
    type="button"
    role="switch"
    aria-label="Menu Debug"
    aria-checked={debugMenuOpen}
    onClick={() => setDebugMenuOpen((open) => !open)}
    className={`relative h-6 w-11 shrink-0 rounded-full transition-glass ${
      debugMenuOpen ? "bg-accent-primary" : "bg-white/15"
    }`}
  >
    <span
      className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-all ${
        debugMenuOpen ? "left-[22px]" : "left-0.5"
      }`}
    />
  </button>
</div>
```

- [ ] **Step 3: Agrupar todos os sete blocos técnicos**

Envolver, sem alterar handlers ou ordem interna, desde `<details>` de `Detalhes técnicos` até `<RivalryPerceptionPanel />` em:

```jsx
{debugMenuOpen && (
  <>
    {/* blocos técnicos existentes */}
  </>
)}
```

Manter `yellowMsg` fora do condicional. Não chamar comandos de desligamento quando o menu fechar.

- [ ] **Step 4: Executar o teste focado até ficar verde**

Run: `npm run test:ui -- src/pages/Settings.test.jsx`

Expected: PASS, sem warnings React.

- [ ] **Step 5: Revisar o diff restrito**

Run: `git diff --check -- src/pages/Settings.jsx`

Expected: exit code 0; em Windows pode haver apenas aviso de normalização LF/CRLF. Comparar o patch atual de `Settings.jsx` ao baseline do Task 1 e confirmar que os hunks preexistentes permanecem, acrescidos somente do toggle/condicional.

Run: `git status --short -- src/pages/Settings.jsx src/pages/Settings.test.jsx`

Expected: `Settings.jsx` modificado e `Settings.test.jsx` novo, sem outros caminhos afetados por esta tarefa.

### Task 3: Verificação integrada

**Files:**
- Verify: `src/pages/Settings.jsx`
- Verify: `src/pages/Settings.test.jsx`

- [ ] **Step 1: Executar toda a suíte de UI e comparar ao baseline**

Run: `npm run test:ui`

Expected: o novo `Settings.test.jsx` passa e não existem falhas novas. Se as duas falhas preexistentes de locale/Dashboard continuarem, registrá-las separadamente sem atribuí-las ao toggle.

- [ ] **Step 2: Executar novamente o teste focado de Configurações**

Run: `npm run test:ui -- src/pages/Settings.test.jsx`

Expected: os dois testes passam. A suíte estrutural não é gate desta tarefa: o baseline completo tem 14 falhas preexistentes, e até o arquivo estrutural que menciona Configurações falha antes em `NewsTab.jsx` inexistente.

- [ ] **Step 3: Gerar o build de produção**

Run: `npm run build`

Expected: build concluído com exit code 0.

- [ ] **Step 4: Auditar o resultado final**

Run: `git diff --check -- src/pages/Settings.jsx`

Expected: exit code 0, admitindo apenas aviso LF/CRLF. Revisar também o conteúdo completo do novo teste e confirmar a lista de requisitos da especificação `docs/superpowers/specs/2026-07-21-settings-debug-menu-design.md`.
