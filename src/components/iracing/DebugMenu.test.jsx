import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import DebugMenu from "./DebugMenu";

// O Menu Debug saiu do `Settings.jsx` inteiro. Estes casos são o que a tela grande só
// conseguia checar de fora: a hidratação acontece na MONTAGEM (mesmo fechado), abrir não
// dispara escrita nenhuma, e cada ferramenta chama o comando certo com o argumento certo.

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("./RivalryPerceptionPanel", () => ({
  default: () => <div>Rivalidades percebidas (debug)</div>,
}));

const LEITURAS = new Set(["overlay_demo_enabled", "race_capture_status"]);

const blocos = [
  "Detalhes técnicos",
  "Comando de chat (teste)",
  "Quebra ao vivo (teste)",
  "Testar overlay de rádio",
  "Gravar corrida (debug)",
  "Rivalidades percebidas (debug)",
];

const yellowStatus = {
  app_ini_found: true,
  app_ini_path: "C:\\iRacing\\app.ini",
  slot: 3,
  original: "You're welcome",
  current_value: "!y$",
};

let captura;
let demoLigada;
let props;

/// Só os dois comandos de LEITURA respondem por padrão. Escrita é declarada caso a caso,
/// para nenhum teste disparar uma sem dizer.
function configurarBackend(respostas = {}) {
  captura = { active: false, frames: 0, dir: "C:\\Loop\\debug\\race_captures" };
  demoLigada = false;
  invoke.mockReset();
  invoke.mockImplementation(async (comando, args) => {
    if (comando in respostas) {
      const r = respostas[comando];
      return typeof r === "function" ? r(args) : r;
    }
    if (!LEITURAS.has(comando)) throw new Error(`Comando de escrita não esperado: ${comando}`);
    if (comando === "overlay_demo_enabled") return demoLigada;
    return { ...captura };
  });
}

function montar(extra = {}) {
  props = {
    yellowStatus,
    chatText: "",
    setChatText: vi.fn(),
    chatMsg: "",
    chatBusy: false,
    sendChatTest: vi.fn(),
    ...extra,
  };
  return render(<DebugMenu {...props} />);
}

const interruptor = () => screen.getByRole("switch", { name: "Menu Debug" });

beforeEach(() => configurarBackend());

