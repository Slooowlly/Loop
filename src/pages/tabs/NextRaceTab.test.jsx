import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import NextRaceTab from "./NextRaceTab";

const mockSimulateRace = vi.fn();
const mockFinishSpecialBlock = vi.fn();
const mockSkipAllPendingRaces = vi.fn();
const mockRunConvocationWindow = vi.fn();
const mockAdvanceSeason = vi.fn();
const mockStartCalendarAdvance = vi.fn();
let mockState = {};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

describe("NextRaceTab", () => {
  beforeEach(() => {
    mockSimulateRace.mockReset();
    mockFinishSpecialBlock.mockReset();
    mockSkipAllPendingRaces.mockReset();
    mockRunConvocationWindow.mockReset();
    mockAdvanceSeason.mockReset();
    mockStartCalendarAdvance.mockReset();
    invoke.mockReset();
    invoke.mockImplementation((command, args) => {
      if (command === "get_drivers_by_category") {
        return Promise.resolve([
          {
            id: "drv-player",
            nome: "R. Silva",
            nacionalidade: "Brasil",
            idade: 24,
            skill: 84,
            equipe_id: "team-1",
            equipe_nome: "Equipe Aurora",
            equipe_nome_curto: "Aurora",
            equipe_cor: "#58a6ff",
            is_jogador: true,
            pontos: 88,
            vitorias: 2,
            podios: 4,
            posicao_campeonato: 2,
            results: [
              { position: 3, is_dnf: false },
              { position: 2, is_dnf: false },
              { position: 1, is_dnf: false },
              { position: 4, is_dnf: false },
              null,
              null,
              null,
            ],
          },
          {
            id: "drv-rival",
            nome: "M. Costa",
            nacionalidade: "Portugal",
            idade: 27,
            skill: 86,
            equipe_id: "team-9",
            equipe_nome: "Mercedes-AMG",
            equipe_nome_curto: "AMG",
            equipe_cor: "#ff7b72",
            is_jogador: false,
            pontos: 94,
            vitorias: 3,
            podios: 5,
            posicao_campeonato: 1,
            results: [
              { position: 1, is_dnf: false },
              { position: 4, is_dnf: false },
              { position: 2, is_dnf: false },
              { position: 2, is_dnf: false },
              null,
              null,
              null,
            ],
          },
          {
            id: "drv-teammate",
            nome: "A. Lima",
            nacionalidade: "Argentina",
            idade: 23,
            skill: 79,
            equipe_id: "team-1",
            equipe_nome: "Equipe Aurora",
            equipe_nome_curto: "Aurora",
            equipe_cor: "#58a6ff",
            is_jogador: false,
            pontos: 72,
            vitorias: 1,
            podios: 3,
            posicao_campeonato: 4,
            results: [
              { position: 6, is_dnf: false },
              { position: 3, is_dnf: false },
              { position: 2, is_dnf: false },
              { position: 6, is_dnf: false },
              null,
              null,
              null,
            ],
          },
          {
            id: "drv-prado",
            nome: "E. Prado",
            nacionalidade: "Brasil",
            idade: 29,
            skill: 76,
            equipe_id: "team-9",
            equipe_nome: "Scuderia Costa",
            equipe_nome_curto: "Costa",
            equipe_cor: "#ff7b72",
            is_jogador: false,
            pontos: 64,
            vitorias: 0,
            podios: 1,
            posicao_campeonato: 5,
            results: [
              { position: 5, is_dnf: false },
              { position: 6, is_dnf: false },
              { position: 4, is_dnf: false },
              { position: 5, is_dnf: false },
              null,
              null,
              null,
            ],
          },
          {
            id: "drv-duarte",
            nome: "C. Duarte",
            nacionalidade: "Chile",
            idade: 26,
            skill: 74,
            equipe_id: "team-7",
            equipe_nome: "Sierra Racing",
            equipe_nome_curto: "Sierra",
            equipe_cor: "#f5c76d",
            is_jogador: false,
            pontos: 58,
            vitorias: 0,
            podios: 1,
            posicao_campeonato: 6,
            results: [
              { position: 9, is_dnf: false },
              { position: 6, is_dnf: false },
              { position: 4, is_dnf: false },
              { position: 3, is_dnf: false },
              null,
              null,
              null,
            ],
          },
        ]);
      }

      if (command === "get_teams_standings") {
        return Promise.resolve([
          {
            posicao: 1,
            id: "team-9",
            nome: "Mercedes-AMG",
            nome_curto: "AMG",
            cor_primaria: "#ff7b72",
            pontos: 134,
            vitorias: 4,
            piloto_1_nome: "M. Costa",
            piloto_2_nome: "E. Prado",
            trofeus: [],
          },
          {
            posicao: 2,
            id: "team-1",
            nome: "Equipe Aurora",
            nome_curto: "Aurora",
            cor_primaria: "#58a6ff",
            pontos: 129,
            vitorias: 3,
            piloto_1_nome: "R. Silva",
            piloto_2_nome: "A. Lima",
            trofeus: [],
          },
        ]);
      }

      if (command === "get_briefing_phrase_history") {
        return Promise.resolve({
          season_number: 1,
          entries: [],
        });
      }

      if (command === "save_briefing_phrase_history") {
        return Promise.resolve({
          season_number: args.seasonNumber,
          entries: args.entries.map((entry) => ({
            season_number: args.seasonNumber,
            ...entry,
          })),
        });
      }

      return Promise.resolve(null);
    });

    mockState = {
      careerId: "career-1",
      player: {
        id: "drv-player",
        nome: "R. Silva",
      },
      playerTeam: {
        id: "team-1",
        nome: "Equipe Aurora",
        nome_curto: "Aurora",
        categoria: "mazda_amador",
      },
      season: {
        numero: 1,
        ano: 2026,
        total_rodadas: 20,
        rodada_atual: 5,
      },
      nextRace: {
        id: "race-5",
        rodada: 5,
        track_name: "Interlagos",
        clima: "Wet",
        duracao_corrida_min: 35,
        status: "Pendente",
        temperatura: 28,
        horario: "14:00",
        display_date: "2026-03-25",
        event_interest: {
          display_value: 84200,
          tier_label: "Evento principal",
        },
      },
      nextRaceBriefing: {
        track_history: {
          has_data: true,
          starts: 4,
          best_finish: 1,
          last_finish: 3,
          dnfs: 1,
          last_visit_season: 1,
          last_visit_round: 4,
        },
        primary_rival: {
          driver_id: "drv-rival",
          driver_name: "M. Costa",
          championship_position: 1,
          gap_points: 6,
          is_ahead: true,
          rivalry_label: null,
        },
        weekend_stories: [
          {
            id: "story-1",
            icon: "X",
            title: "Duelo esquenta a abertura",
            summary: "O paddock trata a disputa pela ponta como o assunto central desta rodada.",
            importance: "Alta",
          },
          {
            id: "story-2",
            icon: "+",
            title: "Aurora quer encostar nos lideres",
            summary: "A equipe chega tratando esta etapa como chance real de mexer na tabela.",
            importance: "Media",
          },
        ],
      },
      temporalSummary: {
        pending_in_phase: 0,
      },
      isSimulating: false,
      isAdvancing: false,
      isConvocating: false,
      simulateRace: mockSimulateRace,
      finishSpecialBlock: mockFinishSpecialBlock,
      skipAllPendingRaces: mockSkipAllPendingRaces,
      runConvocationWindow: mockRunConvocationWindow,
      advanceSeason: mockAdvanceSeason,
      startCalendarAdvance: mockStartCalendarAdvance,
      enterPreseason: vi.fn(),
    };
  });

  it("shows the current race dashboard and handles simulate and export actions", async () => {
    render(<NextRaceTab />);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("get_drivers_by_category", {
        careerId: "career-1",
        category: "mazda_amador",
      });
    });

    expect(screen.getByText(/sala de estrat.gia/i)).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /^interlagos$/i })).toBeInTheDocument();
    expect(screen.getByText(/etapa 5 de 20/i)).toBeInTheDocument();
    expect(screen.getByText(/25\/03/i)).toBeInTheDocument();
    expect(screen.getByText(/condi..o de pista/i)).toBeInTheDocument();
    expect(screen.getByText(/^p.blico$/i)).toBeInTheDocument();
    expect(screen.getByText(/narrativa da etapa/i)).toBeInTheDocument();
    expect(screen.getByText(/voz da equipe/i)).toBeInTheDocument();
    expect(screen.getByText(/meta equipe/i)).toBeInTheDocument();
    expect(screen.getByText(/meta pessoal/i)).toBeInTheDocument();
    expect(screen.getByText(/meta t.tulo/i)).toBeInTheDocument();
    expect(screen.getByText(/os 5 favoritos ao p.dio/i)).toBeInTheDocument();
    expect(screen.getByText(/tabela geral do campeonato/i)).toBeInTheDocument();
    expect(screen.getAllByText(/84\.200/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/equipe aurora/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/^sierra racing$/i)).toBeInTheDocument();
    expect(screen.getByAltText("Mercedes-AMG logo")).toBeInTheDocument();
    const championshipTable = screen.getByRole("table");
    const championshipRows = within(championshipTable).getAllByRole("row");
    expect(championshipRows).toHaveLength(6);
    expect(within(championshipTable).getByText(/^r\. silva$/i)).toBeInTheDocument();
    expect(within(championshipTable).getByText(/^m\. costa$/i)).toBeInTheDocument();
    expect(within(championshipTable).getByText(/^a\. lima$/i)).toBeInTheDocument();
    expect(within(championshipTable).getByText(/^e\. prado$/i)).toBeInTheDocument();
    expect(within(championshipTable).getByText(/^c\. duarte$/i)).toBeInTheDocument();
    expect(within(championshipTable).getByText(/^94$/i)).toBeInTheDocument();
    expect(within(championshipTable).getByText(/^88$/i)).toBeInTheDocument();
    expect(within(championshipTable).getByText(/^58$/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /simular corrida/i }));
    expect(mockSimulateRace).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: /exportar/i }));
    expect(screen.getByText(/exportação para o iracing chega em breve/i)).toBeInTheDocument();
  });

  it("pula o bloco especial pelo CTA principal quando nao ha corrida jogavel do jogador", async () => {
    mockState.nextRace = null;
    mockState.season = {
      ...mockState.season,
      fase: "BlocoEspecial",
    };

    render(<NextRaceTab />);

    fireEvent.click(screen.getByRole("button", { name: /pular bloco especial/i }));

    await waitFor(() => {
      expect(mockFinishSpecialBlock).toHaveBeenCalledTimes(1);
    });
  });

  it("abre a janela de convocação manualmente ao fim do bloco regular", async () => {
    mockState.nextRace = null;
    mockState.season = {
      ...mockState.season,
      fase: "BlocoRegular",
    };

    render(<NextRaceTab />);

    fireEvent.click(screen.getByRole("button", { name: /avançar para convocação/i }));

    await waitFor(() => {
      expect(mockRunConvocationWindow).toHaveBeenCalledTimes(1);
    });
  });

  it("não abre convocação enquanto ainda existem corridas regulares pendentes", async () => {
    mockState.nextRace = null;
    mockState.season = {
      ...mockState.season,
      fase: "BlocoRegular",
    };
    mockState.temporalSummary = {
      pending_in_phase: 4,
    };

    render(<NextRaceTab />);

    fireEvent.click(screen.getByRole("button", { name: /avançar calendário/i }));

    await waitFor(() => {
      expect(mockStartCalendarAdvance).toHaveBeenCalledTimes(1);
    });
    expect(mockRunConvocationWindow).not.toHaveBeenCalled();
  });

  it("encerra a temporada apenas depois do PosEspecial", async () => {
    mockState.nextRace = null;
    mockState.season = {
      ...mockState.season,
      fase: "PosEspecial",
    };

    render(<NextRaceTab />);

    fireEvent.click(screen.getByRole("button", { name: /encerrar temporada/i }));

    await waitFor(() => {
      expect(mockAdvanceSeason).toHaveBeenCalledTimes(1);
    });
    expect(mockRunConvocationWindow).not.toHaveBeenCalled();
    expect(mockFinishSpecialBlock).not.toHaveBeenCalled();
  });

  it("mostra o erro detalhado ao falhar ao pular a temporada sem equipe", async () => {
    mockState.nextRace = null;
    mockState.playerTeam = null;
    mockSkipAllPendingRaces.mockRejectedValue({
      toString() {
        return "Falha detalhada do backend";
      },
    });

    render(<NextRaceTab />);

    fireEvent.click(screen.getByRole("button", { name: /pular temporada/i }));

    expect(
      await screen.findByText(/falha detalhada do backend/i),
    ).toBeInTheDocument();
  });

  it("uses a more realistic briefing when the title fight is already unlikely", async () => {
    invoke.mockImplementation((command, args) => {
      if (command === "get_drivers_by_category") {
        return Promise.resolve([
          {
            id: "drv-player",
            nome: "R. Silva",
            nacionalidade: "Brasil",
            idade: 24,
            skill: 72,
            equipe_id: "team-1",
            equipe_nome: "Equipe Aurora",
            equipe_nome_curto: "Aurora",
            equipe_cor: "#58a6ff",
            is_jogador: true,
            pontos: 9,
            vitorias: 0,
            podios: 0,
            posicao_campeonato: 8,
            results: [
              null,
              null,
              null,
              { position: 8, is_dnf: false },
              { position: 9, is_dnf: false },
              { position: 8, is_dnf: false },
            ],
          },
          {
            id: "drv-rival",
            nome: "M. Costa",
            nacionalidade: "Portugal",
            idade: 27,
            skill: 91,
            equipe_id: "team-9",
            equipe_nome: "Scuderia Costa",
            equipe_nome_curto: "Costa",
            equipe_cor: "#ff7b72",
            is_jogador: false,
            pontos: 50,
            vitorias: 5,
            podios: 6,
            posicao_campeonato: 1,
            results: [
              { position: 1, is_dnf: false },
              { position: 2, is_dnf: false },
              { position: 1, is_dnf: false },
            ],
          },
        ]);
      }

      if (command === "get_teams_standings") {
        return Promise.resolve([
          {
            posicao: 5,
            id: "team-1",
            nome: "Equipe Aurora",
            nome_curto: "Aurora",
            cor_primaria: "#58a6ff",
            pontos: 21,
            vitorias: 0,
            piloto_1_nome: "R. Silva",
            piloto_2_nome: "A. Lima",
            trofeus: [],
          },
        ]);
      }

      if (command === "get_briefing_phrase_history") {
        return Promise.resolve({
          season_number: 1,
          entries: [],
        });
      }

      if (command === "save_briefing_phrase_history") {
        return Promise.resolve({
          season_number: args.seasonNumber,
          entries: args.entries.map((entry) => ({
            season_number: args.seasonNumber,
            ...entry,
          })),
        });
      }

      return Promise.resolve(null);
    });

    mockState.season = {
      numero: 1,
      ano: 2026,
      total_rodadas: 20,
      rodada_atual: 18,
    };
    mockState.nextRace = {
      ...mockState.nextRace,
      rodada: 19,
    };

    render(<NextRaceTab />);

    await waitFor(() => {
      expect(screen.getByText(/os 5 favoritos ao p.dio/i)).toBeInTheDocument();
    });

    expect(
      screen.getByText(
        /dignidade competitiva|salvar lastro esportivo|reagir com maturidade|pe firme|resposta honesta/i,
      ),
    ).toBeInTheDocument();
    expect(screen.getByText(/tabela geral do campeonato/i)).toBeInTheDocument();

    const championshipTable = screen.getByRole("table");
    expect(within(championshipTable).getAllByRole("row")).toHaveLength(3);
    expect(within(championshipTable).getByText(/^r\. silva$/i)).toBeInTheDocument();
    expect(within(championshipTable).getByText(/^m\. costa$/i)).toBeInTheDocument();
    expect(screen.queryByText(/tentando encurtar a distancia para/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/atacar a liderança agora que a distância é curta/i)).not.toBeInTheDocument();
  });
});
