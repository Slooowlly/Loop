import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

import useCareerStore from "../useCareerStore";
import { initialState } from "./state";

// O pré-carregamento da etapa, que roda DURANTE a animação de avanço do calendário. Ele
// existe para uma coisa só: a Sala de Estratégia abrir com o texto pronto, sem o flash
// template→IA. Todos os modos de falha aqui são silenciosos por desenho — a Sala se vira
// sozinha —, e é por isso que só um teste mostra quando ele para de funcionar.

const estado = () => useCareerStore.getState();

const RETRATO = {
  get_drivers_by_category: [{ id: "D1", posicao_campeonato: 1 }],
  get_teams_standings: [{ id: "T1" }],
  get_briefing_phrase_history: { season_number: 3, entries: [] },
  get_breakdown_forecast: { available: true, overall_level: "baixo", parts: [] },
  get_grid_breakdown_risk: ["T7"],
  get_weekend_modifiers: [{ driver_id: "D1", forma: 2 }],
  pre_race_briefing_ai: {
    headline: "Manchete",
    narrative: "Narrativa",
    team_voice: "Voz da equipe",
  },
};

function comBackend(overrides = {}) {
  invoke.mockImplementation((comando) => {
    if (comando in overrides) return Promise.resolve(overrides[comando]).then((v) => v);
    return Promise.resolve(RETRATO[comando] ?? null);
  });
}

/// Carreira no meio da temporada, com etapa marcada — o estado em que o prefetch dispara.
function carreiraNaEtapa(raceId = "R9") {
  useCareerStore.setState({
    careerId: "C1",
    player: { id: "P1", nome: "Jogador" },
    playerTeam: { id: "T1", categoria: "gt3", nome: "Equipe" },
    season: { id: "S1", numero: 3, ano: 2026, fase: "Temporada" },
    nextRace: { id: raceId, rodada: 4, track_name: "Monza", display_date: "2026-08-20" },
  });
}

beforeEach(() => {
  invoke.mockReset();
  useCareerStore.setState({ ...initialState });
});

describe("prefetchPreRaceBriefing", () => {
  it("guarda o retrato INTEIRO da etapa, chaveado pela corrida", async () => {
    carreiraNaEtapa("R9");
    comBackend();

    await estado().prefetchPreRaceBriefing();

    const cache = estado().preRaceStandings;
    expect(cache.raceId).toBe("R9");
    // A carreira faz parte da chave: R001, R002... existem em todo save.
    expect(cache.careerId).toBe("C1");
    expect(cache.driverStandings).toEqual(RETRATO.get_drivers_by_category);
    expect(cache.teamStandings).toEqual(RETRATO.get_teams_standings);
    expect(cache.phraseHistory).toEqual(RETRATO.get_briefing_phrase_history);
    // Os dois que antes ficavam de fora do cache e chegavam depois da tela.
    expect(cache.breakdownRiskTeamIds).toEqual(["T7"]);
    expect(cache.weekendModifierRows).toEqual(RETRATO.get_weekend_modifiers);
  });

  it("gera a prévia por IA e a guarda com os nomes do DTO do backend", async () => {
    carreiraNaEtapa("R9");
    comBackend();

    await estado().prefetchPreRaceBriefing();

    const chamada = invoke.mock.calls.find(([c]) => c === "pre_race_briefing_ai");
    expect(chamada[1]).toMatchObject({ careerId: "C1", raceId: "R9" });
    expect(typeof chamada[1].facts).toBe("string");

    // `team_voice` (snake) → `teamVoice` (camel). Renomear um dos dois é o erro que a
    // Sala mostra como texto de template no lugar da prévia.
    expect(estado().preRaceAi).toEqual({
      careerId: "C1",
      raceId: "R9",
      headline: "Manchete",
      narrative: "Narrativa",
      teamVoice: "Voz da equipe",
    });
  });

  it("prévia incompleta é descartada — meia prévia é pior que nenhuma", async () => {
    carreiraNaEtapa("R9");
    comBackend({ pre_race_briefing_ai: { headline: "Só a manchete" } });

    await estado().prefetchPreRaceBriefing();

    expect(estado().preRaceAi).toBeNull();
    // Os standings continuam em cache: só a IA falhou.
    expect(estado().preRaceStandings?.raceId).toBe("R9");
  });

  it("sem etapa, sem carreira ou sem equipe, não busca nada", async () => {
    comBackend();

    await estado().prefetchPreRaceBriefing();
    expect(invoke).not.toHaveBeenCalled();

    useCareerStore.setState({ careerId: "C1", nextRace: { id: "R9" }, playerTeam: null });
    await estado().prefetchPreRaceBriefing();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("com IA e standings já em cache desta etapa, não repete a busca", async () => {
    carreiraNaEtapa("R9");
    useCareerStore.setState({
      preRaceAi: { careerId: "C1", raceId: "R9", narrative: "n", teamVoice: "v" },
      preRaceStandings: { careerId: "C1", raceId: "R9", driverStandings: [] },
    });
    comBackend();

    await estado().prefetchPreRaceBriefing();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("com só os standings em cache, rebusca mas NÃO regenera a IA", async () => {
    carreiraNaEtapa("R9");
    useCareerStore.setState({
      preRaceAi: { careerId: "C1", raceId: "R9", narrative: "n", teamVoice: "v" },
    });
    comBackend();

    await estado().prefetchPreRaceBriefing();

    expect(invoke.mock.calls.map(([c]) => c)).not.toContain("pre_race_briefing_ai");
    expect(estado().preRaceStandings?.raceId).toBe("R9");
  });

  it("etapa que virou no meio da busca aborta a gravação — cache de corrida errada é pior", async () => {
    carreiraNaEtapa("R9");
    invoke.mockImplementation((comando) => {
      if (comando === "get_drivers_by_category") {
        // O jogador avançou o calendário enquanto o prefetch estava em voo.
        useCareerStore.setState({ nextRace: { id: "R10" } });
      }
      return Promise.resolve(RETRATO[comando] ?? null);
    });

    await estado().prefetchPreRaceBriefing();

    expect(estado().preRaceStandings).toBeNull();
    expect(estado().preRaceAi).toBeNull();
  });

  it("qualquer falha é silenciosa — o prefetch não pode derrubar o avanço do calendário", async () => {
    carreiraNaEtapa("R9");
    invoke.mockRejectedValue(new Error("banco travado"));

    await expect(estado().prefetchPreRaceBriefing()).resolves.toBeUndefined();
    expect(estado().error).toBeNull();
    expect(estado().preRaceStandings).toBeNull();
  });
});