describe("DebugMenu", () => {
  it("nasce fechado e não desenha nenhum dos blocos", () => {
    montar();
    expect(interruptor()).not.toBeChecked();
    blocos.forEach((b) => expect(screen.queryByText(b)).not.toBeInTheDocument());
  });

  it("hidrata as ferramentas na montagem, antes de o menu ser aberto", async () => {
    demoLigada = true;
    captura = { active: true, frames: 42, dir: "C:\\Loop\\debug\\race_captures" };
    montar();

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("overlay_demo_enabled");
      expect(invoke).toHaveBeenCalledWith("race_capture_status");
    });

    // Ao abrir, os controles já aparecem no estado de verdade em vez de piscarem desligados.
    fireEvent.click(interruptor());
    await waitFor(() =>
      expect(screen.getByRole("switch", { name: "Testar overlay de rádio" })).toBeChecked(),
    );
    expect(screen.getByRole("button", { name: "Parar (42 frames)" })).toBeInTheDocument();
  });

  it("abrir revela os seis blocos, sem escrever nada", async () => {
    montar();
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("race_capture_status"));

    fireEvent.click(interruptor());

    expect(interruptor()).toBeChecked();
    blocos.forEach((b) => expect(screen.getByText(b)).toBeInTheDocument());
    expect(invoke.mock.calls.every(([c]) => LEITURAS.has(c))).toBe(true);
  });

  it("fechar e reabrir preserva o estado das ferramentas sem redisparar as ações", async () => {
    demoLigada = true;
    captura = { active: true, frames: 42, dir: "C:\\Loop\\debug\\race_captures" };
    montar();
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("overlay_demo_enabled"));

    fireEvent.click(interruptor());
    fireEvent.click(interruptor());
    fireEvent.click(interruptor());

    await waitFor(() =>
      expect(screen.getByRole("switch", { name: "Testar overlay de rádio" })).toBeChecked(),
    );
    const comandos = invoke.mock.calls.map(([c]) => c);
    expect(comandos).not.toContain("overlay_set_demo");
    expect(comandos).not.toContain("race_capture_stop");
  });

  it("o interruptor do rádio manda o estado NOVO", async () => {
    configurarBackend({ overlay_set_demo: null });
    montar();
    fireEvent.click(interruptor());

    fireEvent.click(screen.getByRole("switch", { name: "Testar overlay de rádio" }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith("overlay_set_demo", { on: true }));
  });

  it("o gravador inicia e diz onde salvou", async () => {
    configurarBackend({ race_capture_start: "C:\\Loop\\debug\\corrida.jsonl" });
    montar();
    fireEvent.click(interruptor());

    fireEvent.click(screen.getByRole("button", { name: "Iniciar gravação" }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith("race_capture_start"));
    expect(await screen.findByText(/corrida\.jsonl/)).toBeInTheDocument();
  });

  it("o gravador ativo para e informa o arquivo", async () => {
    configurarBackend({ race_capture_stop: "C:\\Loop\\debug\\corrida.jsonl" });
    captura = { active: true, frames: 7, dir: "C:\\Loop\\debug" };
    montar();
    fireEvent.click(interruptor());

    fireEvent.click(await screen.findByRole("button", { name: "Parar (7 frames)" }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith("race_capture_stop"));
    expect(await screen.findByText(/Salvo: C:\\Loop\\debug\\corrida\.jsonl/)).toBeInTheDocument();
  });

  it("armar a quebra no meu carro conta o resultado, inclusive quando o iRacing recusa", async () => {
    configurarBackend({ iracing_arm_test_breakdown: false });
    montar();
    fireEvent.click(interruptor());

    fireEvent.click(screen.getByRole("button", { name: "Armar (meu carro)" }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_arm_test_breakdown"));
    expect(await screen.findByText(/Não deu pra armar/)).toBeInTheDocument();
  });

  it("armar a grade inteira usa o outro comando", async () => {
    configurarBackend({ iracing_arm_test_breakdown_grid: null });
    montar();
    fireEvent.click(interruptor());

    fireEvent.click(screen.getByRole("button", { name: "Armar (grade toda)" }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_arm_test_breakdown_grid"));
    expect(await screen.findByText(/Grade armada!/)).toBeInTheDocument();
  });

  it("o campo de chat vem de fora: Enter dispara o envio da tela", () => {
    montar({ chatText: "!black #1 20" });
    fireEvent.click(interruptor());

    const campo = screen.getByPlaceholderText("!black #1 20");
    fireEvent.keyDown(campo, { key: "Enter" });

    expect(props.sendChatTest).toHaveBeenCalled();
  });

  it("sem texto, o botão de enviar fica desabilitado", () => {
    montar({ chatText: "   " });
    fireEvent.click(interruptor());

    expect(screen.getByRole("button", { name: "Enviar" })).toBeDisabled();
  });

  it("os detalhes técnicos leem o status do app.ini que veio da tela", () => {
    montar();
    fireEvent.click(interruptor());

    expect(screen.getByText("Encontrado")).toBeInTheDocument();
    expect(screen.getByText("C:\\iRacing\\app.ini")).toBeInTheDocument();
    expect(screen.getByText("Slot AutoChatStr3")).toBeInTheDocument();
  });

  it("sem app.ini, os detalhes dizem isso em vez de fingir que achou", () => {
    montar({ yellowStatus: { app_ini_found: false } });
    fireEvent.click(interruptor());

    expect(screen.getByText("Não encontrado")).toBeInTheDocument();
  });
});
