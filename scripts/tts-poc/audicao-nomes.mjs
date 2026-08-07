#!/usr/bin/env node
// AUDIÇÃO: sobrenome colado a frase do engenheiro.
//
// A pergunta: `[sobrenome] + [trecho]` sobrevive à colagem? Se sobreviver, o rádio do
// engenheiro custa 355 sobrenomes + 105 trechos em vez de 355 × 105 gravações.
//
// Gera a matriz inteira (cada sobrenome × cada frase) e, para alguns pares, a MESMA
// frase numa tomada só. Sem a tomada única não há veredito: "soou bem" isolado não diz
// nada, porque não há com o que comparar — a única pergunta que importa é se dá para
// distinguir a colada da inteira.
//
// A ordem é a que a POC fixou para fala MONTADA: gerar cru → aparar → colar → filtrar.
// Filtrar cada peça antes de colar dá salto de nível na emenda (o compressor chega em
// cada uma com histórico diferente).
//
// Uso: node scripts/tts-poc/audicao-nomes.mjs

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { aparar, aplicarRadio, buracoInterno, escreverWav, lerWav } from "./filtro-radio.mjs";

const ENDPOINT = "https://texttospeech.googleapis.com/v1/text:synthesize";
const VOZ = "pt-BR-Chirp3-HD-Algenib";
const TAXA = 24000;
const DESTINO = path.join("docs", "tts-poc", "audicao-nomes");
// Silêncio entre nome e verbo. Menor que os 90 ms da colagem de frases: aqui não há
// vírgula no texto de destino ("Silva sente o motor…"), é uma fronteira dentro da
// mesma oração.
const PAUSA_MS = 60;

// Sobrenomes REAIS de `src-tauri/src/generators/names/dados/`. Metade escolhida por
// ser fácil para uma voz pt-BR, metade por ser exatamente o contrário.
const SOBRENOMES = [
  { chave: "silva", texto: "Silva", origem: "br", nota: "fácil" },
  { chave: "romano", texto: "Romano", origem: "it", nota: "fácil" },
  { chave: "taylor", texto: "Taylor", origem: "gb", nota: "fácil" },
  // Guardado no pool SEM separação — é assim que o mundo gera o piloto.
  { chave: "vandijk", texto: "vanDijk", origem: "nl", nota: "concatenado" },
  // Guardados SEM acento: o original é Hämäläinen.
  { chave: "hamalainen", texto: "Hamalainen", origem: "fi", nota: "sem acento" },
  { chave: "wisniewski", texto: "Wisniewski", origem: "pl", nota: "consoantes" },
];

// Trechos REAIS de `commands/overlay/radio.rs` e `dnf_frase()` — os três formatos que
// o rádio produz: aviso leve, aviso grave e abandono.
const FRASES = [
  { chave: "leve", texto: "sente o motor perdendo fôlego" },
  { chave: "grave", texto: "está com o câmbio travando" },
  { chave: "dnf", texto: "está fora — problemas na suspensão" },
];

// Pares que ganham uma tomada ÚNICA para comparar. Um fácil, um difícil, e o abandono
// (que carrega o travessão, a pontuação mais capaz de mudar a entoação).
const REFERENCIAS = [
  ["silva", "leve"],
  ["wisniewski", "grave"],
  ["vandijk", "dnf"],
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
  if (!resposta.ok) throw new Error(`HTTP ${resposta.status}: ${(await resposta.text()).slice(0, 400)}`);
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

const avisos = [];

// ── As peças ──
// O sobrenome sai com VÍRGULA no fim de propósito: sozinho e com ponto, o modelo lhe
// dá entoação de fim de frase, e colar isso num verbo produz duas orações em vez de
// uma. A vírgula pede continuação. É o mesmo truque que salvou "Volta em," na POC.
console.log("── peças ──");
const nomes = {};
for (const s of SOBRENOMES) {
  const p = await peca(`${s.texto},`);
  nomes[s.chave] = p;
  if (p.buraco > 0) avisos.push(`${s.chave}: ${p.buraco.toFixed(2)}s de silêncio interno`);
  console.log(`  ${s.chave.padEnd(12)} ${(p.amostras.length / p.taxa).toFixed(2)}s  (${s.origem}, ${s.nota})`);
}
const caudas = {};
for (const f of FRASES) {
  const p = await peca(`${f.texto}.`);
  caudas[f.chave] = p;
  if (p.buraco > 0) avisos.push(`${f.chave}: ${p.buraco.toFixed(2)}s de silêncio interno`);
  console.log(`  ${f.chave.padEnd(12)} ${(p.amostras.length / p.taxa).toFixed(2)}s`);
}

// ── A matriz colada ──
console.log("\n── colagem ──");
const taxa = nomes[SOBRENOMES[0].chave].taxa;
const pausa = Math.round((PAUSA_MS / 1000) * taxa);
for (const s of SOBRENOMES) {
  for (const f of FRASES) {
    const a = nomes[s.chave].amostras;
    const b = caudas[f.chave].amostras;
    const junto = new Float32Array(a.length + pausa + b.length);
    junto.set(a, 0);
    junto.set(b, a.length + pausa);
    const final = aplicarRadio(junto, taxa);
    const destino = path.join(DESTINO, `${s.chave}-${f.chave}.wav`);
    escreverWav(destino, final, taxa);
    console.log(`  ${(s.chave + "-" + f.chave).padEnd(24)} ${(final.length / taxa).toFixed(2)}s`);
  }
}

// ── As tomadas únicas ──
console.log("\n── referência (tomada única) ──");
for (const [chaveNome, chaveFrase] of REFERENCIAS) {
  const s = SOBRENOMES.find((x) => x.chave === chaveNome);
  const f = FRASES.find((x) => x.chave === chaveFrase);
  const p = await peca(`${s.texto} ${f.texto}.`);
  const final = aplicarRadio(p.amostras, p.taxa);
  const destino = path.join(DESTINO, `ref-${s.chave}-${f.chave}.wav`);
  escreverWav(destino, final, p.taxa);
  const colada = fs.statSync(path.join(DESTINO, `${s.chave}-${f.chave}.wav`)).size;
  const dur = final.length / p.taxa;
  console.log(
    `  ${("ref-" + s.chave + "-" + f.chave).padEnd(24)} ${dur.toFixed(2)}s  ` +
      `(colada: ${((colada - 44) / (p.taxa * 2)).toFixed(2)}s)  "${s.texto} ${f.texto}."`,
  );
}

console.log(`\n${DESTINO}`);
for (const a of avisos) console.warn(`  ⚠  ${a}`);
