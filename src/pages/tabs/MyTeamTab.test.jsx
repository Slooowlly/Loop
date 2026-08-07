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
        // Cor do rival é o que liga o painel de duelo: sem ela o dossiê cai no
        // estado "sem rival consolidado", que é o caminho errado para este mock.
        color: isVector ? "#c81d2e" : "#1d7ac8",
        origin_kind: "Nasceu na pista",
        historical_intensity: 62,
        recent_activity: 41,
        perceived_intensity: 49.4,
        head_to_head_wins: isVector ? 12 : 9,
        head_to_head_losses: isVector ? 8 : 11,
        last_meeting: { year: 2026, round: 7, position: 3, rival_position: 5, weeks_ago: 30 },
      },
      symbol_driver: isVector ? "Piloto Símbolo Vector" : "Piloto Símbolo Real",
      symbol_driver_detail: isVector
        ? "20 corridas, 9 vitórias, 16 pódios pela equipe."
        : "16 corridas, 7 vitórias, 12 pódios pela equipe.",
      symbol_driver_races: isVector ? 20 : 16,
      symbol_driver_wins: isVector ? 9 : 7,
      symbol_driver_podiums: isVector ? 16 : 12,
      profile_races: isVector ? 44 : 38,
      profile_wins: isVector ? 11 : 8,
      profile_podiums: isVector ? 22 : 14,
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
      // Livro-caixa agregado: é o que a aba Gestão usa para desenhar a curva e a
      // repartição. Só a Vector traz — a Falcon fica como o caso sem histórico
      // gravado, que precisa continuar renderizando os cards de prosa sem quebrar.
      ledger: isVector
        ? {
            seasons: 2,
            rounds: 4,
            first_season: 1,
            last_season: 2,
            peak_cash: 8800000,
            peak_cash_season: 2,
            peak_cash_round: 2,
            worst_debt: 1500000,
            worst_debt_season: 1,
            worst_debt_round: 2,
            healthy_seasons: 1,
            // A repartição cobre só a temporada 2 — a 1 é backstory, com prêmio de
            // construtores e mais nada. É o caso real de todo save: 26 temporadas
            // sorteadas antes da carreira contra as jogadas de verdade.
            flow_seasons: 1,
            flow_first_season: 2,
            flow_last_season: 2,
            flow_note: "",
            income_total: 12000000,
            expenses_total: 9000000,
            income_lines: [
              { id: "sponsorship_income", value: 7200000 },
              { id: "constructor_prize_income", value: 3600000 },
              { id: "gate_income", value: 1200000 },
            ],
            expense_lines: [
              { id: "salary_expense", value: 6300000 },
              { id: "technical_investment_cost", value: 2700000 },
            ],
            cash_curve: [
              { season_number: 1, round: 1, cash_balance: 400000, debt_balance: 0, is_season_close: false },
              { season_number: 1, round: 2, cash_balance: 0, debt_balance: 1500000, is_season_close: true },
              { season_number: 2, round: 1, cash_balance: 3200000, debt_balance: 0, is_season_close: false },
              { season_number: 2, round: 2, cash_balance: 8800000, debt_balance: 0, is_season_close: true },
            ],
          }
        : {
            // Carreira sem temporada jogada: tem história de caixa (as temporadas de
            // backstory gravam o saldo em cada fechamento) mas NENHUMA repartição —
            // o sorteio histórico não simula economia. É o caso real
            // que a aba precisa explicar em vez de esconder.
            seasons: 3,
            rounds: 3,
            first_season: 1,
            last_season: 3,
            peak_cash: 9900000,
            peak_cash_season: 3,
            peak_cash_round: 1000,
            worst_debt: 1200000,
            worst_debt_season: 1,
            worst_debt_round: 1000,
            healthy_seasons: 2,
            flow_seasons: 0,
            flow_first_season: 0,
            flow_last_season: 0,
            flow_note: "Nenhuma temporada jogada ainda.",
            income_total: 0,
            expenses_total: 0,
            income_lines: [],
            expense_lines: [],
            cash_curve: [
              { season_number: 1, round: 1000, cash_balance: 0, debt_balance: 1200000, is_season_close: true },
              { season_number: 2, round: 1000, cash_balance: 4100000, debt_balance: 0, is_season_close: true },
              { season_number: 3, round: 1000, cash_balance: 9900000, debt_balance: 0, is_season_close: true },
            ],
          },
    },
    title_categories: isVector
      ? []
      : [{ category: "GT4", year: "2025", color: "#f2c46d" }],
    // Subiu ao GT3 e voltou: é o caso que a agregação antiga colapsava num "GT4
    // 2024-2026" só, escondendo o rebaixamento. As duas passagens de GT4 têm de
    // sobreviver até a tela.
    movement: {
      promotions: 1,
      relegations: 1,
      time_by_category: "GT4: 2 anos · GT3: 1 ano",
      peak_category: "GT3",
      home_category: "GT4",
      // Uma linha por categoria, com as duas idas ao GT4 já somadas, do degrau
      // mais alto para o mais baixo — a mesma direção da pirâmide.
      time_lines: [
        { category: "GT3", category_id: "gt3", tier: 4, seasons: 1, races: 12, wins: 0, podiums: 1 },
        { category: "GT4", category_id: "gt4", tier: 3, seasons: 2, races: 24, wins: 3, podiums: 10 },
      ],
      ladder: [
        {
          category: "GT4",
          category_id: "gt4",
          tier: 3,
          visited: true,
          is_peak: false,
          is_current: true,
          seasons: 2,
          years: "2024-2026",
        },
        {
          category: "GT3",
          category_id: "gt3",
          tier: 4,
          visited: true,
          is_peak: true,
          is_current: false,
          seasons: 1,
          years: "2025",
        },
        {
          category: "Endurance",
          category_id: "endurance",
          tier: 6,
          visited: false,
          is_peak: false,
          is_current: false,
          seasons: 0,
          years: "",
        },
      ],
    },
    category_path: [
      {
        category: "GT4",
        category_id: "gt4",
        years: "2024",
        start_year: 2024,
        end_year: 2024,
        detail: "Categoria de estreia da equipe.",
        color: "#58a6ff",
        movement: "start",
        tier: 3,
      },
      {
        category: "GT3",
        category_id: "gt3",
        years: "2025",
        start_year: 2025,
        end_year: 2025,
        detail: "Promoção: subiu de categoria.",
        color: "#f2c46d",
        movement: "promotion",
        tier: 4,
      },
      {
        category: "GT4",
        category_id: "gt4",
        years: "2026",
        start_year: 2026,
        end_year: 2026,
        detail: "Rebaixamento: caiu de categoria.",
        color: "#58a6ff",
        movement: "relegation",
        tier: 3,
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
        presenca_publica: 63.5,
      },
    };
  });

  it("shows the public presence that multiplies sponsorship income", async () => {
    render(<MyTeamTab />);

    const panel = await screen.findByTestId("public-presence");
    // Valor CRU do backend — a tela não recalcula a média ponderada do lineup.
    expect(panel).toHaveTextContent("63.5");
    expect(panel).toHaveTextContent(/patroc[ií]nio/i);
  });

  it("hides the public presence panel when the backend has no lineup reading", async () => {
    mockState.playerTeam = { ...mockState.playerTeam, presenca_publica: 0 };
    render(<MyTeamTab />);

    expect(await screen.findByText(/^Caixa$/i)).toBeInTheDocument();
    expect(screen.queryByTestId("public-presence")).not.toBeInTheDocument();
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
    expect(teamLogo).toHaveClass("h-16", "w-24");
    expect(within(drawer).queryByText(/Arquivo compacto/i)).not.toBeInTheDocument();
    // A categoria atual é selo do cabeçalho no v2 — o v1 a omitia.
    expect(within(drawer).getByText("GT4 Series")).toBeInTheDocument();
    expect(within(drawer).queryByText("Estável")).not.toBeInTheDocument();
    expect(within(drawer).queryByText("Operação moderna")).not.toBeInTheDocument();
    expect(within(drawer).getByText("Projeto consolidado")).toBeInTheDocument();
    expect(within(drawer).getByText("Fundada em 2002")).toBeInTheDocument();
    expect(within(ranking).getByText("Falcon Motorsport").closest("tr")).toHaveClass("ring-1");
    expect(drawer.closest("[data-testid='team-history-layer']")).toHaveClass("z-[90]");
    // O dossiê abre centralizado, ocupando a tela — não colado numa borda. A
    // largura mora no wrapper que ancora a calha de setas; o painel ocupa ela toda.
    expect(drawer.parentElement).toHaveClass("w-[min(100%,1180px)]");
    expect(drawer).toHaveClass("w-full");
    expect(drawer).not.toHaveClass("right-0");
    expect(drawer).not.toHaveClass("left-0");
    expect(drawer).toHaveClass("bg-[#07101d]");
    expect(screen.getByLabelText(/Fechar histórico da equipe/i)).toHaveClass("bg-black/70");
    expect(within(drawer).getByRole("tab", { name: /Records/i })).toBeInTheDocument();
    // O retrato esportivo virou a aba "Identidade"; a antiga Identidade virou
    // "Rival", com o duelo no lugar da pilha de cards.
    expect(within(drawer).getByRole("tab", { name: /Identidade/i })).toBeInTheDocument();
    expect(within(drawer).getByRole("tab", { name: /Rival/i })).toBeInTheDocument();
    expect(within(drawer).getByRole("tab", { name: /Gestão/i })).toBeInTheDocument();
    expect(within(drawer).getByRole("tab", { name: /Categorias/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Equipe anterior/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Próxima equipe/i })).toBeEnabled();
    expect(within(drawer).getByText(/Comparativo em/i)).toBeInTheDocument();
    // O v2 nomeia a seção na coluna lateral — o título repetido dentro do conteúdo saiu.
    expect(within(drawer).getByRole("tab", { name: /Records/i })).toHaveAttribute("aria-selected", "true");
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

    fireEvent.click(within(drawer).getByRole("tab", { name: /Identidade/i }));

    // Temporadas disputadas vive no âncora do cabeçalho, com o número em fonte
    // grande e a unidade em fonte pequena — dois nós de texto, não um.
    expect(drawer.querySelector("[data-anchor='seasons']").textContent).toMatch(/2\s*Temporadas reais/);
    // As duas sequências saíram de Esportivo: "3 pódios consecutivos" é a fita de
    // forma recente, e lá dá para ver QUANDO — a frase era a versão pior do
    // mesmo dado, ocupando a primeira dobra da seção.
    expect(within(drawer).queryByText(/3 Pódios consecutivos reais/)).not.toBeInTheDocument();
    // Taxa de pódio saiu de Esportivo: é card de Records, e lá vem com a média
    // do grupo e a posição no ranking. Aqui era o mesmo número, sem contexto.
    expect(within(drawer).queryByText("75%")).not.toBeInTheDocument();

    fireEvent.click(within(drawer).getByRole("tab", { name: /Records/i }));
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

    fireEvent.click(within(drawer).getByRole("tab", { name: /Rival/i }));

    // O duelo abre a aba: as duas equipes de frente uma para a outra e o placar
    // do confronto direto no meio, cada número na cor de quem representa.
    expect(within(drawer).getByText(/Maior rival histórico/i)).toBeInTheDocument();
    expect(within(drawer).getByText("Falcon Motorsport")).toBeInTheDocument();
    expect(within(drawer).getByText("Nasceu na pista")).toBeInTheDocument();
    expect(within(drawer).getByText("12")).toBeInTheDocument();
    expect(within(drawer).getByText("8")).toBeInTheDocument();
    expect(within(drawer).getByText(/Confronto direto/i)).toBeInTheDocument();
    // O encontro é datado em tempo decorrido, não em "temporada X, rodada Y": 30
    // semanas viram "Há 7 meses", que é o que diz se a rivalidade está viva.
    expect(within(drawer).getByText(/Há 7 meses — 3º contra 5º/i)).toBeInTheDocument();
    // Os fatos da casa descem para a faixa de apoio, mas continuam na aba.
    expect(within(drawer).getByText(/Perfil histórico/i)).toBeInTheDocument();
    expect(within(drawer).getByText("Especialista Real")).toBeInTheDocument();
    expect(within(drawer).getByText("Resumo real da Vector calculado no backend.")).toBeInTheDocument();
    // Origem e atual viraram uma linha só de trajetória em vez de dois cards, e
    // o degrau ATUAL sai na cor da equipe — daí serem dois nós de texto.
    const trajetoria = within(drawer).getByText(/Trajetória/i).parentElement;
    expect(within(trajetoria).getByText("GT4 Origem Vector")).toHaveClass("text-text-muted");
    expect(within(trajetoria).getByText("GT4 Atual Real")).toHaveClass("text-[color:var(--team)]");
    expect(within(drawer).queryByText(/Categoria de origem/i)).not.toBeInTheDocument();
    expect(within(drawer).getByText("Piloto símbolo", { selector: "span" })).toBeInTheDocument();
    expect(within(drawer).getByText("Piloto Símbolo Vector")).toBeInTheDocument();
    // Os números do símbolo viraram métricas em vez de prosa — é o que dá aos
    // três cards da fileira a mesma anatomia e a mesma altura.
    const simbolo = within(drawer).getByText("Piloto Símbolo Vector").closest("div").parentElement;
    expect(within(simbolo).getByText("20")).toBeInTheDocument();
    expect(within(simbolo).getByText("9")).toBeInTheDocument();
    expect(within(simbolo).getByText("16")).toBeInTheDocument();

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
    // "Maior investimento técnico" saiu do rótulo: o valor é o pacote de HOJE, e o
    // superlativo prometia uma série histórica que o card não tem.
    expect(within(drawer).queryByText(/Maior investimento técnico/i)).not.toBeInTheDocument();
    expect(within(drawer).getByText(/Pacote técnico/i)).toBeInTheDocument();
    expect(within(drawer).getByText("Nível 8 - pacote real")).toBeInTheDocument();
    expect(within(drawer).getByText("Investimento real da Vector.")).toBeInTheDocument();

    // A curva de caixa é o que transforma a aba de retrato em história — sem ela o
    // jogador lia "Monitorada" sem saber se a equipe sobe ou afunda.
    expect(within(drawer).getByTestId("team-history-cash-curve")).toBeInTheDocument();
    expect(within(drawer).getByText("pico $8.8M · dívida $1.5M")).toBeInTheDocument();
    // Sankey: receita converge no tronco e sai repartida. Os rótulos das linhas vêm
    // das MESMAS chaves que a aba My Team usa, não de uma segunda tabela de nomes.
    const fluxo = within(drawer).getByTestId("team-history-money-flow");
    expect(within(fluxo).getByText("Patrocínios")).toBeInTheDocument();
    expect(within(fluxo).getByText("Salários")).toBeInTheDocument();
    // O tronco é a receita total; o saldo ($3M sobre $12M = 25%) entra como nó à
    // direita, ao lado dos custos — é o que faz a conta fechar dos dois lados.
    expect(within(fluxo).getByText("Receita total $12,000,000")).toBeInTheDocument();
    expect(within(fluxo).getByText("Saldo")).toBeInTheDocument();
    expect(within(fluxo).getByText("25%")).toBeInTheDocument();
    expect(within(fluxo).queryByText("Reservas e dívida")).not.toBeInTheDocument();
    // A legenda anuncia a JANELA medida, não a carreira inteira: as temporadas de
    // backstory não têm repartição para somar.
    expect(within(fluxo).getByText("temporada 2")).toBeInTheDocument();
  });

  it("explains the missing money flow instead of hiding the block", async () => {
    render(<MyTeamTab />);

    const ranking = await screen.findByRole("table", { name: /Ranking da categoria/i });
    fireEvent.doubleClick(within(ranking).getByText("Falcon Motorsport"));

    const drawer = await screen.findByRole("dialog", { name: /Falcon Motorsport/i });
    fireEvent.click(within(drawer).getByRole("tab", { name: /Gestão/i }));

    // A curva CONTINUA: o saldo de cada fechamento é registro real, mesmo nas
    // temporadas que não têm repartição.
    expect(within(drawer).getByTestId("team-history-cash-curve")).toBeInTheDocument();
    // Já o Sankey não tem o que desenhar — e em vez de sumir, diz por quê. Sumir era
    // o pior estado: o jogador não distinguia "não tem economia de rodada" de
    // "quebrou".
    const fluxo = within(drawer).getByTestId("team-history-money-flow");
    expect(within(fluxo).queryByTestId("team-history-money-flow-chart")).not.toBeInTheDocument();
    expect(
      within(fluxo).getByText("Nenhuma temporada jogada ainda."),
    ).toBeInTheDocument();
    expect(within(drawer).getByText("Gestão real da Falcon calculada no backend.")).toBeInTheDocument();
    expect(within(drawer).getByText("$9,900,000")).toBeInTheDocument();
  });

  it("draws the whole ladder, not only the rungs the team stepped on", async () => {
    render(<MyTeamTab />);

    const ranking = await screen.findByRole("table", { name: /Ranking da categoria/i });
    fireEvent.doubleClick(within(ranking).getByText("Falcon Motorsport"));

    const drawer = await screen.findByRole("dialog", { name: /Falcon Motorsport/i });
    fireEvent.click(within(drawer).getByRole("tab", { name: /Categorias/i }));

    // A pirâmide traz o degrau NUNCA pisado: é ele que diz quanto falta para o
    // topo, e a lista de passagens sozinha nunca dizia.
    const piramide = within(drawer).getByTestId("team-history-category-pyramid");
    const naoPisado = within(piramide).getByText("Endurance").closest("[data-visited]");
    expect(naoPisado).toHaveAttribute("data-visited", "0");
    expect(within(piramide).getByText("Nunca correu")).toBeInTheDocument();
    expect(within(piramide).getByText("Agora")).toBeInTheDocument();
    expect(within(piramide).getByText("Teto")).toBeInTheDocument();

    // Teto e casa no lugar de "melhor / mais difícil categoria", que empatavam
    // entre si em quem correu numa categoria só.
    expect(within(drawer).getByText("Teto alcançado")).toBeInTheDocument();
    expect(within(drawer).getByText("Degrau de casa")).toBeInTheDocument();
    expect(within(drawer).queryByText("Categoria mais difícil")).not.toBeInTheDocument();

    // O saldo é por CATEGORIA, somando as duas idas ao GT4 numa linha só — os
    // cards por passagem gastavam três linhas cada para repetir a pirâmide.
    const porCategoria = within(drawer).getByTestId("team-history-category-time");
    const gt4 = porCategoria.querySelector('[data-tally="gt4"]');
    expect(gt4).toHaveTextContent("3 vitórias · 10 pódios");
    // Do topo para a base, como a pirâmide logo acima — os dois blocos desenham
    // a mesma escada e precisam concordar sobre onde fica o alto.
    expect(
      [...porCategoria.querySelectorAll("[data-tally]")].map((no) => no.dataset.tally),
    ).toEqual(["gt3", "gt4"]);
    expect(porCategoria.querySelector('[data-tally="gt3"]')).toHaveTextContent("0 vitórias · 1 pódio");
    expect(within(drawer).queryByText("Passagens")).not.toBeInTheDocument();
    expect(within(drawer).queryByText("Rebaixamento: caiu de categoria.")).not.toBeInTheDocument();

    // A faixa ano a ano existe e marca o ano da subida com a cor do GT3.
    const faixa = within(drawer).getByTestId("team-history-category-trajectory");
    expect(faixa.querySelector('[data-year="2025"][data-category="gt3"]')).not.toBeNull();
    expect(faixa.querySelector('[data-year="2026"][data-category="gt4"]')).not.toBeNull();
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
