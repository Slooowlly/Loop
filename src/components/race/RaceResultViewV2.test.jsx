import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import RaceResultViewV2 from "./RaceResultViewV2";
import { buildMockBreakdowns } from "./__mockTelemetry";

let mockState = {};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

const DRIVERS = [
  { id: "drv-1", nome: "M. Costa", equipe_nome: "Equipe Aurora", equipe_cor: "#58a6ff" },
  { id: "drv-2", nome: "R. Silva", equipe_nome: "Nordeste Racing", equipe_cor: "#ff7b72" },
];

function entry(id, name, team, pos, overrides = {}) {
  return {
    pilot_id: id,
    pilot_name: name,
    team_id: `team-${id}`,
    team_name: team,
    grid_position: pos,
    finish_position: pos,
    positions_gained: 0,
    best_lap_time_ms: 92_000,
    gap_to_winner_ms: 0,
    is_dnf: false,
    dnf_reason: null,
    incidents_count: 0,
    has_fastest_lap: false,
    points_earned: 10,
    is_jogador: false,
    laps_completed: 20,
    ...overrides,
  };
}

const RESULT = {
  track_name: "Interlagos",
  weather: "Dry",
  total_laps: 20,
  race_results: [
    entry("drv-1", "M. Costa", "Equipe Aurora", 1, { is_jogador: true }),
    entry("drv-2", "R. Silva", "Nordeste Racing", 2, {
      is_dnf: true,
      dnf_reason: "motor fundiu por superaquecimento",
      points_earned: 0,
    }),
  ],
};

const EVALUATION = {
  assessment: "Dentro",
  grade: 7.2,
  headline: "Corrida sólida.",
  team_read: "O carro aguentou.",
  target_low: 1,
  target_high: 3,
};

/// Uma quebra do jogador (leve, custou tempo) e uma da IA (encerrou a corrida dela).
const BREAKDOWNS = [
  {
    driver_id: "drv-1",
    driver_name: "M. Costa",
    part: "gearbox",
    part_name: "Câmbio",
    lap: 12,
    severity: "heavy",
    penalty_secs: 17,
    label: "câmbio perdeu a 3ª marcha",
    is_player: true,
  },
  {
    driver_id: "drv-2",
    driver_name: "R. Silva",
    part: "engine",
    part_name: "Motor",
    lap: 8,
    severity: "dnf",
    penalty_secs: null,
    label: "motor fundiu por superaquecimento",
    is_player: false,
  },
];

/// Roteia por comando: a tela dispara `get_drivers_by_category`, `post_race_debrief_ai` e
/// `get_race_breakdowns` em paralelo, e cada um espera uma forma diferente de resposta.
function mockInvoke({ breakdowns = [] } = {}) {
  invoke.mockImplementation((cmd) => {
    if (cmd === "get_race_breakdowns") return Promise.resolve(breakdowns);
    if (cmd === "get_drivers_by_category") return Promise.resolve(DRIVERS);
    return Promise.resolve(null); // post_race_debrief_ai: sem IA, cai no texto do cérebro
  });
}

function renderView(props = {}) {
  return render(
    <RaceResultViewV2
      result={RESULT}
      evaluation={EVALUATION}
      telemetry={null}
      maintenance={{ total: 1200, items: [] }}
      onDismiss={vi.fn()}
      {...props}
    />,
  );
}

/// Repercussão como o backend manda: rótulos JÁ traduzidos (`EventRepercussionSummary`)
/// e o delta em valor de exibição. A tela não pode recalcular nem retraduzir nada disso.
const REPERCUSSION = {
  expected_display_value: 25_000,
  expected_tier: "Alto",
  expected_tier_label: "Grande público",
  final_display_value: 31_500,
  final_tier: "MuitoAlto",
  final_tier_label: "Evento de destaque",
  delta_display_value: 6_500,
  headline_strength: "Forte",
  headline_strength_label: "Manchete forte",
};

