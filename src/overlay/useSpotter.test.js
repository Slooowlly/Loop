// O anúncio do spotter chega ao front por dois caminhos: o evento `spotter-evento`,
// que o Rust empurra no instante da confirmação, e o poll de rede. O empurrão existe
// porque em corrida a janela do webview fica coberta pelo simulador e o navegador
// estrangula o `setInterval` — a fala chegava tarde ou não chegava.
//
// Estes testes travam as duas coisas que um push mal ligado quebra em silêncio: o
// evento precisa virar anúncio (senão o mecanismo fica sem consumidor, que é o
// estado de onde ele saiu), e o mesmo id não pode soar duas vezes quando o poll
// devolve o que o push já entregou.

import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

// Assinantes do evento, na ordem em que se registraram.
let assinantes;
let desinscritos;

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (nome, cb) => {
    assinantes.push({ nome, cb });
    return () => {
      desinscritos.push(nome);
    };
  }),
}));

vi.mock("../lib/tauri", () => ({ estaNoTauri: () => true }));

import { useNovosAnuncios, EVENTO_SPOTTER } from "./useSpotter";

/** Dispara o evento como o Rust dispara: um `EventoSpotter` por vez. */
function empurrar(evento) {
  for (const a of assinantes) {
    if (a.nome === EVENTO_SPOTTER) a.cb({ payload: evento });
  }
}

const evento = (id, chave) => ({ id, chave, sessao_s: 0, duracao_s: 0 });

/** Snapshot do poll, no formato de `EstadoSpotter`. */
const snapshot = (eventos) => ({
  bruto: 0,
  vizinhanca: "livre",
  ao_lado: false,
  tres_largos: false,
  esquerda: false,
  direita: false,
  eventos,
});

describe("useNovosAnuncios", () => {
  beforeEach(() => {
    assinantes = [];
    desinscritos = [];
  });

  it("assina o evento que o Rust empurra", async () => {
    await act(async () => {
      renderHook(() => useNovosAnuncios(null));
    });
    expect(assinantes.map((a) => a.nome)).toContain(EVENTO_SPOTTER);
  });

  it("entrega o anúncio empurrado mesmo sem nenhum poll ter chegado", async () => {
    let hook;
    await act(async () => {
      hook = renderHook(() => useNovosAnuncios(null));
    });
    act(() => empurrar(evento(7, "esquerda")));
    expect(hook.result.current.map((e) => e.chave)).toEqual(["esquerda"]);
  });

  it("não repete o anúncio quando o poll traz o que o push já entregou", async () => {
    let hook;
    await act(async () => {
      hook = renderHook(({ estado }) => useNovosAnuncios(estado), {
        initialProps: { estado: null },
      });
    });
    act(() => empurrar(evento(7, "esquerda")));
    expect(hook.result.current.map((e) => e.id)).toEqual([7]);

    // O poll chega depois com o MESMO evento, mais um que só ele viu.
    await act(async () => {
      hook.rerender({ estado: snapshot([evento(7, "esquerda"), evento(8, "livre")]) });
    });
    expect(hook.result.current.map((e) => e.id)).toEqual([8]);
  });

  it("desregistra o assinante ao desmontar", async () => {
    let hook;
    await act(async () => {
      hook = renderHook(() => useNovosAnuncios(null));
    });
    await act(async () => {
      hook.unmount();
    });
    expect(desinscritos).toContain(EVENTO_SPOTTER);
  });
});
