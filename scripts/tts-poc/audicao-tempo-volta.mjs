#!/usr/bin/env node
// AUDIÇÃO: o tempo de volta partido no MINUTO.
//
// A POC decidiu gravar o tempo FUNDIDO — `"um trinta e dois e quatro."` num arquivo só — e
// rejeitou de ouvido tanto a decomposição por dígitos quanto a versão qualitativa. O desenho
// que saiu dali eram 1.201 peças cobrindo 0:00,0 a 1:59,9.
//
// A faixa está errada. Medida na tabela de tempos base do jogo (618 entradas de
// `simulation/profile/lap_times.rs`), a volta vai de 30,0 s a 703,0 s, e 18,3% delas passam de
// dois minutos. O desenho original deixaria quase uma volta em cinco muda — e cobrir tudo
// fundido custaria ~6.900 arquivos.
//
// A saída candidata é uma emenda: gravar o MINUTO à parte e os segundos+décimos fundidos.
// 11 minutos + 600 combinações = 611 peças, cobrindo até 11:59,9. O que esta audição decide é
// se a emenda no minuto sobrevive — e ela é a mais arriscada do acervo, porque cai DENTRO de
// um número, não entre orações.
//
// Uso: node scripts/tts-poc/audicao-tempo-volta.mjs

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { aparar, aplicarRadio, buracoInterno, escreverWav, lerWav } from "./filtro-radio.mjs";

const ENDPOINT = "https://texttospeech.googleapis.com/v1/text:synthesize";
const VOZ = "pt-BR-Chirp3-HD-Algenib";
const TAXA = 24000;
const DESTINO = path.join("docs", "tts-poc", "audicao-tempo-volta");

// A emenda do minuto é a mais apertada que existe: ela cai dentro de um número. 40 ms é o
// valor de ARTIGO da fala de quebra ("…da" + "Kitsune"), a junção mais colada já aprovada.
const MINUTO_MS = 40;
const ORACAO_MS = 60;
const VIRGULA_MS = 90;

const PECAS = {
  min_um: "um,",
  min_seis: "seis,",
  t_32_4: "trinta e dois e quatro.",
  t_58_2: "cinquenta e oito e dois.",
  t_45_3: "quarenta e cinco e três.",
  t_09_0: "nove e zero.",
  lead_volta: "Volta em,",
  lead_melhor: "A volta mais rápida da corrida é do",
  nm_cooper: "Cooper,",
  falta_3: "Faltam três décimos para a melhor volta.",
};

const FORMAS = [
  {
    chave: "min-partido",
    pecas: ["min_um", "t_32_4"],
    pausas: [MINUTO_MS],
    inteiro: "Um trinta e dois e quatro.",
    nota: "A EMENDA QUE DECIDE TUDO — minuto + segundos",
  },
  {
    chave: "min-longo",
    pecas: ["min_seis", "t_58_2"],
    pausas: [MINUTO_MS],
    inteiro: "Seis cinquenta e oito e dois.",
    nota: "volta longa (Nordschleife) — o caso que a faixa antiga não cobria",
  },
  {
    chave: "sem-minuto",
    pecas: ["t_45_3"],
    pausas: [],
    inteiro: "Quarenta e cinco e três.",
    nota: "abaixo de um minuto — peça única, sem emenda",
  },
  {
    chave: "decimo-cravado",
    pecas: ["min_um", "t_09_0"],
    pausas: [MINUTO_MS],
    inteiro: "Um nove e zero.",
    nota: "segundo de um dígito e décimo zero — a forma mais curta com minuto",
  },
  {
    chave: "com-lead",
    pecas: ["lead_volta", "min_um", "t_32_4"],
    pausas: [ORACAO_MS, MINUTO_MS],
    inteiro: "Volta em, um trinta e dois e quatro.",
    nota: "resposta ao push-to-talk: duas emendas",
  },
  {
    chave: "melhor-da-corrida",
    pecas: ["lead_melhor", "nm_cooper", "min_um", "t_32_4"],
    pausas: [ORACAO_MS, VIRGULA_MS, MINUTO_MS],
    inteiro: "A volta mais rápida da corrida é do Cooper, um trinta e dois e quatro.",
    nota: "a forma mais longa: lead + nome + minuto + tempo",
  },
  {
    chave: "faltam",
    pecas: ["falta_3"],
    pausas: [],
    inteiro: "Faltam três décimos para a melhor volta.",
    nota: "o aviso de aproximação — peça única, fundida",
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

async function peca(texto) {
  const wav = await sintetizar(texto);
  const tmp = path.join(DESTINO, ".tmp.wav");
  fs.writeFileSync(tmp, wav);
  const { amostras, taxa } = lerWav(tmp);
  fs.unlinkSync(tmp);
  const cortado = aparar(amostras, taxa);
  return { amostras: cortado, taxa, buraco: buracoInterno(cortado, taxa) };
}

const avisos = [];

console.log("── peças ──");
const geradas = {};
for (const [chave, texto] of Object.entries(PECAS)) {
  const p = await peca(texto);
  geradas[chave] = p;
  if (p.buraco > 0.15) avisos.push(`${chave}: ${p.buraco.toFixed(2)}s de silêncio interno`);
  console.log(`  ${chave.padEnd(13)} ${(p.amostras.length / p.taxa).toFixed(2)}s  "${texto}"`);
}

console.log("\n── montagem ──");
const taxa = geradas.t_32_4.taxa;
const duracoes = {};
for (const forma of FORMAS) {
  const partes = forma.pecas.map((c) => geradas[c].amostras);
  const sil = forma.pausas.map((ms) => Math.round((ms / 1000) * taxa));
  const total = partes.reduce((s, a) => s + a.length, 0) + sil.reduce((s, n) => s + n, 0);
  const junto = new Float32Array(total);
  let cursor = 0;
  partes.forEach((a, i) => {
    junto.set(a, cursor);
    cursor += a.length + (sil[i] ?? 0);
  });
  const final = aplicarRadio(junto, taxa);
  escreverWav(path.join(DESTINO, `${forma.chave}.wav`), final, taxa);
  duracoes[forma.chave] = final.length / taxa;
  console.log(`  ${forma.chave.padEnd(20)} ${duracoes[forma.chave].toFixed(2)}s  — ${forma.nota}`);
}

console.log("\n── referência (tomada única) ──");
for (const forma of FORMAS) {
  const p = await peca(forma.inteiro);
  const final = aplicarRadio(p.amostras, p.taxa);
  escreverWav(path.join(DESTINO, `ref-${forma.chave}.wav`), final, p.taxa);
  const dur = final.length / p.taxa;
  const delta = duracoes[forma.chave] - dur;
  console.log(
    `  ${("ref-" + forma.chave).padEnd(20)} ${dur.toFixed(2)}s  ` +
      `(montada ${delta >= 0 ? "+" : ""}${delta.toFixed(2)}s)  "${forma.inteiro}"`,
  );
}

console.log(`\n${DESTINO}`);
console.log("Ouça `min-partido.wav` contra `ref-min-partido.wav`. Se der para dizer qual é qual,");
console.log("a emenda no minuto não serve e o desenho volta para a mesa.");
for (const a of avisos) console.warn(`  ⚠  ${a}`);
