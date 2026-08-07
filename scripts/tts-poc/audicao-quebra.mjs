#!/usr/bin/env node
// AUDIÇÃO: a fala de QUEBRA montada em três e quatro peças.
//
// `audicao-nomes.mjs` provou UMA emenda: `[sobrenome] + [trecho]`. A fala de quebra que
// vamos gravar tem duas ou três, porque o piloto ganha um enquadramento — "Seu rival,"
// antes do nome, ou "que lidera o campeonato," depois dele, ou um comentário no fim.
// Cada emenda nova é uma chance de o ouvido perceber a costura, e nada do que já medimos
// diz o que acontece com a segunda.
//
// Por isso a comparação contra TOMADA ÚNICA é obrigatória, como foi lá: "soou bem" isolado
// não é veredito, porque não há com o que comparar. A única pergunta é se dá para
// distinguir a montada da inteira — e, se der, em qual das formas.
//
// A ordem é a que a POC fixou para fala montada: gerar cru → aparar → colar → filtrar.
//
// Uso: node scripts/tts-poc/audicao-quebra.mjs

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { aparar, aplicarRadio, buracoInterno, escreverWav, lerWav } from "./filtro-radio.mjs";

const ENDPOINT = "https://texttospeech.googleapis.com/v1/text:synthesize";
const VOZ = "pt-BR-Chirp3-HD-Algenib";
const TAXA = 24000;
const DESTINO = path.join("docs", "tts-poc", "audicao-quebra");

// As pausas não são uma só, porque as junções não são a mesma coisa.
//
//   ARTIGO  — "…da" + "Kitsune": dentro do sintagma, o mais apertado que existe.
//   ORACAO  — "Cooper," + "está com…": fronteira dentro da mesma oração (o valor da POC).
//   VIRGULA — "Seu rival," + "Cooper,": vírgula real no texto de destino, respiro curto.
//   FRASE   — "…na suspensão." + "Ótima notícia": ponto final, a única emenda barata,
//             porque a prosódia de fim de frase da peça anterior é a certa ali.
const ARTIGO = 40;
const ORACAO = 60;
const VIRGULA = 90;
const FRASE = 220;

// As peças. Texto EXATO do que seria gravado — inclusive a pontuação, que é o que decide
// a entoação: peça terminada em vírgula pede continuação, terminada em ponto encerra.
const PECAS = {
  ab_rival: "Seu rival,",
  ab_piloto2: "O piloto dois da",
  nm_cooper: "Cooper,",
  nm_wisniewski: "Wisniewski,",
  eq_kitsune: "Kitsune Racing,",
  ap_lider: "que lidera o campeonato,",
  ap_frente: "logo à sua frente na tabela,",
  tr_motor: "está com o motor em pane.",
  tr_cambio: "abandona a corrida, problemas no câmbio.",
  tr_suspensao: "está fora, problemas na suspensão.",
  co_otima: "Ótima notícia pra nós.",
};

