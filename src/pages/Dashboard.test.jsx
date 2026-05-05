import { fireEvent, render, screen } from "@testing-library/react";

import Dashboard from "./Dashboard";

let mockState = {};

vi.mock("../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("../components/layout/MainLayout", () => ({
  default: ({ children, activeTab, hideHeader = false, onTabChange }) => (
    <div
      data-testid="main-layout"
      data-active-tab={activeTab}
      data-hide-header={hideHeader ? "true" : "false"}
    >
      <button type="button" onClick={() => onTabChange?.("calendar")}>
        Ir para calendario
      </button>
      {children}
    </div>
  ),
}));

vi.mock("../components/race/RaceResultView", () => ({
  default: () => <div>Classificação final</div>,
}));

vi.mock("./tabs/NextRaceTab", () => ({
  default: () => <div>Briefing pre-corrida</div>,
}));

vi.mock("./tabs/CalendarTab", () => ({
  default: ({ activeTab, raceArrivalFeedbackActive = false }) => (
    <div
      data-testid="calendar-tab-prop"
      data-race-arrival-feedback-active={raceArrivalFeedbackActive ? "true" : "false"}
    >
      {activeTab ?? "sem-prop"}
    </div>
  ),
}));

vi.mock("./tabs/StandingsTab", () => ({
  default: ({ onOpenGlobalDrivers }) => (
    <div>
      <div>Classificacao de pilotos</div>
      <button type="button" onClick={() => onOpenGlobalDrivers?.("D001")}>
        Abrir panorama
      </button>
    </div>
  ),
}));

vi.mock("./tabs/GlobalDriversTab", () => ({
  default: ({ selectedDriverId, onBack }) => (
    <div>
      <div>Panorama {selectedDriverId}</div>
      <button type="button" onClick={onBack}>
        Voltar
      </button>
    </div>
  ),
}));

vi.mock("../components/season/ConvocationView", () => ({
  default: () => <div>Janela de convocacao</div>,
}));

describe("Dashboard", () => {
  beforeEach(() => {
    mockState = {
      isLoaded: true,
      showRaceBriefing: true,
      showResult: false,
      lastRaceResult: null,
      dismissResult: vi.fn(),
      showEndOfSeason: false,
      endOfSeasonResult: null,
      showPreseason: false,
      showConvocation: false,
    };
  });

  it("renders the pre-race briefing before the regular tabs", () => {
    render(<Dashboard />);

    expect(screen.getByTestId("main-layout")).toBeInTheDocument();
    expect(screen.getByText("Briefing pre-corrida")).toBeInTheDocument();
  });

  it("starts on the drivers tab when loading a save", () => {
    mockState.showRaceBriefing = false;

    render(<Dashboard />);

    expect(screen.getByTestId("main-layout")).toHaveAttribute("data-active-tab", "standings");
    expect(screen.getByText("Classificacao de pilotos")).toBeInTheDocument();
  });

  it("opens the hidden global drivers tab from the standings callback and returns", () => {
    mockState.showRaceBriefing = false;

    render(<Dashboard />);

    fireEvent.click(screen.getByRole("button", { name: /Abrir panorama/i }));

    expect(screen.getByText("Panorama D001")).toBeInTheDocument();
    expect(screen.getByTestId("main-layout")).toHaveAttribute("data-active-tab", "global-drivers");

    fireEvent.click(screen.getByRole("button", { name: /^Voltar$/i }));

    expect(screen.getByTestId("main-layout")).toHaveAttribute("data-active-tab", "standings");
    expect(screen.getByText("Classificacao de pilotos")).toBeInTheDocument();
  });

  it("hides the main header while showing the final classification screen", () => {
    mockState.showRaceBriefing = false;
    mockState.showResult = true;
    mockState.lastRaceResult = { track_name: "Interlagos", race_results: [] };

    render(<Dashboard />);

    expect(screen.getByTestId("main-layout")).toHaveAttribute("data-hide-header", "true");
    expect(screen.getByText("Classificação final")).toBeInTheDocument();
  });

  it("renders the convocation screen before the regular tabs", () => {
    mockState.showRaceBriefing = false;
    mockState.showConvocation = true;

    render(<Dashboard />);

    expect(screen.getByText("Janela de convocacao")).toBeInTheDocument();
  });

  it("passes the active calendar tab to CalendarTab so current-day UI can render", () => {
    mockState.showRaceBriefing = false;

    render(<Dashboard />);

    fireEvent.click(screen.getByRole("button", { name: /Ir para calendario/i }));

    expect(screen.getByTestId("main-layout")).toHaveAttribute("data-active-tab", "calendar");
    expect(screen.getByTestId("calendar-tab-prop")).toHaveTextContent("calendar");
  });

  it("briefly holds the race briefing when the calendar tab was active and exposes the race-arrival feedback state", async () => {
    vi.useFakeTimers();
    mockState.showRaceBriefing = false;

    const { rerender } = render(<Dashboard />);

    fireEvent.click(screen.getByRole("button", { name: /Ir para calendario/i }));

    mockState.showRaceBriefing = true;
    rerender(<Dashboard />);

    expect(screen.queryByText("Briefing pre-corrida")).not.toBeInTheDocument();
    expect(screen.getByTestId("main-layout")).toHaveAttribute("data-active-tab", "calendar");
    expect(screen.getByTestId("calendar-tab-prop")).toHaveAttribute("data-race-arrival-feedback-active", "true");

    await vi.advanceTimersByTimeAsync(279);

    expect(screen.queryByText("Briefing pre-corrida")).not.toBeInTheDocument();

    await vi.advanceTimersByTimeAsync(1);

    expect(screen.getByText("Briefing pre-corrida")).toBeInTheDocument();
    vi.useRealTimers();
  });
});
