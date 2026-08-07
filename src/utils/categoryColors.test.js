import { describe, expect, it } from "vitest";

import { getCategoryColor } from "./categoryColors";

describe("categoryColors", () => {
  it("returns the shared championship palette for rookie and amateur categories", () => {
    expect(getCategoryColor("mazda_rookie")).toBe("#FFD400");
    expect(getCategoryColor("toyota_rookie")).toBe("#FFD400");
    expect(getCategoryColor("mazda_amador")).toBe("#E73F47");
    expect(getCategoryColor("toyota_amador")).toBe("#E73F47");
  });

  it("returns the shared championship palette for production and upper categories", () => {
    expect(getCategoryColor("bmw_m2")).toBe("#E00010");
    expect(getCategoryColor("production_challenger")).toBe("#8020D0");
    expect(getCategoryColor("gt4")).toBe("#2070F0");
    expect(getCategoryColor("gt3")).toBe("#00F0F0");
    expect(getCategoryColor("lmp2")).toBe("#F2CC60");
    expect(getCategoryColor("endurance")).toBe("#3fb950");
  });

  it("distingue as classes da Endurance, que sao divisoes que o piloto troca", () => {
    expect(getCategoryColor("endurance:gt3")).toBe("#3fb950");
    expect(getCategoryColor("endurance:lmp2")).toBe("#F2CC60");
    expect(getCategoryColor("endurance:lmp2")).not.toBe(getCategoryColor("endurance:gt3"));
    // A GT3 da Endurance NAO pode ser a mesma cor da GT3 solo: sao degraus
    // diferentes da piramide, e a escada existe justamente para mostrar a subida.
    expect(getCategoryColor("endurance:gt3")).not.toBe(getCategoryColor("gt3"));
  });

  it("cai na categoria-base quando a classe nao tem cor propria", () => {
    expect(getCategoryColor("production_challenger:mazda")).toBe("#8020D0");
    expect(getCategoryColor("production_challenger:bmw")).toBe("#8020D0");
  });

  it("devolve o fallback para chave vazia ou desconhecida", () => {
    expect(getCategoryColor("")).toBe("#58a6ff");
    expect(getCategoryColor(null)).toBe("#58a6ff");
    expect(getCategoryColor("categoria_que_nao_existe")).toBe("#58a6ff");
    expect(getCategoryColor("nada:nenhuma", "#123456")).toBe("#123456");
  });
});