// As quatro formas, da mais provável à pior. `pausas` tem sempre um elemento a menos que
// `pecas` — é a junção ENTRE cada par.
const FORMAS = [
  {
    chave: "rival",
    pecas: ["ab_rival", "nm_cooper", "tr_motor"],
    pausas: [VIRGULA, ORACAO],
    nota: "2 emendas — abertura de vínculo",
  },
  {
    chave: "lider",
    pecas: ["nm_cooper", "ap_lider", "tr_cambio"],
    pausas: [VIRGULA, VIRGULA],
    nota: "2 emendas — aposto no meio (o nome deixa de ser a primeira peça)",
  },
  {
    chave: "equipe",
    pecas: ["ab_piloto2", "eq_kitsune", "tr_motor"],
    pausas: [ARTIGO, ORACAO],
    nota: "2 emendas — sem vínculo, pela equipe",
  },
  {
    chave: "coda",
    pecas: ["nm_wisniewski", "ap_frente", "tr_suspensao", "co_otima"],
    pausas: [VIRGULA, VIRGULA, FRASE],
    nota: "3 emendas — a forma mais longa que o rádio vai produzir",
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
  const resposta = await fetch(ENDPOINT, {
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
  if (!resposta.ok)
    throw new Error(`HTTP ${resposta.status}: ${(await resposta.text()).slice(0, 400)}`);
  const { audioContent } = await resposta.json();
  return Buffer.from(audioContent, "base64");
}

/** Gera e devolve as amostras JÁ APARADAS, sem rádio (o filtro é depois da colagem). */
async function peca(texto) {
  const wav = await sintetizar(texto);
  const tmp = path.join(DESTINO, ".tmp.wav");
  fs.writeFileSync(tmp, wav);
  const { amostras, taxa } = lerWav(tmp);
  fs.unlinkSync(tmp);
  const cortado = aparar(amostras, taxa);
  return { amostras: cortado, taxa, buraco: buracoInterno(cortado, taxa) };
}

/** O texto que a forma montada DEVERIA soar — é ele que vira a tomada única. */
function textoInteiro(forma) {
  return forma.pecas
    .map((c) => PECAS[c])
    .join(" ")
    .replace(/, (?=[A-ZÁÉÍÓÚÂÊÔÃÕÇ])/g, ", ");
}

const avisos = [];

console.log("── peças ──");
const geradas = {};
for (const [chave, texto] of Object.entries(PECAS)) {
  const p = await peca(texto);
  geradas[chave] = p;
  if (p.buraco > 0) avisos.push(`${chave}: ${p.buraco.toFixed(2)}s de silêncio interno`);
  const destino = path.join(DESTINO, `peca-${chave}.wav`);
  escreverWav(destino, aplicarRadio(p.amostras, p.taxa), p.taxa);
  console.log(`  ${chave.padEnd(14)} ${(p.amostras.length / p.taxa).toFixed(2)}s  "${texto}"`);
}

console.log("\n── montagem ──");
const taxa = geradas.nm_cooper.taxa;
const duracoes = {};
for (const forma of FORMAS) {
  const partes = forma.pecas.map((c) => geradas[c].amostras);
  const silencios = forma.pausas.map((ms) => Math.round((ms / 1000) * taxa));
  const total = partes.reduce((s, a) => s + a.length, 0) + silencios.reduce((s, n) => s + n, 0);
  const junto = new Float32Array(total);
  let cursor = 0;
  partes.forEach((a, i) => {
    junto.set(a, cursor);
    cursor += a.length + (silencios[i] ?? 0);
  });
  const final = aplicarRadio(junto, taxa);
  escreverWav(path.join(DESTINO, `${forma.chave}.wav`), final, taxa);
  duracoes[forma.chave] = final.length / taxa;
  console.log(`  ${forma.chave.padEnd(14)} ${duracoes[forma.chave].toFixed(2)}s  ${forma.nota}`);
}

console.log("\n── referência (tomada única) ──");
for (const forma of FORMAS) {
  const texto = textoInteiro(forma);
  const p = await peca(texto);
  const final = aplicarRadio(p.amostras, p.taxa);
  escreverWav(path.join(DESTINO, `ref-${forma.chave}.wav`), final, p.taxa);
  const dur = final.length / p.taxa;
  const delta = duracoes[forma.chave] - dur;
  if (p.buraco > 0) avisos.push(`ref-${forma.chave}: ${p.buraco.toFixed(2)}s de silêncio interno`);
  console.log(
    `  ${("ref-" + forma.chave).padEnd(14)} ${dur.toFixed(2)}s  ` +
      `(montada ${delta >= 0 ? "+" : ""}${delta.toFixed(2)}s)\n      "${texto}"`,
  );
}

console.log(`\n${DESTINO}`);
console.log("Ouça em pares: `X.wav` contra `ref-X.wav`. A pergunta é se dá para dizer qual é qual.");
for (const a of avisos) console.warn(`  ⚠  ${a}`);
