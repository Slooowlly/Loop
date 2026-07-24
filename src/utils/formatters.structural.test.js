import { describe, expect, it } from "vitest";

import { extractFlag, extractNationalityCode, formatSalary } from "./formatters";

// Estas verificações moravam no guard estrutural scripts/tests/driver-detail-modal.test.mjs,
// que importava formatters.js direto no node:test. Depois que o módulo passou a puxar a
// cadeia de i18n, o Node recusou o common.json por falta de import attribute — o Vite
// resolve JSON nativamente, então o lugar certo delas é aqui.
describe("formatters usados na ficha do piloto", () => {
  it("formata salário em dólar sem centavos", () => {
    expect(formatSalary(12500)).toBe("$12,500");
  });

  it("devolve traço quando não há valor", () => {
    expect(formatSalary(null)).toBe("-");
    expect(formatSalary(0)).toBe("$0");
  });

  it("extrai o código de nacionalidade de strings com código de país", () => {
    expect(extractNationalityCode("JP Japones")).toBe("jp");
  });

  it("resolve a bandeira em emoji a partir do código de país", () => {
    expect(extractFlag("JP Japones")).toBe("🇯🇵");
  });
});
