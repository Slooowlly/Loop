import { render, screen } from "@testing-library/react";

import WeekendReadingPanel from "./WeekendReadingPanel";

// Dados FABRICADOS de propósito: a fase 3 está bloqueada no motor (a função pura sai de
// `simulation/forma.rs`, em refatoração). O contrato do DTO é o que este teste trava, para
// que ligar o fio depois seja só ligar o fio.
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key, args) => (args ? `${key}:${JSON.stringify(args)}` : key),
  }),
}));

function camada(band, overrides = {}) {
  return { band, qualifying_band: band, trend: null, support: null, ...overrides };
}

function leitura(overrides = {}) {
  return {
    race_id: "C001",
    available: true,
    track_affinity: camada(1),
    driver_form: camada(0),
    car_setup: camada(-1),
    ...overrides,
  };
}

describe("WeekendReadingPanel", () => {
  it("não renderiza nada sem leitura disponível", () => {
    // Regra do vazio: três "neutro" fabricados diriam que o fim de semana está morno
    // quando o jogo não sabe. Leitura errada é pior que leitura ausente.
    expect(render(<WeekendReadingPanel reading={null} />).container).toBeEmptyDOMElement();
    expect(
      render(<WeekendReadingPanel reading={leitura({ available: false })} />).container,
    ).toBeEmptyDOMElement();
  });

  it("não renderiza nada se faltar qualquer uma das três camadas", () => {
    const { container } = render(
      <WeekendReadingPanel reading={leitura({ car_setup: null })} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("mostra as três camadas separadas, na ordem do relógio de cada uma", () => {
    const { container } = render(<WeekendReadingPanel reading={leitura()} />);
    const rotulos = [...container.querySelectorAll(".uppercase")].map((n) =>
      // O título carrega um emoji de rádio; o que importa aqui é a ordem das camadas.
      n.textContent.replace(/^\W+/, ""),
    );
    // Do mais lento (permanente por pista) ao mais rápido (evento isolado). Três, nunca
    // duas: fundir duas quaisquer mistura história com ruído.
    expect(rotulos).toEqual([
      "nextRaceTab.weekendReading.title",
      "nextRaceTab.weekendReading.layers.track",
      "nextRaceTab.weekendReading.layers.form",
      "nextRaceTab.weekendReading.layers.setup",
    ]);
  });

  it("traduz cada faixa ordinal na palavra certa, nas bordas inclusive", () => {
    render(
      <WeekendReadingPanel
        reading={leitura({
          track_affinity: camada(2),
          driver_form: camada(0),
          car_setup: camada(-2),
        })}
      />,
    );
    expect(screen.getByText("nextRaceTab.weekendReading.bands.strongFavor")).toBeTruthy();
    expect(screen.getByText("nextRaceTab.weekendReading.bands.neutral")).toBeTruthy();
    expect(screen.getByText("nextRaceTab.weekendReading.bands.against")).toBeTruthy();
  });

  it("nunca imprime o valor bruto da camada", () => {
    // O número exato na tela convida engenharia reversa e promete precisão que a campanha
    // de calibração vai desmentir. A faixa é ordinal; o valor não cruza a ponte.
    const { container } = render(
      <WeekendReadingPanel reading={leitura({ driver_form: camada(2, { trend: 1 }) }) } />,
    );
    expect(container.textContent).not.toMatch(/[-+]?\d+([.,]\d+)?/);
  });

  it("mostra tendência só na camada que tem autocorrelação", () => {
    render(
      <WeekendReadingPanel
        reading={leitura({
          driver_form: camada(-1, { trend: -1 }),
          // A pista e o acerto vêm com trend null: prometer tendência onde ρ = 0 seria
          // inventar arco a partir de ruído.
          track_affinity: camada(1, { trend: null }),
          car_setup: camada(1, { trend: null }),
        })}
      />,
    );
    const setas = screen.getAllByTitle(/weekendReading\.trends\./);
    expect(setas).toHaveLength(1);
    expect(setas[0].getAttribute("title")).toBe("nextRaceTab.weekendReading.trends.falling");
  });

  it("cita o canal de classificação só quando ele diverge do ritmo", () => {
    const { rerender } = render(
      <WeekendReadingPanel
        reading={leitura({ track_affinity: camada(1, { qualifying_band: 2 }) })}
      />,
    );
    // Divergente: é o que explica voar no sábado e não converter no domingo.
    expect(
      screen.getByText(/qualifyingSplit:.*bands\.strongFavor/),
    ).toBeTruthy();

    rerender(<WeekendReadingPanel reading={leitura({ track_affinity: camada(1) })} />);
    expect(screen.queryByText(/qualifyingSplit/)).toBeNull();
  });

  it("mostra o fato de apoio verificável quando existe", () => {
    render(
      <WeekendReadingPanel
        reading={leitura({
          track_affinity: camada(1, { support: "3 corridas aqui, melhor resultado P4" }),
        })}
      />,
    );
    // É o que torna a afirmação checável contra a memória do jogador em vez de uma
    // alegação do jogo sobre si mesmo.
    expect(screen.getByText("3 corridas aqui, melhor resultado P4")).toBeTruthy();
  });
});
