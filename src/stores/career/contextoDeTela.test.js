import { vi, describe, it, expect, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

import useCareerStore from "../useCareerStore";
import {
  CHAVES_DE_CONTEXTO_DE_TELA,
  buildResumeUiState,
  contextoDeTelaLimpo,
} from "./helpers";
import { initialState } from "./state";

// O contexto de tela (qual sobreposição está aberta e com que dados) precisa morrer INTEIRO
// a cada troca de contexto. Antes estas listas eram copiadas à mão em quatro lugares e as
// cópias divergiam: um pedaço do contexto anterior sobrevivia à troca.

/// Valores "sujos" para todas as chaves de contexto, como se uma carreira estivesse aberta
/// no meio do mercado com oferta especial aceita.
function contextoSujo() {
  return {
    showEndOfSeason: true,
    showPreseason: true,
    showConvocation: true,
    endOfSeasonResult: { campeao: "velho" },
    preseasonState: { season_number: 3 },
    preseasonWeeks: [{ week_number: 1, events: [] }],
    playerProposals: [{ id: "P1" }],
    preseasonFreeAgents: [{ id: "D9" }],
    convocationResult: { ok: true },
    specialWindowState: { player_offers: [] },
    playerSpecialOffers: [{ id: "O1" }],
    acceptedSpecialOffer: { id: "O1" },
  };
}

describe("contextoDeTelaLimpo", () => {
  it("zera toda chave de contexto com o mesmo valor do estado inicial", () => {
    const limpo = contextoDeTelaLimpo();

    for (const chave of CHAVES_DE_CONTEXTO_DE_TELA) {
      expect(limpo[chave]).toEqual(initialState[chave]);
    }
  });

  it("devolve arrays novos, para o estado limpo não compartilhar referência com o inicial", () => {
    const limpo = contextoDeTelaLimpo();

    expect(limpo.preseasonWeeks).not.toBe(initialState.preseasonWeeks);
    expect(limpo.playerProposals).not.toBe(initialState.playerProposals);
  });

  it("deixa o chamador sobrescrever só o que ele está abrindo", () => {
    const limpo = contextoDeTelaLimpo({ showPreseason: true, preseasonState: { a: 1 } });

    expect(limpo.showPreseason).toBe(true);
    expect(limpo.preseasonState).toEqual({ a: 1 });
    expect(limpo.showEndOfSeason).toBe(false);
    expect(limpo.preseasonFreeAgents).toEqual([]);
  });
});

describe("buildResumeUiState", () => {
  it("sem contexto salvo, devolve o conjunto completo zerado", async () => {
    const estado = await buildResumeUiState("C1", null);

    for (const chave of CHAVES_DE_CONTEXTO_DE_TELA) {
      expect(estado).toHaveProperty(chave);
      expect(estado[chave]).toEqual(initialState[chave]);
    }
  });

  it("no ramo de fim de temporada, zera os agentes livres da carreira anterior", async () => {
    const estado = await buildResumeUiState("C1", {
      active_view: "end_of_season",
      end_of_season_result: { campeao: "novo" },
    });

    expect(estado.showEndOfSeason).toBe(true);
    expect(estado.endOfSeasonResult).toEqual({ campeao: "novo" });
    // A omissão que existia: a lista de agentes livres sobrevivia à troca de carreira.
    expect(estado.preseasonFreeAgents).toEqual([]);
    expect(estado.playerSpecialOffers).toEqual([]);
  });

  it("num active_view desconhecido, zera tudo em vez de deixar o contexto anterior de pé", async () => {
    const estado = await buildResumeUiState("C1", { active_view: "tela_que_nao_existe_mais" });

    for (const chave of CHAVES_DE_CONTEXTO_DE_TELA) {
      expect(estado[chave]).toEqual(initialState[chave]);
    }
  });
});

describe("troca de contexto no store", () => {
  beforeEach(() => {
    invoke.mockReset();
    useCareerStore.setState({ ...initialState });
  });

  it("loadCareer zera o contexto da carreira anterior antes de pedir a nova", async () => {
    useCareerStore.setState({ careerId: "C_ANTIGA", ...contextoSujo() });

    // A carreira nova nem chega a carregar: o que importa é o estado logo após o reset.
    invoke.mockRejectedValue(new Error("falha proposital"));

    await expect(useCareerStore.getState().loadCareer("C_NOVA")).rejects.toThrow();

    const estado = useCareerStore.getState();
    for (const chave of CHAVES_DE_CONTEXTO_DE_TELA) {
      expect(estado[chave]).toEqual(initialState[chave]);
    }
  });

  it("finalizePreseason zera as ofertas do bloco especial da temporada que acabou", async () => {
    useCareerStore.setState({ careerId: "C1", ...contextoSujo() });

    invoke.mockImplementation((comando) => {
      if (comando === "load_career") {
        return Promise.resolve({
          career_id: "C1",
          difficulty: "Normal",
          player: { id: "P1" },
          player_team: { id: "T1", categoria: "gt3" },
          season: { id: "S2", fase: "Temporada" },
          next_race: null,
          total_drivers: 200,
          total_teams: 60,
        });
      }
      return Promise.resolve(null);
    });

    await useCareerStore.getState().finalizePreseason();

    const estado = useCareerStore.getState();
    for (const chave of CHAVES_DE_CONTEXTO_DE_TELA) {
      expect(estado[chave]).toEqual(initialState[chave]);
    }
  });
});
