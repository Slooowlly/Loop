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

// Header usa useNavigate (menu da equipe). Stub p/ não exigir um Router no teste.
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return { ...actual, useNavigate: () => vi.fn() };
});

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

  it("maps known track names to the wide banner images (Pistas Header)", () => {
    mockState.nextRace = {
      ...mockState.nextRace,
      track_name: "Charlotte Motor Speedway - Roval",
    };

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    const image = screen.getByAltText("Charlotte Motor Speedway - Roval");
    expect(image).toHaveAttribute(
      "src",
      "/utilities/tracks/Pistas%20Header/chartlotte.jpg",
    );
  });

  it("falls back to the thumbnail when a track has no wide banner image", () => {
    mockState.nextRace = {
      ...mockState.nextRace,
      track_name: "Circuito Desconhecido XYZ",
    };

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    const image = screen.getByAltText("Circuito Desconhecido XYZ");
    expect(image).toHaveAttribute(
      "src",
      "/utilities/tracks/Circuito%20Desconhecido%20XYZ.webp",
    );
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
      {
        id: "P001",
        nome: "Thomas Baker",
        posicao_campeonato: 1,
        pontos: 240,
        vitorias: 7,
        podios: 11,
        equipe_nome: "Apex Racing",
        equipe_cor: "#e03a3a",
        is_jogador: true,
      },
      { id: "P002", nome: "R. Silva", posicao_campeonato: 2, pontos: 198, vitorias: 3, podios: 8 },
    ]);

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    expect(await screen.findByText("Temporada 2026 Encerrada")).toBeInTheDocument();
    expect(screen.getByText("Campeão")).toBeInTheDocument();
    expect(screen.getByText("Thomas Baker")).toBeInTheDocument();
    // O placar do ano é o que o card ganhou no lugar da frase de resumo.
    expect(screen.getByText("240")).toBeInTheDocument();
    expect(screen.getByText("+42")).toBeInTheDocument();
    // A linha de apoio existe nos dois estados, para o card não mudar de altura.
    expect(screen.getByText("Título fechado com 42 pontos de vantagem sobre R. Silva")).toBeInTheDocument();
    expect(screen.queryByText(/temporada 1 /i)).not.toBeInTheDocument();
    expect(screen.queryByText(/sem corrida pendente/i)).not.toBeInTheDocument();
  });

  it("mostra a posição do jogador no pôster quando quem levou o título foi a IA", async () => {
    mockState.nextRace = null;
    mockState.season = {
      numero: 1,
      ano: 2026,
      total_rodadas: 12,
      rodada_atual: 13,
    };
    invoke.mockResolvedValue([
      {
        id: "P001",
        nome: "Thomas Baker",
        posicao_campeonato: 1,
        pontos: 240,
        vitorias: 7,
        podios: 11,
        is_jogador: false,
      },
      {
        id: "P002",
        nome: "R. Silva",
        posicao_campeonato: 2,
        pontos: 198,
        vitorias: 3,
        podios: 8,
      },
      {
        id: "P009",
        nome: "Você",
        posicao_campeonato: 9,
        pontos: 54,
        vitorias: 0,
        podios: 1,
        is_jogador: true,
      },
    ]);

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    expect(await screen.findByText("Thomas Baker")).toBeInTheDocument();
    expect(screen.getByText("Você terminou em P9, com 54 pontos")).toBeInTheDocument();
  });

  it("mostra o pôster do campeão ao abrir outra categoria com o ano já encerrado", async () => {
    mockState.homeCategory = "mazda_production";
    invoke.mockImplementation((comando) => {
      if (comando === "get_calendar_for_category") {
        return Promise.resolve([
          {
            id: "r10",
            track_name: "Charlotte Motor Speedway",
            rodada: 10,
            display_date: "2026-11-04",
            status: "Concluida",
          },
        ]);
      }
      if (comando === "get_drivers_by_category") {
        return Promise.resolve([
          {
            id: "P100",
            nome: "Matteo Bianchi",
            posicao_campeonato: 1,
            pontos: 255,
            vitorias: 10,
            podios: 12,
            equipe_nome: "First Gear Motorsport",
            equipe_cor: "#3aa0ff",
          },
          {
            id: "P101",
            nome: "Connor Martin",
            posicao_campeonato: 2,
            pontos: 144,
            vitorias: 0,
            podios: 4,
          },
        ]);
      }
      return Promise.resolve([]);
    });

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    expect(await screen.findByText("Matteo Bianchi")).toBeInTheDocument();
    expect(screen.getByText("Campeão")).toBeInTheDocument();
    expect(screen.getByText("+111")).toBeInTheDocument();
    expect(
      screen.getByText("Título fechado com 111 pontos de vantagem sobre Connor Martin"),
    ).toBeInTheDocument();
    // A última etapa disputada não pode voltar a se passar por próxima corrida.
    expect(screen.queryByText("Charlotte Motor Speedway")).not.toBeInTheDocument();
    // O botão de avanço continua na barra do topo: outra categoria é informativa.
    expect(screen.getByRole("button", { name: /avançar calendário/i })).toBeInTheDocument();
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

  it("advances the new Encerramento phase from the header without convocation", () => {
    mockState.nextRace = null;
    mockState.season = {
      ...mockState.season,
      fase: "Encerramento",
    };
    mockState.temporalSummary = {
      current_display_date: "2026-11-21",
      next_event_display_date: null,
      days_until_next_event: null,
      pending_in_phase: 0,
    };

    render(<Header activeTab="standings" onTabChange={vi.fn()} />);

    // Fim do campeonato: o primeiro clique é o desvio pelas Notícias.
    fireEvent.click(screen.getByRole("button", { name: /ver o fechamento do ano/i }));
    fireEvent.click(screen.getByRole("button", { name: /avançar para pré-temporada/i }));

    expect(mockAdvanceSeason).toHaveBeenCalledTimes(1);
    expect(mockRunConvocationWindow).not.toHaveBeenCalled();
    expect(mockFinishSpecialBlock).not.toHaveBeenCalled();
  });

  it("gasta o primeiro clique de fim de temporada abrindo as Notícias do encerramento", () => {
    mockState.nextRace = null;
    mockState.season = { ...mockState.season, fase: "Encerramento" };
    mockState.temporalSummary = {
      current_display_date: "2026-11-21",
      next_event_display_date: null,
      days_until_next_event: null,
      pending_in_phase: 0,
    };
    const onTabChange = vi.fn();

    render(<Header activeTab="standings" onTabChange={onTabChange} />);

    fireEvent.click(screen.getByRole("button", { name: /ver o fechamento do ano/i }));

    expect(onTabChange).toHaveBeenCalledWith("news");
    expect(mockAdvanceSeason).not.toHaveBeenCalled();

    // Segundo clique: agora sim o mercado.
    fireEvent.click(screen.getByRole("button", { name: /avançar para pré-temporada/i }));

    expect(mockAdvanceSeason).toHaveBeenCalledTimes(1);
  });

  it("não desvia pelas Notícias quando ainda há corrida no calendário", () => {
    const onTabChange = vi.fn();

    render(<Header activeTab="calendar" onTabChange={onTabChange} />);

    fireEvent.click(screen.getByRole("button", { name: /avançar calendário/i }));

    expect(onTabChange).not.toHaveBeenCalledWith("news");
    expect(mockStartCalendarAdvance).toHaveBeenCalledTimes(1);
  });
});
