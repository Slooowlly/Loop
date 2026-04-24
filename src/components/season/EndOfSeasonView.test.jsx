import { fireEvent, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import EndOfSeasonView from "./EndOfSeasonView";

const mockEnterPreseason = vi.fn();

let mockState = {};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

describe("EndOfSeasonView", () => {
  beforeEach(() => {
    mockEnterPreseason.mockReset();
    mockState = {
      careerId: "career-1",
      enterPreseason: mockEnterPreseason,
      isEnteringPreseason: false,
      endOfSeasonResult: {
        growth_reports: [
          {
            driver_id: "DRV001",
            driver_name: "Ana Promovida",
            overall_delta: 3,
            changes: [{ attribute: "skill", delta: 2 }],
          },
          {
            driver_id: "DRV002",
            driver_name: "Bruno Estavel",
            overall_delta: 1,
            changes: [{ attribute: "consistencia", delta: 1 }],
          },
          {
            driver_id: "DRV003",
            driver_name: "Caio Rebaixado",
            overall_delta: -1,
            changes: [{ attribute: "mentalidade", delta: -1 }],
          },
          {
            driver_id: "DRV004",
            driver_name: "Diego Rookie",
            overall_delta: 1,
            changes: [{ attribute: "foco", delta: 1 }],
          },
        ],
        motivation_reports: [],
        retirements: [],
        rookies_generated: [],
        new_season_id: "season-2",
        new_year: 2027,
        licenses_earned: [
          {
            driver_id: "DRV001",
            driver_name: "Ana Promovida",
            license_level: 4,
            category: "gt3",
          },
        ],
        promotion_result: {
          movements: [
            {
              team_id: "TEAM001",
              team_name: "Equipe GT",
              from_category: "gt4",
              to_category: "gt3",
              movement_type: "Promocao",
              reason: "Subiu no campeonato",
            },
          ],
          pilot_effects: [
            {
              driver_id: "DRV003",
              driver_name: "Caio Rebaixado",
              team_id: "TEAM001",
              effect: "FreedNoLicense",
              reason: "Sem licenca para a nova categoria",
            },
          ],
          attribute_deltas: [],
          errors: [],
        },
        preseason_initialized: false,
        preseason_total_weeks: 3,
      },
    };

    invoke.mockImplementation(async (command, payload) => {
      if (command !== "get_driver_detail") return null;

      const categories = {
        DRV001: "gt3",
        DRV002: "gt4",
        DRV003: "gt4",
        DRV004: "mazda_rookie",
      };

      return {
        trajetoria: {
          categoria_atual: categories[payload.driverId],
        },
      };
    });
  });

  it("merges evolution into the pilot licenses tab grouped by final license", async () => {
    render(<EndOfSeasonView />);

    expect(screen.queryByText(/evolução dos pilotos/i)).not.toBeInTheDocument();
    expect(screen.getAllByText(/licenças de pilotos/i).length).toBeGreaterThan(0);

    const superEliteButton = await screen.findByRole("button", { name: /super elite/i });
    const eliteButton = (await screen.findAllByRole("button")).find((button) => (
      (button.textContent || "").includes("Elite") && !(button.textContent || "").includes("Super Elite")
    ));

    expect(superEliteButton).toBeInTheDocument();
    expect(eliteButton).toBeDefined();
    expect(screen.queryByText("Bruno Estavel")).not.toBeInTheDocument();

    fireEvent.click(eliteButton);

    expect((await screen.findAllByText(/não alterou/i)).length).toBeGreaterThan(0);
    expect(screen.getByText("Bruno Estavel")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Bruno Estavel"));

    expect(screen.getByText("+1")).toBeInTheDocument();

    const amadoraButton = screen.getByRole("button", { name: /amadora/i });
    fireEvent.click(amadoraButton);

    expect(screen.getByText(/manteve a licença rookie para a próxima temporada/i)).toBeInTheDocument();

    fireEvent.click(eliteButton);

    expect(screen.queryByText("Bruno Estavel")).not.toBeInTheDocument();
  });

  it("shows feedback while the preseason market is opening", () => {
    mockState = {
      ...mockState,
      isEnteringPreseason: true,
    };

    render(<EndOfSeasonView />);

    expect(screen.getByText(/abrindo mercado de transferências/i)).toBeInTheDocument();
    expect(screen.getByText(/carregando equipes, propostas e pilotos disponíveis/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /aguarde/i })).toBeDisabled();
  });

  it("shows team logos in the team movements tab", () => {
    mockState = {
      ...mockState,
      endOfSeasonResult: {
        ...mockState.endOfSeasonResult,
        promotion_result: {
          ...mockState.endOfSeasonResult.promotion_result,
          movements: [
            {
              team_id: "TEAM-AMG",
              team_name: "Mercedes-AMG",
              from_category: "gt4",
              to_category: "gt3",
              movement_type: "Promocao",
              reason: "Subiu no campeonato",
            },
          ],
        },
      },
    };

    render(<EndOfSeasonView />);

    fireEvent.click(screen.getByRole("button", { name: /movimenta/i }));

    const logo = screen.getByAltText("Mercedes-AMG logo");
    expect(logo).toHaveAttribute("src", expect.stringContaining("TimesNormalized"));
    expect(logo).toHaveClass("object-contain");
  });
});
