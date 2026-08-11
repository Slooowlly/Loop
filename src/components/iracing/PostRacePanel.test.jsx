import { render, screen, waitFor, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import PostRacePanel from "./PostRacePanel";

// O painel de pós-corrida do iRacing: a metade de VOLTA do ciclo exportar-correr-importar.
// 690 linhas, 5 pontos de `invoke`, e o único teste do diretório `iracing/` até 11/08/2026
// era o de PttEngenheiroSettings — a vistoria de 10/08 marcou isto como [Alta].
//
// RESSALVA DE ALCANCE, medida em 11/08/2026: este componente NÃO É IMPORTADO por nenhum
// arquivo do frontend. O ciclo que o jogador percorre hoje passa por
// `race/nextrace/useIracingExport.js`, e este painel é a bancada anterior, que ficou. O
// roadmap (§ integração com o iRacing) já registra os dois painéis como decisão pendente:
// ligar ou apagar.
//
// Estes casos foram escritos assim mesmo, e o primeiro deles já pagou: o arquivo não
// COMPILAVA sob o parser estrito — `Tooltip` importado duas vezes, do recharts e do app —, e
// o balão do gráfico de batalha estava morto em silêncio desde então. Se a decisão for
// ligar, a rede existe; se for apagar, este arquivo vai junto.
//
// Os casos abaixo cobrem o que quebra de verdade: a FORMA do payload que o painel espera do
// `iracing_get_race_history` (um campo de lista que vire null estoura o `.length` na primeira
// linha do handler), o auto-salvamento que não pode disparar duas vezes na mesma tentativa, e
// o poll de 3s que precisa parar quando o jogador congela a tela.

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// Os gráficos do recharts não medem nada em jsdom e só somam ruído aqui.
vi.mock("../race/RaceTraceChart", () => ({ default: () => <div data-testid="trace" /> }));

/// Um retrato de volta. O `cars` NÃO é opcional: o painel faz `for (const c of snap.cars)` em
/// dois pontos, sem defesa, então uma volta sem a lista derruba o render inteiro. É a mesma
/// classe de fragilidade dos campos de lista do histórico.
const volta = (lap, cars = []) => ({ lap, cars });

/// Um histórico como o Rust entrega. Os campos de lista são o contrato frágil: o painel lê
/// `.length` de quatro deles para montar a assinatura, sem defesa.
function historico(extra = {}) {
  return {
    attempt_number: 1,
    finished: false,
    laps: [],
    player_laps: [],
    yellow_laps: [],
    player_track: [],
    cars: [],
    ...extra,
  };
}

beforeEach(() => {
  invoke.mockReset();
  vi.useFakeTimers({ shouldAdvanceTime: true });
});

afterEach(() => {
  vi.useRealTimers();
});

/// Responde cada comando do painel; o histórico é o único que varia por caso.
function responder(hist, { salvarComo = "corrida-1.json", salvas = [] } = {}) {
  invoke.mockImplementation((cmd) => {
    if (cmd === "iracing_get_race_history") return Promise.resolve(hist);
    if (cmd === "iracing_list_saved_races") return Promise.resolve(salvas);
    if (cmd === "iracing_save_race_history") return Promise.resolve(salvarComo);
    return Promise.resolve(null);
  });
}

describe("contrato com o backend", () => {
  it("monta sem estourar com um histórico vazio", async () => {
    // Antes da primeira volta TODAS as listas chegam vazias. É o estado em que o jogador
    // abre o painel na maioria das vezes.
    responder(historico());
    const { container } = render(<PostRacePanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_get_race_history"));
    expect(container).not.toBeEmptyDOMElement();
  });

  it("usa exatamente os cinco comandos registrados no Rust", async () => {
    // O nome do comando é string dos dois lados. O guard estrutural
    // `invoke-contra-generate-handler` cobre a existência; aqui o que se trava é que a tela
    // não passe a chamar um comando NOVO sem que alguém repare.
    responder(historico({ finished: true, laps: [volta(1)] }));
    render(<PostRacePanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    const chamados = new Set(invoke.mock.calls.map(([cmd]) => cmd));
    for (const cmd of chamados) {
      expect([
        "iracing_get_race_history",
        "iracing_list_saved_races",
        "iracing_save_race_history",
        "iracing_load_saved_race",
        "iracing_delete_saved_race",
      ]).toContain(cmd);
    }
  });

  it("engole o erro do poll automático e mostra o do carregamento pedido pelo jogador", async () => {
    // A distinção é deliberada e vale travar: o poll de 3s roda em modo SILENCIOSO, senão o
    // painel piscaria uma faixa de erro a cada três segundos com o iRacing fechado — que é o
    // estado normal quando o jogador abre a aba para rever uma corrida salva. O erro só sobe
    // à tela quando ele PEDE a atualização.
    invoke.mockImplementation((cmd) => {
      if (cmd === "iracing_get_race_history") return Promise.reject("sim fechado");
      return Promise.resolve([]);
    });
    render(<PostRacePanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_get_race_history"));
    expect(screen.queryByText(/sim fechado/)).not.toBeInTheDocument();

    const botao = screen
      .getAllByRole("button")
      .find((b) => /atualizar|refresh|carregar|load/i.test(b.textContent));
    expect(botao, "o painel precisa ter um botão de carregar manual").toBeTruthy();
    await act(async () => {
      botao.click();
    });
    await waitFor(() => expect(screen.getByText(/sim fechado/)).toBeInTheDocument());
  });

  it("uma pasta de salvas inexistente não derruba a tela", async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === "iracing_list_saved_races") return Promise.reject("sem pasta");
      if (cmd === "iracing_get_race_history") return Promise.resolve(historico());
      return Promise.resolve(null);
    });
    const { container } = render(<PostRacePanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_list_saved_races"));
    expect(container).not.toBeEmptyDOMElement();
  });
});

describe("auto-salvamento", () => {
  it("salva uma vez quando a corrida encerra com dados", async () => {
    responder(historico({ finished: true, laps: [volta(1)] }));
    render(<PostRacePanel />);
    await waitFor(() =>
      expect(invoke.mock.calls.filter(([c]) => c === "iracing_save_race_history")).toHaveLength(1),
    );
  });

  it("não salva de novo a cada poll da MESMA tentativa", async () => {
    // O poll roda a cada 3s enquanto a tela está aberta. Sem a trava por número de tentativa
    // o jogador acumularia um arquivo por poll — dezenas por corrida.
    responder(historico({ finished: true, laps: [volta(1)] }));
    render(<PostRacePanel />);
    await waitFor(() =>
      expect(invoke.mock.calls.filter(([c]) => c === "iracing_save_race_history")).toHaveLength(1),
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10_000);
    });
    expect(invoke.mock.calls.filter(([c]) => c === "iracing_save_race_history")).toHaveLength(1);
  });

  it("não salva corrida encerrada sem nenhum dado", async () => {
    // `finished` chega verdadeiro também quando o jogador só entrou e saiu da sessão. Salvar
    // aí gera arquivo vazio que depois aparece na lista como se fosse corrida.
    responder(historico({ finished: true }));
    render(<PostRacePanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_get_race_history"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(4000);
    });
    expect(invoke.mock.calls.filter(([c]) => c === "iracing_save_race_history")).toHaveLength(0);
  });

  it("não salva enquanto a corrida está em andamento", async () => {
    responder(historico({ finished: false, laps: [volta(1), volta(2)] }));
    render(<PostRacePanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_get_race_history"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(7000);
    });
    expect(invoke.mock.calls.filter(([c]) => c === "iracing_save_race_history")).toHaveLength(0);
  });

  it("uma falha ao salvar não impede o painel de continuar atualizando", async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === "iracing_get_race_history") {
        return Promise.resolve(historico({ finished: true, laps: [volta(1)] }));
      }
      if (cmd === "iracing_save_race_history") return Promise.reject("disco cheio");
      return Promise.resolve([]);
    });
    render(<PostRacePanel />);
    await waitFor(() =>
      expect(invoke.mock.calls.filter(([c]) => c === "iracing_save_race_history")).toHaveLength(1),
    );
    const antes = invoke.mock.calls.filter(([c]) => c === "iracing_get_race_history").length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(9500);
    });
    expect(
      invoke.mock.calls.filter(([c]) => c === "iracing_get_race_history").length,
    ).toBeGreaterThan(antes);
  });
});

describe("poll automático", () => {
  it("puxa o histórico repetidamente enquanto ligado", async () => {
    responder(historico());
    render(<PostRacePanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_get_race_history"));
    const antes = invoke.mock.calls.filter(([c]) => c === "iracing_get_race_history").length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(9500);
    });
    const depois = invoke.mock.calls.filter(([c]) => c === "iracing_get_race_history").length;
    expect(depois).toBeGreaterThanOrEqual(antes + 3);
  });

  it("para de puxar quando o componente sai da tela", async () => {
    // O painel vive dentro de uma aba. Um intervalo sobrevivente continuaria falando com o
    // SDK depois de o jogador ter saído — e o SDK é lido a 60 Hz por outra thread.
    responder(historico());
    const { unmount } = render(<PostRacePanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_get_race_history"));
    unmount();
    const antes = invoke.mock.calls.length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(9000);
    });
    expect(invoke.mock.calls.length).toBe(antes);
  });
});
