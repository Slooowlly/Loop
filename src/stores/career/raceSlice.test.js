import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

import useCareerStore from "../useCareerStore";
import { initialState } from "./state";

// Slice de CORRIDA. O guard estrutural (`invoke-contra-generate-handler`) já garante que
// todo comando invocado existe no `generate_handler!`. O que ele NÃO pega — e é o que se
// testa aqui — é o resto do contrato: o nome dos ARGUMENTOS que atravessam a ponte e o
// nome dos CAMPOS que se lê da resposta. Um `raceId` que vira `race_id`, ou um
// `player_race` renomeado no DTO, passa pelo guard e chega à tela como corrida em branco.

const estado = () => useCareerStore.getState();

const RESULTADO = {
  player_race: { position: 3, driver: "Jogador" },
  evaluation: { nota: 7 },
  maintenance: { custo: 120 },
  event_repercussion: { manchete: "Pódio" },
  other_categories: [{ categoria: "gt4" }],
};

beforeEach(() => {
  invoke.mockReset();
  useCareerStore.setState({ ...initialState });
});

describe("simulateRace", () => {
  it("manda careerId e raceId com esses nomes exatos", async () => {
    useCareerStore.setState({ careerId: "C1", nextRace: { id: "R9" } });
    invoke.mockResolvedValue(RESULTADO);

    await estado().simulateRace();

    expect(invoke).toHaveBeenCalledWith("simulate_race_weekend", {
      careerId: "C1",
      raceId: "R9",
    });
  });

  it("lê os campos do resultado pelos nomes do DTO", async () => {
    useCareerStore.setState({ careerId: "C1", nextRace: { id: "R9" } });
    invoke.mockResolvedValue(RESULTADO);

    await estado().simulateRace();

    const s = estado();
    expect(s.lastRaceResult).toEqual(RESULTADO.player_race);
    expect(s.lastRaceEvaluation).toEqual(RESULTADO.evaluation);
    expect(s.lastRaceMaintenance).toEqual(RESULTADO.maintenance);
    expect(s.lastRaceRepercussion).toEqual(RESULTADO.event_repercussion);
    expect(s.otherCategoriesResult).toEqual(RESULTADO.other_categories);
    expect(s.showResult).toBe(true);
    expect(s.showRaceBriefing).toBe(false);
  });

  it("corrida simulada não tem telemetria — o campo fica nulo de propósito", async () => {
    useCareerStore.setState({ careerId: "C1", nextRace: { id: "R9" } });
    invoke.mockResolvedValue(RESULTADO);

    await estado().simulateRace();

    expect(estado().lastRaceTelemetry).toBeNull();
    expect(estado().lastRaceOrigem).toBe("simulada");
  });

  it("marca a final da temporada pelo slot temático da etapa", async () => {
    useCareerStore.setState({
      careerId: "C1",
      nextRace: { id: "R9", thematic_slot: "FinalDaTemporada" },
    });
    invoke.mockResolvedValue(RESULTADO);

    await estado().simulateRace();
    expect(estado().lastRaceWasFinale).toBe(true);
  });

  it("sem etapa pendente, recusa em vez de chamar o backend", async () => {
    useCareerStore.setState({ careerId: "C1", nextRace: null });
    await expect(estado().simulateRace()).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("não roda duas vezes ao mesmo tempo", async () => {
    useCareerStore.setState({ careerId: "C1", nextRace: { id: "R9" }, isSimulating: true });
    await expect(estado().simulateRace()).resolves.toBeNull();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("falha limpa o `isSimulating` — senão o botão fica travado para sempre", async () => {
    useCareerStore.setState({ careerId: "C1", nextRace: { id: "R9" } });
    invoke.mockRejectedValue(new Error("motor de simulação caiu"));

    await expect(estado().simulateRace()).rejects.toThrow();
    expect(estado().isSimulating).toBe(false);
    expect(estado().error).toBeTruthy();
  });
});

describe("pollIracingResult", () => {
  const PAYLOAD = {
    race_result: { position: 1 },
    evaluation: { nota: 9 },
    telemetry: { voltas: [] },
    summary: {
      race_id: "R9",
      maintenance: { custo: 0 },
      event_repercussion: { manchete: "Vitória" },
      repair_cost: 0,
    },
  };

  it("lê o resultado dos campos aninhados em `summary`", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockResolvedValue(PAYLOAD);

    await estado().pollIracingResult();

    expect(invoke).toHaveBeenCalledWith("iracing_auto_import_if_ready", { careerId: "C1" });
    const s = estado();
    expect(s.lastRaceId).toBe("R9");
    expect(s.lastRaceRepercussion).toEqual(PAYLOAD.summary.event_repercussion);
    expect(s.lastRaceTelemetry).toEqual(PAYLOAD.telemetry);
    expect(s.lastRaceOrigem).toBe("iracing");
  });

  it("só abre o pop-up de conserto quando custou dinheiro", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockResolvedValue(PAYLOAD);
    await estado().pollIracingResult();
    expect(estado().iracingRepair).toBeNull();

    useCareerStore.setState({ ...initialState, careerId: "C1" });
    invoke.mockResolvedValue({
      ...PAYLOAD,
      summary: { ...PAYLOAD.summary, repair_cost: 1500 },
    });
    await estado().pollIracingResult();
    expect(estado().iracingRepair?.repair_cost).toBe(1500);
  });

  it("sem resultado gravado, não mexe na tela — é o caso comum do poller", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockResolvedValue({ race_result: null });

    await estado().pollIracingResult();

    expect(estado().showResult).toBe(false);
    expect(estado().iracingImporting).toBe(false);
  });

  it("erro é silencioso: 'ainda não pronto' não é falha, e o poller volta em 4s", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockRejectedValue(new Error("sessão em andamento"));

    await expect(estado().pollIracingResult()).resolves.toBeUndefined();
    expect(estado().error).toBeNull();
    // O portão precisa reabrir, senão o poller para de tentar para sempre.
    expect(estado().iracingImporting).toBe(false);
  });

  it("não importa por cima de uma tela de resultado já aberta", async () => {
    useCareerStore.setState({ careerId: "C1", showResult: true });
    await estado().pollIracingResult();
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("openSavedRaceScreen", () => {
  it("pede a tela salva por categoria e rodada, com esses nomes", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockResolvedValue({ race_result: { position: 4 }, race_id: "R4" });

    await expect(estado().openSavedRaceScreen("gt3", 4)).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledWith("get_saved_race_screen", {
      careerId: "C1",
      category: "gt3",
      rodada: 4,
    });
  });

  it("reabertura NÃO conta como corrida recém-terminada", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockResolvedValue({ race_result: { position: 4 } });

    await estado().openSavedRaceScreen("gt3", 4);

    // `resultIsFresh` é o que decide se o pós-corrida leva às Notícias e se o pop-up de
    // Campeão abre. Reabrir uma corrida antiga não pode disparar nenhum dos dois.
    expect(estado().resultIsFresh).toBe(false);
    expect(estado().lastRaceWasFinale).toBe(false);
    expect(estado().lastRaceOrigem).toBe("reaberta");
  });

  it("sem tela salva, devolve false sem abrir nada", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockResolvedValue(null);

    await expect(estado().openSavedRaceScreen("gt3", 99)).resolves.toBe(false);
    expect(estado().showResult).toBe(false);
  });

  it("argumento faltando não chega a virar chamada", async () => {
    useCareerStore.setState({ careerId: "C1" });
    await expect(estado().openSavedRaceScreen(null, 4)).resolves.toBe(false);
    await expect(estado().openSavedRaceScreen("gt3", null)).resolves.toBe(false);
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("dismissResult", () => {
  it("devolve a carreira RECARREGADA — quem fecha precisa do estado já atualizado", async () => {
    useCareerStore.setState({ careerId: "C1", showResult: true, resultIsFresh: true });
    invoke.mockImplementation((comando) =>
      comando === "load_career"
        ? Promise.resolve({
            career_id: "C1",
            player: { id: "P1" },
            player_team: { id: "T1", categoria: "gt3" },
            season: { id: "S1", fase: "Temporada" },
            next_race: null,
          })
        : Promise.resolve(null),
    );

    const recarregada = await estado().dismissResult();

    // O Dashboard lê `next_race` daqui para decidir se abre o pop-up de Campeão.
    expect(recarregada).toHaveProperty("next_race", null);
    expect(estado().showResult).toBe(false);
    expect(estado().resultIsFresh).toBe(false);
  });

  it("falha ao recarregar devolve null em vez de derrubar o pós-corrida", async () => {
    useCareerStore.setState({ careerId: "C1", showResult: true });
    invoke.mockRejectedValue(new Error("save corrompido"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(estado().dismissResult()).resolves.toBeNull();
    expect(estado().showResult).toBe(false);
  });

  it("sem carreira, fecha a tela e para por aí", async () => {
    useCareerStore.setState({ careerId: null, showResult: true });
    await expect(estado().dismissResult()).resolves.toBeNull();
    expect(invoke).not.toHaveBeenCalled();
  });
});
