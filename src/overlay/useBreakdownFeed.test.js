// A ENTREGA dos canais do engenheiro: poll, cutucão e o cursor entre os dois.
//
// O modo de falha aqui não parece falha. Em corrida a janela do webview fica coberta pelo
// simulador ou minimizada, e o navegador estrangula o `setInterval` — medido em 11/08/2026, o
// intervalo de 800 ms virou 20 segundos. O Rust registra o evento no `loop.log`, o cursor está
// correto, ninguém erra nada, e o jogador simplesmente não ouve.
//
// Por isso o intervalo de poll destes casos é ABSURDO de propósito (um minuto). Com ele, uma
// mensagem que chega ao estado só pode ter vindo pela outra porta — que é exatamente o que se
// quer provar.

import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/tauri", () => ({ estaNoTauri: () => true }));

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args) => invoke(...args) }));

// O ouvinte do Tauri, capturado. É por ele que o teste faz o papel do Rust.
const ouvintes = [];
vi.mock("@tauri-apps/api/event", () => ({
  listen: async (nome, cb) => {
    const inscrito = { nome, cb };
    ouvintes.push(inscrito);
    return () => {
      const i = ouvintes.indexOf(inscrito);
      if (i >= 0) ouvintes.splice(i, 1);
    };
  },
}));

const { usePlayerWarnings, useClassificacaoFeed } = await import("./useBreakdownFeed");

/** O Rust cutucando o canal. */
async function cutucar(canal) {
  await act(async () => {
    for (const o of [...ouvintes]) {
      if (o.nome === "radio-cutucao") o.cb({ payload: canal });
    }
    await Promise.resolve();
  });
}

/** Espera o `listen` assíncrono ter registrado o ouvinte. */
const ouvindo = () => waitFor(() => expect(ouvintes.length).toBeGreaterThan(0));

const aviso = (id) => ({ id, text: `aviso ${id}`, pecas: [`p${id}`] });

// Um poll que, na prática, nunca acontece — é a janela coberta.
const ESTRANGULADO = { intervalMs: 60000 };

beforeEach(() => {
  invoke.mockReset();
  ouvintes.length = 0;
});

describe("o cutucão entrega o que o poll estrangulado não entregaria", () => {
  it("a mensagem chega sem que o intervalo de poll tenha corrido", async () => {
    let feed = [];
    invoke.mockImplementation(async () => feed);

    const { result } = renderHook(() => usePlayerWarnings(true, ESTRANGULADO));
    // A 1ª leitura só ANCORA: nada do que aconteceu antes de a tela abrir deve soar agora.
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
    expect(result.current).toBeNull();
    await ouvindo();

    feed = [aviso(1)];
    await cutucar("aviso");

    await waitFor(() => expect(result.current?.id).toBe(1));
  });

  it("cutucão de OUTRO canal não move este", async () => {
    // Os cinco canais compartilham o evento e se separam pelo nome. Um filtro frouxo faria
    // cada quebra na grade disparar uma leitura dos outros quatro comandos, 60 vezes por corrida.
    invoke.mockImplementation(async () => [aviso(1)]);
    renderHook(() => usePlayerWarnings(true, ESTRANGULADO));
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
    await ouvindo();

    await cutucar("quebra");
    await cutucar("ritmo");
    await cutucar("ocasiao");
    await cutucar("classificacao");

    expect(invoke).toHaveBeenCalledTimes(1);
  });
});

describe("o canal de avisos drena um por vez", () => {
  it("dois avisos entre duas leituras e nenhum se perde", async () => {
    // Este canal fala do carro do JOGADOR — peça em risco, quebra, castigo da classificação.
    // Enquanto ele saltava para a mais nova, dois avisos caindo entre duas leituras faziam o
    // primeiro desaparecer: sem card, sem áudio e sem erro em lugar nenhum. E, diferente das
    // quebras da grade, o Rust não funde os simultâneos.
    let feed = [];
    invoke.mockImplementation(async () => feed);

    const { result } = renderHook(() => usePlayerWarnings(true, ESTRANGULADO));
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
    await ouvindo();

    feed = [aviso(1), aviso(2)];
    await cutucar("aviso");
    await waitFor(() => expect(result.current?.id).toBe(1));

    await cutucar("aviso");
    await waitFor(() => expect(result.current?.id).toBe(2));
  });
});

