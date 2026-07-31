import { describe, expect, it } from "vitest";

import { getReadableTeamColor, getVividTeamColor } from "./teamColors";

// Trava de regressão visual: esta tabela congela a saída exata das três telas que
// clareiam cor de equipe (grid de corrida, classificação e revista de notícias).
// Os parâmetros abaixo são os que cada tela usava quando a função era triplicada —
// se algum valor mudar, a cor na tela mudou junto.
const GRID = { fallback: "#58a6ff", mix: 0.58 };
const CLASSIFICACAO = { fallback: "#7d8590", mix: 0.58 };
const REVISTA = { fallback: "#d0d7e2", mix: 0.62 };

describe("getReadableTeamColor", () => {
  it("clareia cor escura com o fator de cada tela", () => {
    expect(getReadableTeamColor("#1a1a2e", GRID)).toBe("rgb(159, 159, 167)");
    expect(getReadableTeamColor("#1a1a2e", CLASSIFICACAO)).toBe("rgb(159, 159, 167)");
    // A revista clareia 4% a mais que as outras duas.
    expect(getReadableTeamColor("#1a1a2e", REVISTA)).toBe("rgb(168, 168, 176)");
  });

  it("devolve a cor original, e não rgb(), quando a luminância já é alta", () => {
    expect(getReadableTeamColor("#ffd400", GRID)).toBe("#ffd400");
    expect(getReadableTeamColor("#7d8590", CLASSIFICACAO)).toBe("#7d8590");
    // Maiúsculas passam na regex e voltam sem normalização.
    expect(getReadableTeamColor("#AABBCC", REVISTA)).toBe("#AABBCC");
  });

  it("trata o limiar 0.32 como exclusivo", () => {
    // #525252 tem luminância 0.3216 — fica de fora.
    expect(getReadableTeamColor("#525252", GRID)).toBe("#525252");
    // #515151 tem luminância 0.3176 — entra.
    expect(getReadableTeamColor("#515151", GRID)).toBe("rgb(182, 182, 182)");
  });

  it("cai no fallback de cada tela quando a cor é ausente ou inválida", () => {
    expect(getReadableTeamColor(null, GRID)).toBe("#58a6ff");
    expect(getReadableTeamColor("", CLASSIFICACAO)).toBe("#7d8590");
    expect(getReadableTeamColor("vermelho", REVISTA)).toBe("#d0d7e2");
    expect(getReadableTeamColor("#FFF", GRID)).toBe("#58a6ff");
    expect(getReadableTeamColor("#1a1a2e77", GRID)).toBe("#58a6ff");
  });

  it("usa cinza neutro e 0.58 quando ninguém passa opções", () => {
    expect(getReadableTeamColor(null)).toBe("#c9d1d9");
    expect(getReadableTeamColor("#1a1a2e")).toBe("rgb(159, 159, 167)");
  });
});

// A cor da equipe quando ela vira DADO num gráfico escuro. Clarear resolve
// visibilidade; não resolve identidade — e num dossiê inteiro pintado de cinza
// não dá para distinguir a linha da equipe da grade que está atrás dela.
describe("getVividTeamColor", () => {
  it("devolve o matiz a uma cor escura e lavada", () => {
    // #2f3542 (Thunderline Academy) é azul de 17% de saturação e luminância 0.21:
    // clarear sozinho dava rgb(172, 174, 179), ou seja, cinza claro.
    expect(getVividTeamColor("#2f3542")).toBe("rgb(98, 133, 209)");
  });

  it("não toca em quem já é vivo e já contrasta", () => {
    // O ciano da Track Day Heroes passa dos 4,5:1 contra o card e tem saturação
    // acima do piso — sai igual ao que entrou.
    expect(getVividTeamColor("#37b6c9")).toBe("rgb(55, 182, 201)");
  });

  it("não inventa matiz para cinza de verdade", () => {
    // Sem matiz não há identidade a resgatar: só clareia até dar para ver.
    expect(getVividTeamColor("#808080")).toBe("rgb(133, 133, 133)");
    expect(getVividTeamColor("#000000")).toBe("rgb(133, 133, 133)");
  });

  it("cai no cinza neutro quando a cor é ausente ou inválida", () => {
    expect(getVividTeamColor(null)).toBe("#c9d1d9");
    expect(getVividTeamColor("#FFF")).toBe("#c9d1d9");
    expect(getVividTeamColor("azul", { fallback: "#58a6ff" })).toBe("#58a6ff");
  });
});
