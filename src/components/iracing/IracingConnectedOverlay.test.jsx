import { render, screen, waitFor, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import IracingConnectedOverlay from "./IracingConnectedOverlay";

// O overlay de "iRacing conectado" é a terceira tela da ponte exportar-correr-importar, e a
// única que fica na frente do jogador DURANTE a corrida. São 624 linhas que vivem de um único
// DTO — o `RaceHistory` do `race_monitor` — lido por nome de campo, em `snake_case`, sem
// nenhuma camada entre a ponte e o gráfico.
//
// É exatamente aí que mora o risco que estes casos travam: renomear um campo no Rust não
// quebra nada visível aqui. O acesso devolve `undefined`, o `?? []` transforma em lista vazia,
// e o overlay abre bonito mostrando "aguardando dados" a corrida inteira. Nenhum erro, nenhum
// console, nenhum teste vermelho — só um jogador olhando gráfico vazio.
//
// O foco é o CONTRATO: os nomes que atravessam a ponte, o argumento nomeado do comando de
// cores, e o portão de documento oculto que existe por causa de um "Out of Memory" real do
// WebView2 numa corrida longa.

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("../../stores/useCareerStore", () => ({
  default: (seletor) => seletor({ careerId: "C1" }),
}));

// O race trace é o único gráfico cujo EIXO é montado aqui dentro, a partir do par
// `lap`/`progress` do DTO. Trocamos o componente por um espião das props para poder afirmar
// sobre o eixo sem depender do recharts, que não mede nada em jsdom.
const { propsDoTrace } = vi.hoisted(() => ({ propsDoTrace: [] }));
vi.mock("../race/RaceTraceChart", () => ({
  default: (props) => {
    propsDoTrace.push(props);
    return <div data-testid="trace" />;
  },
}));

// O `ResponsiveContainer` do recharts mede o container com `ResizeObserver`, que o jsdom não
// tem. Sem este calço o overlay derruba o render inteiro no efeito de montagem. Medir zero é
// esperado e não atrapalha: o que se afere aqui é o estado "tem dado / não tem", que é decidido
// FORA do gráfico.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
};

/// Histórico com os nomes de campo que o Rust serializa hoje (`race_monitor::RaceHistory`).
/// Voltas a partir da 2 e sem parada de box, que é o filtro do `cleanLaps`.
function historico(extra = {}) {
  return {
    laps: [
      { lap: 2, cars: [{ idx: 0, position: 1, gap_leader: 0 }] },
      { lap: 3, cars: [{ idx: 0, position: 1, gap_leader: 0 }] },
    ],
    player_laps: [
      { lap: 2, time: 95.1 },
      { lap: 3, time: 94.7 },
      { lap: 4, time: 95.4 },
    ],
    player_track: [
      { ahead_idx: 3, gap_ahead: 1.2 },
      { ahead_idx: 3, gap_ahead: 0.8 },
      { ahead_idx: 3, gap_ahead: 1.4 },
    ],
    yellow_laps: [],
    player_incidents: [],
    player_pit_laps: [],
    player_car_idx: 0,
    car_laps: [{ car_idx: 3, lap: 2, time: 95.9 }],
    cars_meta: [
      { idx: 0, class_id: 1, is_pace: false, car_number: 7 },
      { idx: 3, class_id: 1, is_pace: false, car_number: 12 },
    ],
    class_names: { 1: "GT3" },
    driver_names: { 0: "Piloto Teste", 3: "Rival" },
    ...extra,
  };
}

/// Responde os comandos do overlay. `feedback` é o `RaceHistory`; `conectado` fecha a tela
/// quando vira falso.
function responder({ feedback = historico(), conectado = true, cores } = {}) {
  invoke.mockImplementation((cmd) => {
    if (cmd === "iracing_connected") return Promise.resolve(conectado);
    if (cmd === "iracing_get_race_feedback") return Promise.resolve(feedback);
    if (cmd === "iracing_car_colors")
      return Promise.resolve(cores ?? { by_name: {}, player_color: "#ff0000" });
    return Promise.resolve(null);
  });
}

/// Abre o overlay pelo caminho real: o app ganha foco e o iRacing está conectado.
async function abrir() {
  render(<IracingConnectedOverlay />);
  await act(async () => {
    window.dispatchEvent(new Event("focus"));
  });
}

/// Quantos cartões estão no estado "sem dados". São quatro no total; o número é o quanto do
/// DTO chegou vivo até o gráfico.
function cartoesVazios() {
  return screen.queryAllByText(/aguardando|waiting/i).length;
}

beforeEach(() => {
  invoke.mockReset();
  propsDoTrace.length = 0;
  // O overlay abre com foco/visibilidade; o jsdom começa visível.
  Object.defineProperty(document, "hidden", { value: false, configurable: true });
  Object.defineProperty(document, "visibilityState", { value: "visible", configurable: true });
});

describe("abertura e fechamento", () => {
  it("só aparece quando o iRacing está conectado", async () => {
    responder({ conectado: false });
    render(<IracingConnectedOverlay />);
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(invoke).toHaveBeenCalledWith("iracing_connected");
    expect(cartoesVazios()).toBe(0);
  });

  it("abre no foco do app e puxa o histórico da sessão", async () => {
    responder();
    await abrir();
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_get_race_feedback"));
  });

  it("pede as cores dos times com o argumento nomeado que o Rust espera", async () => {
    // `careerId` é casado por NOME no Rust. Trocar a grafia aqui não dá erro: o comando
    // recebe `None`, devolve o mapa vazio, e o grid inteiro fica cinza.
    responder();
    await abrir();
    await waitFor(() => {
      const chamada = invoke.mock.calls.find(([c]) => c === "iracing_car_colors");
      expect(chamada, "o overlay não pediu as cores").toBeTruthy();
      expect(chamada[1]).toEqual({ careerId: "C1" });
    });
  });

  it("um histórico nulo não derruba a tela", async () => {
    // O comando devolve `Option`; um `None` chega como `null` e todo acesso a campo seria
    // um erro em tempo de execução no meio da corrida.
    responder({ feedback: null });
    const { container } = render(<IracingConnectedOverlay />);
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(container).not.toBeEmptyDOMElement();
  });
});

