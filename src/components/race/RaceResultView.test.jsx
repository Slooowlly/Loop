import { render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import RaceResultView from "./RaceResultView";

let mockState = {};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

describe("RaceResultView", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue([
      {
        id: "drv-1",
        nome: "M. Costa",
        equipe_nome: "Mercedes-AMG",
        equipe_cor: "#ff7b72",
      },
      {
        id: "drv-2",
        nome: "R. Silva",
        equipe_nome: "Equipe Aurora",
        equipe_cor: "#58a6ff",
      },
    ]);

    mockState = {
      careerId: "career-1",
      playerTeam: {
        id: "team-1",
        nome: "Equipe Aurora",
        categoria: "gt3",
      },
      otherCategoriesResult: {
        total_races_simulated: 0,
        categories_simulated: [],
      },
    };
  });

  it("renders the team logo in the official race table", async () => {
    render(
      <RaceResultView
        onDismiss={vi.fn()}
        result={{
          track_name: "Spa",
          weather: "Wet",
          total_laps: 24,
          qualifying_results: [
            {
              pilot_id: "drv-1",
              pilot_name: "M. Costa",
              is_pole: true,
              best_lap_time_ms: 120345,
            },
          ],
          race_results: [
            {
              pilot_id: "drv-1",
              pilot_name: "M. Costa",
              team_name: "Mercedes-AMG",
              finish_position: 1,
              positions_gained: 2,
              total_race_time_ms: 3600123,
              gap_to_winner_ms: 0,
              best_lap_time_ms: 120111,
              has_fastest_lap: true,
              is_jogador: false,
              is_dnf: false,
              grid_position: 3,
            },
            {
              pilot_id: "drv-player",
              pilot_name: "R. Silva",
              team_name: "Equipe Aurora",
              finish_position: 4,
              positions_gained: -1,
              total_race_time_ms: 3609456,
              gap_to_winner_ms: 9333,
              best_lap_time_ms: 121000,
              has_fastest_lap: false,
              is_jogador: true,
              is_dnf: false,
              grid_position: 3,
            },
          ],
        }}
      />,
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("get_drivers_by_category", {
        careerId: "career-1",
        category: "gt3",
      });
    });

    expect(screen.getByText(/tabela oficial da prova/i)).toBeInTheDocument();

    const officialTable = screen.getByRole("table");
    const mercedesRow = within(officialTable).getByText("Mercedes-AMG").closest("tr");
    expect(mercedesRow).not.toBeNull();
    expect(within(mercedesRow).getByAltText("Mercedes-AMG logo")).toBeInTheDocument();
  });

  it("shows larger category logos in a dedicated strip without individual cards", async () => {
    mockState = {
      ...mockState,
      otherCategoriesResult: {
        total_races_simulated: 9,
        categories_simulated: [
          {
            category_id: "production_challenger",
            category_name: "Production Challenger",
          },
          {
            category_id: "endurance",
            category_name: "Endurance Championship",
          },
        ],
      },
    };

    render(
      <RaceResultView
        onDismiss={vi.fn()}
        result={{
          track_name: "Spa",
          weather: "Wet",
          total_laps: 24,
          qualifying_results: [],
          race_results: [
            {
              pilot_id: "drv-1",
              pilot_name: "M. Costa",
              team_name: "Mercedes-AMG",
              finish_position: 1,
              positions_gained: 2,
              total_race_time_ms: 3600123,
              gap_to_winner_ms: 0,
              best_lap_time_ms: 120111,
              has_fastest_lap: false,
              is_jogador: false,
              is_dnf: false,
              grid_position: 1,
            },
          ],
        }}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("9 Corridas Processadas")).toBeInTheDocument();
    });

    const otherCategoriesCard = screen.getByText(/outras categorias/i).closest(".rounded-2xl");
    expect(otherCategoriesCard).not.toBeNull();

    const logoStrip = within(otherCategoriesCard).getByTestId("other-categories-logo-strip");
    const productionLogo = within(logoStrip).getByAltText("Production Challenger");
    const enduranceLogo = within(logoStrip).getByAltText("Endurance Championship");

    expect(productionLogo).toHaveAttribute(
      "src",
      "/utilities/categorias/recortadas/PRODUCTION.png",
    );
    expect(enduranceLogo).toHaveAttribute(
      "src",
      "/utilities/categorias/recortadas/ENDURANCE.png",
    );
    expect(productionLogo.className).toContain("h-full");
    expect(enduranceLogo.className).toContain("h-full");
    expect(productionLogo.parentElement?.className ?? "").toContain("h-24");
    expect(productionLogo.parentElement?.className ?? "").toContain("w-[320px]");
    expect(enduranceLogo.parentElement?.className ?? "").toContain("h-24");
    expect(enduranceLogo.parentElement?.className ?? "").toContain("w-[320px]");
    expect(productionLogo.parentElement?.className ?? "").not.toContain("rounded-xl");
    expect(productionLogo.parentElement?.className ?? "").not.toContain("border");
    expect(within(logoStrip).queryByText("Production Challenger")).not.toBeInTheDocument();
    expect(within(logoStrip).queryByText("Endurance Championship")).not.toBeInTheDocument();
  });

  it("crops the mx5 cup logo to hide the green artifact at the bottom", async () => {
    mockState = {
      ...mockState,
      otherCategoriesResult: {
        total_races_simulated: 2,
        categories_simulated: [
          {
            category_id: "mazda_amador",
            category_name: "Mazda MX-5 Championship",
          },
        ],
      },
    };

    render(
      <RaceResultView
        onDismiss={vi.fn()}
        result={{
          track_name: "Spa",
          weather: "Wet",
          total_laps: 24,
          qualifying_results: [],
          race_results: [
            {
              pilot_id: "drv-1",
              pilot_name: "M. Costa",
              team_name: "Mercedes-AMG",
              finish_position: 1,
              positions_gained: 2,
              total_race_time_ms: 3600123,
              gap_to_winner_ms: 0,
              best_lap_time_ms: 120111,
              has_fastest_lap: false,
              is_jogador: false,
              is_dnf: false,
              grid_position: 1,
            },
          ],
        }}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("2 Corridas Processadas")).toBeInTheDocument();
    });

    const logoStrip = screen.getByTestId("other-categories-logo-strip");
    const mazdaLogo = within(logoStrip).getByAltText("Mazda MX-5 Championship");

    expect(mazdaLogo).toHaveAttribute("src", "/utilities/categorias/recortadas/MX5%20CUP.png");
    expect(mazdaLogo.parentElement?.className ?? "").toContain("overflow-hidden");
    expect(mazdaLogo.style.clipPath).toContain("inset(");
    expect(mazdaLogo.style.transform).toBe("");
  });

  it("shows the LMP2 category logo in the other categories strip", async () => {
    mockState = {
      ...mockState,
      otherCategoriesResult: {
        total_races_simulated: 1,
        categories_simulated: [
          {
            category_id: "lmp2",
            category_name: "LMP2 Prototype Championship",
          },
        ],
      },
    };

    render(
      <RaceResultView
        onDismiss={vi.fn()}
        result={{
          track_name: "Spa",
          weather: "Wet",
          total_laps: 24,
          qualifying_results: [],
          race_results: [
            {
              pilot_id: "drv-1",
              pilot_name: "M. Costa",
              team_name: "Mercedes-AMG",
              finish_position: 1,
              positions_gained: 2,
              total_race_time_ms: 3600123,
              gap_to_winner_ms: 0,
              best_lap_time_ms: 120111,
              has_fastest_lap: false,
              is_jogador: false,
              is_dnf: false,
              grid_position: 1,
            },
          ],
        }}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("1 Corrida Processada")).toBeInTheDocument();
    });

    const logoStrip = screen.getByTestId("other-categories-logo-strip");
    const lmp2Logo = within(logoStrip).getByAltText("LMP2 Prototype Championship");

    expect(lmp2Logo).toHaveAttribute("src", "/utilities/categorias/recortadas/LMP2.png");
  });
});
