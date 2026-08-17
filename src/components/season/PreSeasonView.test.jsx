import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import PreSeasonView from "./PreSeasonView";

let mockState = {};

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Esta suíte cobre o layout v1 da pré-temporada, que segue disponível pelo
// interruptor do canto. O v2 é o padrão da tela e tem marcação própria — fixar o
// layout aqui mantém estes guards apontando para o que eles de fato descrevem.
function pinLegacyLayout() {
  window.localStorage.setItem("loop.preseason.layout", "v1");
}

describe("PreSeasonView", () => {
  beforeEach(() => {
    pinLegacyLayout();
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

    // A ficha da proposta mora no modal de ofertas — a coluna só tem a porta para
    // ele, porque aceitar e recusar não existem fora daquela tela.
    fireEvent.click(await screen.findByTestId("proposals-open-modal"));

    const logos = await screen.findAllByAltText("Mercedes-AMG logo");
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
          market_tier: 4,
          seasons_idle: 0,
          eligible_categories: ["gt4", "gt3"],
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
    // Agrupado pela faixa de nível (tier 4 = Master), não pela carteira.
    expect(screen.getByText("Master")).toBeInTheDocument();
    // Etiqueta de destino provável no lugar da licença (escopada ao card do piloto,
    // pois "GT3" também é um chip de filtro no topo).
    const driverCard = screen.getByText("Marco Rossi").closest("div");
    expect(within(driverCard).getByText("GT3")).toBeInTheDocument();
    // A carteira agora fica escondida na coluna.
    expect(screen.queryByTitle("Carteira Super Pro")).not.toBeInTheDocument();
    expect(screen.queryByText("SP")).not.toBeInTheDocument();
    expect(screen.queryByText("AMG")).not.toBeInTheDocument();
  });

  it("groups free agents by brand within a level band (no Toyota/Mazda interleaving)", async () => {
    mockState = {
      ...mockState,
      playerTeam: {}, // sem categoria → filtro do topo em "all" (não recorta a coluna)
      // Duas marcas na MESMA banda (Amador, tier 1). Nomes escolhidos para que a ordem
      // por marca (Toyota antes de Mazda) difira da ordem puramente alfabética.
      preseasonFreeAgents: [
        { driver_id: "m1", driver_name: "Alan Prost", categoria: "mazda_amador", market_tier: 1, seasons_idle: 0, license_sigla: "A", is_rookie: false, eligible_categories: ["mazda_amador", "bmw_m2"] },
        { driver_id: "t1", driver_name: "Yuki Yamamoto", categoria: "toyota_amador", market_tier: 1, seasons_idle: 0, license_sigla: "A", is_rookie: false, eligible_categories: ["toyota_amador", "bmw_m2"] },
        { driver_id: "m2", driver_name: "Bruno Senna", categoria: "mazda_amador", market_tier: 1, seasons_idle: 0, license_sigla: "A", is_rookie: false, eligible_categories: ["mazda_amador", "bmw_m2"] },
        { driver_id: "t2", driver_name: "Austin Moore", categoria: "toyota_amador", market_tier: 1, seasons_idle: 0, license_sigla: "A", is_rookie: false, eligible_categories: ["toyota_amador", "bmw_m2"] },
      ],
    };

    render(<PreSeasonView />);

    await screen.findByText("Austin Moore");

    const order = screen
      .getAllByText(/^(Alan Prost|Bruno Senna|Yuki Yamamoto|Austin Moore)$/)
      .map((node) => node.textContent);

    // Toyota (contígua, por nome) primeiro, depois Mazda (contígua) — não intercalado
    // nem meramente alfabético (que daria Alan, Austin, Bruno, Yuki).
    expect(order).toEqual(["Austin Moore", "Yuki Yamamoto", "Alan Prost", "Bruno Senna"]);

    // Separador físico por marca (1) + etiqueta em cada linha (2 por marca) = 3 ocorrências.
    expect(screen.getAllByText("Toyota Cup")).toHaveLength(3);
    expect(screen.getAllByText("Mazda Cup")).toHaveLength(3);
  });

  it("filters the driver market by the selected top category (eligibility)", async () => {
    mockState = {
      ...mockState,
      playerTeam: {}, // começa em "all"
      preseasonFreeAgents: [
        {
          driver_id: "pro1", driver_name: "Paulo Pro", categoria: "bmw_m2",
          market_tier: 2, seasons_idle: 0, license_sigla: "P", is_rookie: false,
          eligible_categories: ["toyota_amador", "mazda_amador", "bmw_m2", "production_challenger", "gt4"],
        },
        {
          driver_id: "eli1", driver_name: "Elena Elite", categoria: "endurance",
          market_tier: 6, seasons_idle: 0, license_sigla: "SE", is_rookie: false,
          eligible_categories: ["gt3", "endurance"],
        },
      ],
    };

    render(<PreSeasonView />);

    // "Todas" (padrão) mostra os dois.
    await screen.findByText("Paulo Pro");
    expect(screen.getByText("Elena Elite")).toBeInTheDocument();

    // Filtrar por Production → só quem pode pegar vaga production permanece.
    fireEvent.click(screen.getByRole("button", { name: "Production" }));

    expect(screen.getByText("Paulo Pro")).toBeInTheDocument();
    expect(screen.queryByText("Elena Elite")).not.toBeInTheDocument();
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
    expect(categoryLogo).toHaveAttribute("src", "/utilities/categorias/recortadas/GT4.webp");
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

    expect(title).toHaveAttribute("src", "/utilities/categorias/recortadas/GT4.webp");
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

    expect(within(mazdaCupHeader).getByAltText("Mazda Cup")).toHaveAttribute("src", "/utilities/categorias/recortadas/MX5%20CUP.webp");
    expect(within(mazdaRookieHeader).getByAltText("Mazda Rookie")).toHaveAttribute("src", "/utilities/categorias/recortadas/MX5%20ROOKIE.webp");
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

  it("orders all regular market categories from Endurance to rookies when showing all filters", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        categoria: null,
      },
    };

    const categoryTeams = {
      endurance: "Endurance Grid",
      gt3: "GT3 Grid",
      gt4: "GT4 Grid",
      production_challenger: "Production Grid",
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
          classe: category === "endurance" ? "lmp2" : category,
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
      "endurance",
      "gt3",
      "gt4",
      "production_challenger",
      "bmw_m2",
      "toyota_amador",
      "mazda_amador",
      "toyota_rookie",
      "mazda_rookie",
    ]);
    expect(within(screen.getByTestId("preseason-category-header-endurance")).getByAltText("Endurance Championship")).toHaveAttribute(
      "src",
      "/utilities/categorias/recortadas/ENDURANCE.webp",
    );
    expect(within(screen.getByTestId("preseason-category-header-production_challenger")).getByAltText("Production")).toHaveAttribute(
      "src",
      "/utilities/categorias/recortadas/PRODUCTION.webp",
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
      endurance: "Endurance Grid",
      gt3: "GT3 Grid",
      gt4: "GT4 Grid",
      production_challenger: "Production Grid",
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
          classe: category === "endurance" ? "lmp2" : category,
          temp_posicao: 1,
          categoria_anterior: null,
        },
      ];
    });

    render(<PreSeasonView />);

    const getLogo = async (category) => within(await screen.findByTestId(`preseason-category-header-${category}`))
      .getByTestId("preseason-category-logo");

    expect(await getLogo("toyota_amador")).toHaveStyle({ transform: "translateX(0.75%)" });
    expect(await getLogo("endurance")).toHaveAttribute("src", "/utilities/categorias/recortadas/ENDURANCE.webp");
    expect(await getLogo("production_challenger")).toHaveAttribute("src", "/utilities/categorias/recortadas/PRODUCTION.webp");
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

  it("shows Production and Endurance as normal preseason market divisions", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        categoria: null,
      },
    };

    invoke.mockImplementation(async (command, { category } = {}) => {
      if (command !== "get_teams_standings") {
        return [];
      }

      if (category === "production_challenger") {
        return [
          {
            id: "team-production",
            nome: "Vector Production",
            nome_curto: "VPR",
            cor_primaria: "#3fb950",
            pontos: 0,
            vitorias: 0,
            piloto_1_nome: "Ana Costa",
            piloto_1_tenure_seasons: 1,
            piloto_2_nome: "Lia Ramos",
            piloto_2_tenure_seasons: 1,
            trofeus: [],
            classe: "toyota",
            temp_posicao: 1,
            categoria_anterior: null,
          },
        ];
      }

      if (category === "endurance") {
        return [
          {
            id: "team-endurance",
            nome: "Vector Endurance",
            nome_curto: "VEN",
            cor_primaria: "#3671C6",
            pontos: 0,
            vitorias: 0,
            piloto_1_nome: "Nico Voss",
            piloto_1_tenure_seasons: 1,
            piloto_2_nome: "Mia Hart",
            piloto_2_tenure_seasons: 1,
            trofeus: [],
            classe: "lmp2",
            temp_posicao: 1,
            categoria_anterior: null,
          },
        ];
      }

      return [];
    });

    render(<PreSeasonView />);

    await screen.findByText(/Mercado de Transferências/i);

    expect(screen.getByRole("button", { name: /Production/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Endurance/i })).toBeInTheDocument();
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "get_teams_standings",
        expect.objectContaining({ category: "production_challenger" }),
      );
      expect(invoke).toHaveBeenCalledWith(
        "get_teams_standings",
        expect.objectContaining({ category: "endurance" }),
      );
    });
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
    // O rotulo do degrau e so "BMW": "Cup" nao distingue nada (a categoria e a
    // unica BMW da piramide) e colidia com a classe BMW da Production.
    expect(weeklyClosing.getByText(/\bbmw\b/i)).toBeInTheDocument();
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
    expect(weeklyClosing.getByLabelText("Promoção")).toBeInTheDocument();
    expect(weeklyClosing.getByLabelText("Troca lateral")).toBeInTheDocument();
    expect(weeklyClosing.getByLabelText("Estreia")).toBeInTheDocument();
    expect(weeklyClosing.getByLabelText("Contratação")).toBeInTheDocument();
    expect(weeklyClosing.getByLabelText("Saiu da equipe")).toBeInTheDocument();
    expect(weeklyClosing.getByLabelText("Rebaixamento")).toBeInTheDocument();
    expect(weeklyClosing.getByLabelText("Proposta recebida")).toBeInTheDocument();
    expect(weeklyClosing.getByText(new RegExp("^6\\u00ba$"))).toBeInTheDocument();
    expect(weeklyClosing.getByText(new RegExp("^11\\u00ba$"))).toBeInTheDocument();
    expect(screen.getByTestId("weekly-closing-market").textContent).not.toContain("Ã");
    expect(weeklyClosing.queryByText(/austin williams/i)).not.toBeInTheDocument();
    expect(weeklyClosing.queryByText(/vertex bmw/i)).not.toBeInTheDocument();
    expect(weeklyClosing.queryByText(/apex gt4/i)).not.toBeInTheDocument();
    expect(weeklyClosing.queryByText(/^SP$/i)).not.toBeInTheDocument();
  });

  it("steps between weekly closing movements with the side rail of the detail modal", async () => {
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
        ],
      },
    };

    render(<PreSeasonView />);

    const weeklyClosing = within(await screen.findByTestId("weekly-closing-market"));
    fireEvent.click(weeklyClosing.getByText(/marta bianco/i));

    expect(await screen.findByRole("heading", { name: /marta bianco/i })).toBeInTheDocument();

    // Primeiro da lista: só se anda para baixo.
    expect(screen.getByTestId("transfer-detail-step-up")).toBeDisabled();
    fireEvent.click(screen.getByTestId("transfer-detail-step-down"));

    expect(await screen.findByRole("heading", { name: /colin smith/i })).toBeInTheDocument();
    expect(screen.getByTestId("transfer-detail-step-down")).toBeDisabled();

    // E o caminho de volta.
    fireEvent.click(screen.getByTestId("transfer-detail-step-up"));
    expect(await screen.findByRole("heading", { name: /marta bianco/i })).toBeInTheDocument();
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

    const modalTitle = await screen.findByText("3 pilotos ficaram sem vaga");
    const modal = modalTitle.closest("div");

    // A consequência é o que a tela precisa dizer: eles saem da escada. Sem essa
    // frase o modal lista nomes e deixa o jogador sem saber o que aconteceu.
    expect(within(modal).getByText(/não corre esta temporada|Nenhum corre esta temporada/i))
      .toBeInTheDocument();

    expect(within(modal).getAllByText("GT3 Championship")).toHaveLength(1);
    expect(within(modal).getAllByText("GT4 Championship")).toHaveLength(1);

    // Uma linha por piloto: sem rótulo "Ex-equipe" repetido e sem selo de licença.
    expect(within(modal).queryByText("Ex-equipe")).not.toBeInTheDocument();
    expect(within(modal).queryByText("SP")).not.toBeInTheDocument();
    expect(within(modal).queryByText("A")).not.toBeInTheDocument();

    expect(within(modal).getByText("Mercedes-AMG")).toHaveStyle({ color: "#00d2be" });
    expect(within(modal).getByAltText("Mercedes-AMG logo")).toHaveAttribute(
      "src",
      expect.stringContaining("TimesNormalized"),
    );
    expect(within(modal).getByText("12º/20")).toBeInTheDocument();
    expect(within(modal).getByText(/3 temporadas na equipe/)).toBeInTheDocument();
    expect(within(modal).getByText("7º/18")).toBeInTheDocument();
    expect(within(modal).getByText("Racing Spirit")).toHaveStyle({ color: "#58a6ff" });
    expect(within(modal).getByText(/2 temporadas na equipe/)).toBeInTheDocument();
    expect(within(modal).getByText("14º/20")).toBeInTheDocument();
    expect(within(modal).getByText("Wolf Racing Team")).toHaveStyle({ color: "#3fb950" });
    expect(within(modal).getByText(/1 temporada na equipe/)).toBeInTheDocument();

    expect(within(modal).queryByText("AMG")).not.toBeInTheDocument();
    expect(modal).toHaveClass("max-w-4xl");

    // O nome abre a ficha rápida do piloto, como no resto do mercado.
    fireEvent.click(within(modal).getByText("Luca Bianchi"));
    expect(await screen.findByTestId("driver-mini-card")).toBeInTheDocument();
  });

  // Dentro da categoria, do melhor colocado para o pior. A ordem que o backend
  // devolve não tem leitura nenhuma de cima para baixo.
  it("orders displaced drivers by championship result inside each category", async () => {
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
          driver_id: "driver-last",
          driver_name: "Rasmus Kristensen",
          categoria: "gt3",
          previous_team_name: "Wolf Racing Team",
          previous_team_color: "#3fb950",
          seasons_at_last_team: 3,
          license_sigla: "A",
          last_championship_position: 11,
          last_championship_total_drivers: 12,
          is_rookie: false,
        },
        {
          driver_id: "driver-first",
          driver_name: "Guillermo Torres",
          categoria: "gt3",
          previous_team_name: "Mercedes-AMG",
          previous_team_color: "#00d2be",
          seasons_at_last_team: 6,
          license_sigla: "A",
          last_championship_position: 6,
          last_championship_total_drivers: 12,
          is_rookie: false,
        },
      ],
    };

    render(<PreSeasonView />);
    fireEvent.click(screen.getByRole("button", { name: /iniciar temporada/i }));
    await screen.findByText("2 pilotos ficaram sem vaga");

    const rows = screen.getAllByTestId("displaced-driver-row");
    expect(rows[0]).toHaveTextContent("Guillermo Torres");
    expect(rows[1]).toHaveTextContent("Rasmus Kristensen");
  });

  // Seis nomes desconhecidos e, no meio deles, um que o jogador enfrentou 14 vezes.
  // Sem o retrospecto a lista não distingue os dois casos.
  it("shows the head-to-head record and the rival marker for drivers the player has raced", async () => {
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
          driver_id: "driver-known",
          driver_name: "Guillermo Torres",
          categoria: "gt3",
          previous_team_name: "Mercedes-AMG",
          previous_team_color: "#00d2be",
          seasons_at_last_team: 6,
          license_sigla: "A",
          last_championship_position: 6,
          last_championship_total_drivers: 12,
          is_rookie: false,
        },
        {
          driver_id: "driver-unknown",
          driver_name: "Niels Kramer",
          categoria: "gt3",
          previous_team_name: "Wolf Racing Team",
          previous_team_color: "#3fb950",
          seasons_at_last_team: 2,
          license_sigla: "A",
          last_championship_position: 7,
          last_championship_total_drivers: 12,
          is_rookie: false,
        },
      ],
    };

    invoke.mockImplementation((command) => {
      if (command === "get_displaced_driver_context") {
        return Promise.resolve([
          {
            driver_id: "driver-known",
            shared_races: 14,
            player_ahead: 9,
            driver_ahead: 5,
            last_shared_season: 3,
            rival_role: "nemesis",
          },
          {
            driver_id: "driver-unknown",
            shared_races: 0,
            player_ahead: 0,
            driver_ahead: 0,
            last_shared_season: null,
            rival_role: null,
          },
        ]);
      }
      return Promise.resolve([]);
    });

    render(<PreSeasonView />);
    fireEvent.click(screen.getByRole("button", { name: /iniciar temporada/i }));
    await screen.findByText("2 pilotos ficaram sem vaga");

    const placar = await screen.findByTestId("displaced-driver-head-to-head");
    expect(placar).toHaveTextContent("9–5");
    // Quem nunca cruzou com o jogador não ganha marcador nenhum.
    expect(screen.getAllByTestId("displaced-driver-head-to-head")).toHaveLength(1);

    const marcador = screen.getByTestId("displaced-driver-rival-role");
    expect(marcador).toHaveAttribute("aria-label", "Nêmesis");
    expect(screen.getAllByTestId("displaced-driver-rival-role")).toHaveLength(1);
  });

  // Dos pilotos que sobraram, os que saíram da equipe do jogador são os únicos que
  // mudam alguma coisa para ele — no meio da lista não tinham destaque nenhum.
  it("raises the displaced drivers who came from the player's own team", async () => {
    mockState = {
      ...mockState,
      playerTeam: { id: "T001", nome: "Mercedes-AMG", categoria: "gt3" },
      preseasonState: {
        current_week: 4,
        total_weeks: 4,
        is_complete: true,
        current_display_date: "2026-03-28",
      },
      preseasonFreeAgents: [
        {
          driver_id: "driver-mine",
          driver_name: "Marcos Mendes",
          categoria: "gt3",
          previous_team_name: "Mercedes-AMG",
          previous_team_color: "#00d2be",
          seasons_at_last_team: 1,
          license_sigla: "A",
          last_championship_position: 5,
          last_championship_total_drivers: 12,
          is_rookie: false,
        },
        {
          driver_id: "driver-other",
          driver_name: "Bruno Costa",
          categoria: "gt3",
          previous_team_name: "Wolf Racing Team",
          previous_team_color: "#3fb950",
          seasons_at_last_team: 1,
          license_sigla: "A",
          last_championship_position: 9,
          last_championship_total_drivers: 12,
          is_rookie: false,
        },
      ],
    };

    render(<PreSeasonView />);
    fireEvent.click(screen.getByRole("button", { name: /iniciar temporada/i }));
    await screen.findByText("2 pilotos ficaram sem vaga");

    const flag = screen.getByTestId("displaced-player-team-flag");
    expect(flag).toHaveTextContent("Marcos Mendes");
    expect(flag).toHaveTextContent("Mercedes-AMG");
    expect(flag).not.toHaveTextContent("Bruno Costa");

    const rows = screen.getAllByTestId("displaced-driver-row");
    expect(rows[0]).toHaveTextContent("Sua equipe");
    expect(rows[1]).not.toHaveTextContent("Sua equipe");
  });
});

