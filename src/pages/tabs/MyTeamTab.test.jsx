import { fireEvent, render, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import MyTeamTab from "./MyTeamTab";

let mockState = {};
let mockFinanceReport = null;

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

// Report financeiro REAL simulado (backend `get_team_finance_report`). Últimas rodadas
// com as 9 linhas reais + acumulado da temporada + timeline de caixa.
function buildFinanceReport(overrides = {}) {
  return {
    rounds_recorded: 6,
    latest: {
      season_number: 1,
      round: 6,
      sponsorship_income: 200_000,
      gate_income: 45_000,
      result_bonus: 90_000,
      partial_prize_income: 60_000,
      aid_income: 0,
      salary_expense: 120_000,
      event_operations_cost: 70_000,
      structural_maintenance_cost: 40_000,
      technical_investment_cost: 25_000,
      debt_service_cost: 10_000,
      income_total: 350_000,
      expenses_total: 265_000,
      net: 85_000,
    },
    season: {
      season_number: 1,
      round: 6,
      sponsorship_income: 1_200_000,
      result_bonus: 540_000,
      partial_prize_income: 360_000,
      aid_income: 0,
      salary_expense: 720_000,
      event_operations_cost: 420_000,
      structural_maintenance_cost: 240_000,
      technical_investment_cost: 150_000,
      debt_service_cost: 60_000,
      income_total: 2_100_000,
      expenses_total: 1_590_000,
      net: 510_000,
    },
    cash_timeline: [
      { season_number: 1, round: 1, cash_balance: 6_000_000, net: 80_000 },
      { season_number: 1, round: 2, cash_balance: 6_120_000, net: 120_000 },
      { season_number: 1, round: 3, cash_balance: 6_240_000, net: 120_000 },
      { season_number: 1, round: 4, cash_balance: 6_330_000, net: 90_000 },
      { season_number: 1, round: 5, cash_balance: 6_415_000, net: 85_000 },
      { season_number: 1, round: 6, cash_balance: 6_500_000, net: 85_000 },
    ],
    expected_constructor_prize: 0,
    current_position: 0,
    grid_size: 0,
    ...overrides,
  };
}

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

function buildHistoryDossier(teamId = "T010") {
  const isVector = teamId === "T020";
  return {
    team_id: teamId,
    category: "gt4",
    record_scope: "Grupo GT4",
    has_history: true,
    records: [
      { label: "Títulos", rank: isVector ? "2º" : "1º", value: isVector ? "0" : "1" },
      { label: "Vitórias", rank: isVector ? "1º" : "2º", value: isVector ? "9" : "7" },
      { label: "Pódios", rank: isVector ? "1º" : "2º", value: isVector ? "16" : "12" },
      { label: "Taxa de pódio", rank: isVector ? "1º" : "2º", value: isVector ? "80%" : "75%" },
      { label: "Taxa de vitória", rank: isVector ? "1º" : "2º", value: isVector ? "45%" : "25%" },
    ],
    sport: {
      seasons: isVector ? "3 Temporadas reais" : "2 Temporadas reais",
      current_streak: isVector ? "3 Temporadas seguidas no Grupo GT4" : "2 Temporadas seguidas no Grupo GT4",
      best_streak: isVector ? "4 Pódios consecutivos reais" : "3 Pódios consecutivos reais",
      podium_rate: isVector ? "80%" : "75%",
      win_rate: isVector ? "45%" : "25%",
      races: isVector ? 20 : 16,
      wins: isVector ? 9 : 7,
      podiums: isVector ? 16 : 12,
    },
    timeline: [
      { year: "2024", text: "Primeira corrida registrada pelo backend." },
      { year: "2025", text: "Primeira vitória real registrada pelo backend." },
    ],
    identity: {
      origin: isVector ? "GT4 Origem Vector" : "GT4 Origem Real",
      current: "GT4 Atual Real",
      profile: isVector ? "Especialista Real" : "Dominante Real",
      summary: isVector
        ? "Resumo real da Vector calculado no backend."
        : "Resumo real da Falcon calculado no backend.",
      rival: {
        name: isVector ? "Falcon Motorsport" : "Vector Racing",
        current_category: "GT4 Atual Real",
        note: isVector
          ? "20 disputas diretas reais contra Falcon Motorsport."
          : "16 disputas diretas reais contra Vector Racing.",
      },
      symbol_driver: isVector ? "Piloto Símbolo Vector" : "Piloto Símbolo Real",
      symbol_driver_detail: isVector
        ? "20 corridas, 9 vitórias, 16 pódios pela equipe."
        : "16 corridas, 7 vitórias, 12 pódios pela equipe.",
    },
    management: {
      operation_health: isVector ? "Saudável real" : "Pressionada real",
      peak_cash: isVector ? "$8,800,000" : "$9,900,000",
      worst_crisis: isVector ? "Sem dívida real registrada" : "$1,200,000 de dívida real",
      healthy_years: isVector ? "3 Temporadas" : "4 Temporadas",
      efficiency: isVector ? "18,4 pts/R$ mi real" : "22,1 pts/R$ mi real",
      biggest_investment: isVector ? "Nível 8 - pacote real" : "Nível 9 - pacote real",
      summary: isVector
        ? "Gestão real da Vector calculada no backend."
        : "Gestão real da Falcon calculada no backend.",
      peak_cash_detail: isVector
        ? "Pico real da Vector vindo do backend."
        : "Pico real da Falcon vindo do backend.",
      worst_crisis_detail: isVector
        ? "Crise real da Vector vinda do backend."
        : "Crise real da Falcon vinda do backend.",
      healthy_years_detail: isVector
        ? "Temporadas saudáveis reais da Vector."
        : "Temporadas saudáveis reais da Falcon.",
      efficiency_detail: isVector
        ? "Eficiência real da Vector."
        : "Eficiência real da Falcon.",
      investment_detail: isVector
        ? "Investimento real da Vector."
        : "Investimento real da Falcon.",
    },
    title_categories: isVector
      ? []
      : [{ category: "GT4", year: "2025", color: "#f2c46d" }],
    category_path: [
      {
        category: "GT4",
        years: isVector ? "2023-2026" : "2024-2026",
        detail: "Resultados reais registrados nesse recorte.",
        color: "#58a6ff",
      },
    ],
  };
}

describe("MyTeamTab", () => {
  beforeEach(() => {
    invoke.mockReset();
    mockFinanceReport = buildFinanceReport();
    invoke.mockImplementation((command, args = {}) => {
      if (command === "get_drivers_by_category") {
        return Promise.resolve([
          {
            id: "P001",
            nome: "Piloto Jogador",
            nacionalidade: "Brasil",
            skill: 82,
            salario_anual: 250_000,
          },
          {
            id: "P002",
            nome: "Colega IA",
            nacionalidade: "Portugal",
            skill: 76,
            salario_anual: 170_000,
          },
        ]);
      }

      if (command === "get_teams_standings") {
        return Promise.resolve([
          {
            posicao: 1,
            id: "T010",
            nome: "Falcon Motorsport",
            nome_curto: "FAL",
            cor_primaria: "#facc15",
            cash_balance: 132_565_957,
            car_performance: 9,
            car_level: 9,
            car_build_profile: "power_intermediate",
            confiabilidade: 92,
            pit_crew_quality: 84,
            founded_year: 2002,
            pontos: 188,
          },
          {
            posicao: 5,
            id: "T001",
            nome: "Aurora GT",
            nome_curto: "AUR",
            cor_primaria: "#58a6ff",
            cash_balance: 6_500_000,
            car_performance: 7,
            car_level: 7,
            car_build_profile: "balanced",
            confiabilidade: 72,
            pit_crew_quality: 68,
            pontos: 96,
          },
          {
            posicao: 2,
            id: "T020",
            nome: "Vector Racing",
            nome_curto: "VEC",
            cor_primaria: "#22c55e",
            cash_balance: 1_000_000,
            car_performance: 10,
            car_level: 10,
            car_build_profile: "handling_intermediate",
            confiabilidade: 38,
            pit_crew_quality: 18,
            pontos: 120,
          },
        ]);
      }

      if (command === "get_team_history_dossier") {
        return Promise.resolve(buildHistoryDossier(args.teamId));
      }

      if (command === "get_team_finance_report") {
        return Promise.resolve(mockFinanceReport);
      }

      return Promise.resolve([]);
    });
    mockState = {
      careerId: "career-1",
      player: { id: "P001" },
      playerTeam: {
        id: "T001",
        nome: "Aurora GT",
        nome_curto: "AUR",
        cor_primaria: "#58a6ff",
        cor_secundaria: "#0d1117",
        categoria: "gt4",
        car_performance: 8,
        car_level: 8,
        car_build_profile: "balanced",
        confiabilidade: 72,
        pit_strategy_risk: 42,
        pit_crew_quality: 68,
        budget: 72,
        cash_balance: 6_500_000,
        debt_balance: 1_250_000,
        spending_power: 2_800_000,
        salary_ceiling: 420_000,
        budget_index: 72,
        financial_state: "healthy",
        season_strategy: "balanced",
        last_round_income: 380_000,
        last_round_expenses: 255_000,
        last_round_net: 125_000,
        parachute_payment_remaining: 0,
        piloto_1_id: "P001",
        piloto_1_nome: "Piloto Jogador",
        piloto_1_salario_anual: 250_000,
        piloto_2_id: "P002",
        piloto_2_nome: "Colega IA",
        piloto_2_salario_anual: 170_000,
      },
    };
  });

  it("shows real money finance readouts instead of the legacy budget bar", async () => {
    render(<MyTeamTab />);

    expect(await screen.findByText(/^Caixa$/i)).toBeInTheDocument();
    expect(screen.getByText(/Poder de gasto/i)).toBeInTheDocument();
    expect(screen.getAllByText(/Teto salarial/i).length).toBeGreaterThan(0);
    expect(screen.queryByText(/^Budget$/i)).not.toBeInTheDocument();
  });

  it("shows cash balance instead of points in the command header", async () => {
    render(<MyTeamTab />);

    const header = await screen.findByTestId("my-team-command-header");
    const financeStat = within(header).getByTestId("header-finance-stat");
    expect(within(financeStat).getByText("$6,500,000")).toBeInTheDocument();
    expect(within(financeStat).getByText(/Saudável/i)).toBeInTheDocument();
    expect(within(financeStat).getByText(/Posição/i)).toBeInTheDocument();
    expect(within(financeStat).getByText("5º")).toBeInTheDocument();
    expect(within(financeStat).queryByText(/Saldo/i)).not.toBeInTheDocument();
    expect(within(financeStat).queryByText(/Caixa disponível/i)).not.toBeInTheDocument();
    expect(financeStat).not.toHaveClass("border-accent-primary/35");
    expect(financeStat).not.toHaveClass("rounded-[24px]");
    expect(financeStat).not.toHaveClass("py-5");
    expect(within(financeStat).getByText("$6,500,000")).toHaveClass("text-5xl");
    expect(financeStat.querySelector("[data-testid='header-finance-ornament']")).not.toBeInTheDocument();
    expect(within(financeStat).queryByText(/^Estado$/i)).not.toBeInTheDocument();
    expect(within(header).queryByTestId("header-position-stat")).not.toBeInTheDocument();
    expect(within(header).queryByText(/Pontos/i)).not.toBeInTheDocument();
  });

  it("shows the team logo in the management command header", async () => {
    mockState.playerTeam.nome = "Ferrari";
    mockState.playerTeam.cor_primaria = "#dc0000";

    render(<MyTeamTab />);

    const header = await screen.findByTestId("my-team-command-header");
    const logo = within(header).getByTestId("my-team-command-logo");
    expect(within(logo).getByAltText("Ferrari logo")).toBeInTheDocument();
    expect(logo.parentElement).not.toHaveClass("rounded-2xl");
    expect(logo.parentElement).not.toHaveClass("border");
    expect(logo.parentElement).not.toHaveClass("bg-white/[0.03]");
  });

  it("colors the financial state pill according to the real team state", async () => {
    mockState.playerTeam.financial_state = "crisis";

    render(<MyTeamTab />);

    const header = await screen.findByTestId("my-team-command-header");
    const statePill = within(header).getByText(/Em crise/i);
    expect(statePill).toHaveClass("text-status-red");
    expect(statePill).not.toHaveClass("text-status-green");
  });

  it("renders the management dossier with salaries, compact technical tabs and final ranking", async () => {
    render(<MyTeamTab />);

    expect(await screen.findByText(/Dossiê financeiro/i)).toBeInTheDocument();
    expect(screen.getByText(/Entradas da rodada/i)).toBeInTheDocument();
    expect(screen.getByText(/Saídas da rodada/i)).toBeInTheDocument();
    expect(screen.getByText(/Caixa ao fim de cada rodada/i)).toBeInTheDocument();
    expect(screen.getByText(/Patrocínios/i)).toBeInTheDocument();
    // Fase 3 do Estrelato: bilheteria aparece como linha de receita própria.
    expect(screen.getByText(/Bilheteria/i)).toBeInTheDocument();
    expect(screen.getAllByText(/Salários/i).length).toBeGreaterThan(0);

    expect(screen.getByText(/Salário N1/i)).toBeInTheDocument();
    expect(screen.getByText(/Salário N2/i)).toBeInTheDocument();
    expect(screen.getByAltText("Brasil")).toBeInTheDocument();
    expect(screen.getByAltText("Portugal")).toBeInTheDocument();
    expect(screen.getAllByText(/^Peso na folha$/i)).toHaveLength(2);

    expect(screen.getByText(/Eixos técnicos/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Desenvolvimento/i })).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /Confiabilidade/i }).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: /Pit e corrida/i })).toBeInTheDocument();

    expect(await screen.findByText(/Ranking da categoria/i)).toBeInTheDocument();
    expect(screen.getByText("Falcon Motorsport")).toBeInTheDocument();
    expect(screen.getAllByText("Aurora GT").length).toBeGreaterThan(0);
  });

  it("shows team logos in the category ranking while keeping team history double click", async () => {
    invoke.mockImplementation((command, args = {}) => {
      if (command === "get_drivers_by_category") {
        return Promise.resolve([]);
      }

      if (command === "get_teams_standings") {
        return Promise.resolve([
          {
            posicao: 1,
            id: "TFER",
            nome: "Ferrari",
            nome_curto: "FER",
            cor_primaria: "#dc0000",
            cash_balance: 12_000_000,
            car_performance: 9,
            car_level: 9,
            car_build_profile: "power_intermediate",
            pontos: 144,
          },
          {
            posicao: 2,
            id: "TAMG",
            nome: "Mercedes-AMG",
            nome_curto: "AMG",
            cor_primaria: "#00d2be",
            cash_balance: 10_000_000,
            car_performance: 8,
            car_level: 8,
            car_build_profile: "balanced",
            pontos: 132,
          },
        ]);
      }

      if (command === "get_team_history_dossier") {
        return Promise.resolve(buildHistoryDossier(args.teamId));
      }

      if (command === "get_team_finance_report") {
        return Promise.resolve(mockFinanceReport);
      }

      return Promise.resolve([]);
    });

    mockState.playerTeam = {
      ...mockState.playerTeam,
      id: "TFER",
      nome: "Ferrari",
      nome_curto: "FER",
      cor_primaria: "#dc0000",
      categoria: "gt3",
    };

    render(<MyTeamTab />);

    const ranking = await screen.findByRole("table", { name: /Ranking da categoria/i });
    expect(within(ranking).getByAltText("Ferrari logo")).toBeInTheDocument();
    expect(within(ranking).getByAltText("Mercedes-AMG logo")).toBeInTheDocument();

    fireEvent.doubleClick(within(ranking).getByText("Ferrari"));

    expect(await screen.findByRole("dialog", { name: /Ferrari/i })).toBeInTheDocument();
  });

  it("shows the car level column and hides the (retired) car build type", async () => {
    render(<MyTeamTab />);

    expect(await screen.findByText("5º")).toBeInTheDocument();
    // Sistema de Nível do Carro: o jogador vê só o Nível do Carro; o "tipo/shape" foi aposentado.
    expect(screen.getAllByText(/Nível do carro/i).length).toBeGreaterThan(0);
    expect(screen.queryByText(/Tipo do carro/i)).not.toBeInTheDocument();
  });

  it("shows car, reliability and pit crew as five qualitative color tiers", async () => {
    render(<MyTeamTab />);

    const ranking = await screen.findByRole("table", { name: /Ranking da categoria/i });
    expect(within(ranking).getByRole("button", { name: /Confiabilidade/i })).toBeInTheDocument();
    expect(within(ranking).getByRole("button", { name: /Pit crew/i })).toBeInTheDocument();

    expect(within(ranking).getByTestId("ranking-car-tier-T001")).toHaveTextContent("Referência");
    expect(within(ranking).getByTestId("ranking-car-tier-T001")).toHaveStyle({ color: "#7ee787" });
    expect(within(ranking).getByTestId("ranking-reliability-tier-T001")).toHaveTextContent("Robusto");
    expect(within(ranking).getByTestId("ranking-reliability-tier-T001")).toHaveStyle({ color: "#7ee787" });
    expect(within(ranking).getByTestId("ranking-pit-crew-tier-T001")).toHaveTextContent("Forte");
    expect(within(ranking).getByTestId("ranking-pit-crew-tier-T001")).toHaveStyle({ color: "#7ee787" });

    expect(within(ranking).getByTestId("ranking-car-tier-T010")).toHaveTextContent("Dominante");
    expect(within(ranking).getByTestId("ranking-reliability-tier-T010")).toHaveTextContent("Inquebrável");
    expect(within(ranking).getByTestId("ranking-pit-crew-tier-T020")).toHaveTextContent("Muito fraco");
    expect(within(ranking).getByTestId("ranking-pit-crew-tier-T020")).toHaveStyle({ color: "#f85149" });
  });

  it("shows a cleaner development axis for the technical operation", async () => {
    render(<MyTeamTab />);

    expect(await screen.findByText(/Pacote do carro/i)).toBeInTheDocument();
    // O Nível do Carro é a ÚNICA leitura de pacote. A barra de "Desempenho na pista" lia o
    // escalar `car_performance` — hoje derivado do MESMO nível, então só repetia a de cima.
    expect(screen.queryByText(/Desempenho na pista/i)).not.toBeInTheDocument();
    // Shape do carro escondido: nada de "Foco do projeto"/"Equilíbrio do acerto".
    expect(screen.queryByText(/Foco do projeto/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Distribuição técnica/i)).not.toBeInTheDocument();
  });

  it("reads the car package from car_level only, never from the legacy scalar", async () => {
    // `car_level` ausente (save antigo/payload incompleto) NÃO pode cair no escalar legado:
    // ele tem escala própria por categoria e ignora o sistema de peças, então a UI passaria
    // a mostrar diferença de carro que a simulação não aplica (grid spec de rookie).
    delete mockState.playerTeam.car_level;
    mockState.playerTeam.car_performance = 8;

    render(<MyTeamTab />);

    expect(await screen.findByText(/Pacote do carro/i)).toBeInTheDocument();
    expect(screen.getByText("Nível 1/10")).toBeInTheDocument();
    expect(screen.queryByText("Nível 8/10")).not.toBeInTheDocument();
  });

  it("does not render generic explanatory helper copy", async () => {
    render(<MyTeamTab />);

    expect(await screen.findByText(/Dossiê financeiro/i)).toBeInTheDocument();
    expect(screen.queryByText(/Modo compacto para alternar/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Sala operacional da escuderia/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Extrato detalhado da gestão/i)).not.toBeInTheDocument();
  });

  it("uses short conditional executive reading instead of generic ranking copy", async () => {
    mockState.playerTeam.spending_power = -150_000;
    mockState.playerTeam.debt_balance = 4_000_000;

    render(<MyTeamTab />);

    expect(await screen.findByText(/Leitura executiva/i)).toBeInTheDocument();
    expect(screen.getByText(/Rodada positiva/i)).toBeInTheDocument();
    expect(screen.getByText(/Dívida alta/i)).toBeInTheDocument();
    expect(screen.getByText(/Gasto restrito/i)).toBeInTheDocument();
    expect(screen.queryByText(/O ranking no fim da tela/i)).not.toBeInTheDocument();
  });

  it("describes negative round finance as a loss in the last round", async () => {
    mockState.playerTeam.last_round_net = -10_000;

    render(<MyTeamTab />);

    expect(await screen.findByText(/Rodada negativa/i)).toBeInTheDocument();
    expect(screen.getByText(/Perda de .* na última rodada/i)).toBeInTheDocument();
    expect(screen.queryByText(/ultimo evento/i)).not.toBeInTheDocument();
  });

  it("highlights negative cash timeline bars in red", async () => {
    mockFinanceReport = buildFinanceReport({
      cash_timeline: [
        { season_number: 1, round: 1, cash_balance: 200_000, net: -100_000 },
        { season_number: 1, round: 2, cash_balance: -50_000, net: -250_000 },
        { season_number: 1, round: 3, cash_balance: 120_000, net: 170_000 },
      ],
    });

    render(<MyTeamTab />);

    const negativeBars = await screen.findAllByTestId("cash-timeline-negative");
    expect(negativeBars.length).toBeGreaterThan(0);
    expect(negativeBars[0]).toHaveClass("from-status-red");
  });

  it("projects the season close in the green when the estimated constructor prize covers the deficit", async () => {
    mockFinanceReport = buildFinanceReport({
      season: { ...buildFinanceReport().season, net: -2_100_000 },
      expected_constructor_prize: 5_200_000,
      current_position: 1,
      grid_size: 14,
    });

    render(<MyTeamTab />);

    const projection = await screen.findByTestId("season-projection");
    expect(within(projection).getByText(/Se a temporada terminasse agora/i)).toBeInTheDocument();
    // Posição atual + prêmio estimado + veredito no verde (deficit -2.1M + prêmio 5.2M = +3.1M).
    expect(within(projection).getByText(/1º de 14/i)).toBeInTheDocument();
    expect(within(projection).getByText(/Prêmio estimado/i)).toBeInTheDocument();
    expect(within(projection).getByText(/fecha a temporada no verde/i)).toBeInTheDocument();
  });

  it("renders the constructor prize as a real income line and a distinct season-close bar", async () => {
    mockFinanceReport = buildFinanceReport({
      latest: {
        ...buildFinanceReport().latest,
        constructor_prize_income: 5_200_000,
        income_total: 5_200_000,
      },
      cash_timeline: [
        { season_number: 1, round: 5, cash_balance: 6_415_000, net: 85_000, is_season_close: false },
        { season_number: 1, round: 6, cash_balance: 6_500_000, net: 85_000, is_season_close: false },
        { season_number: 1, round: 1000, cash_balance: 11_700_000, net: 5_200_000, is_season_close: true },
      ],
    });

    render(<MyTeamTab />);

    expect(await screen.findAllByText(/Prêmio de construtores/i)).not.toHaveLength(0);
    const closeBar = screen.getByTestId("cash-timeline-season-close");
    expect(closeBar).toHaveClass("from-status-yellow/70");
  });

  it("places the accumulated cost distribution in the side operations rail", async () => {
    render(<MyTeamTab />);

    const sideRail = await screen.findByTestId("my-team-side-rail");
    expect(within(sideRail).getByText(/Distribuição dos custos acumulados/i)).toBeInTheDocument();
  });

  it("colors team names in the category comparison with their team colors", async () => {
    render(<MyTeamTab />);

    const falconName = await screen.findByText("Falcon Motorsport");
    expect(falconName).toHaveStyle({ color: "#facc15" });
  });

  it("sorts the category ranking when a comparison column is clicked", async () => {
    render(<MyTeamTab />);

    const ranking = await screen.findByRole("table", { name: /Ranking da categoria/i });
    expect(within(ranking).getAllByTestId("ranking-team-name").map((cell) => cell.textContent)).toEqual([
      "Falcon Motorsport",
      "Aurora GT",
      "Vector Racing",
    ]);

    fireEvent.click(screen.getByRole("button", { name: /Nível do carro/i }));

    expect(within(ranking).getAllByTestId("ranking-team-name").map((cell) => cell.textContent)).toEqual([
      "Vector Racing",
      "Falcon Motorsport",
      "Aurora GT",
    ]);
  });

  it("expands cash projection with strategy, debt and round flow details", async () => {
    mockState.playerTeam.parachute_payment_remaining = 500_000;

    render(<MyTeamTab />);

    expect(await screen.findByText(/Histórico de caixa/i)).toBeInTheDocument();
    expect(screen.getByText(/Estratégia da temporada/i)).toBeInTheDocument();
    expect(screen.getByText("Equilíbrio")).toBeInTheDocument();
    expect(screen.getAllByText(/^Dívida$/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/Caixa antes da rodada/i)).toBeInTheDocument();
    expect(screen.getByText(/Caixa atual/i)).toBeInTheDocument();
    expect(screen.getByText(/Auxílio de rebaixamento restante/i)).toBeInTheDocument();
  });

  it("keeps secondary cash projection indicators collapsed until requested", async () => {
    render(<MyTeamTab />);

    expect(await screen.findByText(/Histórico de caixa/i)).toBeInTheDocument();
    expect(screen.queryByText(/Pico de caixa/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Pior trecho/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Média por rodada/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Folha anual/i)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Ver indicadores secundários/i }));

    expect(screen.getByText(/Pico de caixa/i)).toBeInTheDocument();
    expect(screen.getByText(/Pior trecho/i)).toBeInTheDocument();
    expect(screen.getByText(/Média por rodada/i)).toBeInTheDocument();
    expect(screen.getByText(/Folha anual/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Ocultar indicadores secundários/i }));

    expect(screen.queryByText(/Pico de caixa/i)).not.toBeInTheDocument();
  });

  it("shows a financial risk panel in the cash projection", async () => {
    render(<MyTeamTab />);

    expect(await screen.findByText(/Histórico de caixa/i)).toBeInTheDocument();
    expect(screen.queryByText(/Painel de risco financeiro/i)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Ver indicadores secundários/i }));

    expect(screen.getByText(/Painel de risco financeiro/i)).toBeInTheDocument();
    expect(screen.getByText(/Saldo líquido/i)).toBeInTheDocument();
    expect(screen.getByText(/Margem da rodada/i)).toBeInTheDocument();
    expect(screen.getByText(/Fôlego operacional/i)).toBeInTheDocument();
  });

  it("opens a compact team history drawer directly from the category ranking", async () => {
    render(<MyTeamTab />);

    const ranking = await screen.findByRole("table", { name: /Ranking da categoria/i });
    fireEvent.doubleClick(within(ranking).getByText("Falcon Motorsport"));

    const drawer = await screen.findByRole("dialog", { name: /Falcon Motorsport/i });
    const teamLogo = within(drawer).getByTestId("team-history-logo");
    const teamTitle = within(drawer).getByRole("heading", { name: /Falcon Motorsport/i });

    expect(teamLogo.compareDocumentPosition(teamTitle) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(teamLogo).toHaveClass("h-28", "w-[168px]");
    expect(within(drawer).queryByText(/Arquivo compacto/i)).not.toBeInTheDocument();
    expect(within(drawer).queryByText("GT4 Series")).not.toBeInTheDocument();
    expect(within(drawer).queryByText("Estável")).not.toBeInTheDocument();
    expect(within(drawer).queryByText("Operação moderna")).not.toBeInTheDocument();
    expect(within(drawer).getByText("Projeto consolidado")).toBeInTheDocument();
    expect(within(drawer).getByText("Fundada em 2002")).toBeInTheDocument();
    expect(within(ranking).getByText("Falcon Motorsport").closest("tr")).toHaveClass("ring-1");
    expect(drawer.closest("[data-testid='team-history-layer']")).toHaveClass("z-[90]");
    expect(drawer).toHaveClass("w-[min(50vw,720px)]");
    expect(drawer).toHaveClass("right-0");
    expect(drawer).toHaveClass("border-l");
    expect(drawer).not.toHaveClass("left-0");
    expect(drawer).toHaveClass("bg-[#07101d]");
    expect(screen.getByLabelText(/Fechar histórico da equipe/i)).toHaveClass("bg-black/70");
    expect(within(drawer).getByRole("tab", { name: /Records/i })).toBeInTheDocument();
    expect(within(drawer).getByRole("tab", { name: /Esportivo/i })).toBeInTheDocument();
    expect(within(drawer).getByRole("tab", { name: /Identidade/i })).toBeInTheDocument();
    expect(within(drawer).getByRole("tab", { name: /Gestão/i })).toBeInTheDocument();
    expect(within(drawer).getByRole("tab", { name: /Categorias/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Equipe anterior/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Próxima equipe/i })).toBeEnabled();
    expect(within(drawer).getByText(/Comparativo em/i)).toBeInTheDocument();
    expect(within(drawer).getByText(/Records históricos/i)).toBeInTheDocument();
    expect(within(drawer).getByText(/Taxa de pódio/i)).toBeInTheDocument();
    expect(within(drawer).queryByText(/Taxa de podio/i)).not.toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("get_team_history_dossier", {
      careerId: "career-1",
      teamId: "T010",
      category: "gt4",
    });

    fireEvent.click(within(drawer).getByRole("tab", { name: /Gestão/i }));

    const pressuredHealth = within(drawer).getByText("Pressionada real");
    expect(pressuredHealth).toHaveClass("text-status-red");
    expect(pressuredHealth.closest("div")).toHaveClass("border-status-red/30");
    expect(within(drawer).queryByText("22,1 pts/R$ mi real")).not.toBeInTheDocument();
    expect(within(drawer).queryByText("Eficiência real da Falcon.")).not.toBeInTheDocument();

    fireEvent.click(within(drawer).getByRole("tab", { name: /Esportivo/i }));

    expect(await within(drawer).findByText(/2 Temporadas reais/)).toBeInTheDocument();
    expect(within(drawer).getByText(/3 Pódios consecutivos reais/)).toBeInTheDocument();
    expect(within(drawer).getByText("75%")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Próxima equipe/i }));

    expect(await screen.findByRole("dialog", { name: /Vector Racing/i })).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("get_team_history_dossier", {
      careerId: "career-1",
      teamId: "T020",
      category: "gt4",
    });
    expect(within(ranking).getByText("Falcon Motorsport").closest("tr")).not.toHaveClass("ring-1");
    expect(within(ranking).getByText("Vector Racing").closest("tr")).toHaveClass("ring-1");
    expect(screen.getByRole("button", { name: /Equipe anterior/i })).toBeEnabled();

    fireEvent.click(within(drawer).getByRole("tab", { name: /Identidade/i }));

    expect(within(drawer).getByText(/Perfil histórico/i)).toBeInTheDocument();
    expect(within(drawer).getByText("Especialista Real")).toBeInTheDocument();
    expect(within(drawer).getByText("Resumo real da Vector calculado no backend.")).toBeInTheDocument();
    expect(within(drawer).getByText("GT4 Origem Vector")).toBeInTheDocument();
    expect(within(drawer).getByText("GT4 Atual Real")).toBeInTheDocument();
    expect(within(drawer).getByText(/Categoria de origem/i)).toBeInTheDocument();
    expect(within(drawer).getByText(/Maior rival histórico/i)).toBeInTheDocument();
    expect(within(drawer).getByText("Falcon Motorsport")).toBeInTheDocument();
    expect(within(drawer).getByText(/20 disputas diretas reais contra Falcon Motorsport/i)).toBeInTheDocument();
    expect(within(drawer).getByText("Piloto símbolo", { selector: "span" })).toBeInTheDocument();
    expect(within(drawer).getByText("Piloto Símbolo Vector")).toBeInTheDocument();
    expect(within(drawer).getByText(/20 corridas, 9 vitórias, 16 pódios pela equipe/i)).toBeInTheDocument();

    fireEvent.click(within(drawer).getByRole("tab", { name: /Gestão/i }));

    expect(within(drawer).getByText(/Saúde da operação/i)).toBeInTheDocument();
    expect(within(drawer).getByText("Saudável real")).toBeInTheDocument();
    expect(within(drawer).queryByText("18,4 pts/R$ mi real")).not.toBeInTheDocument();
    expect(within(drawer).getByText("Gestão real da Vector calculada no backend.")).toBeInTheDocument();
    expect(within(drawer).getByText(/Maior saldo histórico/i)).toBeInTheDocument();
    expect(within(drawer).getAllByText("$8,800,000")).not.toHaveLength(0);
    expect(within(drawer).getByText("Pico real da Vector vindo do backend.")).toBeInTheDocument();
    expect(within(drawer).getByText(/Pior crise financeira/i)).toBeInTheDocument();
    expect(within(drawer).getByText("Sem dívida real registrada")).toBeInTheDocument();
    expect(within(drawer).getByText("Crise real da Vector vinda do backend.")).toBeInTheDocument();
    expect(within(drawer).getByText("3 Temporadas")).toBeInTheDocument();
    expect(within(drawer).queryByText("Eficiência real da Vector.")).not.toBeInTheDocument();
    expect(within(drawer).getByText(/Maior investimento técnico/i)).toBeInTheDocument();
    expect(within(drawer).getByText("Nível 8 - pacote real")).toBeInTheDocument();
    expect(within(drawer).getByText("Investimento real da Vector.")).toBeInTheDocument();
  });

  it("uses real GT3 heritage dates instead of generated founding years", async () => {
    invoke.mockImplementation((command, args = {}) => {
      if (command === "get_drivers_by_category") {
        return Promise.resolve([]);
      }

      if (command === "get_teams_standings") {
        return Promise.resolve([
          {
            posicao: 1,
            id: "TFER",
            nome: "Ferrari",
            nome_curto: "FER",
            cor_primaria: "#dc0000",
            cash_balance: 42_000_000,
            car_performance: 10,
            car_level: 10,
            car_build_profile: "power_extreme",
            founded_year: 1929,
            pontos: 240,
          },
          {
            posicao: 12,
            id: "TOBS",
            nome: "Obsidian",
            nome_curto: "OBS",
            cor_primaria: "#3f3f46",
            cash_balance: 800_000,
            car_performance: 5,
            car_level: 5,
            car_build_profile: "balanced",
            pontos: 14,
          },
        ]);
      }

      if (command === "get_team_history_dossier") {
        return Promise.resolve({
          ...buildHistoryDossier(args.teamId),
          title_categories: [{ category: "GT3", year: "2003", color: "#dc0000" }],
        });
      }

      if (command === "get_team_finance_report") {
        return Promise.resolve(mockFinanceReport);
      }

      return Promise.resolve([]);
    });

    mockState.playerTeam = {
      ...mockState.playerTeam,
      id: "TFER",
      nome: "Ferrari",
      nome_curto: "FER",
      cor_primaria: "#dc0000",
      categoria: "gt3",
    };

    render(<MyTeamTab />);

    const ranking = await screen.findByRole("table", { name: /Ranking da categoria/i });
    fireEvent.doubleClick(within(ranking).getByText("Ferrari"));

    const drawer = await screen.findByRole("dialog", { name: /Ferrari/i });
    expect(within(drawer).getByText("Equipe histórica")).toBeInTheDocument();
    expect(within(drawer).getByText("Fundada em 1929")).toBeInTheDocument();
    expect(within(drawer).queryByText("GT3 Series")).not.toBeInTheDocument();
  });

  // ── Clima da garagem (módulo `hierarchy` do backend) ──────────────────────────

  it("hides the garage climate panel when the payload has no hierarchy data", async () => {
    render(<MyTeamTab />);

    expect(await screen.findByText(/Dossiê financeiro/i)).toBeInTheDocument();
    expect(screen.queryByTestId("garage-panel")).not.toBeInTheDocument();
  });

  it("shows the garage climate with the real tension reading", async () => {
    mockState.playerTeam.hierarquia_n1_id = "P001";
    mockState.playerTeam.hierarquia_n2_id = "P002";
    mockState.playerTeam.hierarquia_status = "competitivo";
    mockState.playerTeam.hierarquia_tensao = 32;

    render(<MyTeamTab />);

    const panel = await screen.findByTestId("garage-panel");
    expect(within(panel).getByText(/Clima da garagem/i)).toBeInTheDocument();
    expect(within(panel).getByTestId("garage-climate-label")).toHaveTextContent("Competitivo");
    expect(within(panel).getByTestId("garage-tension-value")).toHaveTextContent("32");
    expect(within(panel).getByTestId("garage-tension-bar")).toHaveClass("bg-status-yellow");
    // Só acima de 50 a tensão pune a moral — 32 ainda não é alerta.
    expect(within(panel).queryByTestId("garage-morale-warning")).not.toBeInTheDocument();
  });

  it("warns when tension is high enough to hurt team morale", async () => {
    mockState.playerTeam.hierarquia_n1_id = "P001";
    mockState.playerTeam.hierarquia_n2_id = "P002";
    mockState.playerTeam.hierarquia_status = "crise";
    mockState.playerTeam.hierarquia_tensao = 92;

    render(<MyTeamTab />);

    const panel = await screen.findByTestId("garage-panel");
    expect(within(panel).getByTestId("garage-climate-label")).toHaveTextContent("Crise");
    expect(within(panel).getByTestId("garage-tension-bar")).toHaveClass("bg-status-red");
    expect(within(panel).getByTestId("garage-morale-warning")).toBeInTheDocument();
  });

  // REGRESSÃO: a inversão troca a hierarquia sem mexer nos assentos. Se a UI ler
  // `piloto_1_id`, ela mostra o N1 errado justo depois do evento mais dramático do módulo.
  it("reads the driver pair from the hierarchy, not from the seat order", async () => {
    mockState.playerTeam.hierarquia_n1_id = "P002"; // o antigo N2 venceu a política interna
    mockState.playerTeam.hierarquia_n2_id = "P001";
    mockState.playerTeam.hierarquia_status = "inversao";
    mockState.playerTeam.hierarquia_tensao = 80;
    mockState.playerTeam.hierarquia_inversoes_temporada = 1;

    render(<MyTeamTab />);

    const panel = await screen.findByTestId("garage-panel");
    expect(within(panel).getByTestId("garage-inverted-note")).toBeInTheDocument();
    expect(within(panel).getByTestId("garage-inversions")).toHaveTextContent("1 inversão nesta temporada");

    // O salário segue o assento certo: o N1 agora é o P002, que ganha 170k.
    const n1Salary = screen.getByText(/Salário N1/i).parentElement;
    expect(within(n1Salary).getByText("$170,000")).toBeInTheDocument();
  });

  it("uses recent founding years for rookie teams", async () => {
    invoke.mockImplementation((command, args = {}) => {
      if (command === "get_drivers_by_category") {
        return Promise.resolve([]);
      }

      if (command === "get_teams_standings") {
        return Promise.resolve([
          {
            posicao: 1,
            id: "TRKA",
            nome: "Nova Rookie",
            nome_curto: "NVR",
            cor_primaria: "#38bdf8",
            cash_balance: 1_100_000,
            car_performance: 5,
            car_level: 5,
            car_build_profile: "balanced",
            founded_year: 2020,
            pontos: 0,
          },
          {
            posicao: 6,
            id: "TRKB",
            nome: "Startup Cup",
            nome_curto: "STC",
            cor_primaria: "#fb7185",
            cash_balance: 850_000,
            car_performance: 4,
            car_level: 4,
            car_build_profile: "balanced",
            pontos: 0,
          },
        ]);
      }

      if (command === "get_team_history_dossier") {
        return Promise.resolve({
          ...buildHistoryDossier(args.teamId),
          category: "mazda_rookie",
          record_scope: "Mazda Rookie",
          title_categories: [],
        });
      }

      if (command === "get_team_finance_report") {
        return Promise.resolve(mockFinanceReport);
      }

      return Promise.resolve([]);
    });

    mockState.playerTeam = {
      ...mockState.playerTeam,
      id: "TRKA",
      nome: "Nova Rookie",
      nome_curto: "NVR",
      cor_primaria: "#38bdf8",
      categoria: "mazda_rookie",
    };

    render(<MyTeamTab />);

    const ranking = await screen.findByRole("table", { name: /Ranking da categoria/i });
    fireEvent.doubleClick(within(ranking).getByText("Nova Rookie"));

    const drawer = await screen.findByRole("dialog", { name: /Nova Rookie/i });
    expect(within(drawer).getByText("Projeto consolidado")).toBeInTheDocument();
    expect(within(drawer).getByText("Fundada em 2020")).toBeInTheDocument();
    expect(within(drawer).queryByText("Operação moderna")).not.toBeInTheDocument();
  });
});
