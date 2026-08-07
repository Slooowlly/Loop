#!/usr/bin/env node
// AUDIÇÃO: duas quebras no mesmo instante, numa fala só.
//
// A pergunta é a conjunção. `"Cooper e Silva tiveram problemas"` precisa de um "e" entre os
// dois nomes, e "e" é MONOSSÍLABO — exatamente o caso que reprovou o `"um,"` do tempo de volta,
// onde a peça isolada saiu com 0,40 s, acento pleno e contorno de fim de frase.
//
// Três saídas na mesa, e esta audição escolhe:
//
//   (a) LISTA — `"Cooper," + "Silva," + "tiveram problemas no carro."` Sem conjunção nenhuma.
//       Custa zero peça nova além do trecho plural; a dúvida é se soa recortado.
//   (b) CONJUNÇÃO SOLTA — `"Cooper," + "e," + "Silva," + …`. Uma peça nova, e o risco do "um".
//   (c) CONJUNÇÃO FUNDIDA no segundo nome — `"e Silva,"`. Resolve o problema e custa 355
//       peças, um segundo banco de sobrenomes inteiro. É o preço que só vale pagar se as
//       outras duas falharem.
//
// Uso: node scripts/tts-poc/audicao-quebra-dupla.mjs

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { aparar, aplicarRadio, buracoInterno, escreverWav, lerWav } from "./filtro-radio.mjs";
import { pausasDoRadio } from "../../src/lib/pausasDoRadio.js";

const ENDPOINT = "https://texttospeech.googleapis.com/v1/text:synthesize";
const VOZ = "pt-BR-Chirp3-HD-Algenib";
const TAXA = 24000;
const DESTINO = path.join("docs", "tts-poc", "audicao-quebra-dupla");
const ORIGEM = path.join("src", "assets", "engenheiro");

// Peças NOVAS a gerar. Os sobrenomes vêm do acervo de produção — reaproveitá-los é metade do
// ponto do teste: a fala real vai colar tomada antiga com tomada nova.
const NOVAS = {
  conj_e: "e,",
  conj_e_silva: "e Silva,",
  qb_dupla_dnf_0: "abandonaram a corrida com problemas no carro.",
  qb_dupla_heavy_0: "tiveram problemas no carro.",
};

const DO_ACERVO = ["nm_cooper", "nm_silva"];

const FORMAS = [
  {
    chave: "a-lista",
    pecas: ["nm_cooper", "nm_silva", "qb_dupla_dnf_0"],
    inteiro: "Cooper, Silva, abandonaram a corrida com problemas no carro.",
    nota: "(a) LISTA — sem conjunção",
  },
  {
    chave: "b-conjuncao-solta",
    pecas: ["nm_cooper", "conj_e", "nm_silva", "qb_dupla_dnf_0"],
    inteiro: "Cooper e Silva abandonaram a corrida com problemas no carro.",
    nota: "(b) CONJUNÇÃO SOLTA — o risco do monossílabo",
  },
  {
    chave: "c-conjuncao-fundida",
    pecas: ["nm_cooper", "conj_e_silva", "qb_dupla_dnf_0"],
    inteiro: "Cooper e Silva abandonaram a corrida com problemas no carro.",
    nota: "(c) CONJUNÇÃO FUNDIDA — custa 355 peças",
  },
  {
    chave: "a-lista-grave",
    pecas: ["nm_cooper", "nm_silva", "qb_dupla_heavy_0"],
    inteiro: "Cooper, Silva, tiveram problemas no carro.",
    nota: "(a) na severidade grave — a frase mais curta",
  },
];

function gcloud(args, erro) {
  try {
    return execFileSync("gcloud", args, { encoding: "utf8", shell: true }).trim();
  } catch {
    console.error(erro);
    process.exit(1);
  }
}

const acesso =
  process.env.GCP_TOKEN?.trim() ||
  gcloud(
    ["auth", "application-default", "print-access-token"],
    "Sem credencial. Rode: gcloud auth application-default login",
  );
