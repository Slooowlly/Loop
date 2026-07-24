import { render, screen, waitFor } from "@testing-library/react";
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

function renderView() {
  return render(
    <RaceResultViewV2
      result={RESULT}
      evaluation={EVALUATION}
      telemetry={null}
      maintenance={{ total: 1200, items: [] }}
      onDismiss={vi.fn()}
    />,
  );
}

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
