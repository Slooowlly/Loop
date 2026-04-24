import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import StandingsTab from "./StandingsTab";

let mockState = {};

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

function specialTeam({
  id,
  nome,
  classe,
  pontos,
  vitorias = 0,
  piloto1,
  piloto2,
  cor = "#bc8cff",
}) {
  return {
    id,
    nome,
    nome_curto: id,
    cor_primaria: cor,
    classe,
    pontos,
    vitorias,
    posicao: 1,
    piloto_1_nome: piloto1,
    piloto_2_nome: piloto2,
  };
}

describe("StandingsTab", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (command) => {
      if (command === "get_previous_champions") {
        return { driver_champion_id: null, constructor_champions: [] };
      }
      return [];
    });

    mockState = {
      careerId: "career-1",
      playerTeam: {
        categoria: "production_challenger",
      },
      season: {
        ano: 2025,
        rodada_atual: 8,
        total_rodadas: 8,
        fase: "BlocoEspecial",
      },
    };
  });

  it("reloads standings when the season phase changes after skipping the special block", async () => {
    const { rerender } = render(<StandingsTab />);

    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(3));

    mockState = {
      ...mockState,
      season: {
        ...mockState.season,
        fase: "PosEspecial",
      },
    };
    rerender(<StandingsTab />);

    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(6));
    expect(invoke).toHaveBeenLastCalledWith("get_previous_champions", {
      careerId: "career-1",
      category: "production_challenger",
    });
  });

  it("forces production standings during the special block for production-ladder drivers", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        categoria: "bmw_m2",
      },
    };

    render(<StandingsTab />);

    await waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith("get_previous_champions", {
        careerId: "career-1",
        category: "production_challenger",
      }),
    );
  });

  it("returns to endurance automatically when the user tries to change categories during the special block", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        categoria: "gt4",
      },
    };

    render(<StandingsTab />);

    await waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith("get_previous_champions", {
        careerId: "career-1",
        category: "endurance",
      }),
    );

    fireEvent.click(screen.getByTitle("Categoria inferior"));

    await waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith("get_previous_champions", {
        careerId: "career-1",
        category: "endurance",
      }),
    );
  });

  it("groups special driver and team standings by car class", async () => {
    invoke.mockImplementation(async (command) => {
      if (command === "get_drivers_by_category") {
        return [
          {
            id: "D1",
            nome: "Bianca Rossi",
            nacionalidade: "it",
            idade: 24,
            equipe_id: "TBMW",
            equipe_nome: "BMW Works",
            equipe_nome_curto: "BMW",
            equipe_cor: "#bc8cff",
            classe: "bmw",
            pontos: 88,
            vitorias: 3,
            podios: 4,
            posicao_campeonato: 1,
            results: [{ position: 1, is_dnf: false }],
          },
          {
            id: "D2",
            nome: "Taro Sato",
            nacionalidade: "jp",
            idade: 22,
            equipe_id: "TTOY",
            equipe_nome: "Toyota Spirit",
            equipe_nome_curto: "TOY",
            equipe_cor: "#f2cc60",
            classe: "toyota",
            pontos: 74,
            vitorias: 2,
            podios: 4,
            posicao_campeonato: 2,
            results: [{ position: 1, is_dnf: false }],
          },
          {
            id: "D3",
            nome: "Marta Vega",
            nacionalidade: "es",
            idade: 21,
            equipe_id: "TMAZ",
            equipe_nome: "Mazda Club",
            equipe_nome_curto: "MAZ",
            equipe_cor: "#c8102e",
            classe: "mazda",
            pontos: 66,
            vitorias: 1,
            podios: 3,
            posicao_campeonato: 3,
            results: [{ position: 1, is_dnf: false }],
          },
        ];
      }
      if (command === "get_teams_standings") {
        return [
          specialTeam({
            id: "TBMW",
            nome: "BMW Works",
            classe: "bmw",
            pontos: 120,
            vitorias: 4,
            piloto1: "Bianca Rossi",
            piloto2: "Luca Neri",
          }),
          specialTeam({
            id: "TBM2",
            nome: "BMW Junior",
            classe: "bmw",
            pontos: 110,
            piloto1: "Ana Longname-Silva",
            piloto2: "Carlo Verylongname",
          }),
          specialTeam({
            id: "TBM3",
            nome: "BMW Academy",
            classe: "bmw",
            pontos: 100,
            piloto1: "Nina Park",
            piloto2: "Otto Klein",
          }),
          specialTeam({
            id: "TBM4",
            nome: "BMW North",
            classe: "bmw",
            pontos: 90,
            piloto1: "Iris Blue",
            piloto2: "Theo Gray",
          }),
          specialTeam({
            id: "TBM5",
            nome: "BMW South",
            classe: "bmw",
            pontos: 80,
            piloto1: "Maya Sun",
            piloto2: null,
          }),
          {
            id: "TTOY",
            nome: "Toyota Spirit",
            nome_curto: "TOY",
            cor_primaria: "#f2cc60",
            classe: "toyota",
            pontos: 108,
            vitorias: 3,
            posicao: 2,
            piloto_1_nome: "Taro Sato",
            piloto_2_nome: "Aiko Tanaka",
          },
          {
            id: "TMAZ",
            nome: "Mazda Club",
            nome_curto: "MAZ",
            cor_primaria: "#c8102e",
            classe: "mazda",
            pontos: 96,
            vitorias: 2,
            posicao: 3,
            piloto_1_nome: "Marta Vega",
            piloto_2_nome: "Diego Sol",
          },
        ];
      }
      if (command === "get_previous_champions") {
        return { driver_champion_id: null, constructor_champions: [] };
      }
      return [];
    });

    render(<StandingsTab />);

    await screen.findByText("Bianca Rossi");
    const driverTable = screen.getByRole("table");
    expect(within(driverTable).getByText("BMW M2")).toBeInTheDocument();
    expect(within(driverTable).getByText("Toyota GR86")).toBeInTheDocument();
    expect(within(driverTable).getByText("Mazda MX-5")).toBeInTheDocument();
    expect(within(driverTable).getByText("BMW M2").closest("div")).toHaveClass("sticky", "left-0", "justify-center");
    expect(within(driverTable).getByText("BMW M2").closest("div")).not.toHaveClass("rounded-xl", "border");
    expect(within(driverTable).getByText("BMW M2")).toHaveClass("text-[17px]", "text-center");
    expect(within(driverTable).queryByText(/inscrito/i)).not.toBeInTheDocument();
    expect(within(screen.getByText("Bianca Rossi").closest("tr")).getByText("1")).toBeInTheDocument();
    expect(within(screen.getByText("Taro Sato").closest("tr")).getByText("1")).toBeInTheDocument();
    expect(within(screen.getByText("Marta Vega").closest("tr")).getByText("1")).toBeInTheDocument();

    expect(screen.getAllByText("BMW M2")).toHaveLength(2);
    expect(screen.getAllByText("Toyota GR86")).toHaveLength(2);
    expect(screen.getAllByText("Mazda MX-5")).toHaveLength(2);
    expect(screen.getByText("Bianca Rossi / Luca Neri")).toHaveClass("whitespace-nowrap");
    expect(screen.getByText("Maya Sun / -")).toHaveClass("whitespace-nowrap");
    expect(screen.queryByText(/Ã/)).not.toBeInTheDocument();
    expect(screen.queryByText("REBAIXAMENTO ↓")).not.toBeInTheDocument();
    expect(screen.getByText("BMW Academy").closest("[data-relegation-zone]")).toHaveAttribute(
      "data-relegation-zone",
      "true",
    );
    expect(screen.getByText("BMW Works").closest("[data-relegation-zone]")).toBeNull();
  });

  it("keeps normal team driver names readable when a seat has no assigned driver", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        categoria: "gt4",
      },
    };

    invoke.mockImplementation(async (command) => {
      if (command === "get_teams_standings") {
        return [
          {
            id: "GT4A",
            nome: "GT4 Atlas",
            nome_curto: "ATL",
            cor_primaria: "#58a6ff",
            pontos: 42,
            vitorias: 1,
            posicao: 1,
            piloto_1_nome: "Alex Stone",
            piloto_2_nome: null,
          },
        ];
      }
      if (command === "get_previous_champions") {
        return { driver_champion_id: null, constructor_champions: [] };
      }
      return [];
    });

    render(<StandingsTab />);

    await screen.findByText("GT4 Atlas");
    const driverLine = screen.getByText("Alex Stone / -");
    expect(driverLine).toHaveClass("whitespace-nowrap");
    expect(driverLine).toHaveAttribute("title", "Alex Stone / -");
    expect(screen.queryByText(/Ã/)).not.toBeInTheDocument();
  });

  it("does not show relegation zones for BMW, GT4 and GT3 standings", async () => {
    const categories = [
      { category: "bmw_m2", teamName: "BMW Atlas" },
      { category: "gt4", teamName: "GT4 Atlas" },
      { category: "gt3", teamName: "GT3 Atlas" },
    ];

    for (const { category, teamName } of categories) {
      mockState = {
        ...mockState,
        playerTeam: {
          categoria: category,
        },
        season: {
          ano: 2025,
          rodada_atual: 4,
          total_rodadas: 8,
          fase: "BlocoRegular",
        },
      };

      invoke.mockImplementation(async (command) => {
        if (command === "get_drivers_by_category") {
          return [];
        }
        if (command === "get_teams_standings") {
          return [
            {
              id: `${category}-1`,
              nome: teamName,
              nome_curto: "A",
              cor_primaria: "#58a6ff",
              pontos: 70,
              vitorias: 3,
              posicao: 1,
              piloto_1_nome: "Driver A1",
              piloto_2_nome: "Driver A2",
            },
            {
              id: `${category}-2`,
              nome: `${teamName} B`,
              nome_curto: "B",
              cor_primaria: "#f2cc60",
              pontos: 60,
              vitorias: 2,
              posicao: 2,
              piloto_1_nome: "Driver B1",
              piloto_2_nome: "Driver B2",
            },
            {
              id: `${category}-3`,
              nome: `${teamName} C`,
              nome_curto: "C",
              cor_primaria: "#c8102e",
              pontos: 50,
              vitorias: 1,
              posicao: 3,
              piloto_1_nome: "Driver C1",
              piloto_2_nome: "Driver C2",
            },
            {
              id: `${category}-4`,
              nome: `${teamName} D`,
              nome_curto: "D",
              cor_primaria: "#7d8590",
              pontos: 40,
              vitorias: 0,
              posicao: 4,
              piloto_1_nome: "Driver D1",
              piloto_2_nome: "Driver D2",
            },
          ];
        }
        if (command === "get_previous_champions") {
          return { driver_champion_id: null, constructor_champions: [] };
        }
        return [];
      });

      const { unmount } = render(<StandingsTab />);

      await screen.findByText(teamName);
      expect(screen.queryByText("REBAIXAMENTO ↓")).not.toBeInTheDocument();

      unmount();
    }
  });

  it("shows team logos in driver and constructor standings", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        categoria: "gt3",
      },
      season: {
        ano: 2025,
        rodada_atual: 2,
        total_rodadas: 8,
        fase: "BlocoRegular",
      },
    };

    invoke.mockImplementation(async (command) => {
      if (command === "get_drivers_by_category") {
        return [
          {
            id: "D44",
            nome: "Lena Hart",
            nacionalidade: "de",
            idade: 27,
            equipe_id: "TAMG",
            equipe_nome: "Mercedes-AMG",
            equipe_nome_curto: "AMG",
            equipe_cor: "#00d2be",
            pontos: 25,
            vitorias: 1,
            podios: 1,
            posicao_campeonato: 1,
            results: [{ position: 1, is_dnf: false }],
          },
        ];
      }
      if (command === "get_teams_standings") {
        return [
          {
            id: "TAMG",
            nome: "Mercedes-AMG",
            nome_curto: "AMG",
            cor_primaria: "#00d2be",
            pontos: 25,
            vitorias: 1,
            posicao: 1,
            piloto_1_nome: "Lena Hart",
            piloto_2_nome: "Nico Voss",
          },
        ];
      }
      if (command === "get_previous_champions") {
        return { driver_champion_id: null, constructor_champions: [] };
      }
      return [];
    });

    render(<StandingsTab />);

    await screen.findByText("Lena Hart");
    const mercedesLogos = screen.getAllByAltText("Mercedes-AMG logo");
    expect(mercedesLogos).toHaveLength(2);
    expect(mercedesLogos[0]).toHaveAttribute("src", expect.stringContaining("TimesNormalized"));
    expect(mercedesLogos[0]).toHaveClass("object-contain");
    expect(mercedesLogos[0].parentElement.className).toContain("aspect-[3/2]");
    expect(mercedesLogos[0].parentElement).not.toHaveAttribute("data-logo-contrast-frame");
    expect(mercedesLogos[0].parentElement.className).not.toContain("bg-[linear-gradient");
    expect(mercedesLogos[0].parentElement.className).not.toContain("border");
  });

  it("shows team logos for previous BMW and GT3 team names stored in existing saves", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        categoria: "bmw_m2",
      },
      season: {
        ano: 2025,
        rodada_atual: 2,
        total_rodadas: 8,
        fase: "BlocoRegular",
      },
    };

    invoke.mockImplementation(async (command, args = {}) => {
      if (command === "get_drivers_by_category") {
        if (args.category === "gt3") {
          return [
            {
              id: "D51",
              nome: "Marco Rossi",
              nacionalidade: "it",
              idade: 29,
              equipe_id: "TFER",
              equipe_nome: "Ferrari AF Corse",
              equipe_nome_curto: "FAC",
              equipe_cor: "#dc0000",
              pontos: 18,
              vitorias: 0,
              podios: 1,
              posicao_campeonato: 2,
              results: [{ position: 2, is_dnf: false }],
            },
          ];
        }

        return [
          {
            id: "D12",
            nome: "Nina Weber",
            nacionalidade: "de",
            idade: 24,
            equipe_id: "TBRD",
            equipe_nome: "Bayern Racing Division",
            equipe_nome_curto: "BRD",
            equipe_cor: "#004e98",
            pontos: 25,
            vitorias: 1,
            podios: 1,
            posicao_campeonato: 1,
            results: [{ position: 1, is_dnf: false }],
          },
        ];
      }
      if (command === "get_teams_standings") {
        if (args.category === "gt3") {
          return [
            {
              id: "TFER",
              nome: "Ferrari AF Corse",
              nome_curto: "FAC",
              cor_primaria: "#dc0000",
              pontos: 18,
              vitorias: 0,
              posicao: 2,
              piloto_1_nome: "Marco Rossi",
              piloto_2_nome: "Luca Bianchi",
            },
          ];
        }

        return [
          {
            id: "TBRD",
            nome: "Bayern Racing Division",
            nome_curto: "BRD",
            cor_primaria: "#004e98",
            pontos: 25,
            vitorias: 1,
            posicao: 1,
            piloto_1_nome: "Nina Weber",
            piloto_2_nome: "Otto Klein",
          },
        ];
      }
      if (command === "get_previous_champions") {
        return { driver_champion_id: null, constructor_champions: [] };
      }
      return [];
    });

    const { unmount } = render(<StandingsTab />);

    await screen.findByText("Nina Weber");
    expect(screen.getAllByAltText("Bayern Racing Division logo")).toHaveLength(2);

    unmount();
    mockState = {
      ...mockState,
      playerTeam: {
        categoria: "gt3",
      },
    };
    render(<StandingsTab />);

    await screen.findByText("Marco Rossi");
    expect(screen.getAllByAltText("Ferrari AF Corse logo")).toHaveLength(2);
  });

  it("opens the team history drawer directly from the team standings on double click", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        id: "T001",
        categoria: "gt4",
        nome: "Aurora GT",
        cor_primaria: "#58a6ff",
        cash_balance: 6_500_000,
        financial_state: "healthy",
        piloto_1_nome: "Alex Stone",
        piloto_2_nome: "Cole Vega",
      },
      season: {
        ano: 2025,
        rodada_atual: 8,
        total_rodadas: 8,
        fase: "BlocoRegular",
      },
    };

    invoke.mockImplementation(async (command, args = {}) => {
      if (command === "get_drivers_by_category") {
        return [
          {
            id: "D1",
            nome: "Alex Stone",
            nacionalidade: "br",
            idade: 25,
            equipe_id: "T001",
            equipe_nome: "Aurora GT",
            equipe_nome_curto: "AUR",
            equipe_cor: "#58a6ff",
            pontos: 96,
            vitorias: 2,
            podios: 6,
            posicao_campeonato: 4,
            results: [{ position: 4, is_dnf: false }],
          },
        ];
      }
      if (command === "get_teams_standings") {
        return [
          {
            id: "T010",
            nome: "Falcon Motorsport",
            nome_curto: "FAL",
            cor_primaria: "#facc15",
            cash_balance: 132_565_957,
            car_performance: 9,
            car_build_profile: "power_intermediate",
            pontos: 188,
            vitorias: 7,
            posicao: 1,
            piloto_1_nome: "Nico Vale",
            piloto_2_nome: "Luca Berni",
          },
          {
            id: "T001",
            nome: "Aurora GT",
            nome_curto: "AUR",
            cor_primaria: "#58a6ff",
            cash_balance: 6_500_000,
            car_performance: 7,
            car_build_profile: "balanced",
            pontos: 96,
            vitorias: 2,
            posicao: 4,
            piloto_1_nome: "Alex Stone",
            piloto_2_nome: "Cole Vega",
          },
          {
            id: "T020",
            nome: "Vector Racing",
            nome_curto: "VEC",
            cor_primaria: "#22c55e",
            cash_balance: 1_000_000,
            car_performance: 10,
            car_build_profile: "handling_intermediate",
            pontos: 120,
            vitorias: 4,
            posicao: 2,
            piloto_1_nome: "Mila Costa",
            piloto_2_nome: "Teo Sanz",
          },
        ];
      }
      if (command === "get_team_history_dossier") {
        return {
          record_scope: "Grupo GT4",
          has_history: true,
          records: [
            { label: "Títulos", rank: "1º", value: "2" },
            { label: "Vitórias", rank: "1º", value: "12" },
          ],
          sport: {
            seasons: "4 Temporadas reais",
            current_streak: "2 Temporadas reais",
            best_streak: "3 Pódios consecutivos reais",
            podium_rate: "75%",
            win_rate: "25%",
          },
          timeline: [
            { year: "2024", text: "Subiu de patamar no grid real." },
          ],
          title_categories: [
            { category: "GT4", year: "2025", color: "#58a6ff" },
          ],
          category_path: [
            { category: "GT4", years: "2023-2025", detail: "Fase real atual.", color: "#58a6ff" },
          ],
          identity: {
            origin: "GT4 Origem Vector",
            current: "GT4 Atual Real",
            profile: "Especialista Real",
            summary: "Resumo real da Vector calculado no backend.",
            rival: {
              name: "Falcon Motorsport",
              current_category: "GT4",
              note: "20 disputas diretas reais contra Falcon Motorsport.",
            },
            symbol_driver: "Piloto Símbolo Vector",
            symbol_driver_detail: "20 corridas, 9 vitórias, 16 pódios pela equipe.",
          },
          management: {
            peak_cash: "R$ 8.800.000",
            worst_crisis: "Sem dívida real registrada",
            healthy_years: "3 Temporadas saudáveis reais",
            efficiency: "18,4 pts/R$ mi real",
            biggest_investment: "Nível 8 - pacote real",
            summary: "Gestão real da Vector calculada no backend.",
            peak_cash_detail: "Pico real da Vector vindo do backend.",
            worst_crisis_detail: "Crise real da Vector vinda do backend.",
            healthy_years_detail: "Temporadas saudáveis reais da Vector.",
            efficiency_detail: "Eficiência real da Vector.",
            investment_detail: "Investimento real da Vector.",
          },
        };
      }
      if (command === "get_previous_champions") {
        return { driver_champion_id: null, constructor_champions: [] };
      }
      return [];
    });

    render(<StandingsTab />);

    const vectorTeam = await screen.findByText("Vector Racing");
    fireEvent.doubleClick(vectorTeam);

    const drawer = await screen.findByRole("dialog", { name: /Vector Racing/i });
    expect(within(drawer).getByText(/Records históricos/i)).toBeInTheDocument();
    expect(within(drawer).getByRole("tab", { name: /Identidade/i })).toBeInTheDocument();
    expect(drawer).toHaveClass("left-0");
    expect(drawer).toHaveClass("border-r");
    expect(drawer).not.toHaveClass("right-0");
    expect(drawer).not.toHaveClass("border-l");

    fireEvent.click(within(drawer).getByRole("tab", { name: /Identidade/i }));

    expect(within(drawer).getByText("Especialista Real")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("get_team_history_dossier", {
      careerId: "career-1",
      teamId: "T020",
      category: "gt4",
    });
  });

  it("uses the selected standings category when opening a team history drawer from another category", async () => {
    mockState = {
      ...mockState,
      playerTeam: {
        id: "T001",
        categoria: "gt4",
        nome: "Aurora GT",
        cor_primaria: "#58a6ff",
        cash_balance: 6_500_000,
        financial_state: "healthy",
        piloto_1_nome: "Alex Stone",
        piloto_2_nome: "Cole Vega",
      },
      season: {
        ano: 2025,
        rodada_atual: 8,
        total_rodadas: 8,
        fase: "PosEspecial",
      },
    };

    invoke.mockImplementation(async (command, args = {}) => {
      if (command === "get_drivers_by_category") {
        if (args.category === "gt3") {
          return [
            {
              id: "D9",
              nome: "Lena Hart",
              nacionalidade: "de",
              idade: 27,
              equipe_id: "T900",
              equipe_nome: "GT3 Titan",
              equipe_nome_curto: "TTN",
              equipe_cor: "#f97316",
              pontos: 121,
              vitorias: 4,
              podios: 6,
              posicao_campeonato: 1,
              results: [{ position: 1, is_dnf: false }],
            },
          ];
        }

        return [
          {
            id: "D1",
            nome: "Alex Stone",
            nacionalidade: "br",
            idade: 25,
            equipe_id: "T001",
            equipe_nome: "Aurora GT",
            equipe_nome_curto: "AUR",
            equipe_cor: "#58a6ff",
            pontos: 96,
            vitorias: 2,
            podios: 6,
            posicao_campeonato: 4,
            results: [{ position: 4, is_dnf: false }],
          },
        ];
      }

      if (command === "get_teams_standings") {
        if (args.category === "gt3") {
          return [
            {
              id: "T900",
              nome: "GT3 Titan",
              nome_curto: "TTN",
              cor_primaria: "#f97316",
              cash_balance: 18_000_000,
              car_performance: 10,
              car_build_profile: "power_intermediate",
              pontos: 121,
              vitorias: 4,
              posicao: 1,
              piloto_1_nome: "Lena Hart",
              piloto_2_nome: "Marco Voss",
              categoria: "gt3",
            },
          ];
        }

        return [
          {
            id: "T010",
            nome: "GT4 Atlas",
            nome_curto: "ATL",
            cor_primaria: "#58a6ff",
            cash_balance: 8_000_000,
            car_performance: 8,
            car_build_profile: "balanced",
            pontos: 90,
            vitorias: 2,
            posicao: 1,
            piloto_1_nome: "Alex Stone",
            piloto_2_nome: "Cole Vega",
            categoria: "gt4",
          },
        ];
      }

      if (command === "get_team_history_dossier") {
        if (args.teamId === "T900" && args.category === "gt3") {
          return {
            record_scope: "Grupo GT3",
            has_history: true,
            records: [{ label: "Títulos", rank: "1º", value: "3" }],
            sport: {
              seasons: "5 Temporadas reais",
              current_streak: "3 Temporadas reais",
              best_streak: "4 vitórias consecutivas reais",
              podium_rate: "68%",
              win_rate: "31%",
            },
            identity: {
              origin: "GT4 Academy",
              current: "GT3 World Series",
              profile: "Potência de fábrica",
              summary: "Resumo real da Titan calculado no backend.",
              rival: {
                name: "Falcon Factory",
                current_category: "GT3",
                note: "Rivalidade real do backend.",
              },
              symbol_driver: "Lena Hart",
              symbol_driver_detail: "Piloto símbolo real do backend.",
            },
            management: {
              peak_cash: "R$ 22.000.000",
              worst_crisis: "Crise de 2023",
              healthy_years: "4 temporadas saudáveis",
              efficiency: "19 pts/R$ mi",
              biggest_investment: "Pacote GT3 2025",
              summary: "Gestão real do backend.",
            },
            category_path: [
              { category: "GT4", years: "2021-2023", detail: "Base", color: "#58a6ff" },
              { category: "GT3", years: "2024-2025", detail: "Topo atual", color: "#f97316" },
            ],
          };
        }

        return { has_history: false };
      }

      if (command === "get_previous_champions") {
        return { driver_champion_id: null, constructor_champions: [] };
      }

      return [];
    });

    render(<StandingsTab />);

    await screen.findByText("GT4 Atlas");
    fireEvent.click(screen.getByTitle("Categoria superior"));

    const gt3Team = await screen.findByText("GT3 Titan");
    fireEvent.doubleClick(gt3Team);

    const drawer = await screen.findByRole("dialog", { name: /GT3 Titan/i });
    fireEvent.click(within(drawer).getByRole("tab", { name: /Identidade/i }));

    expect(await within(drawer).findByText("Resumo real da Titan calculado no backend.")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("get_team_history_dossier", {
      careerId: "career-1",
      teamId: "T900",
      category: "gt3",
    });
  });

  it("explains special standings before the special competition has results", async () => {
    mockState = {
      ...mockState,
      season: {
        ...mockState.season,
        fase: "BlocoRegular",
      },
    };

    invoke.mockImplementation(async (command) => {
      if (command === "get_teams_standings") {
        return [
          specialTeam({
            id: "TBMW",
            nome: "BMW Works",
            classe: "bmw",
            pontos: 0,
            piloto1: null,
            piloto2: null,
          }),
        ];
      }
      if (command === "get_previous_champions") {
        return { driver_champion_id: null, constructor_champions: [] };
      }
      return [];
    });

    render(<StandingsTab />);

    await screen.findByText("Competição especial ainda não aconteceu");
    expect(screen.getByText(/acontece depois da temporada regular/i)).toBeInTheDocument();
    expect(screen.queryByText("BMW Works")).not.toBeInTheDocument();
  });
});