describe("RaceResultViewV2 — quebra de peça no debrief", () => {
  beforeEach(() => {
    invoke.mockReset();
    mockState = {
      careerId: "career-1",
      lastRaceId: "R007",
      language: "pt-BR",
      season: { ano: 2026 },
      playerTeam: { id: "team-drv-1", nome: "Equipe Aurora", categoria: "gt3" },
    };
  });

  /// O marcador de quebra da linha de um piloto, achado pelo tooltip (que é único por piloto).
  const marcador = (trechoDoTooltip) => screen.getAllByTitle(trechoDoTooltip)[0];

  it("põe o custo da quebra na linha do piloto, não num painel à parte", async () => {
    mockInvoke({ breakdowns: BREAKDOWNS });
    renderView();

    // Quem perdeu tempo mostra os segundos ao lado da chave inglesa.
    await waitFor(() => expect(marcador(/Câmbio/)).toBeInTheDocument());
    expect(marcador(/Câmbio/)).toHaveTextContent("+17s");
    // Quem abandonou não tem tempo de box — o rótulo diz DNF.
    expect(marcador(/Motor/)).toHaveTextContent("DNF");

    // Nada de painel-resumo repetindo a mesma informação longe da linha.
    expect(screen.queryByText("Problemas de peça")).not.toBeInTheDocument();
  });

  it("guarda peça, volta e problema no tooltip, com a frase começando em maiúscula", async () => {
    mockInvoke({ breakdowns: BREAKDOWNS });
    renderView();

    await waitFor(() => expect(marcador(/Câmbio/)).toBeInTheDocument());
    const tooltip = marcador(/Câmbio/).getAttribute("title");
    expect(tooltip).toContain("V12 · Câmbio");
    // O backend manda em minúscula (a frase entra no meio da notícia); a tela sobe a inicial.
    expect(tooltip).toContain("Câmbio perdeu a 3ª marcha");
    expect(tooltip).toContain("(+17s)");
  });

  it("põe o tempo perdido no box na régua de métricas do jogador", async () => {
    mockInvoke({ breakdowns: BREAKDOWNS });
    renderView();

    await waitFor(() => expect(screen.getByText("perdido no box")).toBeInTheDocument());
    // Escopado à célula da régua: os mesmos "+17s" também aparecem na linha da tabela.
    expect(screen.getByText("perdido no box").parentElement).toHaveTextContent("+17s");
  });

  it("explica o abandono no tooltip do badge DNF, também em maiúscula", async () => {
    mockInvoke({ breakdowns: BREAKDOWNS });
    renderView();

    // Título EXATO: o badge da posição, não o marcador de quebra (que também diz "DNF" e
    // carrega o tooltip longo com peça e volta).
    await waitFor(() =>
      expect(screen.getByTitle("Motor fundiu por superaquecimento")).toHaveTextContent("DNF"),
    );
  });

  it("os dados fake de DEV acendem a UI de quebra casando com o grid real", async () => {
    // O ponto do fake é conferir a UI numa corrida em que ninguém quebrou. Ele precisa se
    // ancorar nos pilot_id REAIS da tela — com nomes inventados, os chips apareceriam mas o
    // 🔧 da tabela e a métrica do jogador não, que é justamente o que se quer ver.
    const fake = buildMockBreakdowns(RESULT.race_results);

    const doJogador = fake.filter((b) => b.is_player);
    expect(doJogador.length).toBeGreaterThan(1); // exercita o tooltip de várias linhas
    expect(doJogador.every((b) => b.driver_id === "drv-1")).toBe(true);

    // Todo id fake tem que existir no resultado, senão o marcador da tabela não acende.
    const idsReais = new Set(RESULT.race_results.map((r) => r.pilot_id));
    expect(fake.every((b) => idsReais.has(b.driver_id))).toBe(true);

    // Cobre os três estados visuais: abandono, penalidade pesada e leve.
    expect(new Set(fake.map((b) => b.severity))).toEqual(
      new Set(["dnf", "heavy", "light"]),
    );
    // O abandono fake vai pra quem de fato abandonou na corrida.
    expect(fake.find((b) => b.severity === "dnf").driver_id).toBe("drv-2");
    // Determinístico: mesma entrada, mesma saída (a tela não pode piscar entre renders).
    expect(buildMockBreakdowns(RESULT.race_results)).toEqual(fake);
  });

  it("o fake não quebra num grid sem jogador e sem abandono", async () => {
    const semNada = [entry("drv-9", "A. Souza", "Equipe X", 1)];
    const fake = buildMockBreakdowns(semNada);
    // Sem jogador e sem DNF, esses papéis somem em vez de estourar.
    expect(fake.every((b) => b.driver_id === "drv-9")).toBe(true);
    expect(buildMockBreakdowns([])).toEqual([]);
    expect(buildMockBreakdowns(undefined)).toEqual([]);
  });

  it("sem quebra nenhuma, não inventa painel nem métrica", async () => {
    mockInvoke({ breakdowns: [] });
    renderView();

    // A régua já renderizou (prova que a tela montou), e nada de quebra aparece.
    await waitFor(() => expect(screen.getByText("incidentes")).toBeInTheDocument());
    expect(screen.queryByText("perdido no box")).not.toBeInTheDocument();
    expect(screen.queryAllByTitle(/V\d+ ·/)).toHaveLength(0);
  });
});

