import { render, screen } from "@testing-library/react";

import RaceCoursePanel from "./RaceCoursePanel";

// O recharts não mede nada em jsdom (largura 0 → não renderiza as linhas), então o
// traçado é dublado. O que este teste protege é a LEITURA — os números e as frases que
// explicam o resultado — e a regra do vazio, não o desenho do SVG.
vi.mock("./RaceTraceChart", () => ({
  default: ({ rows, cars, yellowLaps }) => (
    <div
      data-testid="trace"
      data-rows={JSON.stringify(rows)}
      data-cars={cars.length}
      data-yellow={JSON.stringify(yellowLaps)}
    />
  ),
}));

vi.mock("react-i18next", () => ({
  // Devolve a chave + os args: o teste assevera o CONTRATO da chamada de tradução, não a
  // prosa (que vive no common.json e é coberta pelos testes de paridade).
  useTranslation: () => ({
    t: (key, args) => (args ? `${key}:${JSON.stringify(args)}` : key),
  }),
}));

function carro(overrides = {}) {
  return {
    pilot_id: "p1",
    pilot_name: "M. Costa",
    grid_position: 4,
    finish_position: 6,
    is_dnf: false,
    is_jogador: true,
    posicoes_por_segmento: [],
    gaps_para_da_frente_ms: [],
    segmentos_em_ar_sujo: 0,
    tentativas_ultrapassagem: 0,
    ultrapassagens_concluidas: 0,
    tentativas_sofridas: 0,
    maior_sequencia_preso: 0,
    volta_da_parada: [],
    posicao_antes_da_parada: [],
    posicao_depois: [],
    estrategia_id: "",
    ...overrides,
  };
}

function corrida(jogadorOverrides = {}, raceOverrides = {}) {
  return {
    total_laps: 20,
    safety_cars: [],
    ordem_pre_safety_car: [],
    race_results: [carro(jogadorOverrides)],
    ...raceOverrides,
  };
}

describe("RaceCoursePanel", () => {
  it("não renderiza nada quando a corrida não tem o dado de trecho", () => {
    // Save anterior à v55 ou import do iRacing: traçado ausente é melhor que traçado
    // chutado. É a regra do vazio.
    const { container } = render(<RaceCoursePanel result={corrida()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("não renderiza nada sem piloto do jogador no resultado", () => {
    const { container } = render(
      <RaceCoursePanel
        result={corrida({ is_jogador: false, posicoes_por_segmento: [4, 5, 6, 6, 6] })}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("conta a história em quatro números: largou, parou, voltou, chegou", () => {
    render(
      <RaceCoursePanel
        result={corrida({
          posicoes_por_segmento: [4, 5, 9, 7, 6],
          volta_da_parada: [12],
          posicao_antes_da_parada: [4],
          posicao_depois: [9],
        })}
      />,
    );

    expect(screen.getByText(/raceResult\.course\.started/)).toBeTruthy();
    // O custo do box é o par de posições, não só a volta — é o que faz o jogador
    // perceber que estratégia existe.
    expect(
      screen.getByText(/raceResult\.course\.pit:.*"lap":12.*"before":4.*"after":9/),
    ).toBeTruthy();
    expect(screen.getByText(/raceResult\.course\.finished:.*"pos":6/)).toBeTruthy();
  });

  it("cai no rótulo curto da parada quando o par de posições não foi gravado", () => {
    render(
      <RaceCoursePanel
        result={corrida({
          posicoes_por_segmento: [4, 5, 6, 6, 6],
          volta_da_parada: [12],
          posicao_antes_da_parada: [],
          posicao_depois: [],
        })}
      />,
    );
    expect(screen.getByText(/raceResult\.course\.pitBare:.*"lap":12/)).toBeTruthy();
  });

  it("mapeia trecho em volta e abre a linha na posição de largada", () => {
    render(
      <RaceCoursePanel
        result={corrida({ posicoes_por_segmento: [4, 5, 9, 7, 6] })}
      />,
    );
    const rows = JSON.parse(screen.getByTestId("trace").dataset.rows);
    // 5 trechos numa corrida de 20 voltas → um ponto a cada 4 voltas, mais a largada.
    expect(rows.map((r) => r.lap)).toEqual([0, 4, 8, 12, 16, 20]);
    expect(rows[0].c0).toBe(4); // largada
    expect(rows[3].c0).toBe(9); // pior momento, o 3º trecho
    expect(rows[5].c0).toBe(6); // chegada
  });

  it("mostra a taxa de conversão da ultrapassagem, não só a tentativa", () => {
    render(
      <RaceCoursePanel
        result={corrida({
          posicoes_por_segmento: [4, 5, 6, 6, 6],
          tentativas_ultrapassagem: 2,
          ultrapassagens_concluidas: 0,
          tentativas_sofridas: 5,
          maior_sequencia_preso: 3,
          segmentos_em_ar_sujo: 3,
        })}
      />,
    );
    // Tentou 2, passou 0 — o mecanismo mais novo do motor e o mais invisível.
    expect(screen.getByText("0/2")).toBeTruthy();
    expect(screen.getByText("5")).toBeTruthy();
    expect(screen.getAllByText("3").length).toBe(2); // preso + ar sujo
  });

  it("marca as voltas de safety car no traçado e relaciona a ordem anterior à chegada", () => {
    render(
      <RaceCoursePanel
        result={corrida(
          { posicoes_por_segmento: [4, 5, 6, 6, 6] },
          { safety_cars: [18], ordem_pre_safety_car: [["x1", "p1", "x2"]] },
        )}
      />,
    );
    expect(JSON.parse(screen.getByTestId("trace").dataset.yellow)).toEqual([18]);
    // Estava P2 na ordem registrada (índice 1) e terminou P6 — é o número que transforma
    // "perdi posições" em "a amarela me custou posições".
    const chip = screen.getByTitle(
      /raceResult\.course\.safetyCarTip:.*"lap":18.*"before":2.*"after":6/,
    );
    expect(chip).toBeTruthy();
  });

  it("limita o traçado e garante a linha do jogador mesmo fora do corte", () => {
    const grid = Array.from({ length: 12 }, (_, i) =>
      carro({
        pilot_id: `p${i}`,
        pilot_name: `Piloto ${i}`,
        finish_position: i + 1,
        grid_position: i + 1,
        is_jogador: false,
        posicoes_por_segmento: [i + 1, i + 1, i + 1, i + 1, i + 1],
      }),
    );
    grid[11].is_jogador = true;
    render(<RaceCoursePanel result={{ total_laps: 20, safety_cars: [], ordem_pre_safety_car: [], race_results: grid }} />);
    // 6 do corte + o jogador, que terminou P12 e entra de todo jeito.
    expect(Number(screen.getByTestId("trace").dataset.cars)).toBe(7);
  });
});