const projeto = gcloud(["config", "get-value", "project"], "Projeto de cota não resolvido.");

fs.mkdirSync(DESTINO, { recursive: true });

async function sintetizar(texto) {
  const r = await fetch(ENDPOINT, {
    method: "POST",
    headers: {
      authorization: `Bearer ${acesso}`,
      "x-goog-user-project": projeto,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      input: { text: texto },
      voice: { languageCode: "pt-BR", name: VOZ },
      audioConfig: { audioEncoding: "LINEAR16", sampleRateHertz: TAXA },
    }),
  });
  if (!r.ok) throw new Error(`HTTP ${r.status}: ${(await r.text()).slice(0, 300)}`);
  return Buffer.from((await r.json()).audioContent, "base64");
}

/** Peça NOVA: gerada crua e aparada, com a cadeia de rádio aplicada — como o pacote faz. */
async function pecaNova(texto) {
  const tmp = path.join(DESTINO, ".tmp.wav");
  fs.writeFileSync(tmp, await sintetizar(texto));
  const { amostras, taxa } = lerWav(tmp);
  fs.unlinkSync(tmp);
  const cortado = aparar(amostras, taxa);
  return { amostras: aplicarRadio(cortado, taxa), taxa, buraco: buracoInterno(cortado, taxa) };
}

const avisos = [];
const geradas = {};

console.log("── peças novas ──");
for (const [chave, texto] of Object.entries(NOVAS)) {
  const p = await pecaNova(texto);
  geradas[chave] = p;
  escreverWav(path.join(DESTINO, `peca-${chave}.wav`), p.amostras, p.taxa);
  if (p.buraco > 0.15) avisos.push(`${chave}: ${p.buraco.toFixed(2)}s de silêncio interno`);
  console.log(`  ${chave.padEnd(18)} ${(p.amostras.length / p.taxa).toFixed(2)}s  "${texto}"`);
}

console.log("\n── peças do acervo ──");
for (const chave of DO_ACERVO) {
  const { amostras, taxa } = lerWav(path.join(ORIGEM, `${chave}.wav`));
  geradas[chave] = { amostras, taxa };
  console.log(`  ${chave.padEnd(18)} ${(amostras.length / taxa).toFixed(2)}s`);
}

console.log("\n── montagem ──");
const taxa = geradas.nm_cooper.taxa;
const duracoes = {};
for (const forma of FORMAS) {
  // A conjunção é uma fronteira dentro da mesma oração, como o nome antes do verbo.
  const pausas = pausasDoRadio(forma.pecas).map((ms) => Math.round((ms / 1000) * taxa));
  const partes = forma.pecas.map((c) => geradas[c].amostras);
  const total = partes.reduce((s, a) => s + a.length, 0) + pausas.reduce((s, n) => s + n, 0);
  const junto = new Float32Array(total);
  let cursor = 0;
  partes.forEach((a, i) => {
    junto.set(a, cursor);
    cursor += a.length + (pausas[i] ?? 0);
  });
  escreverWav(path.join(DESTINO, `${forma.chave}.wav`), junto, taxa);
  duracoes[forma.chave] = junto.length / taxa;
  console.log(
    `  ${forma.chave.padEnd(22)} ${duracoes[forma.chave].toFixed(2)}s  ` +
      `[${pausasDoRadio(forma.pecas).join(", ")}] ms  — ${forma.nota}`,
  );
}

console.log("\n── referência (tomada única) ──");
for (const forma of FORMAS) {
  const p = await pecaNova(forma.inteiro);
  escreverWav(path.join(DESTINO, `ref-${forma.chave}.wav`), p.amostras, p.taxa);
  const dur = p.amostras.length / p.taxa;
  const delta = duracoes[forma.chave] - dur;
  console.log(
    `  ${("ref-" + forma.chave).padEnd(22)} ${dur.toFixed(2)}s  ` +
      `(montada ${delta >= 0 ? "+" : ""}${delta.toFixed(2)}s)`,
  );
}

console.log(`\n${DESTINO}`);
for (const a of avisos) console.warn(`  ⚠  ${a}`);