describe("contrato do RaceHistory", () => {
  it("com os nomes do Rust, os gráficos recebem dados", async () => {
    responder();
    await abrir();
    // Os quatro cartões enchem: variação de voltas e ritmo vêm de `player_laps`, o gap vem
    // de `player_track` e o race trace vem de `laps`.
    await waitFor(() => expect(cartoesVazios()).toBe(0));
  });

  it("o mesmo payload em camelCase esvazia os gráficos", async () => {
    // Este é o caso que nenhum erro acusa: o DTO chega inteiro, com os mesmos valores, e a
    // tela abre igual. Se um dia o Rust passar a serializar em camelCase, é ESTE teste que
    // avisa — e não o jogador, olhando "aguardando dados" por trinta voltas.
    //
    // Três dos quatro cartões morrem, não os quatro: `laps` se escreve igual nas duas
    // convenções e é o único que sobrevive. Essa é a parte traiçoeira do sintoma — a tela
    // não fica obviamente vazia, fica pela metade.
    const bruto = historico();
    responder({
      feedback: {
        laps: bruto.laps,
        playerLaps: bruto.player_laps,
        playerTrack: bruto.player_track,
        yellowLaps: bruto.yellow_laps,
        playerIncidents: bruto.player_incidents,
        playerPitLaps: bruto.player_pit_laps,
        playerCarIdx: bruto.player_car_idx,
        carLaps: bruto.car_laps,
        carsMeta: bruto.cars_meta,
        classNames: bruto.class_names,
        driverNames: bruto.driver_names,
      },
    });
    await abrir();
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_get_race_feedback"));
    expect(cartoesVazios()).toBe(3);
  });

  it("campos ausentes valem lista vazia, não tela quebrada", async () => {
    // Save antigo, sessão sem quali, monitor recém-iniciado: vários campos do DTO são
    // `#[serde(default)]` e chegam ausentes de verdade.
    responder({ feedback: { player_car_idx: 0 } });
    await abrir();
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_get_race_feedback"));
    expect(cartoesVazios()).toBe(4);
  });
});

describe("portão do documento oculto", () => {
  it("não busca o histórico enquanto o jogador está dentro do iRacing", async () => {
    // O overlay fica montado com o app oculto atrás do sim em tela cheia. Sem este portão,
    // uma corrida longa acumulava um fetch por segundo de um payload de vários MB
    // re-renderizado em dezenas de séries num documento que ninguém vê — o WebView2
    // estourava a memória DURANTE a corrida.
    responder();
    await abrir();
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_get_race_feedback"));

    Object.defineProperty(document, "hidden", { value: true, configurable: true });
    const antes = invoke.mock.calls.filter(([c]) => c === "iracing_get_race_feedback").length;
    vi.useFakeTimers({ shouldAdvanceTime: true });
    await act(async () => {
      vi.advanceTimersByTime(6000); // três batidas do intervalo de 2s
    });
    const depois = invoke.mock.calls.filter(([c]) => c === "iracing_get_race_feedback").length;
    vi.useRealTimers();
    expect(depois).toBe(antes);
  });
});

describe("eixo do race trace", () => {
  it("o X carrega o progresso do líder, e não só a volta inteira", async () => {
    // O backend grava VÁRIOS pontos dentro da mesma volta do líder: um na virada e um a
    // cada troca de posição. O que separa um do outro é o `progress` (0..1). Lendo só o
    // `lap`, todos caíam no mesmo X: o gráfico apagava a volta inteira de ultrapassagens e
    // desenhava um degrau vertical na virada — o sintoma relatado na volta 1.
    responder({
      feedback: historico({
        laps: [
          { lap: 1, progress: 0, cars: [{ idx: 0, position: 2, gap: 4 }] },
          { lap: 1, progress: 0.5, cars: [{ idx: 0, position: 1, gap: 2 }] },
          { lap: 2, progress: 0, cars: [{ idx: 0, position: 1, gap: 1 }] },
        ],
      }),
    });
    await abrir();

    await waitFor(() => expect(propsDoTrace.length).toBeGreaterThan(0));
    const rows = propsDoTrace[propsDoTrace.length - 1].rows;
    expect(rows.map((r) => r.lap)).toEqual([1, 1.5, 2]);
  });

  it("a faixa de amarela cobre a volta que estava em curso", async () => {
    // `yellow_laps` guarda as voltas COMPLETAS do líder no instante da bandeira, então a
    // volta pintada ocupa [L, L+1] no eixo fracionário. O gráfico desenha a faixa centrada
    // em ±0,5 volta: sem o deslocamento, ela pintava a volta anterior pela metade.
    responder({ feedback: historico({ yellow_laps: [3] }) });
    await abrir();

    await waitFor(() => expect(propsDoTrace.length).toBeGreaterThan(0));
    expect(propsDoTrace[propsDoTrace.length - 1].yellowLaps).toEqual([3.5]);
  });
});
