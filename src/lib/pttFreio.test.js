// O freio do rádio, com o relógio na mão.
//
// O modo de falha perigoso aqui não é o freio deixar de frear — é ele frear DEMAIS. Um
// castigo que não solta transforma "um minuto de silêncio" em "o rádio parou de
// funcionar", e nada na tela diria por quê.

import { describe, expect, it } from "vitest";
import { CASTIGO_MS, criarFreio, JANELA_MS, LIMITE_AVISO, LIMITE_CORTE } from "./pttFreio";

/** Um freio com relógio de mentira. `t.avancar(ms)` move o tempo. */
function comRelogio() {
  let agora = 1_000_000;
  const freio = criarFreio({ agora: () => agora });
  return { freio, avancar: (ms) => (agora += ms) };
}

describe("a conta", () => {
  it("perguntas espaçadas nunca acionam nada", () => {
    const { freio, avancar } = comRelogio();
    for (let i = 0; i < 20; i += 1) {
      expect(freio.registrar()).toBe("ok");
      avancar(JANELA_MS / 2 + 1); // duas por janela: dentro do razoável
    }
  });

  it("a janela DESLIZA: o que saiu dela não conta mais", () => {
    const { freio, avancar } = comRelogio();
    for (let i = 0; i < LIMITE_AVISO - 1; i += 1) freio.registrar();
    avancar(JANELA_MS + 1);
    // A janela virou. A próxima é a primeira de novo, e não a que estouraria o aviso.
    expect(freio.registrar()).toBe("ok");
    expect(freio.naJanela()).toBe(1);
  });
});

describe("o empurrão", () => {
  it("sai na quarta pergunta do minuto, junto com a resposta", () => {
    const { freio } = comRelogio();
    for (let i = 1; i < LIMITE_AVISO; i += 1) expect(freio.registrar()).toBe("ok");
    expect(freio.registrar()).toBe("aviso");
  });

  it("sai UMA vez por janela — repetido vira a tagarelice que ele reclama", () => {
    const { freio } = comRelogio();
    for (let i = 1; i < LIMITE_AVISO; i += 1) freio.registrar();
    expect(freio.registrar()).toBe("aviso");
    expect(freio.registrar()).toBe("ok"); // a quinta não repete
  });
});

describe("o corte", () => {
  it("cala o rádio na sexta e devolve o tempo que falta", () => {
    const { freio } = comRelogio();
    for (let i = 1; i < LIMITE_CORTE; i += 1) freio.registrar();
    expect(freio.registrar()).toBe("corte");
    expect(freio.bloqueado()).toBe(true);
    expect(freio.restanteMs()).toBe(CASTIGO_MS);
  });

  it("SOLTA quando o minuto passa", () => {
    const { freio, avancar } = comRelogio();
    for (let i = 1; i <= LIMITE_CORTE; i += 1) freio.registrar();
    avancar(CASTIGO_MS - 1);
    expect(freio.bloqueado()).toBe(true);
    avancar(2);
    expect(freio.bloqueado()).toBe(false);
  });

  it("cumprida a pena, a conta recomeça limpa", () => {
    // O defeito que faria o castigo virar permanente: se a janela sobrevivesse ao corte,
    // a primeira pergunta depois do silêncio já a encontraria cheia e cairia noutro corte.
    const { freio, avancar } = comRelogio();
    for (let i = 1; i <= LIMITE_CORTE; i += 1) freio.registrar();
    avancar(CASTIGO_MS + 1);
    expect(freio.naJanela()).toBe(0);
    expect(freio.registrar()).toBe("ok");
  });
});

describe("zerar", () => {
  it("larga o castigo e a conta — trocou de sessão", () => {
    const { freio } = comRelogio();
    for (let i = 1; i <= LIMITE_CORTE; i += 1) freio.registrar();
    expect(freio.bloqueado()).toBe(true);
    freio.zerar();
    expect(freio.bloqueado()).toBe(false);
    expect(freio.naJanela()).toBe(0);
  });
});