// A ficha de contrato é o único lugar onde o jogador está olhando no instante em que
// acha que assinou. Se o backend recusar a vaga e a folha fechar mesmo assim, ele sai
// dali convencido de que tem equipe — e só descobre no fim da janela, jogado num time
// que nunca escolheu. Foi exatamente o que aconteceu no save "TEST 3".
describe("PreSeasonView — assinatura recusada", () => {
  const oferta = {
    seat_id: "T005#Numero2",
    team_id: "T005",
    team_name: "Apex Academy Racing",
    team_color: "#58a6ff",
    category: "mazda_amador",
    category_label: "Mazda Cup",
    category_tier: 1,
    role: "N2",
    salary: 90000,
    car_performance_rating: 70,
    offer_duration: 1,
  };

  function montarEstado(advanceMarketWeek) {
    return {
      careerId: "career-1",
      preseasonState: {
        current_week: 3,
        total_weeks: 9,
        signings_start_week: 3,
        is_complete: false,
        current_display_date: "2026-03-07",
        player_has_team: false,
      },
      preseasonWeeks: [],
      lastMarketWeekResult: null,
      playerProposals: [],
      preseasonFreeAgents: [],
      transferWindow: { player_offers: [oferta], player_tier: 1, player_name: "TEST 3" },
      isAdvancingWeek: false,
      isRespondingProposal: false,
      advanceMarketWeek,
      respondToProposal: vi.fn(),
      finalizePreseason: vi.fn(),
      playerTeam: null,
    };
  }

  async function abrirEAssinar() {
    fireEvent.click(await screen.findByText("Ver ofertas (1)"));
    fireEvent.click(await screen.findByText("Ver contrato"));
    fireEvent.click(await screen.findByText("Assinar contrato"));
    // A assinatura é escrita por ~1,55 s antes de a oferta ir ao backend.
    await vi.advanceTimersByTimeAsync(1600);
  }

  beforeEach(() => {
    pinLegacyLayout();
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("mantem a folha aberta e mostra o motivo quando o backend recusa", async () => {
    mockState = montarEstado(vi.fn().mockRejectedValue("Vaga 'T005#Numero2' nao encontrada."));
    render(<PreSeasonView />);

    await abrirEAssinar();

    expect(await screen.findByTestId("contract-sign-error")).toHaveTextContent(
      "Vaga 'T005#Numero2' nao encontrada.",
    );
    expect(screen.getByText("Assinar contrato")).toBeInTheDocument();
  });

  it("fecha a folha quando a assinatura passa", async () => {
    mockState = montarEstado(vi.fn().mockResolvedValue({}));
    render(<PreSeasonView />);

    await abrirEAssinar();

    await waitFor(() => expect(screen.queryByTestId("contract-sign-error")).toBeNull());
    expect(screen.queryByText("Assinar contrato")).toBeNull();
  });
});
