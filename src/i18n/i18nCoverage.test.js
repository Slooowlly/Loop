import { describe, it, expect } from "vitest";
import { runAudit } from "../../scripts/i18nAudit.mjs";

// Guarda de COBERTURA de i18n: falha quando aparece texto de UI em português que ainda
// não passou por `t()` — o aviso de "tem coisa nova pra traduzir". Roda junto com a suíte
// (`npm run test:ui`); vermelho = pendência. Complementa o `localeParity.test.js` (que
// garante que as chaves pt/en batem) cuidando do passo ANTERIOR: virar string em chave.
//
// Ao adicionar UI nova: envolva em t("chave") + adicione a chave nos DOIS common.json.
// Se a string PT é intencional (dado de exemplo, placeholder de gate, endônimo de idioma),
// marque com {/* i18n-ignore */} na linha ou // i18n-ignore-file no topo do arquivo.
describe("i18n coverage (UI sem string PT crua fora de t())", () => {
  it("não há strings de UI em português não traduzidas", () => {
    const violations = runAudit();
    if (violations.length > 0) {
      const list = violations.map((v) => `  ${v.file}:${v.line}  "${v.text}"`).join("\n");
      throw new Error(
        `${violations.length} string(s) de UI em português fora de t():\n${list}\n\n` +
          `→ Envolva em t("chave") (+ chave nos dois common.json), ou marque como intencional ` +
          `com {/* i18n-ignore */} / // i18n-ignore-file.`,
      );
    }
    expect(violations).toEqual([]);
  });
});