describe("RaceResultViewV2 — repercussão do evento", () => {
  beforeEach(() => {
    invoke.mockReset();
    mockState = {
      careerId: "career-1",
      lastRaceId: "R007",
      language: "pt-BR",
      season: { ano: 2026 },
      playerTeam: { id: "team-drv-1", nome: "Equipe Aurora", categoria: "gt3" },
    };
  });

  /// O segmento do cabeçalho, achado pelo rótulo da faixa de contexto.
  const segmento = () => screen.getByText("repercussão").parentElement;

  it("fica na faixa de contexto do cabeçalho, ao lado de voltas e temporada", async () => {
    mockInvoke({ breakdowns: [] });
    renderView({ repercussion: REPERCUSSION });

    await waitFor(() => expect(screen.getByText("repercussão")).toBeInTheDocument());
    // Mesmo bloco de "Voltas"/"Temporada" — é contexto do EVENTO, não métrica do piloto.
    // Ancorado no nome da pista (único na tela) → sobe até a faixa de contexto.
    const faixa = screen.getByText("Interlagos").parentElement.parentElement;
    expect(faixa).toContainElement(screen.getByText("repercussão"));
    // E não sobrou nada na régua do debrief, onde ficava antes.
    expect(screen.getByText("incidentes").parentElement.parentElement).not.toContainElement(
      screen.getByText("repercussão"),
    );
  });

  it("mostra o tier alcançado e o saldo contra o esperado", async () => {
    mockInvoke({ breakdowns: [] });
    renderView({ repercussion: REPERCUSSION });

    await waitFor(() => expect(screen.getByText("repercussão")).toBeInTheDocument());
    // O PÚBLICO é o número em destaque; o tier é a legenda embaixo.
    expect(segmento()).toHaveTextContent("31.500");
    // O rótulo vem PRONTO do backend — a tela exibe, não traduz.
    expect(segmento()).toHaveTextContent("Evento de destaque");
    // Entregou mais do que prometia: seta pra cima com o delta do backend.
    expect(segmento()).toHaveTextContent("▲6.500");
  });

  it("mantém o número no eixo com um espelho invisível do delta", async () => {
    mockInvoke({ breakdowns: [] });
    renderView({ repercussion: REPERCUSSION });

    await waitFor(() => expect(screen.getByText("repercussão")).toBeInTheDocument());
    // Dois nós com o mesmo texto: o visível à direita do número e o espelho à esquerda,
    // que só ocupa espaço. Sem ele o número escorrega do centro do segmento — e escorrega
    // um tanto diferente a cada corrida, porque a largura do delta varia.
    const ambos = within(segmento()).getAllByText("▲6.500");
    expect(ambos).toHaveLength(2);
    const espelho = ambos.find((n) => n.className.includes("invisible"));
    expect(espelho).toBeTruthy();
    expect(espelho).toHaveAttribute("aria-hidden", "true");
  });

  it("sem saldo contra o esperado, não sobra espelho nem seta", async () => {
    mockInvoke({ breakdowns: [] });
    renderView({
      repercussion: { ...REPERCUSSION, final_display_value: 25_000, delta_display_value: 0 },
    });

    await waitFor(() => expect(screen.getByText("repercussão")).toBeInTheDocument());
    expect(segmento()).not.toHaveTextContent("▲");
    expect(segmento()).not.toHaveTextContent("▼");
    expect(segmento()).toHaveTextContent("25.000");
  });

  it("abre o card do confronto ao passar o mouse, e não o tooltip nativo do sistema", async () => {
    mockInvoke({ breakdowns: [] });
    renderView({ repercussion: REPERCUSSION });

    await waitFor(() => expect(screen.getByText("repercussão")).toBeInTheDocument());
    const gatilho = screen.getByTestId("repercussion-segment");
    // Nada de `title`: o tooltip nativo do SO não tem a linguagem visual do app.
    expect(gatilho).not.toHaveAttribute("title");

    const card = screen.getByTestId("repercussion-card");
    expect(card).toHaveClass("opacity-0");
    fireEvent.mouseEnter(gatilho);
    expect(screen.getByTestId("repercussion-card")).toHaveClass("opacity-100");

    // O confronto completo mora no card.
    expect(card).toHaveTextContent("Grande público");
    expect(card).toHaveTextContent("25.000 espectadores");
    expect(card).toHaveTextContent("Evento de destaque");
    expect(card).toHaveTextContent("31.500 espectadores");
    expect(card).toHaveTextContent("+6.500");
    expect(card).toHaveTextContent("Manchete forte");

    fireEvent.mouseLeave(gatilho);
    expect(screen.getByTestId("repercussion-card")).toHaveClass("opacity-0");
  });

  it("o card fica FORA da faixa recortada, senão seria cortado pelo overflow", async () => {
    mockInvoke({ breakdowns: [] });
    renderView({ repercussion: REPERCUSSION });

    await waitFor(() => expect(screen.getByText("repercussão")).toBeInTheDocument());
    const faixa = screen.getByText("Interlagos").parentElement.parentElement;
    expect(faixa.className).toContain("overflow-hidden");
    expect(faixa).not.toContainElement(screen.getByTestId("repercussion-card"));
  });

  it("aponta pra baixo quando a corrida entrega menos do que prometia", async () => {
    mockInvoke({ breakdowns: [] });
    renderView({
      repercussion: {
        ...REPERCUSSION,
        final_display_value: 18_500,
        final_tier: "Moderado",
        final_tier_label: "Interesse moderado",
        delta_display_value: -6_500,
      },
    });

    await waitFor(() => expect(screen.getByText("repercussão")).toBeInTheDocument());
    expect(segmento()).toHaveTextContent("▼6.500");
    expect(screen.getByTestId("repercussion-card")).toHaveTextContent("-6.500");
  });

  it("sem repercussão no payload (save antigo), o segmento some em vez de mostrar zero", async () => {
    mockInvoke({ breakdowns: [] });
    renderView();

    await waitFor(() => expect(screen.getByText("incidentes")).toBeInTheDocument());
    expect(screen.queryByText("repercussão")).not.toBeInTheDocument();
  });
});

