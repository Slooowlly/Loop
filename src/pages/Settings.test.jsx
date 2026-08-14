import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

import Settings from "./Settings";

const mockInvoke = vi.fn();
const mockNavigate = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args) => mockInvoke(...args),
}));

vi.mock("react-router-dom", async (importOriginal) => ({
  ...(await importOriginal()),
  useNavigate: () => mockNavigate,
}));

vi.mock("../components/ui/ParticleBackdrop", () => ({
  default: () => <div data-testid="particle-backdrop" />,
}));

vi.mock("../components/iracing/RivalryPerceptionPanel", () => ({
  default: () => <div>Rivalidades percebidas (debug)</div>,
}));

const configFixture = {
  language: "pt-BR",
  autosave_enabled: true,
  version: "1.0.0",
};

const yellowStatusFixture = {
  app_ini_found: true,
  installed: true,
  app_ini_path: "C:\\iRacing\\app.ini",
  slot: 3,
  original: "You're welcome",
  current_value: "!y$",
};

const idleCaptureFixture = {
  active: false,
  frames: 0,
  dir: "C:\\Loop\\debug\\race_captures",
};

const activeCaptureFixture = {
  active: true,
  frames: 42,
  dir: "C:\\Loop\\debug\\race_captures",
};

const debugGroupLabels = [
  "Detalhes técnicos",
  "Comando de chat (teste)",
  "Quebra ao vivo (teste)",
  "Testar overlay de rádio",
  "Gravar corrida (debug)",
  "Rivalidades percebidas (debug)",
];

const readOnlyCommands = new Set([
  "get_config",
  "iracing_yellow_macro_status",
  "iracing_auto_yellow_enabled",
  "overlay_demo_enabled",
  "race_capture_status",
  "iracing_spotter_status",
  "iracing_modo_janela_status",
  "list_saves",
]);

const modoJanelaFixture = {
  encontrado: true,
  em_janela: true,
  simulador_aberto: false,
  pode_desfazer: true,
  arquivos: [],
};

let radioDemoEnabled;
let captureFixture;
let spotterStatusFixture;
let savesFixture;

function renderSettings() {
  return render(
    <MemoryRouter initialEntries={["/settings"]}>
      <Settings />
    </MemoryRouter>,
  );
}

async function waitForSettings() {
  await screen.findByText("Idioma");
}

function expectDebugGroupsHidden() {
  debugGroupLabels.forEach((label) => {
    expect(screen.queryByText(label)).not.toBeInTheDocument();
  });
}

function expectDebugGroupsVisible() {
  debugGroupLabels.forEach((label) => {
    expect(screen.getByText(label)).toBeInTheDocument();
  });
}

function expectRegularSettingsVisible() {
  expect(screen.getByText("Idioma")).toBeInTheDocument();
  expect(screen.getByText("Salvamento automático")).toBeInTheDocument();
  expect(screen.getByText("Bandeira amarela automática")).toBeInTheDocument();
}

/// Backend de leitura. `extras` acrescenta respostas para comandos que MUDAM algo — o
/// padrão é recusá-los, para um teste nunca disparar uma escrita sem dizer.
function configurarBackend(extras = {}) {
  radioDemoEnabled = false;
  captureFixture = idleCaptureFixture;
  spotterStatusFixture = { app_ini_found: true, enabled: false };
  savesFixture = [{ career_id: "C1", player_name: "Ana" }];

  mockNavigate.mockReset();
  mockInvoke.mockReset();
  mockInvoke.mockImplementation(async (command) => {
    if (command in extras) {
      const resposta = extras[command];
      return typeof resposta === "function" ? resposta() : resposta;
    }
    if (!readOnlyCommands.has(command)) {
      throw new Error(`Unexpected mutable command: ${command}`);
    }
    if (command === "get_config") return { ...configFixture };
    if (command === "iracing_yellow_macro_status") return { ...yellowStatusFixture };
    if (command === "iracing_auto_yellow_enabled") return true;
    if (command === "overlay_demo_enabled") return radioDemoEnabled;
    if (command === "race_capture_status") return { ...captureFixture };
    if (command === "iracing_spotter_status") return { ...spotterStatusFixture };
    if (command === "iracing_modo_janela_status") return { ...modoJanelaFixture };
    if (command === "list_saves") return savesFixture;
    return null;
  });
}

