import { describe, it, expect } from "vitest";
import { baselineOrfa, runAudit } from "../../scripts/i18nAudit.mjs";

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

  // O baseline de `.js` (scripts/i18nBaseline.mjs) congela o passivo anterior a 11/08/2026.
  // Entrada que sobra depois da frase ser traduzida volta a liberar aquele texto: o próximo
  // que escrever a mesma coisa passa batido, e o guard perde justamente o caso que ele existe
  // para pegar.
  it("não há entrada morta no baseline de .js", () => {
    const orfas = baselineOrfa();
    if (orfas.length > 0) {
      const list = orfas.map((o) => `  ${o.file}  "${o.text}"`).join("\n");
      throw new Error(
        `${orfas.length} entrada(s) do baseline que o auditor não acha mais:\n${list}\n\n` +
          `→ Apague essas linhas de scripts/i18nBaseline.mjs.`,
      );
    }
    expect(orfas).toEqual([]);
  });
});
