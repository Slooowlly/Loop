import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

import useCareerStore from "../useCareerStore";
import { initialState } from "./state";

// Slice de TEMPORADA: virada de ano, atalhos de bancada e o pop-up de Campeão. O que se
// guarda é o contrato de ponte (nomes de argumento) e as decisões que não aparecem no
// nome do comando — em especial a do Campeão, que é BEST-EFFORT: ela roda no meio do
// pós-corrida, e uma exceção escapando dali derrubaria a tela de resultado.

const estado = () => useCareerStore.getState();

const CARREIRA = {
  career_id: "C1",
  player: { id: "P1" },
  player_team: { id: "T1", categoria: "gt3" },
  season: { id: "S2", numero: 4, fase: "Temporada" },
  next_race: null,
};

beforeEach(() => {
  invoke.mockReset();
  useCareerStore.setState({ ...initialState });
});

describe("advanceSeason", () => {
  it("manda `careerId` e emenda direto na pré-temporada", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockImplementation((comando) => {
      if (comando === "advance_season") return Promise.resolve({ campeao: "Fulano" });
      if (comando === "get_preseason_state") return Promise.resolve({ season_number: 5 });
      if (comando === "get_news") return Promise.resolve([]);
      if (comando === "load_career") return Promise.resolve(CARREIRA);
      return Promise.resolve(null);
    });

    await estado().advanceSeason();

    expect(invoke).toHaveBeenCalledWith("advance_season", { careerId: "C1" });
    // A tela de Resumo está fora do fluxo: guardamos o resultado e seguimos ao mercado.
    expect(estado().endOfSeasonResult).toEqual({ campeao: "Fulano" });
    const comandos = invoke.mock.calls.map(([c]) => c);
    expect(comandos).toContain("get_preseason_state");
  });

  it("limpa a etapa e o calendário da temporada que acabou", async () => {
    useCareerStore.setState({
      careerId: "C1",
      nextRace: { id: "R9" },
      calendarDisplayDate: "2026-11-30",
      displayDaysUntilNextEvent: 3,
      lastRaceResult: { position: 1 },
    });
    invoke.mockResolvedValue(null);

    await estado().advanceSeason().catch(() => {});

    const s = estado();
    expect(s.nextRace).toBeNull();
    expect(s.calendarDisplayDate).toBeNull();
    expect(s.displayDaysUntilNextEvent).toBeNull();
    expect(s.lastRaceResult).toBeNull();
  });

  it("sem carreira carregada, nem chega ao backend", async () => {
    await expect(estado().advanceSeason()).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("falha destrava o botão e registra o erro", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockRejectedValue(new Error("temporada inconsistente"));

    await expect(estado().advanceSeason()).rejects.toThrow();
    expect(estado().isAdvancing).toBe(false);
    expect(estado().error).toBeTruthy();
  });
});

describe("skipAllPendingRaces", () => {
  it("simula o que falta e ENTÃO vira o ano, nessa ordem", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockResolvedValue(null);

    await estado().skipAllPendingRaces().catch(() => {});

    const comandos = invoke.mock.calls.map(([c]) => c);
    expect(comandos[0]).toBe("skip_all_pending_races");
    expect(comandos).toContain("advance_season");
    expect(comandos.indexOf("skip_all_pending_races")).toBeLessThan(
      comandos.indexOf("advance_season"),
    );
  });

  it("se a simulação falha, o ano NÃO vira", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockImplementation((comando) =>
      comando === "skip_all_pending_races"
        ? Promise.reject(new Error("corrida travada"))
        : Promise.resolve(null),
    );

    await expect(estado().skipAllPendingRaces()).rejects.toThrow();
    expect(invoke.mock.calls.map(([c]) => c)).not.toContain("advance_season");
  });
});

