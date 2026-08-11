// A gravidade da lesão atravessa a ponte como PROSA EM PORTUGUÊS, e os dois lados precisam
// escrever exatamente igual.
//
// O Rust guarda `InjuryType` no banco como texto ("Leve", "Moderada", "Grave", "Critica") e o
// frontend traduz esse texto para chave de i18n, comparando string por string em
// `race/raceFactsContext.js`. Não há enum serde no meio: é `&str` de um lado e chave de
// objeto do outro.
//
// O modo de falha é MUDO e parece uma correção de qualidade: alguém acentua "Critica" para
// "Crítica" no Rust — que é o que a regra de acentuação do projeto pede em toda copy — e o
// mapa do frontend deixa de casar. `INJURY_SEVERITY_KEY[tipo]` devolve `undefined`, a
// gravidade some da prévia pré-corrida e o texto que vai para a IA perde a informação, sem
// erro em lugar nenhum.
//
// Este guard existe para que essa correção QUEBRE, e quem a fizer saiba que precisa mexer
// nos dois lados. Aqui "Critica" sem acento é contrato de dado, não copy.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const ler = (p) => fs.readFileSync(path.join(raiz, p), "utf8");

/// Os textos que `InjuryType::as_str` devolve, na ordem em que aparecem.
function gravidadesDoRust() {
  const rs = ler("src-tauri/src/models/enums/piloto.rs");
  const bloco = /InjuryType::(\w+) => "(\w+)",/g;
  const pares = [...rs.matchAll(bloco)].map(([, variante, texto]) => [variante, texto]);
  assert.ok(
    pares.length >= 4,
    `só ${pares.length} gravidades extraídas de models/enums/piloto.rs — a extração furou`,
  );
  return pares;
}

/// As chaves do mapa `INJURY_SEVERITY_KEY` do frontend.
function gravidadesDoFront() {
  const js = ler("src/components/race/raceFactsContext.js");
  const bloco = /const INJURY_SEVERITY_KEY = \{([\s\S]*?)\n\};/.exec(js);
  assert.ok(bloco, "INJURY_SEVERITY_KEY sumiu de race/raceFactsContext.js");
  const pares = [...bloco[1].matchAll(/^\s*(\w+):\s*"(\w+)",/gm)].map(([, tipo, chave]) => [tipo, chave]);
  assert.ok(pares.length >= 4, `só ${pares.length} entradas no mapa do frontend — a extração furou`);
  return pares;
}

test("toda gravidade que o Rust escreve tem entrada no mapa do frontend", () => {
  const doFront = new Set(gravidadesDoFront().map(([tipo]) => tipo));
  const faltando = gravidadesDoRust()
    .map(([, texto]) => texto)
    .filter((texto) => !doFront.has(texto));
  assert.deepEqual(
    faltando,
    [],
    `gravidade sem tradução no frontend: ${faltando}\n\n` +
      `O Rust grava este texto no banco e o mapa INJURY_SEVERITY_KEY de ` +
      `src/components/race/raceFactsContext.js o traduz para chave de i18n. Sem a entrada, ` +
      `a gravidade some da prévia pré-corrida sem erro nenhum.`,
  );
});

test("o mapa do frontend não inventa gravidade que o Rust não emite", () => {
  // A direção contrária também importa: uma entrada órfã é código que nunca roda, e passa a
  // impressão de que a gravidade existe.
  const doRust = new Set(gravidadesDoRust().map(([, texto]) => texto));
  const orfas = gravidadesDoFront()
    .map(([tipo]) => tipo)
    .filter((tipo) => !doRust.has(tipo));
  assert.deepEqual(orfas, [], `gravidade no frontend que o Rust não emite: ${orfas}`);
});

test("cada gravidade aponta para uma chave i18n que existe nos dois locales", () => {
  // A chave é o outro lado da tradução. Uma entrada apontando para chave inexistente devolve
  // a própria chave crua, e o jogador lê "severe" no meio de uma frase em português.
  const locales = {
    "pt-BR": JSON.parse(ler("src/i18n/locales/pt-BR/common.json")),
    "en-US": JSON.parse(ler("src/i18n/locales/en-US/common.json")),
  };
  const js = ler("src/components/race/raceFactsContext.js");
  // Onde a chave é consumida: descobre o prefixo real em vez de assumi-lo.
  const uso = /INJURY_SEVERITY_KEY\[[^\]]+\]/.test(js);
  assert.ok(uso, "INJURY_SEVERITY_KEY deixou de ser consultado — o mapa virou código morto");

  const prefixo = /t\(\s*[`"']([\w.]+)\.\$\{?\s*(?:sev|severity|gravidade)/.exec(js);
  // Sem o prefixo localizável o guard não tem como resolver a chave inteira; ele cobra então
  // apenas que os quatro valores sejam distintos, que é o que impede duas gravidades
  // colapsarem no mesmo texto.
  const valores = gravidadesDoFront().map(([, chave]) => chave);
  assert.equal(new Set(valores).size, valores.length, `duas gravidades com a mesma chave: ${valores}`);

  if (!prefixo) return;
  for (const [locale, json] of Object.entries(locales)) {
    for (const valor of valores) {
      const caminho = `${prefixo[1]}.${valor}`.split(".");
      let no = json;
      for (const parte of caminho) no = no?.[parte];
      assert.ok(no, `${locale} não tem a chave ${caminho.join(".")}`);
    }
  }
});

test("a gravidade crítica continua sem acento dos dois lados", () => {
  // O caso específico que motivou o guard. "Critica" é o valor gravado no banco desde a
  // primeira migração; acentuá-lo é uma correção de copy que aqui é quebra de contrato de
  // dado, e que ainda exigiria migrar os saves existentes.
  const rs = ler("src-tauri/src/models/enums/piloto.rs");
  assert.match(rs, /InjuryType::Critica => "Critica",/, "o Rust mudou o texto da gravidade crítica");
  const js = ler("src/components/race/raceFactsContext.js");
  assert.match(js, /^\s*Critica:/m, "o frontend mudou a chave da gravidade crítica");
});
