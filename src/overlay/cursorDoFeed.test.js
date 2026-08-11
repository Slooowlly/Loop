import { describe, it, expect } from "vitest";
import { passoDoCursor } from "./cursorDoFeed";

const msg = (id) => ({ id, text: `m${id}` });

describe("cursor dos feeds do overlay", () => {
  it("ancora na primeira leitura sem exibir o que já existia", () => {
    const r = passoDoCursor({ feed: [msg(3), msg(7)], seen: -1, primed: false });
    expect(r.mostrar).toBeNull();
    expect(r.seen).toBe(7);
    expect(r.primed).toBe(true);
  });

  it("a primeira fala de uma sessão NÃO é engolida pela âncora", () => {
    // O defeito real, medido em 2026-08-11: os logs do monitor nascem vazios a cada
    // tentativa. Ancorando só na primeira leitura não vazia, a âncora ficava pendente e
    // consumia a mensagem inaugural — o castigo da classificação apareceu no log do Rust e
    // nunca no rádio.
    let e = { feed: [], seen: -1, primed: false };
    let r = passoDoCursor(e);
    expect(r.primed).toBe(true);
    expect(r.mostrar).toBeNull();

    r = passoDoCursor({ feed: [msg(0)], seen: r.seen, primed: r.primed });
    expect(r.mostrar).toEqual(msg(0));
    expect(r.seen).toBe(0);
  });

  it("não repete a mensagem já exibida", () => {
    const r = passoDoCursor({ feed: [msg(4)], seen: 4, primed: true });
    expect(r.mostrar).toBeNull();
    expect(r.seen).toBe(4);
  });

  it("drena uma por vez quando pedido, sem pular a do meio", () => {
    // Duas quebras entre dois polls: pular para a mais nova sumia com a primeira, contra a
    // regra de que toda quebra fala.
    const feed = [msg(1), msg(2), msg(3)];
    const a = passoDoCursor({ feed, seen: 0, primed: true, umaPorVez: true });
    expect(a.mostrar).toEqual(msg(1));
    const b = passoDoCursor({ feed, seen: a.seen, primed: true, umaPorVez: true });
    expect(b.mostrar).toEqual(msg(2));
  });

  it("sem `umaPorVez`, vale a mais nova", () => {
    const feed = [msg(1), msg(2), msg(3)];
    const r = passoDoCursor({ feed, seen: 0, primed: true });
    expect(r.mostrar).toEqual(msg(3));
    expect(r.seen).toBe(3);
  });

  it("um log esvaziado não faz o cursor voltar atrás", () => {
    // O `radio_epoch` do Rust garante que os ids nunca recuam; aqui garantimos que o cursor
    // não se rearma sozinho ao ver o log vazio no meio do caminho.
    const r = passoDoCursor({ feed: [], seen: 9, primed: true });
    expect(r.seen).toBe(9);
    expect(r.primed).toBe(true);
    expect(r.mostrar).toBeNull();
  });
});
