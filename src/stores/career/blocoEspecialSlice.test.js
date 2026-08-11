import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

import useCareerStore from "../useCareerStore";
import { createBlocoEspecialSlice } from "./blocoEspecialSlice";
import { initialState } from "./state";

// Bloco Especial — LEGADO 9D. Só roda para saves pré-v33 em voo, e é justamente por isso
// que precisa de rede: ninguém exercita este caminho jogando, então uma regressão aqui só
// apareceria no save de alguém que atravessou a versão.
//
// O slice acabou de sair do `seasonSlice` para um arquivo próprio. Estes testes fixam que
// a mudança de arquivo NÃO mudou a API pública: os mesmos nomes de ação, no mesmo store,
// escrevendo nas mesmas chaves de estado.

const estado = () => useCareerStore.getState();

const JANELA = {
  player_offers: [{ id: "O1", special_category: "endurance" }],
  dia_atual: 2,
};

beforeEach(() => {
  invoke.mockReset();
  useCareerStore.setState({ ...initialState });
});

describe("a separação em arquivo próprio preservou a API", () => {
  it("todas as ações do bloco continuam expostas no store composto", () => {
    const acoes = Object.keys(createBlocoEspecialSlice(vi.fn(), vi.fn()));
    expect(acoes.length).toBeGreaterThan(0);
    acoes.forEach((acao) => {
      expect(typeof estado()[acao]).toBe("function");
    });
  });

  it("o slice compartilha o estado com os irmãos, e não um estado próprio", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockResolvedValue(JANELA);

    await estado().loadSpecialWindowState();

    // `showConvocation` é lido pelo Dashboard, que não sabe de qual slice ele veio.
    expect(estado().showConvocation).toBe(true);
    expect(estado().specialWindowState).toEqual(JANELA);
  });
});

describe("runConvocationWindow", () => {
  it("transiciona e roda a convocação, nessa ordem", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockImplementation((comando) =>
      Promise.resolve(comando === "get_special_window_state" ? JANELA : { ok: true }),
    );

    await estado().runConvocationWindow();

    const comandos = invoke.mock.calls.map(([c]) => c);
    expect(comandos.slice(0, 2)).toEqual([
      "advance_to_convocation_window",
      "run_convocation_window",
    ]);
    expect(estado().playerSpecialOffers).toEqual(JANELA.player_offers);
    expect(estado().isDirty).toBe(true);
  });

  it("a leitura da janela é opcional — falhar nela não desfaz a convocação", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockImplementation((comando) =>
      comando === "get_special_window_state"
        ? Promise.reject(new Error("janela ausente"))
        : Promise.resolve({ ok: true }),
    );

    await expect(estado().runConvocationWindow()).resolves.toEqual({ ok: true });
    expect(estado().showConvocation).toBe(true);
    expect(estado().playerSpecialOffers).toEqual([]);
  });

  it("falha destrava o `isConvocating`", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockRejectedValue(new Error("fase errada"));

    await expect(estado().runConvocationWindow()).rejects.toThrow();
    expect(estado().isConvocating).toBe(false);
    expect(estado().error).toBeTruthy();
  });
});

describe("acceptSpecialOfferForDay / advanceSpecialWindowDay", () => {
  it("mandam `offerId` com esse nome e absorvem a janela devolvida", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockResolvedValue(JANELA);

    await estado().acceptSpecialOfferForDay("O1");

    expect(invoke).toHaveBeenCalledWith("accept_special_offer_for_day", {
      careerId: "C1",
      offerId: "O1",
    });
    expect(estado().specialWindowState).toEqual(JANELA);
  });

  it("avançar o dia manda só a carreira", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockResolvedValue(JANELA);

    await estado().advanceSpecialWindowDay();

    expect(invoke).toHaveBeenCalledWith("advance_special_window_day", { careerId: "C1" });
    expect(estado().isConvocating).toBe(false);
  });
});

describe("finishSpecialBlock", () => {
  it("simula, encerra e roda o pós-especial, nessa ordem", async () => {
    useCareerStore.setState({ careerId: "C1" });
    invoke.mockImplementation((comando) =>
      Promise.resolve(
        comando === "load_career"
          ? {
              career_id: "C1",
              player: { id: "P1" },
              player_team: { id: "T1", categoria: "gt3" },
              season: { id: "S1", fase: "PosEspecial" },
              next_race: null,
            }
          : null,
      ),
    );

    await estado().finishSpecialBlock();

    const comandos = invoke.mock.calls.map(([c]) => c);
    expect(comandos.slice(0, 3)).toEqual([
      "simulate_special_block",
      "encerrar_bloco_especial",
      "run_pos_especial",
    ]);
  });

  it("não roda duas vezes ao mesmo tempo", async () => {
    useCareerStore.setState({ careerId: "C1", isConvocating: true });
    await expect(estado().finishSpecialBlock()).resolves.toBeNull();
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("respondToSpecialOffer", () => {
  it("aceitar guarda a oferta com a categoria que o backend confirmou", async () => {
    useCareerStore.setState({
      careerId: "C1",
      playerSpecialOffers: [{ id: "O1", special_category: "gt4" }],
    });
    invoke.mockResolvedValue({ remaining_offers: 0, special_category: "endurance" });

    await estado().respondToSpecialOffer("O1", true);

    expect(invoke).toHaveBeenCalledWith("respond_player_special_offer", {
      careerId: "C1",
      offerId: "O1",
      accept: true,
    });
    // A categoria da RESPOSTA manda sobre a da oferta: o backend pode ter realocado.
    expect(estado().acceptedSpecialOffer).toEqual({
      id: "O1",
      special_category: "endurance",
    });
  });

  it("com ofertas restantes, rebusca a lista; sem restantes, esvazia sem pedir", async () => {
    useCareerStore.setState({
      careerId: "C1",
      playerSpecialOffers: [{ id: "O1" }, { id: "O2" }],
    });
    invoke.mockImplementation((comando) =>
      Promise.resolve(
        comando === "respond_player_special_offer"
          ? { remaining_offers: 1 }
          : [{ id: "O2" }],
      ),
    );

    await estado().respondToSpecialOffer("O1", false);
    expect(invoke).toHaveBeenCalledWith("get_player_special_offers", { careerId: "C1" });
    expect(estado().playerSpecialOffers).toEqual([{ id: "O2" }]);

    invoke.mockReset();
    useCareerStore.setState({ careerId: "C1", playerSpecialOffers: [{ id: "O2" }] });
    invoke.mockResolvedValue({ remaining_offers: 0 });

    await estado().respondToSpecialOffer("O2", false);
    expect(invoke.mock.calls.map(([c]) => c)).not.toContain("get_player_special_offers");
    expect(estado().playerSpecialOffers).toEqual([]);
  });

  it("recusar preserva a oferta já aceita — recusar uma não desfaz a outra", async () => {
    const jaAceita = { id: "O0", special_category: "endurance" };
    useCareerStore.setState({
      careerId: "C1",
      playerSpecialOffers: [{ id: "O1" }],
      acceptedSpecialOffer: jaAceita,
    });
    invoke.mockResolvedValue({ remaining_offers: 0 });

    await estado().respondToSpecialOffer("O1", false);

    expect(estado().acceptedSpecialOffer).toEqual(jaAceita);
  });
});
