// O acervo de voz mora no git, e este guard é o gatilho que a decisão prometeu.
//
// A vistoria de 10/08/2026 perguntou se os `.opus` deviam continuar no repositório ou virar
// artefato de release. A resposta, medida no pack em 11/08/2026, foi FICAR: das 222 MB de
// blobs da história, 112 MB são imagens de pista em `public/utilities`, 58 MB são PNG de
// `src/assets/utilities` e só 29 MB são voz. Tirar o áudio do git resolveria 13% do peso do
// clone e cobraria o preço de o `.opus` — que o Vite importa em tempo de build — virar passo
// obrigatório de setup e de CI, com todos os guards de acervo passando a depender de um
// download.
//
// Uma decisão dessas envelhece sozinha. O acervo só cresce, nenhuma peça sai, e o dia em que a
// conta virar não vem anunciado: vem como um clone que demora e ninguém liga a coisa uma à
// outra. Por isso o teto está AQUI, e não numa frase de documento — quando ele estourar, a
// suíte diz o que fazer.
//
// O teto é de tamanho, não de contagem. 4 mil peças de 9 KB não são problema nenhum; 4 mil
// peças de 60 KB seriam. Quem gera acervo novo compara o número, não a intuição.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const ACERVOS = ["src/assets/engenheiro", "src/assets/spotter"];

/** Bytes e contagem dos `.opus` de um diretório de acervo. */
function pesoDoAcervo(relativo) {
  const dir = path.join(raiz, relativo);
  if (!fs.existsSync(dir)) return { bytes: 0, pecas: 0 };
  let bytes = 0;
  let pecas = 0;
  for (const arquivo of fs.readdirSync(dir)) {
    if (!arquivo.endsWith(".opus")) continue;
    bytes += fs.statSync(path.join(dir, arquivo)).size;
    pecas += 1;
  }
  return { bytes, pecas };
}

test("o acervo de voz cabe no teto que decidiu mantê-lo no git", () => {
  const medidas = ACERVOS.map((rel) => [rel, pesoDoAcervo(rel)]);
  const bytes = medidas.reduce((s, [, m]) => s + m.bytes, 0);
  const pecas = medidas.reduce((s, [, m]) => s + m.pecas, 0);
  const mb = bytes / 1048576;

  // Piso: os dois diretórios existem e têm acervo. Sem ele, um clone parcial ou um caminho
  // renomeado faria este teste medir zero e passar — verde justamente por ter perdido o alvo.
  assert.ok(
    pecas >= 3000,
    `só ${pecas} peças .opus encontradas nos acervos (piso 3.000, 3.999 em 11/08/2026) — ` +
      "ou o acervo encolheu de verdade, ou o caminho mudou e o guard ficou sem alvo.",
  );

  // 29,3 MB em 11/08/2026. O teto de 100 MB é o ponto em que a conta vira: aí a voz deixa de
  // ser 13% do clone e passa a competir com as imagens de pista.
  assert.ok(
    mb <= 100,
    `o acervo de voz está em ${mb.toFixed(1)} MB (${pecas} peças) e o teto é 100 MB.\n\n` +
      "Este é o gatilho combinado: passou daqui, o .opus vira artefato de release em vez de " +
      "arquivo versionado. O caminho é publicar o pacote junto do instalador e baixá-lo no " +
      "setup e no CI — e, junto, revisar os guards de acervo que hoje conferem o pé de disco " +
      "(quebra-pecas-contrato, engenheiro-pecas-do-app, engenheiro-pecas-do-spotter).",
  );
});

test("os masters WAV continuam fora do git", () => {
  // O que torna o acervo sustentável no git é o Opus, não o áudio. Em PCM 24 kHz mono as
  // mesmas peças dão 326 MB, e WAV não delta-comprime: cada regravação somaria o arquivo
  // inteiro à história para sempre. Uma regra apagada por engano do .gitignore não daria erro
  // nenhum — daria um `git add` de 300 MB no commit seguinte.
  const ignore = fs.readFileSync(path.join(raiz, ".gitignore"), "utf8");
  for (const acervo of ACERVOS) {
    assert.match(
      ignore,
      new RegExp(`^${acervo}/\\*\\.wav$`, "m"),
      `o .gitignore não protege mais os masters de ${acervo}`,
    );
  }
});