describe("o canal da classificação chega à UI", () => {
  it("a fala que o Rust põe no feed vira a mensagem do card", async () => {
    // O canal existia pela metade: `quali.rs` decidia a fala, empilhava no `classificacao_log`
    // e ninguém lia esse log — nenhum comando, nenhum hook, nenhum card. Este caso é a ponta
    // do front da costura; a do Rust está em `commands/overlay/tests/mod.rs`.
    let feed = [];
    invoke.mockImplementation(async () => feed);

    const { result } = renderHook(() => useClassificacaoFeed(true, ESTRANGULADO));
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
    expect(invoke).toHaveBeenCalledWith("get_classificacao_feed", undefined);
    expect(result.current).toBeNull(); // a 1ª leitura só ancora
    await ouvindo();

    feed = [{ id: 7, severity: "quali", text: "Vamos lá.", pecas: ["cl_despedida_0"] }];
    await cutucar("classificacao");

    await waitFor(() => expect(result.current?.text).toBe("Vamos lá."));
    expect(result.current.pecas).toEqual(["cl_despedida_0"]);
  });

  it("drena uma por vez — a despedida não engole o comentário da volta morta", async () => {
    // As duas falas desta família são eventos DISCRETOS, e o Rust não funde nada aqui. Saltar
    // para a mais nova faria a primeira sumir sem card e sem áudio.
    let feed = [];
    invoke.mockImplementation(async () => feed);

    const { result } = renderHook(() => useClassificacaoFeed(true, ESTRANGULADO));
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
    await ouvindo();

    feed = [
      { id: 1, text: "Vamos lá.", pecas: ["cl_despedida_0"] },
      { id: 2, text: "Perdeu essa.", pecas: ["cl_perdeu_0"] },
    ];
    await cutucar("classificacao");
    await waitFor(() => expect(result.current?.id).toBe(1));

    await cutucar("classificacao");
    await waitFor(() => expect(result.current?.id).toBe(2));
  });
});

describe("duas portas, um cursor", () => {
  it("leituras concorrentes não entregam a mesma mensagem duas vezes", async () => {
    // O cursor é uma leitura seguida de escrita com um `await` no meio. Agora que há poll E
    // cutucão batendo no mesmo tique, duas leituras em voo leriam o mesmo `seen` e o mesmo
    // aviso soaria duas vezes.
    let soltar;
    let feed = [];
    invoke.mockImplementation(
      () =>
        new Promise((r) => {
          soltar = () => r(feed);
        }),
    );

    const vistos = [];
    const { result, rerender } = renderHook(() => usePlayerWarnings(true, ESTRANGULADO));
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
    await ouvindo();
    await act(async () => {
      soltar(); // a âncora
      await Promise.resolve();
    });

    feed = [aviso(1)];
    // Dois cutucões com a leitura ainda no ar: o segundo marca a vez em vez de abrir outra.
    await cutucar("aviso");
    await cutucar("aviso");
    expect(invoke).toHaveBeenCalledTimes(2);

    await act(async () => {
      soltar();
      await Promise.resolve();
    });
    rerender();
    vistos.push(result.current?.id);

    // A leitura marcada roda no fim da anterior, e o cursor já sabe que o 1 saiu.
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(3));
    await act(async () => {
      soltar();
      await Promise.resolve();
    });
    rerender();
    vistos.push(result.current?.id);

    expect(vistos).toEqual([1, 1]); // a mesma mensagem, entregue uma vez só
  });
});
