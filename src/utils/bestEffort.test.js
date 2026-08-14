import { vi, describe, it, expect, beforeEach } from "vitest";

// O helper que substituiu os `.catch(() => {})` do caminho de corrida (achado V6.2 da vistoria
// de 11/08/2026). Ele tem uma única promessa: engolir para a UI exatamente como o padrão antigo
// engolia, e deixar em disco a linha que o padrão antigo não deixava.
//
// O que precisa de trava aqui é o caminho de FALHA, porque ele roda quando nada mais está
// funcionando: um erro dentro do registro transformaria um erro engolido num erro visível — o
// oposto do combinado — e derrubaria a tela justamente na hora em que ela precisa aguentar.

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

import { bestEffort, bestEffortComRetorno } from "./bestEffort";

/// Os argumentos do registro, ou `null` se nada foi registrado.
function registro() {
  const chamada = invoke.mock.calls.find(([nome]) => nome === "diagnostico_registrar");
  return chamada ? chamada[1] : null;
}

describe("bestEffort", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(null);
  });

  it("devolve o valor e não registra nada quando dá certo", async () => {
    const valor = await bestEffort(Promise.resolve(42), "comando_x");
    expect(valor).toBe(42);
    expect(registro()).toBeNull();
  });

  it("resolve com undefined em vez de rejeitar quando falha", async () => {
    // É o contrato que o `.catch(() => {})` já tinha: quem encadeia um `.then` depois
    // precisa tolerar `undefined`, e nunca vê uma rejeição.
    const valor = await bestEffort(Promise.reject(new Error("sem app.ini")), "comando_x");
    expect(valor).toBeUndefined();
  });

  it("manda o rótulo e a mensagem para o log de diagnóstico", async () => {
    await bestEffort(Promise.reject(new Error("sem app.ini")), "iracing_install_yellow_macro");
    expect(registro()).toEqual({
      rotulo: "iracing_install_yellow_macro",
      mensagem: "sem app.ini",
    });
  });

  it("aceita o erro em STRING, que é como o Rust devolve", async () => {
    // Um comando Tauri devolve `Err(String)`, e ele chega ao front como string crua, sem
    // virar `Error`. Passar por `String(...)` cegamente daria "[object Object]" no caso do
    // objeto e perderia a mensagem aqui.
    await bestEffort(Promise.reject("Pasta do iRacing não encontrada"), "iracing_generate_roster");
    expect(registro().mensagem).toBe("Pasta do iRacing não encontrada");
  });

  it("não deixa a falha do próprio registro escapar", async () => {
    // O registro é o último recurso da cadeia. Se ELE rejeitar, não há para onde dizer — e
    // uma rejeição solta aqui vira "unhandled rejection" no webview durante a corrida.
    invoke.mockRejectedValue(new Error("ponte fora do ar"));
    await expect(bestEffort(Promise.reject(new Error("falhou")), "comando_x")).resolves.toBeUndefined();
  });

  it("sobrevive a um invoke que estoura de forma síncrona", async () => {
    // Fora do Tauri (navegador, teste sem mock) a ponte pode nem existir.
    invoke.mockImplementation(() => {
      throw new Error("sem ponte");
    });
    await expect(bestEffort(Promise.reject(new Error("falhou")), "comando_x")).resolves.toBeUndefined();
  });

  it("aceita um valor que não é promessa", async () => {
    await expect(bestEffort("pronto", "comando_x")).resolves.toBe("pronto");
  });
});

describe("bestEffortComRetorno", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(null);
  });

  it("entrega o valor com ok verdadeiro", async () => {
    await expect(bestEffortComRetorno(Promise.resolve(true), "comando_x")).resolves.toEqual({
      ok: true,
      valor: true,
    });
  });

  it("entrega a falha a quem chamou, e registra do mesmo jeito", async () => {
    // A diferença para o `bestEffort`: aqui o chamador PRECISA da falha para decidir o que
    // dizer na tela. O rastro em disco continua saindo sem ele pedir.
    const erro = new Error("simulador aberto");
    const r = await bestEffortComRetorno(Promise.reject(erro), "iracing_modo_janela_aplicar");
    expect(r.ok).toBe(false);
    expect(r.falha).toBe(erro);
    expect(registro().rotulo).toBe("iracing_modo_janela_aplicar");
  });
});