/// Fatura do fim de semana: os itens vêm do backend já agrupados em carro / logística /
/// equipe / reparo, e a cor do total é informação — âmbar SÓ quando houve conserto.
const FATURA = {
  total: 11_100,
  repair_total: 0,
  items: [
    { key: "gasolina", label: "Gasolina", cost: 1_800, group: "carro" },
    { key: "pneus", label: "Pneus", cost: 1_900, group: "carro" },
    { key: "frete", label: "Frete do carro", cost: 3_200, group: "logistica" },
    { key: "inscricao", label: "Inscrição na etapa", cost: 1_200, group: "logistica" },
    { key: "diarias", label: "Diárias da equipe", cost: 1_500, group: "equipe" },
    { key: "estrutura", label: "Estrutura móvel", cost: 1_500, group: "equipe" },
  ],
};

describe("RaceResultViewV2 — fatura do fim de semana", () => {
  beforeEach(() => {
    invoke.mockReset();
    mockState = {
      careerId: "career-1",
      lastRaceId: "R007",
      language: "pt-BR",
      season: { ano: 2026 },
    };
  });

  it("agrupa a fatura em blocos em vez de despejar todos os custos numa lista só", async () => {
    mockInvoke({ breakdowns: [] });
    renderView({ maintenance: FATURA });

    await waitFor(() => expect(screen.getByText("custos da corrida")).toBeInTheDocument());
    for (const bloco of ["Carro", "Logística", "Equipe"]) {
      expect(screen.getByText(bloco)).toBeInTheDocument();
    }
    // O bloco de reparo só existe quando bateu.
    expect(screen.queryByText("Reparo")).not.toBeInTheDocument();
    expect(screen.getByText("Frete do carro")).toBeInTheDocument();
  });

  it("fim de semana limpo não pinta o total de alerta; com conserto, pinta", async () => {
    mockInvoke({ breakdowns: [] });
    const { unmount } = renderView({ maintenance: FATURA });
    await waitFor(() => expect(screen.getByText("custos da corrida")).toBeInTheDocument());
    const limpo = screen.getByText("custos da corrida").nextElementSibling;
    expect(limpo).toHaveStyle({ color: "#c9d1d9" });
    unmount();

    renderView({
      maintenance: {
        ...FATURA,
        repair_total: 4_000,
        items: [
          ...FATURA.items,
          { key: "carroceria", label: "Carroceria", cost: 4_000, group: "reparo" },
        ],
      },
    });
    await waitFor(() => expect(screen.getByText("custos da corrida")).toBeInTheDocument());
    expect(screen.getByText("custos da corrida").nextElementSibling).toHaveStyle({
      color: "#e0a458",
    });
    expect(screen.getByText("Reparo")).toBeInTheDocument();
  });
});