describe("Settings debug menu", () => {
  beforeEach(() => {
    configurarBackend();
  });

  it("keeps regular settings visible and resets the closed debug menu on remount", async () => {
    const view = renderSettings();
    await waitForSettings();

    expectRegularSettingsVisible();

    const debugToggle = screen.getByRole("switch", { name: "Menu Debug" });
    expect(debugToggle).not.toBeChecked();
    expectDebugGroupsHidden();

    fireEvent.click(debugToggle);

    expect(debugToggle).toBeChecked();
    expectDebugGroupsVisible();
    expectRegularSettingsVisible();

    fireEvent.click(debugToggle);

    expect(debugToggle).not.toBeChecked();
    expectDebugGroupsHidden();
    expectRegularSettingsVisible();

    fireEvent.click(debugToggle);
    expect(debugToggle).toBeChecked();

    view.unmount();
    renderSettings();
    await waitForSettings();

    expect(screen.getByRole("switch", { name: "Menu Debug" })).not.toBeChecked();
    expectDebugGroupsHidden();
  });

  it("preserves active debug controls across close and reopen without triggering their actions", async () => {
    radioDemoEnabled = true;
    captureFixture = activeCaptureFixture;

    renderSettings();
    await waitForSettings();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("overlay_demo_enabled");
      expect(mockInvoke).toHaveBeenCalledWith("race_capture_status");
    });

    const hydrationCallCount = mockInvoke.mock.calls.length;

    const debugToggle = screen.getByRole("switch", { name: "Menu Debug" });
    fireEvent.click(debugToggle);

    const radioDemoToggle = await screen.findByRole("switch", {
      name: /Testar overlay de rádio/i,
    });
    await waitFor(() => expect(radioDemoToggle).toBeChecked());
    expect(await screen.findByRole("button", { name: "Parar (42 frames)" })).toBeInTheDocument();

    fireEvent.click(debugToggle);

    expectDebugGroupsHidden();

    fireEvent.click(debugToggle);

    const reopenedRadioDemoToggle = await screen.findByRole("switch", {
      name: /Testar overlay de rádio/i,
    });
    await waitFor(() => expect(reopenedRadioDemoToggle).toBeChecked());
    expect(await screen.findByRole("button", { name: "Parar (42 frames)" })).toBeInTheDocument();

    const invokedCommands = mockInvoke.mock.calls.map(([command]) => command);
    const additionalCommands = invokedCommands.slice(hydrationCallCount);
    expect(additionalCommands.filter((command) => !readOnlyCommands.has(command))).toEqual([]);
    expect(invokedCommands).not.toContain("overlay_set_demo");
    expect(invokedCommands).not.toContain("race_capture_stop");
  });
});

// ── Desfazer o que o Loop escreveu na pasta do iRacing ───────────────────────────────
//
// O painel tem teste próprio em `components/iracing/IracingDesfazerPanel.test.jsx`. O que
// esta tela precisa garantir é o POSICIONAMENTO: a volta é do jogador, então ela não pode
// estar atrás do interruptor do Menu Debug, e mora junto do interruptor que impede as
// próximas pinturas.
describe("Settings — desfazer alterações no iRacing", () => {
  it("aparece sem depender do Menu Debug, logo abaixo da pintura automática", async () => {
    configurarBackend();
    renderSettings();
    await waitForSettings();

    expect(screen.getByRole("switch", { name: "Menu Debug" })).not.toBeChecked();
    expect(screen.getByText("Desfazer alterações no iRacing")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Desfazer modo janela" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Desfazer pintura do carro" })).toBeInTheDocument();

    const rotulos = [...document.querySelectorAll("p")].map((p) => p.textContent);
    expect(rotulos.indexOf("Pintar meu carro na cor da equipe")).toBeLessThan(
      rotulos.indexOf("Desfazer alterações no iRacing"),
    );
  });

  it("abrir a tela lê o status do modo janela e não escreve nada", async () => {
    configurarBackend();
    renderSettings();
    await waitForSettings();

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("iracing_modo_janela_status"),
    );
    const escritas = mockInvoke.mock.calls
      .map(([command]) => command)
      .filter((command) => !readOnlyCommands.has(command));
    expect(escritas).toEqual([]);
  });
});

// ── Spotter nativo do iRacing ────────────────────────────────────────────────────────
//
// O único interruptor da tela que NÃO passa pelo `handleToggle` genérico: a preferência
// e o `app.ini` precisam mudar juntos, e quem faz as duas coisas é um comando dedicado.
// A regra que precisa de rede é a do erro — o interruptor NÃO volta quando a escrita
// falha, porque o backend grava a preferência ANTES de tocar no arquivo. Voltar faria a
// tela discordar do disco, e o boot seguinte "desfaria" a escolha do jogador sozinho.
describe("Settings — spotter do Loop", () => {
  it("liga o interruptor mandando o estado NOVO, e não o atual", async () => {
    configurarBackend({ iracing_spotter_set: { app_ini_found: true, enabled: true } });
    renderSettings();
    await waitForSettings();

    fireEvent.click(screen.getByText("Spotter do Loop"));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("iracing_spotter_set", { enabled: true }),
    );
  });

  it("sem app.ini, o interruptor não dispara nada", async () => {
    configurarBackend();
    spotterStatusFixture = { app_ini_found: false };
    renderSettings();
    await waitForSettings();

    // O texto de apoio diz por que está desabilitado, em vez de só apagar o controle.
    await screen.findByText(/app\.ini/i);
    fireEvent.click(screen.getByText("Spotter do Loop"));

    expect(mockInvoke).not.toHaveBeenCalledWith("iracing_spotter_set", expect.anything());
  });

  it("falha na escrita AVISA e o interruptor NÃO volta — a preferência já está salva", async () => {
    configurarBackend({
      iracing_spotter_set: () => Promise.reject(new Error("app.ini aberto no sim")),
    });
    renderSettings();
    await waitForSettings();

    fireEvent.click(screen.getByText("Spotter do Loop"));

    expect(await screen.findByText(/app\.ini aberto no sim/)).toBeInTheDocument();
    // Reler o status depois da falha é o que mantém a tela alinhada ao disco.
    await waitFor(() => {
      const leituras = mockInvoke.mock.calls.filter(([c]) => c === "iracing_spotter_status");
      expect(leituras.length).toBeGreaterThan(1);
    });
  });
});

