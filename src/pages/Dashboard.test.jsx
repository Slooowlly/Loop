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

vi.mock("../components/race/RaceResultViewV2", () => ({
  default: ({ onDismiss }) => (
    <div>
      <div>Classificação final</div>
      <button type="button" onClick={onDismiss}>
        Continuar debriefing
      </button>
    </div>
  ),
}));

vi.mock("./tabs/NewsMagazineTab", () => ({
  default: () => <div>Notícias do paddock</div>,
}));

vi.mock("./tabs/NextRaceTab", () => ({
  default: () => <div>Briefing pre-corrida</div>,
}));

vi.mock("./tabs/CalendarTabRedesign", () => ({
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
  default: ({ onOpenGlobalDrivers, onOpenGlobalTeams, onOpenTeamRecords }) => (
    <div>
      <div>Classificacao de pilotos</div>
      <button
        type="button"
        onClick={() => onOpenTeamRecords?.({ metric: "wins", category: "gt3", teamId: "T001" })}
      >
        Abrir recordes pela classificacao
      </button>
      <button type="button" onClick={() => onOpenGlobalDrivers?.("D001")}>
        Abrir panorama
      </button>
      <button
        type="button"
        onClick={() => onOpenGlobalTeams?.({ id: "T001", categoria: "gt3", classe: "gt3" })}
      >
        Abrir equipes mundiais
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

vi.mock("./tabs/atlas", () => ({
  default: ({ selectedTeamId, selectedTeamCategory, selectedTeamClassName, onBack, onOpenTeamRecords }) => (
    <div>
      <div>
        Equipes mundiais {selectedTeamId} {selectedTeamCategory} {selectedTeamClassName}
      </div>
      <button type="button" onClick={onBack}>
        Voltar equipes
      </button>
      <button
        type="button"
        onClick={() => onOpenTeamRecords?.({ metric: "titles", category: "mazda_rookie", teamId: "T009" })}
      >
        Abrir recordes pelo atlas
      </button>
    </div>
  ),
}));

// O Dashboard importa o SELETOR de versão (./tabs/myteam), não a v1 direto — o mock
// tem de casar com esse caminho, senão a aba real monta no teste sem ninguém notar.
vi.mock("./tabs/myteam", () => ({
  default: ({ onOpenTeamRecords }) => (
    <div>
      <div>Minha equipe</div>
      <button
        type="button"
        onClick={() => onOpenTeamRecords?.({ metric: "wins", category: "gt3", teamId: "T001" })}
      >
        Abrir recordes pelo card
      </button>
    </div>
  ),
}));

vi.mock("./tabs/TeamRecordsTab", () => ({
  default: ({ category, metric, highlightTeamId, onBack }) => (
    <div>
      <div data-testid="records-args">{[metric, category, highlightTeamId].join("|")}</div>
      <button type="button" onClick={onBack}>
        Voltar recordes
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
      lastRaceWasFinale: false,
      resultIsFresh: false,
      season: { numero: 1, ano: 2026 },
      careerId: "career-1",
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

  it("opens the hidden global teams tab from the standings callback and returns", () => {
    mockState.showRaceBriefing = false;

    render(<Dashboard />);

    fireEvent.click(screen.getByRole("button", { name: /Abrir equipes mundiais/i }));

    expect(screen.getByText("Equipes mundiais T001 gt3 gt3")).toBeInTheDocument();
    expect(screen.getByTestId("main-layout")).toHaveAttribute("data-active-tab", "global-teams");

    fireEvent.click(screen.getByRole("button", { name: /Voltar equipes/i }));

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

    // A pista pulsa por ~1s no calendário antes de abrir a sala de estratégia.
    await vi.advanceTimersByTimeAsync(999);

    expect(screen.queryByText("Briefing pre-corrida")).not.toBeInTheDocument();

    await vi.advanceTimersByTimeAsync(1);

    expect(screen.getByText("Briefing pre-corrida")).toBeInTheDocument();
    vi.useRealTimers();
  });

  it("lands on Home after dismissing a fresh season finale", () => {
    mockState.showRaceBriefing = false;

    const { rerender } = render(<Dashboard />);

    fireEvent.click(screen.getByRole("button", { name: /Ir para calendario/i }));

    mockState.showResult = true;
    mockState.lastRaceResult = { track_name: "Interlagos", race_results: [] };
    mockState.resultIsFresh = true;
    mockState.lastRaceWasFinale = true;
    rerender(<Dashboard />);

    fireEvent.click(screen.getByRole("button", { name: /Continuar debriefing/i }));

    // O fim do campeonato cai na Home — é lá que o pop-up de Campeão da Temporada
    // abre. As notícias de encerramento vêm no primeiro clique de "Avançar".
    expect(screen.getByTestId("main-layout")).toHaveAttribute("data-active-tab", "standings");
    expect(mockState.dismissResult).toHaveBeenCalledTimes(1);
  });

  it("keeps the current tab when dismissing a reopened season-finale result", () => {
    mockState.showRaceBriefing = false;

    const { rerender } = render(<Dashboard />);

    fireEvent.click(screen.getByRole("button", { name: /Ir para calendario/i }));

    mockState.showResult = true;
    mockState.lastRaceResult = { track_name: "Interlagos", race_results: [] };
    mockState.resultIsFresh = false;
    mockState.lastRaceWasFinale = true;
    rerender(<Dashboard />);

    fireEvent.click(screen.getByRole("button", { name: /Continuar debriefing/i }));

    expect(screen.getByTestId("main-layout")).toHaveAttribute("data-active-tab", "calendar");
    expect(mockState.dismissResult).toHaveBeenCalledTimes(1);
  });

  it("preserves the news landing policy after dismissing a fresh regular-race result", () => {
    localStorage.clear();
    mockState.showRaceBriefing = false;

    const { rerender } = render(<Dashboard />);

    fireEvent.click(screen.getByRole("button", { name: /Ir para calendario/i }));

    mockState.showResult = true;
    mockState.lastRaceResult = { track_name: "Interlagos", race_results: [] };
    mockState.resultIsFresh = true;
    mockState.lastRaceWasFinale = false;
    rerender(<Dashboard />);

    fireEvent.click(screen.getByRole("button", { name: /Continuar debriefing/i }));

    expect(screen.getByTestId("main-layout")).toHaveAttribute("data-active-tab", "news");
    expect(mockState.dismissResult).toHaveBeenCalledTimes(1);
  });
});

// A navegação dos cards de record. É o trecho que não tinha teste: o dossiê já
// prova que o clique dispara com métrica e recorte, e a aba já prova que ordena
// pelo que recebe — faltava provar que o Dashboard liga um no outro.
describe("Dashboard — recordes de equipes", () => {
  beforeEach(() => {
    mockState = {
      isLoaded: true,
      showRaceBriefing: false,
      showResult: false,
      lastRaceResult: null,
      dismissResult: vi.fn(),
      showEndOfSeason: false,
      endOfSeasonResult: null,
      showPreseason: false,
      showConvocation: false,
      lastRaceWasFinale: false,
      resultIsFresh: false,
      season: { numero: 1, ano: 2026 },
      careerId: "career-1",
    };
  });

  it("abre a aba de recordes com a métrica e o recorte do card clicado", () => {
    render(<Dashboard />);

    fireEvent.click(screen.getByText("Abrir recordes pela classificacao"));

    expect(screen.getByTestId("main-layout").dataset.activeTab).toBe("team-records");
    expect(screen.getByTestId("records-args").textContent).toBe("wins|gt3|T001");
  });

  // A tela não está no menu, então o "Voltar" é a única saída — e tem de devolver
  // o jogador para onde ele clicou. Um destino fixo estaria certo para uma aba e
  // errado para as outras.
  it("volta para a aba de onde o clique partiu", () => {
    render(<Dashboard />);

    fireEvent.click(screen.getByText("Abrir equipes mundiais"));
    fireEvent.click(screen.getByText("Abrir recordes pelo atlas"));
    expect(screen.getByTestId("records-args").textContent).toBe("titles|mazda_rookie|T009");

    fireEvent.click(screen.getByText("Voltar recordes"));
    expect(screen.getByTestId("main-layout").dataset.activeTab).toBe("global-teams");
  });
});
