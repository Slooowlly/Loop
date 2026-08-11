import { describe, expect, it } from "vitest";
import {
  formatQualyGap,
  formatSessionClock,
  formatTowerPosition,
  pinsFor,
  sessionCounter,
  shortDriverName,
  towerLayout,
} from "./towerCanvas";
import { buildTowerSections } from "./towerRows";
import { OVERLAY_MOCK } from "./overlayMockData";

describe("towerLayout", () => {
  const sections = buildTowerSections(OVERLAY_MOCK);
  const { items, total } = towerLayout(sections);
  const carros = items.filter((it) => it.kind === "car");

  it("dá uma chave a cada linha e y crescente, sem repetir", () => {
    const chaves = carros.map((c) => c.key);
    expect(chaves.length).toBeGreaterThan(0);
    expect(new Set(chaves).size).toBe(chaves.length);
    const ys = items.map((it) => it.y);
    expect([...ys].sort((a, b) => a - b)).toEqual(ys);
  });

  it("mantém cada linha dentro do corpo da sua classe (o recorte do deslize)", () => {
    items.forEach((it) => {
      expect(it.y).toBeGreaterThanOrEqual(it.bodyTop);
      expect(it.y).toBeLessThan(it.bodyBottom);
    });
  });

  it("o total bate com a última linha (é a altura usada pela área de hover)", () => {
    const ultima = items[items.length - 1];
    expect(total).toBeGreaterThan(ultima.y);
  });
});


describe("formatSessionClock", () => {
  it.each([
    [0, "0:00"],
    [9, "0:09"],
    [192, "3:12"],
    [480, "8:00"], // quali padrão
    [479.9, "7:59"], // trunca: não pula pro minuto antes da hora
    [3600, "1:00:00"], // enduro passa a mostrar hora
    [4200, "1:10:00"],
  ])("formata %s s como %s", (secs, expected) => {
    expect(formatSessionClock(secs)).toBe(expected);
  });

  it("sem valor vira placeholder", () => {
    expect(formatSessionClock(undefined)).toBe("--:--");
    expect(formatSessionClock(null)).toBe("--:--");
    expect(formatSessionClock(-1)).toBe("--:--");
  });
});

describe("sessionCounter", () => {
  it("corrida conta VOLTA, com o total quando ele é conhecido", () => {
    expect(sessionCounter({ type: "R", lap: 36, totalLaps: 40 })).toEqual({
      label: "LAPS",
      big: "36",
      tail: "/40",
    });
    // Total desconhecido: só a volta, sem "/0".
    expect(sessionCounter({ type: "R", lap: 12, totalLaps: 0 }).tail).toBe("");
  });

  it("classificação conta TEMPO: decorrido sobre a duração", () => {
    expect(sessionCounter({ type: "Q", lap: 5, elapsedS: 192, durationS: 480 })).toEqual({
      label: "TIME",
      big: "3:12",
      tail: "/8:00",
    });
  });

  // A regressão que este teste tranca: com a duração ausente, a quali caía no ramo da
  // corrida e anunciava a VOLTA do líder — um número que não diz nada numa sessão de tempo.
  it("classificação sem duração cai no tempo RESTANTE, nunca na volta", () => {
    const c = sessionCounter({ type: "Q", lap: 5, totalLaps: 40, remainingS: 288 });
    expect(c).toEqual({ label: "LEFT", big: "4:48", tail: "" });
  });

  it("classificação sem tempo nenhum mostra placeholder, e ainda assim não mostra volta", () => {
    expect(sessionCounter({ type: "Q", lap: 5, totalLaps: 40 })).toEqual({
      label: "TIME",
      big: "--:--",
      tail: "",
    });
    // Duração conhecida e relógio ainda não: mantém a régua da sessão à vista.
    expect(sessionCounter({ type: "Q", lap: 5, durationS: 480 })).toEqual({
      label: "TIME",
      big: "--:--",
      tail: "/8:00",
    });
  });

  it("treino segue a mesma régua da classificação", () => {
    expect(sessionCounter({ type: "P", lap: 9, remainingS: 600 }).label).toBe("LEFT");
  });
});

describe("formatQualyGap", () => {
  it.each([
    [102_089, 102_089, "—"], // é a referência da classe
    [102_198, 102_089, "+0.109"],
    [104_589, 102_089, "+2.500"],
    [122_089, 102_089, "+20.00"], // acima de 10 s o milésimo sai: a coluna tem largura fixa
    [0, 102_089, "--"], // ainda não marcou volta
    [102_198, 0, "—"], // ninguém marcou: não há de quem se distanciar
    [165_089, 102_089, "+1:03.0"], // acima de um minuto muda de formato
  ])("intervalo de %s ms para %s ms vira %s", (bestMs, refMs, expected) => {
    expect(formatQualyGap(bestMs, refMs)).toBe(expected);
  });
});

describe("formatTowerPosition", () => {
  it.each([
    [1, "1"],
    [27, "27"],
    [0, "–"],
    [-1, "–"],
  ])("formata a posição %s como %s", (position, expected) => {
    expect(formatTowerPosition(position)).toBe(expected);
  });
});

describe("shortDriverName", () => {
  it.each([
    ["Matthew Koontz", "Matthew K."],
    ["Marco I D'Acunto", "Marco I."], // 3º nome em diante cai fora
    ["Pawel T. Okreglicki", "Pawel T."], // 2º já é inicial: não duplica o ponto
    ["Rick Van Zwiet", "Rick V."],
    ["Ayrton", "Ayrton"], // nome único fica inteiro
    ["  Neil   Cooper  ", "Neil C."], // espaços extras não atrapalham
    ["", ""],
  ])("encurta %s -> %s", (full, expected) => {
    expect(shortDriverName(full)).toBe(expected);
  });

  it("aguenta nome ausente", () => {
    expect(shortDriverName(undefined)).toBe("");
    expect(shortDriverName(null)).toBe("");
  });
});

describe("pinsFor", () => {
  const types = (car) => pinsFor(car).map((p) => p.type);

  it("carro limpo não tem pinos", () => {
    expect(pinsFor({})).toEqual([]);
  });

  it("alerta leve vira triângulo laranja; grave, vermelho", () => {
    expect(types({ alert: "light" })).toEqual(["alertLight"]);
    expect(types({ alert: "heavy" })).toEqual(["alertHeavy"]);
  });

  it("DNF usa a bandeira preta (não o triângulo)", () => {
    expect(types({ flag: "black" })).toEqual(["black"]);
  });

  it("tempo de pit NÃO é pino (é desenhado sobre os pneus)", () => {
    expect(types({ pitSecs: 23 })).toEqual([]);
  });

  it("o P (no box) vem ANTES do triângulo de alerta", () => {
    expect(types({ alert: "heavy", fol: true, pit: true })).toEqual([
      "pit",
      "alertHeavy",
      "fastest",
    ]);
  });
});