// ── Destino do botão de baixo ────────────────────────────────────────────────────────
//
// A tela de Configurações é também a primeira que um jogador sem carreira nenhuma vê.
// O botão decide entre voltar ao menu e oferecer a criação da primeira carreira, e a
// decisão depende de uma leitura de ponte que pode falhar.
describe("Settings — botão de salvar", () => {
  it("com carreira existente, volta ao menu sem perguntar nada", async () => {
    configurarBackend();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    renderSettings();
    await waitForSettings();

    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("list_saves"));
    await vi.advanceTimersByTimeAsync(800);
    expect(mockNavigate).toHaveBeenCalledWith("/menu");
    vi.useRealTimers();
  });

  it("sem nenhuma carreira, pergunta antes de levar ao funil de criação", async () => {
    configurarBackend();
    savesFixture = [];
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const confirmar = vi.spyOn(window, "confirm").mockReturnValue(true);
    renderSettings();
    await waitForSettings();

    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));

    await waitFor(() => expect(confirmar).toHaveBeenCalled());
    await vi.advanceTimersByTimeAsync(800);
    expect(mockNavigate).toHaveBeenCalledWith("/new-career");
    vi.useRealTimers();
    confirmar.mockRestore();
  });

  it("recusar a pergunta não navega para lugar nenhum", async () => {
    configurarBackend();
    savesFixture = [];
    const confirmar = vi.spyOn(window, "confirm").mockReturnValue(false);
    renderSettings();
    await waitForSettings();

    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));

    await waitFor(() => expect(confirmar).toHaveBeenCalled());
    expect(mockNavigate).not.toHaveBeenCalled();
    confirmar.mockRestore();
  });

  it("falha ao listar leva ao MENU, e não a criar uma carreira que talvez já exista", async () => {
    configurarBackend({ list_saves: () => Promise.reject(new Error("pasta inacessível")) });
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const confirmar = vi.spyOn(window, "confirm").mockReturnValue(true);
    renderSettings();
    await waitForSettings();

    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));

    await vi.advanceTimersByTimeAsync(800);
    // O ponto: uma leitura que falhou não pode virar "você não tem carreira nenhuma".
    expect(confirmar).not.toHaveBeenCalled();
    expect(mockNavigate).toHaveBeenCalledWith("/menu");
    vi.useRealTimers();
    confirmar.mockRestore();
  });
});

// ── Falha ao carregar o config ───────────────────────────────────────────────────────
//
// O `get_config` é a primeira leitura da tela e a única sem a qual não há nada para
// desenhar. Quando ela falhava, o `catch` só escrevia no console: `loading` caía para
// falso, `config` continuava nulo e a tela ficava no fundo vazio para sempre — com o
// mesmo desenho do estado "ainda carregando", e sem o cabeçalho, onde mora a volta.
describe("Settings — falha ao carregar o config", () => {
  it("diz que falhou, mantém a volta de pé e recarrega no retry", async () => {
    let falhar = true;
    configurarBackend({
      get_config: () =>
        falhar
          ? Promise.reject(new Error("config.json ilegível"))
          : Promise.resolve({ ...configFixture }),
    });

    renderSettings();

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Não foi possível carregar as configurações.",
    );
    // A tela não finge que carregou: nenhuma opção aparece.
    expect(screen.queryByText("Idioma")).not.toBeInTheDocument();

    // A navegação de volta continua no ar — o jogador não fica preso na tela.
    fireEvent.click(screen.getByRole("button", { name: "Voltar" }));
    expect(mockNavigate).toHaveBeenCalledWith("/menu");

    falhar = false;
    fireEvent.click(screen.getByRole("button", { name: "Tentar de novo" }));

    await waitForSettings();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expectRegularSettingsVisible();
  });
});
