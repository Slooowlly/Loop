import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

import { COMANDOS_DA_PRE_CORRIDA, buscarDadosDaPreCorrida } from "./preRaceFetch";

// O retrato estático da etapa. Dois consumidores leem daqui — o pré-carregamento durante
// a animação do calendário e a Sala de Estratégia ao montar —, e o que se testa aqui é a
// DEGRADAÇÃO: quais comandos podem falhar sem derrubar a tela e qual forma sobra quando
// falham. Um `null` escapando daqui vira tela quebrada na Sala.

const RETRATO_OK = {
  get_drivers_by_category: [{ id: "D1", nome: "Piloto" }],
  get_teams_standings: [{ id: "T1", nome: "Equipe" }],
  get_briefing_phrase_history: { season_number: 4, entries: [{ driver_id: "D1" }] },
  get_breakdown_forecast: { available: true, risco: 0.2 },
  get_grid_breakdown_risk: ["T7"],
  get_weekend_modifiers: [{ driver_id: "D1", forma: 3 }],
};

/// Responde o retrato completo, com as substituições pedidas (valor ou `Promise.reject`).
function comBackend(overrides = {}) {
  invoke.mockImplementation((comando) => {
    if (comando in overrides) return Promise.resolve(overrides[comando]).then((v) => v);
    return Promise.resolve(RETRATO_OK[comando] ?? null);
  });
}

describe("COMANDOS_DA_PRE_CORRIDA", () => {
  it("é a lista real do que a função invoca — a documentação não pode mentir", async () => {
    comBackend();
    await buscarDadosDaPreCorrida({ careerId: "C1", categoria: "gt3" });

    const invocados = invoke.mock.calls.map(([comando]) => comando);
    expect([...invocados].sort()).toEqual([...COMANDOS_DA_PRE_CORRIDA].sort());
  });
});

describe("buscarDadosDaPreCorrida", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("passa a categoria só para os comandos que classificam por ela", async () => {
    comBackend();
    await buscarDadosDaPreCorrida({ careerId: "C1", categoria: "gt3" });

    // Um `category` renomeado no DTO não é pego pelo guard de invoke — é pego aqui.
    expect(invoke).toHaveBeenCalledWith("get_drivers_by_category", {
      careerId: "C1",
      category: "gt3",
    });
    expect(invoke).toHaveBeenCalledWith("get_teams_standings", {
      careerId: "C1",
      category: "gt3",
    });
    expect(invoke).toHaveBeenCalledWith("get_breakdown_forecast", { careerId: "C1" });
    expect(invoke).toHaveBeenCalledWith("get_grid_breakdown_risk", { careerId: "C1" });
    expect(invoke).toHaveBeenCalledWith("get_weekend_modifiers", { careerId: "C1" });
  });

  it("devolve o retrato inteiro quando tudo responde", async () => {
    comBackend();
    const retrato = await buscarDadosDaPreCorrida({ careerId: "C1", categoria: "gt3" });

    expect(retrato.driverStandings).toHaveLength(1);
    expect(retrato.teamStandings).toHaveLength(1);
    expect(retrato.phraseHistory.season_number).toBe(4);
    expect(retrato.breakdownForecast).toEqual({ available: true, risco: 0.2 });
    expect(retrato.breakdownRiskTeamIds).toEqual(["T7"]);
    expect(retrato.weekendModifierRows).toEqual([{ driver_id: "D1", forma: 3 }]);
  });

  it("guarda risco e modificadores como ARRAY — Set e Map não sobrevivem ao cache", async () => {
    comBackend();
    const retrato = await buscarDadosDaPreCorrida({ careerId: "C1", categoria: "gt3" });

    expect(Array.isArray(retrato.breakdownRiskTeamIds)).toBe(true);
    expect(Array.isArray(retrato.weekendModifierRows)).toBe(true);
  });

  it("previsão indisponível vira ausência, não um objeto com available:false", async () => {
    comBackend({ get_breakdown_forecast: { available: false } });
    const retrato = await buscarDadosDaPreCorrida({ careerId: "C1", categoria: "gt3" });
    expect(retrato.breakdownForecast).toBeNull();
  });

  it("os quatro comandos opcionais degradam sozinhos, sem derrubar a Sala", async () => {
    comBackend({
      get_briefing_phrase_history: Promise.reject(new Error("sem histórico")),
      get_breakdown_forecast: Promise.reject(new Error("sem previsão")),
      get_grid_breakdown_risk: Promise.reject(new Error("comando não registrado")),
      get_weekend_modifiers: Promise.reject(new Error("save sem etapa pendente")),
    });

    const retrato = await buscarDadosDaPreCorrida({ careerId: "C1", categoria: "gt3" });

    expect(retrato.driverStandings).toHaveLength(1);
    expect(retrato.phraseHistory).toEqual({ season_number: 0, entries: [] });
    expect(retrato.breakdownForecast).toBeNull();
    expect(retrato.breakdownRiskTeamIds).toEqual([]);
    expect(retrato.weekendModifierRows).toEqual([]);
  });

  it("as duas classificações SÃO a Sala: falha nelas sobe para quem chamou", async () => {
    comBackend({ get_drivers_by_category: Promise.reject(new Error("banco travado")) });
    await expect(
      buscarDadosDaPreCorrida({ careerId: "C1", categoria: "gt3" }),
    ).rejects.toThrow("banco travado");

    invoke.mockReset();
    comBackend({ get_teams_standings: Promise.reject(new Error("banco travado")) });
    await expect(
      buscarDadosDaPreCorrida({ careerId: "C1", categoria: "gt3" }),
    ).rejects.toThrow("banco travado");
  });

  it("resposta fora de forma vira o vazio esperado, e não `undefined` na tela", async () => {
    comBackend({
      get_drivers_by_category: null,
      get_teams_standings: "isto não é uma lista",
      get_briefing_phrase_history: { season_number: 9 },
      get_grid_breakdown_risk: null,
      get_weekend_modifiers: { nada: true },
    });

    const retrato = await buscarDadosDaPreCorrida({ careerId: "C1", categoria: "gt3" });

    expect(retrato.driverStandings).toEqual([]);
    expect(retrato.teamStandings).toEqual([]);
    // Histórico sem `entries` é histórico quebrado: vira o vazio canônico.
    expect(retrato.phraseHistory).toEqual({ season_number: 0, entries: [] });
    expect(retrato.breakdownRiskTeamIds).toEqual([]);
    expect(retrato.weekendModifierRows).toEqual([]);
  });

  it("dispara os comandos em paralelo — a Sala não espera seis idas em fila", async () => {
    let emVoo = 0;
    let pico = 0;
    invoke.mockImplementation((comando) => {
      emVoo += 1;
      pico = Math.max(pico, emVoo);
      return new Promise((resolve) => {
        setTimeout(() => {
          emVoo -= 1;
          resolve(RETRATO_OK[comando] ?? null);
        }, 0);
      });
    });

    await buscarDadosDaPreCorrida({ careerId: "C1", categoria: "gt3" });
    expect(pico).toBe(COMANDOS_DA_PRE_CORRIDA.length);
  });
});
