import { render, screen } from "@testing-library/react";

import RaceTelemetryCockpit from "./RaceTelemetryCockpit";

let mockState = {};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve({ by_name: {}, player_color: null })),
}));

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

// Estratégia com uma parada e estratégia SEM nenhuma parada, lado a lado — é o
// caso que o painel precisa representar por inteiro.
const COM_PARADA = {
  car_idx: 1,
  pilot_name: "M. Costa",
  team_name: "Equipe Aurora",
  start_compound: "Dry",
  tire_changes: 1,
  wrong_tire: false,
  stints: [
    { from_lap: 1, compound: "Dry", changed: false, confidence: 1 },
    { from_lap: 8, compound: "Dry", changed: true, confidence: 0.65 },
  ],
  stops: [{ lap: 8, box_secs: 22.4, tire_change: true, track_wet: false }],
  summary: "Largou de seco · 1 troca",
};

const SEM_PARADA = {
  car_idx: 2,
  pilot_name: "R. Silva",
  team_name: "Nordeste Racing",
  start_compound: "Dry",
  tire_changes: 0,
  wrong_tire: false,
  stints: [{ from_lap: 1, compound: "Dry", changed: false, confidence: 0.9 }],
  stops: [],
  summary: "Não parou · largou e terminou de seco · corrida inteira sem trocar pneus",
};

const TELEMETRIA = {
  race_laps: 20,
  charts: {
    cars: [
      { idx: 1, name: "M. Costa", is_player: true, points: [{ lap: 1, gap: 0, position: 1 }] },
      { idx: 2, name: "R. Silva", is_player: false, points: [{ lap: 1, gap: 1.2, position: 2 }] },
    ],
    lap_times: [],
    car_lap_times: [],
    yellow_laps: [],
    rival_gap: [],
    rival_name: "",
  },
  tire_strategies: [COM_PARADA, SEM_PARADA],
  player_tire: COM_PARADA,
};

beforeEach(() => {
  mockState = { careerId: null, lastRaceId: null };
});

describe("RaceTelemetryCockpit — estratégia de pit", () => {
  it("mostra também quem não fez nenhuma parada", () => {
    render(<RaceTelemetryCockpit telemetry={TELEMETRIA} />);
    expect(screen.getAllByText("R. Silva").length).toBeGreaterThan(0);
    expect(screen.getByText("0 paradas")).toBeInTheDocument();
    expect(screen.getByText("Sem parada · Seco do começo ao fim")).toBeInTheDocument();
  });

  it("o painel aparece mesmo numa corrida em que ninguém parou", () => {
    render(<RaceTelemetryCockpit telemetry={{ ...TELEMETRIA, tire_strategies: [SEM_PARADA], player_tire: SEM_PARADA }} />);
    expect(screen.getByText("0 paradas")).toBeInTheDocument();
    expect(screen.queryByText("Sem dados de estratégia desta corrida.")).not.toBeInTheDocument();
  });
});
