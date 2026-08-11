import { beforeEach, describe, expect, it, vi } from "vitest";

// Os dois mocks precisam existir ANTES do import dinâmico que o módulo faz lá dentro.
const invoke = vi.fn(() => Promise.resolve());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

// A detecção do shell entra por MOCK do módulo canônico, e não mexendo no global do Tauri:
// `tauri-detection-single-source.test.mjs` proíbe a segunda cópia dessa detecção em qualquer
// arquivo de `src/`, teste incluído, e ela é o gate que decide se um `invoke` acontece.
const noTauri = vi.fn(() => true);
vi.mock("./tauri", () => ({ estaNoTauri: () => noTauri() }));

// O registro do rádio tem uma obrigação acima de todas: nunca atrapalhar a fala que está
// medindo. Os testes aqui são sobre isso — o que ele faz fora do Tauri, o que faz quando o
// `invoke` explode, e se a primeira fala sobrevive ao import dinâmico.

async function carregarModulo() {
  vi.resetModules();
  return import("./radioRegistro");
}

/** Deixa as microtarefas do import dinâmico rodarem. */
const assentar = () => new Promise((r) => setTimeout(r, 0));

describe("radioRegistro", () => {
  beforeEach(() => {
    invoke.mockClear();
    invoke.mockImplementation(() => Promise.resolve());
    noTauri.mockReturnValue(true);
    localStorage.clear();
  });

  it("fora do shell do Tauri não chama nada", async () => {
    noTauri.mockReturnValue(false);
    const mod = await carregarModulo();
    mod.registrar("spotter", { chaves: ["esquerda"] });
    await assentar();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("dentro do shell manda o registro com canal, fase e desfecho", async () => {
    const mod = await carregarModulo();
    mod.registrar("spotter", { chaves: ["esquerda"], detalhe: { dur_s: 1.2 } });
    await assentar();
    expect(invoke).toHaveBeenCalledTimes(1);
    const [comando, args] = invoke.mock.calls[0];
    expect(comando).toBe("radio_registrar");
    expect(args.registro).toMatchObject({
      canal: "spotter",
      fase: "tocada",
      desfecho: "ok",
      chaves: ["esquerda"],
    });
  });

  it("a primeira fala não se perde enquanto o módulo do Tauri carrega", async () => {
    // O `invoke` chega por import dinâmico, então a primeira chamada acontece antes de ele
    // existir. Ela tem de sair depois, e não sumir — é justamente a fala que diz quando o
    // rádio abriu.
    const mod = await carregarModulo();
    mod.registrar("spotter", { chaves: ["teste"] });
    mod.registrar("spotter", { chaves: ["direita"] });
    await assentar();
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(invoke.mock.calls[0][1].registro.chaves).toEqual(["teste"]);
    expect(invoke.mock.calls[1][1].registro.chaves).toEqual(["direita"]);
  });

  it("um invoke que rejeita não vira erro não tratado nem exceção", async () => {
    invoke.mockImplementation(() => Promise.reject(new Error("sem backend")));
    const mod = await carregarModulo();
    expect(() => mod.registrar("quebra", { texto: "Cooper está fora." })).not.toThrow();
    await assentar();
    expect(invoke).toHaveBeenCalled();
  });

  it("um invoke que lança na hora também é engolido", async () => {
    invoke.mockImplementation(() => {
      throw new Error("ponte morta");
    });
    const mod = await carregarModulo();
    mod.registrar("spotter", { chaves: ["livre"] });
    await assentar();
    expect(() => mod.registrar("spotter", { chaves: ["livre"] })).not.toThrow();
  });

  it("desligado por preferência, não registra nada", async () => {
    const mod = await carregarModulo();
    mod.ligar(false);
    expect(mod.estaLigado()).toBe(false);
    mod.registrar("spotter", { chaves: ["esquerda"] });
    await assentar();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("aceita uma chave solta em vez de lista", async () => {
    const mod = await carregarModulo();
    mod.registrar("aviso", { chaves: "meu_motor" });
    await assentar();
    expect(invoke.mock.calls[0][1].registro.chaves).toEqual(["meu_motor"]);
  });
});
