import { describe, expect, it } from "vitest";
import {
  formatSessionClock,
  formatTowerPosition,
  pinsFor,
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
