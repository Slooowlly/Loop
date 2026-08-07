// Guard estrutural: as peças que o ENGENHEIRO empresta do SPOTTER continuam existindo lá.
//
// O spotter e o engenheiro são a mesma pessoa, separados só pelo imediatismo — o spotter
// alerta sem ser perguntado, o engenheiro responde. Quando os dois precisam da mesma frase,
// o engenheiro TOCA a gravação do spotter em vez de gravar a sua: pedir duas tomadas da mesma
// frase ao mesmo modelo não determinístico faria a mesma pessoa soar diferente conforme quem
// falou, que é exatamente a deriva que o pacote pré-gravado existe para evitar.
//
// O preço desse empréstimo é um acoplamento entre dois arquivos que não se conhecem: a lista
// `PECAS_DO_SPOTTER` vive no Rust, e as falas vivem num objeto JS. Renomear uma fala do
// spotter emudeceria o engenheiro **sem erro nenhum** — o app procuraria um `.opus` que deixou
// de existir e simplesmente não tocaria. Este guard é o que transforma esse silêncio em
// teste vermelho.

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const FALA_RS = path.join(raiz, "src-tauri", "src", "engenheiro", "fala.rs");
const PACK_MJS = path.join(raiz, "scripts", "spotter-pack.mjs");
const ASSETS = path.join(raiz, "src", "assets", "spotter");

/** As chaves declaradas em `PECAS_DO_SPOTTER`, lidas do Rust como texto. */
function pecasEmprestadas() {
  const fonte = fs.readFileSync(FALA_RS, "utf8");
  const bloco = fonte.match(/pub const PECAS_DO_SPOTTER:\s*\[&str;\s*\d+\]\s*=\s*\[([^\]]+)\]/);
  assert.ok(bloco, "não achei PECAS_DO_SPOTTER em fala.rs — o guard perdeu o alvo");
  return [...bloco[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

/** As chaves que o gerador do spotter produz, lidas do objeto de falas. */
function pecasDoSpotter() {
  const fonte = fs.readFileSync(PACK_MJS, "utf8");
  return new Set([...fonte.matchAll(/^\s{2}(\w+):\s*"/gm)].map((m) => m[1]));
}

test("toda peça emprestada existe no gerador do spotter", () => {
  const spotter = pecasDoSpotter();
  assert.ok(spotter.size > 20, `li poucas falas do spotter-pack.mjs (${spotter.size})`);
  for (const chave of pecasEmprestadas()) {
    assert.ok(
      spotter.has(chave),
      `o engenheiro empresta '${chave}', que NÃO existe mais em spotter-pack.mjs. ` +
        "Ou a fala foi renomeada lá, ou a lista PECAS_DO_SPOTTER ficou para trás — nos dois " +
        "casos o engenheiro fica mudo nessa resposta, sem erro nenhum em tempo de execução.",
    );
  }
});

test("toda peça emprestada tem o .opus gravado", () => {
  // O gerador declarar a fala não basta: o pacote pode nunca ter sido regerado depois de ela
  // ser acrescentada. É a diferença entre "está na lista" e "está no disco".
  if (!fs.existsSync(ASSETS)) {
    console.log(`  ${ASSETS} não existe ainda — pulando`);
    return;
  }
  const gravadas = new Set(
    fs
      .readdirSync(ASSETS)
      .filter((f) => f.endsWith(".opus"))
      .map((f) => f.replace(/\.opus$/, "")),
  );
  for (const chave of pecasEmprestadas()) {
    assert.ok(
      gravadas.has(chave),
      `o engenheiro empresta '${chave}', mas ${chave}.opus não está em src/assets/spotter/. ` +
        "Rode scripts/spotter-pack.mjs para gerar o pacote.",
    );
  }
});

test("nenhuma peça emprestada é gerada TAMBÉM pelo engenheiro", () => {
  // O ponto do empréstimo é existir UMA gravação. Se a chave aparecesse nas duas listas, o
  // gerador do engenheiro produziria a segunda tomada que o empréstimo existe para evitar —
  // e, pior, ela venceria por ser gerada depois.
  const fonte = fs.readFileSync(FALA_RS, "utf8");
  for (const chave of pecasEmprestadas()) {
    const geradaAqui = new RegExp(`\\("${chave}"\\.to_string\\(\\),`).test(fonte);
    assert.ok(
      !geradaAqui,
      `'${chave}' é emprestada do spotter E gerada pelo engenheiro — escolha uma das duas.`,
    );
  }
});
