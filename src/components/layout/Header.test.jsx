import { fireEvent, render, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import Header from "./Header";

const mockSimulateRace = vi.fn();
const mockStartCalendarAdvance = vi.fn();
const mockCloseRaceBriefing = vi.fn();
const mockAdvanceSeason = vi.fn();
const mockSkipAllPendingRaces = vi.fn();
const mockRunConvocationWindow = vi.fn();
const mockFinishSpecialBlock = vi.fn();

let mockState = {};

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("Header", () => {
  beforeEach(() => {
    mockSimulateRace.mockReset();
    mockStartCalendarAdvance.mockReset();
    mockCloseRaceBriefing.mockReset();
    mockAdvanceSeason.mockReset();
    mockSkipAllPendingRaces.mockReset();
    mockRunConvocationWindow.mockReset();
    mockFinishSpecialBlock.mockReset();
    invoke.mockReset();
    invoke.mockResolvedValue([]);
    mockState = {
      careerId: "career-1",
      playerTeam: {
        nome: "Equipe Teste",
        cor_primaria: "#58a6ff",
        categoria: "mazda_rookie",
      },
      season: {
        numero: 1,
        ano: 2026,
        total_rodadas: 12,
        rodada_atual: 3,
      },
      nextRace: {
        id: "race-1",
        track_name: "Interlagos",
        rodada: 3,
        display_date: "2026-03-25",
        horario: "14:00",
        clima: "Clear",
        temperatura: 27,
      },
      temporalSummary: {
        current_display_date: "2026-03-18",
        next_event_display_date: "2026-03-25",
        days_until_next_event: 7,
        weeks_until_next_event: 1,
      },
      showRaceBriefing: false,
      isCalendarAdvancing: false,
      isAdvancing: false,
      isSimulating: false,
      simulateRace: mockSimulateRace,
      startCalendarAdvance: mockStartCalendarAdvance,
      advanceSeason: mockAdvanceSeason,
      skipAllPendingRaces: mockSkipAllPendingRaces,
      runConvocationWindow: mockRunConvocationWindow,
      finishSpecialBlock: mockFinishSpecialBlock,
      closeRaceBriefing: mockCloseRaceBriefing,
    };
  });

  it("renders the temporal block and advances the calendar instead of simulating immediately", () => {
    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    expect(screen.getByText(/data 18\/03\/2026/i)).toBeInTheDocument();
    expect(screen.getByText(/próxima corrida em 7 dias/i)).toBeInTheDocument();

    const actionButton = screen.getByRole("button", { name: /avançar calendário/i });
    fireEvent.click(actionButton);

    expect(screen.getByText("Clima")).toBeInTheDocument();
    expect(mockStartCalendarAdvance).toHaveBeenCalledTimes(1);
    expect(mockSimulateRace).not.toHaveBeenCalled();
  });

  it("shows month-based countdowns before switching to weeks and days", () => {
    mockState.temporalSummary = {
      current_display_date: "2026-01-10",
      next_event_display_date: "2026-03-10",
      days_until_next_event: 59,
      weeks_until_next_event: 8,
    };

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    expect(screen.getByText(/próxima corrida em 2 meses/i)).toBeInTheDocument();
  });

  it("maps known track names to the stored thumbnail filenames", () => {
    mockState.nextRace = {
      ...mockState.nextRace,
      track_name: "Charlotte Motor Speedway - Roval",
    };

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    const image = screen.getByAltText("Charlotte Motor Speedway - Roval");
    expect(image).toHaveAttribute("src", "/utilities/tracks/charlotte.png");
  });

  it("hides the standings race banner while the pre-race briefing is open", () => {
    mockState.showRaceBriefing = true;

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    expect(screen.queryByText("Clima")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /voltar/i })).toBeInTheDocument();
  });

  it("shows a back button inside the temporal card while the briefing is open", () => {
    mockState.showRaceBriefing = true;

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    const temporalLabel = screen.getByText(/data 18\/03\/2026/i);
    const temporalCard = temporalLabel.closest(".rounded-2xl");
    expect(temporalCard).not.toBeNull();

    const backButton = within(temporalCard).getByRole("button", { name: /voltar/i });
    fireEvent.click(backButton);

    expect(mockCloseRaceBriefing).toHaveBeenCalledTimes(1);
  });

  it("shows a celebratory season-finished banner when the championship ends", async () => {
    mockState.nextRace = null;
    mockState.season = {
      numero: 1,
      ano: 2026,
      total_rodadas: 12,
      rodada_atual: 13,
    };
    invoke.mockResolvedValue([
      { id: "P001", nome: "Thomas Baker", posicao_campeonato: 1 },
      { id: "P002", nome: "R. Silva", posicao_campeonato: 2 },
    ]);

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    expect(await screen.findByText("Temporada Encerrada")).toBeInTheDocument();
    expect(screen.getByText("Thomas Baker")).toBeInTheDocument();
    expect(screen.getByText(/ano 2026/i)).toBeInTheDocument();
    expect(screen.queryByText(/temporada 1/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/sem corrida pendente/i)).not.toBeInTheDocument();
  });

  it("uses skip-all flow when the player has no team and advances from the header", () => {
    mockState.nextRace = null;
    mockState.playerTeam = null;
    mockState.temporalSummary = {
      current_display_date: "2026-03-18",
      next_event_display_date: null,
      days_until_next_event: null,
      pending_in_phase: 0,
    };

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /pular temporada/i }));

    expect(mockSkipAllPendingRaces).toHaveBeenCalledTimes(1);
    expect(mockAdvanceSeason).not.toHaveBeenCalled();
  });

  it("opens convocation from the header after the regular block ends", () => {
    mockState.nextRace = null;
    mockState.season = {
      ...mockState.season,
      fase: "BlocoRegular",
    };
    mockState.temporalSummary = {
      current_display_date: "2026-09-30",
      next_event_display_date: null,
      days_until_next_event: null,
      pending_in_phase: 0,
    };

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /avançar para convocação/i }));

    expect(mockRunConvocationWindow).toHaveBeenCalledTimes(1);
    expect(mockAdvanceSeason).not.toHaveBeenCalled();
  });

  it("keeps advancing the regular calendar before opening convocation", () => {
    mockState.nextRace = null;
    mockState.season = {
      ...mockState.season,
      fase: "BlocoRegular",
    };
    mockState.temporalSummary = {
      current_display_date: "2026-09-10",
      next_event_display_date: "2026-09-17",
      days_until_next_event: 7,
      pending_in_phase: 3,
    };

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /avançar calendário/i }));

    expect(mockStartCalendarAdvance).toHaveBeenCalledTimes(1);
    expect(mockRunConvocationWindow).not.toHaveBeenCalled();
  });

  it("finishes the special block from the header when the player has no special race", () => {
    mockState.nextRace = null;
    mockState.season = {
      ...mockState.season,
      fase: "BlocoEspecial",
    };
    mockState.temporalSummary = {
      current_display_date: "2026-11-20",
      next_event_display_date: null,
      days_until_next_event: null,
      pending_in_phase: 0,
    };

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /pular bloco especial/i }));

    expect(mockFinishSpecialBlock).toHaveBeenCalledTimes(1);
    expect(mockAdvanceSeason).not.toHaveBeenCalled();
  });

  it("only advances the season from the header after PosEspecial", () => {
    mockState.nextRace = null;
    mockState.season = {
      ...mockState.season,
      fase: "PosEspecial",
    };
    mockState.temporalSummary = {
      current_display_date: "2026-12-15",
      next_event_display_date: null,
      days_until_next_event: null,
      pending_in_phase: 0,
    };

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /encerrar temporada/i }));

    expect(mockAdvanceSeason).toHaveBeenCalledTimes(1);
    expect(mockRunConvocationWindow).not.toHaveBeenCalled();
    expect(mockFinishSpecialBlock).not.toHaveBeenCalled();
  });
});
