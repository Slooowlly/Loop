import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

import useCareerStore from "../useCareerStore";
import { initialState } from "./state";

// Slice de MERCADO, pelo lado da REENTRÂNCIA. `finalizePreseason` fecha a pré-temporada,
// monta o calendário e promove a fase da carreira: rodar duas vezes contra o mesmo save é
// dano, não desperdício. Ela era a única mutadora do store sem trava de voo, e chega ao
// jogador por três botões (o avanço de semana e os dois modais de confirmação da
// PreSeasonView), nenhum deles desabilitado enquanto a chamada roda.

const estado = () => useCareerStore.getState();

const CARREIRA = {
  career_id: "C1",
  difficulty: "Normal",
  player: { id: "P1" },
  player_team: { id: "T1", categoria: "gt3" },
  season: { id: "S2", numero: 5, fase: "Temporada" },
  next_race: null,
  total_drivers: 200,
  total_teams: 60,
};

/// Promessa que só resolve quando o teste mandar — é o que deixa os dois cliques
/// acontecerem com a primeira chamada ainda em voo.
function promessaControlada() {
  let resolver;
  let rejeitar;
  const promessa = new Promise((res, rej) => {
    resolver = res;
    rejeitar = rej;
  });
  return { promessa, resolver, rejeitar };
}

/// Backend com `finalize_preseason` pendurado na promessa controlada.
function backendComFinalizacaoPendurada(pendente) {
  invoke.mockImplementation((comando) => {
    if (comando === "finalize_preseason") return pendente;
    if (comando === "load_career") return Promise.resolve(CARREIRA);
    return Promise.resolve(null);
  });
}

function chamadasDe(comando) {
  return invoke.mock.calls.filter(([c]) => c === comando);
}

beforeEach(() => {
  invoke.mockReset();
  useCareerStore.setState({ ...initialState, careerId: "C1" });
});

describe("finalizePreseason", () => {
  it("dois cliques com a chamada em voo finalizam a pré-temporada UMA vez", async () => {
    const { promessa, resolver } = promessaControlada();
    backendComFinalizacaoPendurada(promessa);

    const primeiro = estado().finalizePreseason();
    const segundo = estado().finalizePreseason();

    expect(estado().isFinalizingPreseason).toBe(true);
    // O segundo clique desiste sem tocar no backend, e desiste em silêncio: quem o
    // disparou não errou nada, só chegou atrasado.
    await expect(segundo).resolves.toBeNull();
    expect(chamadasDe("finalize_preseason")).toHaveLength(1);

    resolver(null);
    await primeiro;

    expect(chamadasDe("finalize_preseason")).toHaveLength(1);
    expect(chamadasDe("load_career")).toHaveLength(1);
  });

  it("a trava cai no sucesso, e a temporada seguinte pode ser iniciada", async () => {
    const { promessa, resolver } = promessaControlada();
    backendComFinalizacaoPendurada(promessa);

    const emVoo = estado().finalizePreseason();
    resolver(null);
    await emVoo;

    expect(estado().isFinalizingPreseason).toBe(false);

    await estado().finalizePreseason();
    expect(chamadasDe("finalize_preseason")).toHaveLength(2);
  });

  it("a trava cai no erro — senão o botão fica morto até o jogador sair da carreira", async () => {
    const { promessa, rejeitar } = promessaControlada();
    backendComFinalizacaoPendurada(promessa);

    const emVoo = estado().finalizePreseason();
    rejeitar(new Error("propostas em aberto"));

    await expect(emVoo).rejects.toThrow("propostas em aberto");
    expect(estado().isFinalizingPreseason).toBe(false);
    expect(estado().error).toBeTruthy();

    // E a tentativa seguinte passa de novo pela ponte.
    invoke.mockImplementation((comando) => {
      if (comando === "load_career") return Promise.resolve(CARREIRA);
      return Promise.resolve(null);
    });
    await estado().finalizePreseason();
    expect(chamadasDe("finalize_preseason")).toHaveLength(2);
  });

  it("sem carreira aberta, recusa antes de acender a trava", async () => {
    useCareerStore.setState({ ...initialState, careerId: null });

    await expect(estado().finalizePreseason()).rejects.toThrow();
    expect(estado().isFinalizingPreseason).toBe(false);
    expect(chamadasDe("finalize_preseason")).toHaveLength(0);
  });
});
