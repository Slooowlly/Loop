import { describe, expect, it } from "vitest";
import { formatTowerPosition, pinsFor, shortDriverName } from "./towerCanvas";

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
