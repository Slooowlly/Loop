import { describe, it, expect } from "vitest";
import ptCommon from "./locales/pt-BR/common.json";
import enCommon from "./locales/en-US/common.json";

// Achata { a: { b: "x" } } → { "a.b": "x" }. Guarda a paridade de chaves entre
// locales: como os testes rodam em PT por padrão, uma chave faltando no EN
// passaria batida (a UI mostraria a chave crua). Este teste falha nesse caso.
function flatEntries(obj, prefix = "") {
  return Object.entries(obj).flatMap(([k, v]) => {
    const key = prefix ? `${prefix}.${k}` : k;
    return v && typeof v === "object" && !Array.isArray(v)
      ? flatEntries(v, key)
      : [[key, v]];
  });
}

describe("paridade de locale (common)", () => {
  it("pt-BR e en-US têm exatamente o mesmo conjunto de chaves", () => {
    const pt = new Set(flatEntries(ptCommon).map(([k]) => k));
    const en = new Set(flatEntries(enCommon).map(([k]) => k));
    const missingInEn = [...pt].filter((k) => !en.has(k)).sort();
    const missingInPt = [...en].filter((k) => !pt.has(k)).sort();
    expect({ missingInEn, missingInPt }).toEqual({ missingInEn: [], missingInPt: [] });
  });

  it("nenhum valor de tradução está vazio", () => {
    const empties = [...flatEntries(ptCommon), ...flatEntries(enCommon)]
      .filter(([, v]) => typeof v !== "string" || v.trim() === "")
      .map(([k]) => k);
    expect(empties).toEqual([]);
  });
});
