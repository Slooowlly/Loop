// O volume do rádio — um número, e três formas de ele virar silêncio.
//
// O que se testa aqui não é "guarda e lê": é o que acontece quando o valor guardado não
// presta. Um `NaN` num `gain.value` emudece o nó inteiro, e o sintoma seria o rádio parar
// de funcionar sem nada no console e sem ninguém ligar a causa ao efeito.

import { beforeEach, describe, expect, it, vi } from "vitest";

let vol;

beforeEach(async () => {
  vi.resetModules();
  localStorage.clear();
  vol = await import("./volumeRadio");
});

describe("o valor", () => {
  it("começa atenuado, e não em ganho 1", async () => {
    // O acervo sai da cadeia em quase escala cheia. Tocar isso a 1 por cima do jogo foi
    // exatamente a queixa que criou este controle.
    expect(vol.volumeRadio()).toBe(vol.VOLUME_PADRAO);
    expect(vol.VOLUME_PADRAO).toBeLessThan(1);
  });

  it("sobrevive à sessão", async () => {
    vol.definirVolume(0.65);
    vi.resetModules();
    const outra = await import("./volumeRadio");
    expect(outra.volumeRadio()).toBe(0.65);
  });

  it("um valor podre no storage volta ao padrão em vez de emudecer o rádio", async () => {
    localStorage.setItem("loop.volumeRadio", "alto pra caramba");
    vi.resetModules();
    const outra = await import("./volumeRadio");
    expect(outra.volumeRadio()).toBe(outra.VOLUME_PADRAO);
  });

  it("prende na faixa: nada abaixo de zero nem acima de um", () => {
    vol.definirVolume(4);
    expect(vol.volumeRadio()).toBe(1);
    vol.definirVolume(-2);
    expect(vol.volumeRadio()).toBe(0);
  });
});

describe("as duas bocas", () => {
  it("avisa quem assinou, para o volume mudar com a fala em curso", () => {
    const vistos = [];
    const desassinar = vol.aoMudarVolume((v) => vistos.push(v));
    vol.definirVolume(0.3);
    vol.definirVolume(0.8);
    desassinar();
    vol.definirVolume(0.1);
    expect(vistos).toEqual([0.3, 0.8]);
  });

  it("um assinante que explode não impede o outro de receber", () => {
    const vistos = [];
    vol.aoMudarVolume(() => {
      throw new Error("boca quebrada");
    });
    vol.aoMudarVolume((v) => vistos.push(v));
    vol.definirVolume(0.5);
    expect(vistos).toEqual([0.5]);
  });
});

describe("aplicarEm", () => {
  it("usa rampa quando o contexto agenda — degrau no meio da fala estala", () => {
    const chamadas = [];
    const no = {
      gain: {
        value: 1,
        cancelScheduledValues: (t) => chamadas.push(["cancela", t]),
        setValueAtTime: (v, t) => chamadas.push(["fixa", v, t]),
        linearRampToValueAtTime: (v, t) => chamadas.push(["rampa", v, t]),
      },
    };
    vol.aplicarEm(no, { currentTime: 10 }, 0.25);
    expect(chamadas).toEqual([
      ["cancela", 10],
      ["fixa", 1, 10],
      ["rampa", 0.25, 10.02],
    ]);
  });

  it("sem agendamento, cai no valor direto em vez de estourar", () => {
    const no = { gain: { value: 1 } };
    vol.aplicarEm(no, null, 0.25);
    expect(no.gain.value).toBe(0.25);
  });

  it("nó ainda não criado não é erro — o contexto de áudio nasce tarde", () => {
    expect(() => vol.aplicarEm(null, null, 0.5)).not.toThrow();
  });
});