describe("loadSeasonChampionOverlay", () => {
  const PAYLOAD = { drivers: [{ id: "D1", nome: "Campeão" }] };

  it("passa categoria e temporada com os nomes do comando", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockResolvedValue(PAYLOAD);

    await estado().loadSeasonChampionOverlay({ category: "gt3", seasonNumber: 3 });

    expect(invoke).toHaveBeenCalledWith("get_season_champion_payload", {
      careerId: "C1",
      category: "gt3",
      seasonNumber: 3,
    });
    expect(estado().championOverlay).toEqual(PAYLOAD);
  });

  it("sem argumentos, pede a temporada ativa (os dois campos vão nulos)", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockResolvedValue(PAYLOAD);

    await estado().loadSeasonChampionOverlay();

    expect(invoke).toHaveBeenCalledWith("get_season_champion_payload", {
      careerId: "C1",
      category: null,
      seasonNumber: null,
    });
  });

  it("payload sem pilotos não abre a tela — ela nunca aparece vazia", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockResolvedValue({ drivers: [] });

    await expect(estado().loadSeasonChampionOverlay()).resolves.toBeNull();
    expect(estado().championOverlay).toBeNull();
  });

  it("é best-effort: falhar aqui não pode derrubar o pós-corrida", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockRejectedValue(new Error("temporada sem etapa disputada"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(estado().loadSeasonChampionOverlay()).resolves.toBeNull();
    expect(estado().championOverlay).toBeNull();
  });
});

describe("debugShowLastSeasonChampion", () => {
  it("prefere o ANO ANTERIOR — é onde o calendário e os recordes estão completos", async () => {
    useCareerStore.setState({ careerId: "C1", season: { numero: 5 } });
    invoke.mockResolvedValue({ drivers: [{ id: "D1" }] });

    await estado().debugShowLastSeasonChampion();

    expect(invoke).toHaveBeenCalledWith("get_season_champion_payload", {
      careerId: "C1",
      category: null,
      seasonNumber: 4,
    });
  });

  it("na primeira temporada, cai direto na ativa", async () => {
    useCareerStore.setState({ careerId: "C1", season: { numero: 1 } });
    invoke.mockResolvedValue({ drivers: [{ id: "D1" }] });

    await estado().debugShowLastSeasonChampion();

    const pedidos = invoke.mock.calls.map(([, args]) => args.seasonNumber);
    expect(pedidos).toEqual([null]);
  });

  it("ano anterior sem corrida do jogador cai na temporada ativa", async () => {
    useCareerStore.setState({ careerId: "C1", season: { numero: 5 } });
    invoke.mockImplementation((_c, args) =>
      Promise.resolve(args.seasonNumber === 4 ? { drivers: [] } : { drivers: [{ id: "D1" }] }),
    );

    await estado().debugShowLastSeasonChampion();

    const pedidos = invoke.mock.calls.map(([, args]) => args.seasonNumber);
    expect(pedidos).toEqual([4, null]);
    expect(estado().championOverlay).toBeTruthy();
  });
});

describe("startCalendarAdvance", () => {
  it("com a corrida marcada para HOJE, abre a Sala sem animar nada", async () => {
    useCareerStore.setState({
      careerId: "C1",
      nextRace: { id: "R9", display_date: "2026-08-11" },
      temporalSummary: { current_display_date: "2026-08-11", days_until_next_event: 0 },
      calendarDisplayDate: "2026-08-11",
      displayDaysUntilNextEvent: 0,
    });

    await estado().startCalendarAdvance();

    expect(estado().showRaceBriefing).toBe(true);
    expect(estado().isCalendarAdvancing).toBe(false);
  });

  it("já animando, o segundo pedido é ignorado", async () => {
    useCareerStore.setState({ careerId: "C1", isCalendarAdvancing: true });
    await estado().startCalendarAdvance();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("sem etapa e sem nada pendente na fase, não avança", async () => {
    useCareerStore.setState({
      careerId: "C1",
      nextRace: null,
      temporalSummary: { pending_in_phase: 0 },
    });

    await estado().startCalendarAdvance();
    expect(estado().showRaceBriefing).toBe(false);
  });
});
