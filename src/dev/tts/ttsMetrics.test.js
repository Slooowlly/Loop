import { describe, it, expect } from "vitest";
import {
  percentil,
  mediana,
  fracaoAbaixo,
  resumir,
  classificar,
  faseDaChamada,
  VEREDITOS,
} from "./ttsMetrics";

function reg(msPrimeiroSom, extras = {}) {
  return { sucesso: true, msPrimeiroSom, interrupcoes: 0, ...extras };
}

describe("percentis", () => {
  it("usa posto mais próximo, sem interpolar", () => {
    const v = [100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];
    expect(percentil(v, 50)).toBe(500);
    expect(percentil(v, 90)).toBe(900);
    expect(percentil(v, 95)).toBe(1000);
  });

  it("ignora valores não numéricos", () => {
    // Sobram [100, 300]; posto 1 de 2 é o 100.
    expect(mediana([100, null, 300, undefined, NaN])).toBe(100);
    expect(mediana([100, null, 300, 500, NaN])).toBe(300);
  });

  it("devolve nulo sem amostra", () => {
    expect(percentil([], 95)).toBeNull();
    expect(fracaoAbaixo([], 1000)).toBeNull();
  });
});

describe("resumir", () => {
  it("separa falhas dos sucessos e conta as faixas", () => {
    const registros = [
      reg(700),
      reg(900),
      reg(1400),
      reg(1900),
      reg(3500),
      { sucesso: false, msPrimeiroSom: null, erro: "HTTP 429" },
    ];
    const r = resumir(registros);
    expect(r.total).toBe(6);
    expect(r.sucessos).toBe(5);
    expect(r.falhas).toBe(1);
    expect(r.abaixo1000).toBeCloseTo(2 / 5);
    expect(r.abaixo1500).toBeCloseTo(3 / 5);
    expect(r.abaixo2000).toBeCloseTo(4 / 5);
    expect(r.acima3000).toBeCloseTo(1 / 5);
    expect(r.melhor).toBe(700);
    expect(r.pior).toBe(3500);
  });
});

describe("classificar (Etapa 8)", () => {
  const encher = (valor, n = 20, extras = {}) =>
    Array.from({ length: n }, () => reg(valor, extras));

  it("não conclui nada com amostra curta", () => {
    expect(classificar(resumir(encher(500, 4)))).toBe(VEREDITOS.indefinido);
  });

  it("aprova como excelente com mediana e P95 baixos", () => {
    expect(classificar(resumir(encher(600))).id).toBe("excelente");
  });

  it("cai para viável quando o P95 passa de 1,5 s", () => {
    // Com n=20 o P95 é o 19º valor; dois lentos bastam para puxá-lo.
    const registros = [...encher(600, 18), reg(1800), reg(1800)];
    expect(classificar(resumir(registros)).id).toBe("viavel");
  });

  it("cai para antecipada com mediana perto de 1,5 s", () => {
    expect(classificar(resumir(encher(1600))).id).toBe("antecipada");
  });

  it("reprova com mediana acima de 2,5 s", () => {
    expect(classificar(resumir(encher(2800))).id).toBe("inadequado");
  });

  it("reprova por taxa de falha mesmo com tempos bons", () => {
    const registros = [
      ...encher(600, 18),
      { sucesso: false, msPrimeiroSom: null },
      { sucesso: false, msPrimeiroSom: null },
    ];
    expect(classificar(resumir(registros)).id).toBe("inadequado");
  });

  it("não deixa a reprodução picotada passar por excelente", () => {
    // Tempo ótimo, mas metade das falas com corte audível.
    const registros = [...encher(600, 10), ...encher(600, 10, { interrupcoes: 2 })];
    expect(classificar(resumir(registros)).id).not.toBe("excelente");
  });
});

describe("faseDaChamada", () => {
  it("marca a primeira do processo", () => {
    expect(faseDaChamada({ primeiraDoProcesso: true, msDesdeUltima: null })).toBe("primeira");
  });

  it("chama de fria a que veio depois de dois minutos parada", () => {
    expect(faseDaChamada({ primeiraDoProcesso: false, msDesdeUltima: 180000 })).toBe("fria");
  });

  it("chama de sequência a que veio logo em seguida", () => {
    expect(faseDaChamada({ primeiraDoProcesso: false, msDesdeUltima: 1500 })).toBe("sequencia");
  });
});
