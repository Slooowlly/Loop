import { fireEvent, render, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import PreSeasonView from "./PreSeasonView";

let mockState = {};

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("PreSeasonView", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue([]);
    mockState = {
      careerId: "career-1",
      preseasonState: {
        current_week: 2,
        total_weeks: 4,
        is_complete: false,
        current_display_date: "2026-03-07",
      },
      preseasonWeeks: [],
      lastMarketWeekResult: null,
      playerProposals: [],
      preseasonFreeAgents: [],
      isAdvancingWeek: false,
      isRespondingProposal: false,
      advanceMarketWeek: vi.fn(),
      respondToProposal: vi.fn(),
      finalizePreseason: vi.fn(),
      playerTeam: {
        categoria: "gt4",
      },
    };
  });

  it("renders the simulated preseason date from state instead of the PC clock", async () => {
    render(<PreSeasonView />);

    expect(await screen.findByText(/7 de março/i)).toBeInTheDocument();
  });

  it("shows normalized team logos in team mapping and pending proposal cards", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        categoria: "gt3",
      },
      playerProposals: [
        {
          proposal_id: "proposal-amg",
          equipe_nome: "Mercedes-AMG",
          equipe_cor_primaria: "#00d2be",
          papel: "Numero1",
          categoria: "gt3",
          categoria_nome: "GT3 Championship",
          salario_oferecido: 250000,
          duracao_anos: 2,
          companheiro_nome: "Nico Voss",
          car_performance_rating: 86,
        },
      ],
    };

    invoke.mockImplementation(async (command, { category }) => {
      if (command !== "get_teams_standings" || category !== "gt3") {
        return [];
      }

      return [
        {
          id: "team-amg",
          nome: "Mercedes-AMG",
          nome_curto: "AMG",
          cor_primaria: "#00d2be",
          pontos: 0,
          vitorias: 0,
          piloto_1_nome: "Lena Hart",
          piloto_1_tenure_seasons: 2,
          piloto_2_nome: "Nico Voss",
          piloto_2_tenure_seasons: 1,
          trofeus: [],
          classe: "gt3",
          temp_posicao: 1,
          categoria_anterior: null,
        },
      ];
    });

    render(<PreSeasonView />);

    await screen.findByText("Lena Hart");

    const logos = screen.getAllByAltText("Mercedes-AMG logo");
    expect(logos).toHaveLength(2);
    logos.forEach((logo) => {
      expect(logo).toHaveAttribute("src", expect.stringContaining("TimesNormalized"));
      expect(logo).toHaveClass("object-contain");
    });
  });

  it("shows previous team logos in the drivers market", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        categoria: "gt3",
      },
      preseasonFreeAgents: [
        {
          driver_id: "driver-amg",
          driver_name: "Marco Rossi",
          categoria: "gt3",
          previous_team_name: "Mercedes-AMG",
          previous_team_color: "#00d2be",
          previous_team_abbr: "AMG",
          seasons_at_last_team: 2,
          total_career_seasons: 6,
          license_sigla: "SP",
          last_championship_position: 5,
          last_championship_total_drivers: 20,
          is_rookie: false,
        },
      ],
    };

    render(<PreSeasonView />);

    await screen.findByText("Marco Rossi");

    const logo = screen.getByAltText("Mercedes-AMG logo");
    expect(logo).toHaveAttribute("src", expect.stringContaining("TimesNormalized"));
    expect(logo).toHaveClass("object-contain");
    expect(screen.getByTitle("Carteira Super Pro")).toHaveAttribute(
      "aria-label",
      "Carteira Super Pro",
    );
    expect(screen.queryByText("AMG")).not.toBeInTheDocument();
  });

  it("shows compact tenure counters in the team mapping", async () => {
    invoke.mockImplementation(async (command, { category }) => {
      if (command !== "get_teams_standings" || category !== "gt4") {
        return [];
      }

      return [
        {
          id: "team-1",
          nome: "Vortex Racing",
          nome_curto: "VRT",
          cor_primaria: "#FF8000",
          pontos: 0,
          vitorias: 0,
          piloto_1_nome: "Luca Bianchi",
          piloto_1_tenure_seasons: 3,
          piloto_2_nome: "Mateo Silva",
          piloto_2_tenure_seasons: 1,
          trofeus: [],
          classe: "gt4",
          temp_posicao: 1,
          categoria_anterior: "production_challenger",
        },
        {
          id: "team-2",
          nome: "Nova Speed",
          nome_curto: "NSP",
          cor_primaria: "#3671C6",
          pontos: 0,
          vitorias: 0,
          piloto_1_nome: "Rafael Costa",
          piloto_1_tenure_seasons: 2,
          piloto_2_nome: "Bruno Alves",
          piloto_2_tenure_seasons: 4,
          trofeus: [],
          classe: "gt4",
          temp_posicao: 2,
          categoria_anterior: null,
        },
        {
          id: "team-3",
          nome: "Legacy Motorsport",
          nome_curto: "LGM",
          cor_primaria: "#f85149",
          pontos: 0,
          vitorias: 0,
          piloto_1_nome: "Thiago Lima",
          piloto_1_tenure_seasons: 5,
          piloto_2_nome: "Caio Mendes",
          piloto_2_tenure_seasons: 2,
          trofeus: [],
          classe: "gt4",
          temp_posicao: 3,
          categoria_anterior: "gt3",
        },
      ];
    });

    render(<PreSeasonView />);

    const teamName = await screen.findByText("Vortex Racing");
    const primaryDriver = await screen.findByText("Luca Bianchi");
    const secondaryDriver = screen.getByText("Mateo Silva");
    const categoryHeader = screen.getByTestId("preseason-category-header-gt4");
    const categoryLogo = within(categoryHeader).getByAltText("GT4 Championship");
    const newcomerTag = screen.getByText("New");
    const orderedTeamNames = screen
      .getAllByText(/^(Vortex Racing|Nova Speed|Legacy Motorsport)$/)
      .map((node) => node.textContent);

    expect(teamName).toHaveClass("text-[19px]", "font-bold");
    expect(categoryHeader).toHaveClass("flex-col", "items-center");
    expect(categoryLogo).toHaveAttribute("src", "/utilities/categorias/recortadas/GT4.png");
    expect(primaryDriver).toHaveClass("text-[15px]", "font-bold");
    expect(primaryDriver).toHaveClass("text-[color:var(--text-primary)]");
    expect(screen.getByText("3 anos")).toBeInTheDocument();
    expect(secondaryDriver).toHaveClass("text-[14px]", "font-semibold");
    expect(secondaryDriver).toHaveClass("text-[color:var(--text-primary)]");
    expect(newcomerTag).toHaveClass("rounded-md");
    expect(screen.queryByText("VR")).not.toBeInTheDocument();
    expect(screen.queryByText("NS")).not.toBeInTheDocument();
    expect(screen.queryByText("LG")).not.toBeInTheDocument();
    expect(screen.getByText("Promovido")).toBeInTheDocument();
    expect(screen.getByText("Relegado")).toBeInTheDocument();
    expect(orderedTeamNames).toEqual(["Nova Speed", "Vortex Racing", "Legacy Motorsport"]);
    expect(screen.queryByText("Confirmado")).not.toBeInTheDocument();
    expect(screen.queryByText("Novo")).not.toBeInTheDocument();
    expect(screen.queryByText("1T")).not.toBeInTheDocument();
    expect(screen.queryByText("3T")).not.toBeInTheDocument();
  });

  it("shows the total open vacancies on the category header", async () => {
    invoke.mockImplementation(async (command, { category }) => {
      if (command !== "get_teams_standings" || category !== "gt4") {
        return [];
      }

      return [
        {
          id: "team-1",
          nome: "Vortex Racing",
          nome_curto: "VRT",
          cor_primaria: "#FF8000",
          pontos: 0,
          vitorias: 0,
          piloto_1_nome: "Luca Bianchi",
          piloto_1_tenure_seasons: 2,
          piloto_2_nome: null,
          piloto_2_tenure_seasons: 0,
          trofeus: [],
          classe: "gt4",
          temp_posicao: 1,
          categoria_anterior: null,
        },
        {
          id: "team-2",
          nome: "Nova Speed",
          nome_curto: "NSP",
          cor_primaria: "#3671C6",
          pontos: 0,
          vitorias: 0,
          piloto_1_nome: null,
          piloto_1_tenure_seasons: 0,
          piloto_2_nome: null,
          piloto_2_tenure_seasons: 0,
          trofeus: [],
          classe: "gt4",
          temp_posicao: 2,
          categoria_anterior: null,
        },
      ];
    });

    render(<PreSeasonView />);

    const header = await screen.findByTestId("preseason-category-header-gt4");
    const title = within(header).getByAltText("GT4 Championship");
    const count = await screen.findByText("3 vagas");

    expect(title).toHaveAttribute("src", "/utilities/categorias/recortadas/GT4.png");
    expect(count).toHaveAttribute("data-testid", "preseason-category-count");
    expect(count).toHaveStyle({ color: "#3080FF", borderColor: "#3080FF55" });
  });

  it("replaces regular market category titles with cropped logos like mazda cup and mazda rookie", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        categoria: null,
      },
    };

    invoke.mockImplementation(async (command, { category }) => {
      if (command !== "get_teams_standings") {
        return [];
      }

      if (category === "mazda_amador") {
        return [
          {
            id: "mazda-cup-team",
            nome: "Aurora Mazda",
            nome_curto: "AMZ",
            cor_primaria: "#C8102E",
            pontos: 0,
            vitorias: 0,
            piloto_1_nome: "L. Ramos",
            piloto_1_tenure_seasons: 2,
            piloto_2_nome: null,
            piloto_2_tenure_seasons: 0,
            trofeus: [],
            classe: "mazda_amador",
            temp_posicao: 1,
            categoria_anterior: null,
          },
        ];
      }

      if (category === "mazda_rookie") {
        return [
          {
            id: "mazda-rookie-team",
            nome: "Nova Rookie",
            nome_curto: "NRK",
            cor_primaria: "#C8102E",
            pontos: 0,
            vitorias: 0,
            piloto_1_nome: "T. Costa",
            piloto_1_tenure_seasons: 1,
            piloto_2_nome: null,
            piloto_2_tenure_seasons: 0,
            trofeus: [],
            classe: "mazda_rookie",
            temp_posicao: 2,
            categoria_anterior: null,
          },
        ];
      }

      return [];
    });

    render(<PreSeasonView />);

    const mazdaCupHeader = await screen.findByTestId("preseason-category-header-mazda_amador");
    const mazdaRookieHeader = await screen.findByTestId("preseason-category-header-mazda_rookie");

    expect(within(mazdaCupHeader).getByAltText("Mazda Cup")).toHaveAttribute("src", "/utilities/categorias/recortadas/MX5%20CUP.png");
    expect(within(mazdaRookieHeader).getByAltText("Mazda Rookie")).toHaveAttribute("src", "/utilities/categorias/recortadas/MX5%20ROOKIE.png");
    expect(within(mazdaCupHeader).getByTestId("preseason-category-count")).toHaveTextContent("1 vaga");
    expect(within(mazdaRookieHeader).getByTestId("preseason-category-count")).toHaveTextContent("1 vaga");
  });

  it("uses the logo palette on the regular market category headers", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        categoria: null,
      },
    };

    invoke.mockImplementation(async (command, { category }) => {
      if (command !== "get_teams_standings") {
        return [];
      }

      if (category === "mazda_amador") {
        return [
          {
            id: "mazda-cup-team",
            nome: "Aurora Mazda",
            nome_curto: "AMZ",
            cor_primaria: "#C8102E",
            pontos: 0,
            vitorias: 0,
            piloto_1_nome: "L. Ramos",
            piloto_1_tenure_seasons: 2,
            piloto_2_nome: null,
            piloto_2_tenure_seasons: 0,
            trofeus: [],
            classe: "mazda_amador",
            temp_posicao: 1,
            categoria_anterior: null,
          },
        ];
      }

      if (category === "mazda_rookie") {
        return [
          {
            id: "mazda-rookie-team",
            nome: "Nova Rookie",
            nome_curto: "NRK",
            cor_primaria: "#C8102E",
            pontos: 0,
            vitorias: 0,
            piloto_1_nome: "T. Costa",
            piloto_1_tenure_seasons: 1,
            piloto_2_nome: null,
            piloto_2_tenure_seasons: 0,
            trofeus: [],
            classe: "mazda_rookie",
            temp_posicao: 2,
            categoria_anterior: null,
          },
        ];
      }

      return [];
    });

    render(<PreSeasonView />);

    const mazdaCupCount = within(await screen.findByTestId("preseason-category-header-mazda_amador"))
      .getByTestId("preseason-category-count");
    const mazdaRookieCount = within(await screen.findByTestId("preseason-category-header-mazda_rookie"))
      .getByTestId("preseason-category-count");

    expect(mazdaCupCount).toHaveStyle({ color: "#F01010", borderColor: "#F0101055" });
    expect(mazdaRookieCount).toHaveStyle({ color: "#FFE000", borderColor: "#FFE00055" });
  });

  it("orders all regular market categories from GT3 to rookies when showing all filters", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        categoria: null,
      },
    };

    const categoryTeams = {
      lmp2: "LMP2 Grid",
      gt3: "GT3 Grid",
      gt4: "GT4 Grid",
      bmw_m2: "BMW Grid",
      toyota_amador: "Toyota Cup Grid",
      mazda_amador: "Mazda Cup Grid",
      toyota_rookie: "Toyota Rookie Grid",
      mazda_rookie: "Mazda Rookie Grid",
    };

    invoke.mockImplementation(async (command, { category }) => {
      if (command !== "get_teams_standings" || !categoryTeams[category]) {
        return [];
      }

      return [
        {
          id: `${category}-team`,
          nome: categoryTeams[category],
          nome_curto: category.slice(0, 3).toUpperCase(),
          cor_primaria: "#3671C6",
          pontos: 0,
          vitorias: 0,
          piloto_1_nome: null,
          piloto_1_tenure_seasons: 0,
          piloto_2_nome: null,
          piloto_2_tenure_seasons: 0,
          trofeus: [],
          classe: category,
          temp_posicao: 1,
          categoria_anterior: null,
        },
      ];
    });

    const { container } = render(<PreSeasonView />);

    await screen.findByTestId("preseason-category-header-mazda_rookie");

    const orderedHeaders = Array.from(
      container.querySelectorAll('[data-testid^="preseason-category-header-"]'),
    ).map((header) => header.getAttribute("data-testid").replace("preseason-category-header-", ""));
    const toyotaRookieSection = screen
      .getByTestId("preseason-category-header-toyota_rookie")
      .closest("section");

    expect(orderedHeaders).toEqual([
      "lmp2",
      "gt3",
      "gt4",
      "bmw_m2",
      "toyota_amador",
      "mazda_amador",
      "toyota_rookie",
      "mazda_rookie",
    ]);
    expect(within(screen.getByTestId("preseason-category-header-lmp2")).getByAltText("LMP2 Prototype Championship")).toHaveAttribute(
      "src",
      "/utilities/categorias/recortadas/LMP2.png",
    );
    expect(toyotaRookieSection).toHaveClass("mt-14");
  });

  it("uses custom logo fit presets for market logos that need visual correction", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        categoria: null,
      },
    };

    const categoryTeams = {
      lmp2: "LMP2 Grid",
      gt3: "GT3 Grid",
      gt4: "GT4 Grid",
      bmw_m2: "BMW Grid",
      toyota_amador: "Toyota Cup Grid",
      mazda_amador: "Mazda Cup Grid",
      toyota_rookie: "Toyota Rookie Grid",
      mazda_rookie: "Mazda Rookie Grid",
    };

    invoke.mockImplementation(async (command, { category }) => {
      if (command !== "get_teams_standings" || !categoryTeams[category]) {
        return [];
      }

      return [
        {
          id: `${category}-team`,
          nome: categoryTeams[category],
          nome_curto: category.slice(0, 3).toUpperCase(),
          cor_primaria: "#3671C6",
          pontos: 0,
          vitorias: 0,
          piloto_1_nome: null,
          piloto_1_tenure_seasons: 0,
          piloto_2_nome: null,
          piloto_2_tenure_seasons: 0,
          trofeus: [],
          classe: category,
          temp_posicao: 1,
          categoria_anterior: null,
        },
      ];
    });

    render(<PreSeasonView />);

    const getLogo = async (category) => within(await screen.findByTestId(`preseason-category-header-${category}`))
      .getByTestId("preseason-category-logo");

    expect(await getLogo("toyota_amador")).toHaveStyle({ transform: "translateX(0.75%)" });
    expect(await getLogo("lmp2")).toHaveAttribute("src", "/utilities/categorias/recortadas/LMP2.png");
    const mazdaCupLogo = await getLogo("mazda_amador");
    const bmwLogo = await getLogo("bmw_m2");

    expect(mazdaCupLogo).toHaveStyle({ transform: "translateX(-1.5%) scale(1.38)" });
    expect(await getLogo("mazda_rookie")).toHaveStyle({ transform: "translateX(-2%) scale(1.18)" });
    expect(await getLogo("toyota_rookie")).toHaveStyle({ transform: "translateY(-2%) scale(1.24)" });
    expect(bmwLogo).toHaveStyle({ transform: "translateX(-3%) scale(1.1)" });
    expect(await getLogo("gt4")).toHaveStyle({ transform: "translateX(2%) scale(1.52)" });
    expect(await getLogo("gt3")).toHaveStyle({ transform: "translateX(-1%) scale(1.5)" });
    expect(mazdaCupLogo).not.toHaveClass("drop-shadow-[0_16px_32px_rgba(0,0,0,0.34)]");
    expect(bmwLogo).not.toHaveClass("drop-shadow-[0_16px_32px_rgba(0,0,0,0.34)]");
  });

  it("keeps special categories out of the normal preseason market", async () => {
    render(<PreSeasonView />);

    await screen.findByText(/Mercado de Transferências/i);

    expect(screen.queryByRole("button", { name: /Production/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Endurance/i })).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith(
      "get_teams_standings",
      expect.objectContaining({ category: "production_challenger" }),
    );
    expect(invoke).not.toHaveBeenCalledWith(
      "get_teams_standings",
      expect.objectContaining({ category: "endurance" }),
    );
  });

  it("shows a compact weekly closing panel grouped by destination category", async () => {
    mockState = {
      ...mockState,
      lastMarketWeekResult: {
        week_number: 2,
        events: [
          {
            event_type: "TransferCompleted",
            driver_name: "Marta Bianco",
            categoria: "gt3",
            from_categoria: "gt4",
            movement_kind: "promotion",
            championship_position: 1,
          },
          {
            event_type: "TransferCompleted",
            driver_name: "Colin Smith",
            categoria: "gt3",
            from_categoria: "gt3",
            movement_kind: "lateral",
            championship_position: 4,
          },
          {
            event_type: "RookieSigned",
            driver_name: "Giovanni Conti",
            categoria: "mazda_rookie",
            movement_kind: "rookie",
            championship_position: 3,
            team_name: "Vertex BMW",
          },
          {
            event_type: "RookieSigned",
            driver_name: "Victor Almeida",
            categoria: "gt3",
            championship_position: 12,
          },
          {
            event_type: "ContractExpired",
            driver_name: "Nicolas Meyer",
            categoria: "bmw_m2",
            movement_kind: "departure",
            championship_position: 11,
          },
          {
            event_type: "TransferCompleted",
            driver_name: "Lucas Prado",
            categoria: "bmw_m2",
            from_categoria: "gt4",
            movement_kind: "relegation",
            championship_position: 8,
          },
          {
            event_type: "PlayerProposalReceived",
            driver_name: "Rodrigo Vieira",
            categoria: "gt4",
            championship_position: 6,
            team_name: "Apex GT4",
          },
          {
            event_type: "ContractRenewed",
            driver_name: "Austin Williams",
            categoria: "gt4",
            movement_kind: "renewal",
            championship_position: 2,
          },
        ],
      },
    };

    render(<PreSeasonView />);

    const weeklyClosing = within(await screen.findByTestId("weekly-closing-market"));

    expect(weeklyClosing.getByText(/fechamento da semana/i)).toBeInTheDocument();
    expect(weeklyClosing.getByText(/gt3 championship/i)).toBeInTheDocument();
    expect(weeklyClosing.getByText(/mazda rookie/i)).toBeInTheDocument();
    expect(weeklyClosing.getByText(/bmw m2 cup/i)).toBeInTheDocument();
    expect(weeklyClosing.getByText(new RegExp("^1\\u00ba$"))).toBeInTheDocument();
    expect(weeklyClosing.getByText(new RegExp("^4\\u00ba$"))).toBeInTheDocument();
    expect(weeklyClosing.getByText(new RegExp("^3\\u00ba$"))).toBeInTheDocument();
    expect(weeklyClosing.getByText(/marta bianco/i)).toBeInTheDocument();
    expect(weeklyClosing.getByText(/colin smith/i)).toBeInTheDocument();
    expect(weeklyClosing.getByText(/giovanni conti/i)).toBeInTheDocument();
    expect(weeklyClosing.getByText(/victor almeida/i)).toBeInTheDocument();
    expect(weeklyClosing.getByText(/nicolas meyer/i)).toBeInTheDocument();
    expect(weeklyClosing.getByText(/lucas prado/i)).toBeInTheDocument();
    expect(weeklyClosing.getByText(/rodrigo vieira/i)).toBeInTheDocument();
    expect(weeklyClosing.getByTitle("Promoção")).toBeInTheDocument();
    expect(weeklyClosing.getByTitle("Troca lateral")).toBeInTheDocument();
    expect(weeklyClosing.getByTitle("Estreia")).toBeInTheDocument();
    expect(weeklyClosing.getByTitle("Contratação")).toBeInTheDocument();
    expect(weeklyClosing.getByTitle("Saiu da equipe")).toBeInTheDocument();
    expect(weeklyClosing.getByTitle("Rebaixamento")).toBeInTheDocument();
    expect(weeklyClosing.getByTitle("Proposta recebida")).toBeInTheDocument();
    expect(weeklyClosing.getByText(new RegExp("^6\\u00ba$"))).toBeInTheDocument();
    expect(weeklyClosing.getByText(new RegExp("^11\\u00ba$"))).toBeInTheDocument();
    expect(screen.getByTestId("weekly-closing-market").textContent).not.toContain("Ã");
    expect(weeklyClosing.queryByText(/austin williams/i)).not.toBeInTheDocument();
    expect(weeklyClosing.queryByText(/vertex bmw/i)).not.toBeInTheDocument();
    expect(weeklyClosing.queryByText(/apex gt4/i)).not.toBeInTheDocument();
    expect(weeklyClosing.queryByText(/^SP$/i)).not.toBeInTheDocument();
  });

  it("groups displaced drivers by category in a larger end-of-preseason modal", async () => {
    mockState = {
      ...mockState,
      preseasonState: {
        current_week: 4,
        total_weeks: 4,
        is_complete: true,
        current_display_date: "2026-03-28",
      },
      preseasonFreeAgents: [
        {
          driver_id: "driver-1",
          driver_name: "Luca Bianchi",
          categoria: "gt3",
          previous_team_name: "Mercedes-AMG",
          previous_team_color: "#00d2be",
          previous_team_abbr: "AMG",
          seasons_at_last_team: 3,
          total_career_seasons: 8,
          license_sigla: "SP",
          last_championship_position: 12,
          last_championship_total_drivers: 20,
          is_rookie: false,
        },
        {
          driver_id: "driver-2",
          driver_name: "Mateo Silva",
          categoria: "gt4",
          previous_team_name: "Racing Spirit",
          previous_team_color: "#58a6ff",
          previous_team_abbr: "RSR",
          seasons_at_last_team: 2,
          total_career_seasons: 5,
          license_sigla: "P",
          last_championship_position: 7,
          last_championship_total_drivers: 18,
          is_rookie: false,
        },
        {
          driver_id: "driver-3",
          driver_name: "Rafael Costa",
          categoria: "gt3",
          previous_team_name: "Wolf Racing Team",
          previous_team_color: "#3fb950",
          previous_team_abbr: "WRT",
          seasons_at_last_team: 1,
          total_career_seasons: 4,
          license_sigla: "A",
          last_championship_position: 14,
          last_championship_total_drivers: 20,
          is_rookie: false,
        },
      ],
    };

    render(<PreSeasonView />);

    fireEvent.click(screen.getByRole("button", { name: /iniciar temporada/i }));

    const modalTitle = await screen.findByText("Pilotos sem vaga");
    const modal = modalTitle.closest("div");

    expect(modalTitle).toBeInTheDocument();
    expect(within(modal).getAllByText("GT3 Championship")).toHaveLength(1);
    expect(within(modal).getAllByText("GT4 Championship")).toHaveLength(1);
    expect(within(modal).getAllByText("Ex-equipe").length).toBeGreaterThan(0);
    const displacedDriver = within(modal).getByText("Luca Bianchi");
    expect(displacedDriver).toHaveClass("text-[17px]");
    const previousTeamLine = within(modal).getByText("Mercedes-AMG").closest("div");
    expect(within(previousTeamLine).getByAltText("Mercedes-AMG logo")).toHaveAttribute("src", expect.stringContaining("TimesNormalized"));
    expect(within(previousTeamLine).getByText("Mercedes-AMG")).toHaveStyle({ color: "#00d2be" });
    expect(within(previousTeamLine).getByText("Mercedes-AMG")).toHaveClass("text-[14px]", "font-semibold");
    expect(within(previousTeamLine).getByText(/12º\/20/)).toBeInTheDocument();
    expect(within(modal).getByText("3 temporadas")).toBeInTheDocument();
    expect(within(modal).getByText(/7º\/18/)).toBeInTheDocument();
    expect(within(modal).getByText("Racing Spirit")).toHaveStyle({ color: "#58a6ff" });
    expect(within(modal).getByText("2 temporadas")).toBeInTheDocument();
    expect(within(modal).getByText(/14º\/20/)).toBeInTheDocument();
    expect(within(modal).getByText("Wolf Racing Team")).toHaveStyle({ color: "#3fb950" });
    expect(within(modal).getByText("1 temporada")).toBeInTheDocument();
    expect(within(modal).getByText("SP")).toHaveClass("min-w-[3.25rem]", "text-[11px]");
    expect(within(modal).queryByText(/Correu pela equipe/i)).not.toBeInTheDocument();
    expect(within(modal).queryByText(/Categoria:/i)).not.toBeInTheDocument();
    expect(within(modal).queryByText(/Carreira:/i)).not.toBeInTheDocument();
    expect(within(modal).queryByText("AMG")).not.toBeInTheDocument();
    expect(modal).toHaveClass("max-w-4xl");
  });
});
