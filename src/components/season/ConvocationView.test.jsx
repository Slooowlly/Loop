import { fireEvent, render, screen, within } from "@testing-library/react";

import ConvocationView from "./ConvocationView";

const mockAcceptSpecialOfferForDay = vi.fn();
const mockAdvanceSpecialWindowDay = vi.fn();
const mockConfirmSpecialBlock = vi.fn();
const mockLoadSpecialWindowState = vi.fn();
let mockState = {};

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

describe("ConvocationView", () => {
  beforeEach(() => {
    mockAcceptSpecialOfferForDay.mockReset();
    mockAdvanceSpecialWindowDay.mockReset();
    mockConfirmSpecialBlock.mockReset();
    mockLoadSpecialWindowState.mockReset();

    mockState = {
      careerId: "career-1",
      season: {
        ano: 2026,
        fase: "JanelaConvocacao",
      },
      specialWindowState: {
        current_day: 3,
        total_days: 7,
        status: "Aberta",
        active_offer_id: "offer-1",
        is_finished: false,
        last_day_log: [
          {
            day: 2,
            event_type: "convocado",
            message: "R. Silva foi convocado para Solar GT4.",
            special_category: "endurance",
            class_name: "gt4",
            team_name: "Solar GT4",
            driver_name: "R. Silva",
            driver_origin_category: "gt4",
            driver_license_sigla: "SP",
            championship_position: 3,
          },
          {
            day: 2,
            event_type: "convocado",
            message: "L. Ramos foi convocado para Aurora Mazda.",
            special_category: "production_challenger",
            class_name: "mazda",
            team_name: "Aurora Mazda",
            driver_name: "L. Ramos",
            driver_origin_category: "mazda_amador",
            driver_license_sigla: "A",
            championship_position: 5,
          },
        ],
        eligible_candidates: [
          {
            driver_id: "drv-1",
            driver_name: "N. Prado",
            origin_category: "gt4",
            license_nivel: "Super Pro",
            license_sigla: "SP",
            desirability: 89,
            production_eligible: true,
            endurance_eligible: true,
            championship_position: 2,
            championship_total_drivers: 20,
          },
          {
            driver_id: "drv-2",
            driver_name: "L. Ramos",
            origin_category: "mazda_amador",
            license_nivel: "Amador",
            license_sigla: "A",
            desirability: 71,
            production_eligible: true,
            endurance_eligible: false,
            championship_position: 5,
            championship_total_drivers: 20,
          },
        ],
        player_offers: [
          {
            id: "offer-1",
            team_id: "end-1",
            team_name: "Solar GT4",
            special_category: "endurance",
            class_name: "gt4",
            papel: "Numero2",
            status: "AceitaAtiva",
            available_from_day: 2,
            is_available_today: true,
          },
        ],
        team_sections: [
          {
            category: "production_challenger",
            label: "Production",
            teams: [
              {
                id: "prod-1",
                nome: "Aurora Mazda",
                nome_curto: "Aurora",
                cor_primaria: "#fff",
                cor_secundaria: "#000",
                categoria: "production_challenger",
                classe: "mazda",
                piloto_1_nome: "L. Ramos",
                piloto_2_nome: null,
                piloto_1_new_badge_day: 3,
                piloto_2_new_badge_day: null,
              },
              {
                id: "prod-2",
                nome: "Vertex BMW",
                nome_curto: "Vertex",
                cor_primaria: "#fff",
                cor_secundaria: "#000",
                categoria: "production_challenger",
                classe: "bmw",
                piloto_1_nome: null,
                piloto_2_nome: null,
                piloto_1_new_badge_day: null,
                piloto_2_new_badge_day: null,
              },
            ],
          },
          {
            category: "endurance",
            label: "Endurance",
            teams: [
              {
                id: "end-1",
                nome: "Solar GT4",
                nome_curto: "Solar",
                cor_primaria: "#fff",
                cor_secundaria: "#000",
                categoria: "endurance",
                classe: "gt4",
                piloto_1_nome: "R. Silva",
                piloto_2_nome: null,
                piloto_1_new_badge_day: null,
                piloto_2_new_badge_day: null,
              },
            ],
          },
        ],
      },
      playerSpecialOffers: [
        {
          id: "offer-1",
          team_id: "end-1",
          team_name: "Solar GT4",
          special_category: "endurance",
          class_name: "gt4",
          papel: "Numero2",
          status: "AceitaAtiva",
          available_from_day: 2,
          is_available_today: true,
        },
      ],
      acceptedSpecialOffer: {
        id: "offer-1",
        team_id: "end-1",
        team_name: "Solar GT4",
        special_category: "endurance",
        class_name: "gt4",
        papel: "Numero2",
      },
      isConvocating: false,
      error: null,
      loadSpecialWindowState: mockLoadSpecialWindowState,
      acceptSpecialOfferForDay: mockAcceptSpecialOfferForDay,
      advanceSpecialWindowDay: mockAdvanceSpecialWindowDay,
      confirmSpecialBlock: mockConfirmSpecialBlock,
    };
  });

  it("renders the seven-day market grid, candidates, and player offers", () => {
    render(<ConvocationView />);

    expect(screen.getByText(/janela especial/i)).toBeInTheDocument();
    expect(screen.getByText(/mercado de convocações/i)).toBeInTheDocument();
    expect(screen.getByText("3/7")).toBeInTheDocument();
    expect(screen.getByText(/pilotos elegíveis/i)).toBeInTheDocument();
    expect(screen.getAllByText(/gt4 series/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/mazda cup/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/suas propostas/i)).toBeInTheDocument();
    expect(screen.getByText(/mapeamento das equipes/i)).toBeInTheDocument();
    expect(screen.getAllByText(/aurora mazda/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/vertex bmw/i)).toBeInTheDocument();
    expect(screen.getAllByText(/solar gt4/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/^MAZDA$/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/^BMW$/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/^GT4$/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/n\. prado/i)).toBeInTheDocument();
    expect(screen.getByText(/^2º$/i)).toBeInTheDocument();
    expect(screen.getAllByText(/^5º$/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/^SP$/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/^A$/i).length).toBeGreaterThan(0);
    expect(screen.queryByText(/2º\/20/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/carteira super pro/i)).not.toBeInTheDocument();
    expect(screen.getByText(/endurance - gt4/i)).toBeInTheDocument();
    expect(screen.getByText(/^new$/i)).toBeInTheDocument();
  });

  it("groups day-closing market movements by category and car class", () => {
    render(<ConvocationView />);

    expect(screen.getByText(/endurance - gt4/i)).toBeInTheDocument();
    expect(screen.getByText(/production - mazda/i)).toBeInTheDocument();
    expect(screen.getByText(/^3(?:º|º)$/i)).toBeInTheDocument();
    expect(screen.getAllByText(/^5(?:º|º)$/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/gt4 series/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/mazda cup/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/solar gt4/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/aurora mazda/i).length).toBeGreaterThan(0);
  });

  it("keeps day-closing movement rows compact inside the closing panel", () => {
    render(<ConvocationView />);

    const dailyLog = within(screen.getByTestId("daily-log-market"));

    expect(dailyLog.getByText(/endurance - gt4/i)).toBeInTheDocument();
    expect(dailyLog.getByText(/production - mazda/i)).toBeInTheDocument();
    expect(dailyLog.getByText(/^3/)).toBeInTheDocument();
    expect(dailyLog.getByText(/^5/)).toBeInTheDocument();
    expect(dailyLog.getByText(/r\. silva/i)).toBeInTheDocument();
    expect(dailyLog.getByText(/l\. ramos/i)).toBeInTheDocument();
    expect(dailyLog.queryByText(/gt4 series/i)).not.toBeInTheDocument();
    expect(dailyLog.queryByText(/mazda cup/i)).not.toBeInTheDocument();
    expect(dailyLog.queryByText(/solar gt4/i)).not.toBeInTheDocument();
    expect(dailyLog.queryByText(/aurora mazda/i)).not.toBeInTheDocument();
    expect(dailyLog.queryByText(/^SP$/i)).not.toBeInTheDocument();
    expect(dailyLog.queryByText(/^A$/i)).not.toBeInTheDocument();
  });

  it("lets the player choose an offer for the day or advance the market", () => {
    render(<ConvocationView />);

    fireEvent.click(screen.getByRole("button", { name: /escolher hoje/i }));
    expect(mockAcceptSpecialOfferForDay).toHaveBeenCalledWith("offer-1");

    fireEvent.click(screen.getByRole("button", { name: /avançar dia/i }));
    expect(mockAdvanceSpecialWindowDay).toHaveBeenCalledTimes(1);
  });

  it("shows NEW only on the day immediately after a pilot appears in the grid", () => {
    const { rerender } = render(<ConvocationView />);

    expect(screen.getByText(/^new$/i)).toBeInTheDocument();

    mockState = {
      ...mockState,
      specialWindowState: {
        ...mockState.specialWindowState,
        current_day: 4,
      },
    };

    rerender(<ConvocationView />);

    expect(screen.queryByText(/^new$/i)).not.toBeInTheDocument();
  });

  it("restores category logos and the previous team order inside production and endurance", () => {
    mockState = {
      ...mockState,
      specialWindowState: {
        ...mockState.specialWindowState,
        team_sections: [
          {
            category: "production_challenger",
            label: "Production",
            teams: [
              {
                id: "prod-mazda",
                nome: "Aurora Mazda",
                categoria: "production_challenger",
                classe: "mazda",
                piloto_1_nome: "L. Ramos",
                piloto_2_nome: null,
              },
              {
                id: "prod-toyota",
                nome: "Nova Toyota",
                categoria: "production_challenger",
                classe: "toyota",
                piloto_1_nome: "M. Sato",
                piloto_2_nome: null,
              },
              {
                id: "prod-bmw",
                nome: "Vertex BMW",
                categoria: "production_challenger",
                classe: "bmw",
                piloto_1_nome: null,
                piloto_2_nome: null,
              },
            ],
          },
          {
            category: "endurance",
            label: "Endurance",
            teams: [
              {
                id: "end-gt4",
                nome: "Solar GT4",
                categoria: "endurance",
                classe: "gt4",
                piloto_1_nome: "R. Silva",
                piloto_2_nome: null,
              },
              {
                id: "end-gt3",
                nome: "Nebula GT3",
                categoria: "endurance",
                classe: "gt3",
                piloto_1_nome: "P. Costa",
                piloto_2_nome: null,
              },
              {
                id: "end-lmp2",
                nome: "Proto LMP2",
                categoria: "endurance",
                classe: "lmp2",
                piloto_1_nome: "T. Nunes",
                piloto_2_nome: null,
              },
            ],
          },
        ],
      },
    };

    render(<ConvocationView />);

    expect(screen.getByAltText(/production/i)).toHaveAttribute("src", "/categorias/recortadas/PRODUCTION.png");
    expect(screen.getByAltText(/endurance/i)).toHaveAttribute("src", "/categorias/recortadas/ENDURANCE.png");

    const sectionTitles = screen.getAllByTestId("convocation-class-title").map((node) => node.textContent);
    expect(sectionTitles).toEqual(["BMW", "TOYOTA", "MAZDA", "LMP2", "GT3", "GT4"]);
  });

  it("centers the category logo, keeps the team count below it, and renders the logo much larger", () => {
    render(<ConvocationView />);

    const productionHeader = screen.getByTestId("convocation-category-header-production_challenger");
    const productionLogo = within(productionHeader).getByAltText(/production/i);
    const productionCount = within(productionHeader).getByTestId("convocation-category-count");

    expect(productionHeader.className).toContain("flex-col");
    expect(productionHeader.className).toContain("items-center");
    expect(productionLogo.className).toContain("h-48");
    expect(productionCount).toHaveTextContent("2 equipes");

    const headerChildren = Array.from(productionHeader.children);
    expect(headerChildren.indexOf(productionLogo)).toBeLessThan(headerChildren.indexOf(productionCount));
  });

  it("shows normalized team logos instead of initials in the special market team grid", () => {
    mockState = {
      ...mockState,
      specialWindowState: {
        ...mockState.specialWindowState,
        team_sections: [
          {
            category: "endurance",
            label: "Endurance",
            teams: [
              {
                id: "end-amg",
                nome: "Mercedes-AMG",
                nome_curto: "AMG",
                cor_primaria: "#00d2be",
                cor_secundaria: "#000",
                categoria: "endurance",
                classe: "gt3",
                piloto_1_nome: "Lena Hart",
                piloto_2_nome: null,
                piloto_1_new_badge_day: null,
                piloto_2_new_badge_day: null,
              },
            ],
          },
        ],
      },
    };

    render(<ConvocationView />);

    const logo = screen.getByAltText("Mercedes-AMG logo");
    expect(logo).toHaveAttribute("src", expect.stringContaining("TimesNormalized"));
    expect(logo).toHaveClass("object-contain");
    expect(screen.queryByText("ME")).not.toBeInTheDocument();
  });

  it("uses the new palette taken from the cropped category logos", () => {
    render(<ConvocationView />);

    const productionHeader = screen.getByTestId("convocation-category-header-production_challenger");
    const enduranceHeader = screen.getByTestId("convocation-category-header-endurance");
    const productionCount = within(productionHeader).getByTestId("convocation-category-count");
    const enduranceCount = within(enduranceHeader).getByTestId("convocation-category-count");

    expect(productionCount).toHaveStyle({ color: "#9030E0", borderColor: "#9030E055" });
    expect(enduranceCount).toHaveStyle({ color: "#30D010", borderColor: "#30D01055" });
  });
});
