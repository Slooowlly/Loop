#!/usr/bin/env node
// UMA fala, para audição de voz e de direção. Nada de bateria, nada de estatística.
//
// Existe porque escolher timbre é trabalho de ouvido, não de tabela: cada troca de voz
// ou de direção pede uma amostra, e disparar a bateria inteira para isso é desperdício
// de cota e de dinheiro.
//
// Uso:
//   node scripts/tts-poc/uma-fala.mjs
//   node scripts/tts-poc/uma-fala.mjs --voz Schedar
//   node scripts/tts-poc/uma-fala.mjs --texto "Daniel, box nesta volta." --direcao "Seco."
//   node scripts/tts-poc/uma-fala.mjs --sem-direcao

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const ENDPOINT = "https://generativelanguage.googleapis.com/v1beta/interactions";
const MODELO = "gemini-3.1-flash-tts-preview";
const TAXA = 24000;
const BYTES_POR_AMOSTRA = 2;

// Voz padrão: `Gacrux` é descrita como "Mature" no catálogo do Gemini — é a que mais
// se aproxima de um engenheiro de pista adulto. `Charon` ("Informative"), usada na
// primeira bateria, soa jovem demais para o papel.
const VOZ = "Gacrux";

// Direção SÓBRIA, e deliberadamente escrita ao contrário da primeira tentativa. A
// direção original ("urgente, seco, alto e muito rápido") pedia energia e o modelo
// entregou energia — o resultado soou caricato. Aqui as instruções são quase todas
// negativas: o que NÃO fazer. Com o Gemini TTS isso importa mais que o adjetivo
// positivo, porque ele obedece bem demais a pedidos de intensidade.
const DIRECAO =
  "Leia em tom contido e profissional, como um engenheiro de pista veterano falando " +
  "ao rádio da equipe. Voz de adulto, grave e estável. Volume normal de conversa. " +
  "Ritmo constante, sem pressa. Não dramatize, não anime a voz, não levante o tom no " +
  "fim das frases e não soe entusiasmado. Fale como quem já fez isso mil vezes.";

const TEXTO =
  "Daniel, o carro da frente está perdendo tempo no segundo setor. " +
  "Continue pressionando, estamos nos aproximando.";

/**
 * Reticências no fim de toda fala, por padrão.
 *
 * O ponto final faz o modelo fechar a frase com queda de entonação e corte seco — soa
 * a locução encerrada, não a rádio de equipe. As reticências deixam a fala morrer
 * aberta, como quem já vai emendar a próxima informação. É o mesmo texto; muda só a
 * prosódia. `--sem-reticencias` desliga.
 */
function comReticencias(texto) {
  const limpo = texto.trim();
  if (/[.!?…]+$/.test(limpo)) return `${limpo.replace(/[.!?…\s]+$/, "")}...`;
  return `${limpo}...`;
}

function lerArgumentos(argv) {
  const o = {
    voz: VOZ,
    texto: TEXTO,
    direcao: DIRECAO,
    modelo: MODELO,
    usarDirecao: true,
    reticencias: true,
  };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    const proximo = () => argv[(i += 1)];
    if (a === "--voz") o.voz = proximo();
    else if (a === "--texto") o.texto = proximo();
    else if (a === "--direcao") o.direcao = proximo();
    else if (a === "--modelo") o.modelo = proximo();
    else if (a === "--sem-direcao") o.usarDirecao = false;
    else if (a === "--sem-reticencias") o.reticencias = false;
  }
  if (o.reticencias) o.texto = comReticencias(o.texto);
  return o;
}

/** PCM cru -> WAV tocável. A API devolve `audio/l16`, sem cabeçalho. */
function embrulharWav(pcm, taxa = TAXA, canais = 1) {
  const c = Buffer.alloc(44);
  c.write("RIFF", 0);
  c.writeUInt32LE(36 + pcm.length, 4);
  c.write("WAVE", 8);
  c.write("fmt ", 12);
  c.writeUInt32LE(16, 16);
  c.writeUInt16LE(1, 20);
  c.writeUInt16LE(canais, 22);
  c.writeUInt32LE(taxa, 24);
  c.writeUInt32LE(taxa * canais * BYTES_POR_AMOSTRA, 28);
  c.writeUInt16LE(canais * BYTES_POR_AMOSTRA, 32);
  c.writeUInt16LE(BYTES_POR_AMOSTRA * 8, 34);
  c.write("data", 36);
  c.writeUInt32LE(pcm.length, 40);
  return Buffer.concat([c, pcm]);
}

const CHAVES_AUDIO = ["data", "audio", "audioData", "audio_data", "inlineData", "inline_data", "b64"];

function coletarAudio(v, saida = []) {
  if (Array.isArray(v)) {
    for (const i of v) coletarAudio(i, saida);
    return saida;
  }
  if (v && typeof v === "object") {
    const ehTexto = v.type === "text";
    for (const [k, x] of Object.entries(v)) {
      if (!ehTexto && CHAVES_AUDIO.includes(k) && typeof x === "string" && x.length >= 64) {
        saida.push(x);
        continue;
      }
      coletarAudio(x, saida);
    }
  }
  return saida;
}

const opcoes = lerArgumentos(process.argv);
const chave = (process.env.GEMINI_API_KEY || process.env.GOOGLE_API_KEY || "").trim();
if (!chave) {
  console.error('Defina GEMINI_API_KEY. PowerShell: $env:GEMINI_API_KEY = "sua-chave"');
  process.exit(1);
}

const entrada = opcoes.usarDirecao ? `${opcoes.direcao}\n\n${opcoes.texto}` : opcoes.texto;

// Só o 3.1 Flash TTS aceita `stream: true`; a família 2.5 devolve o áudio inteiro de
// uma vez. Para AUDIÇÃO isso é irrelevante — ninguém cronometra o primeiro som ao
// escolher timbre —, e para fala pré-gravada também: sem tempo real, streaming não
// compra nada. Por isso o modo é escolhido pelo modelo, sem flag.
const usaStreaming = /3\.1|3-1/.test(opcoes.modelo);
const t0 = performance.now();

const resposta = await fetch(ENDPOINT, {
  method: "POST",
  headers: {
    "x-goog-api-key": chave,
    "content-type": "application/json",
    accept: usaStreaming ? "text/event-stream" : "application/json",
  },
  body: JSON.stringify({
    model: opcoes.modelo,
    input: entrada,
    response_format: { type: "audio" },
    generation_config: { speech_config: [{ voice: opcoes.voz }] },
    stream: usaStreaming,
  }),
});

if (!resposta.ok) {
  console.error(`HTTP ${resposta.status}: ${(await resposta.text()).slice(0, 600)}`);
  process.exit(1);
}

const pedacos = [];
let msPrimeiro = null;
let erro = null;

const receber = (valor) => {
  if (valor?.error) erro = valor.error.message;
  for (const b64 of coletarAudio(valor)) {
    if (msPrimeiro == null) msPrimeiro = performance.now() - t0;
    pedacos.push(Buffer.from(b64, "base64"));
  }
};

if (usaStreaming) {
  const dec = new TextDecoder();
  let sobra = "";
  for await (const p of resposta.body) {
    sobra += dec.decode(p, { stream: true });
    const linhas = sobra.split("\n");
    sobra = linhas.pop() ?? "";
    for (const linha of linhas) {
      const bruto = linha.trim();
      if (!bruto.startsWith("data:")) continue;
      const json = bruto.slice(5).trim();
      if (!json || json === "[DONE]") continue;
      try {
        receber(JSON.parse(json));
      } catch {
        /* linha parcial ou ruído de protocolo */
      }
    }
  }
} else {
  receber(await resposta.json());
}

if (!pedacos.length) {
  console.error(`Nenhum áudio. ${erro ?? "Sem detalhe no stream."}`);
  process.exit(1);
}

const pcm = Buffer.concat(pedacos);
const dir = path.join("docs", "tts-poc", "audicao");
fs.mkdirSync(dir, { recursive: true });
const carimbo = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+$/, "");
// O modelo entra no nome: a audição vive de comparar 3.1 contra 2.5 lado a lado.
const marcaModelo = opcoes.modelo.replace(/^gemini-/, "").replace(/-preview$/, "");
const arquivo = path.join(
  dir,
  `${carimbo}-${marcaModelo}-${opcoes.voz}${opcoes.usarDirecao ? "" : "-sem-direcao"}.wav`,
);
fs.writeFileSync(arquivo, pcm.toString("ascii", 0, 4) === "RIFF" ? pcm : embrulharWav(pcm));

const duracao = pcm.length / (TAXA * BYTES_POR_AMOSTRA);
console.log(`modelo:   ${opcoes.modelo}${usaStreaming ? " (streaming)" : " (resposta única)"}`);
console.log(`voz:      ${opcoes.voz}`);
console.log(`direção:  ${opcoes.usarDirecao ? "sim" : "não"}`);
console.log(`texto:    "${opcoes.texto}"`);
console.log(`1º áudio: ${Math.round(msPrimeiro)} ms | total ${Math.round(performance.now() - t0)} ms`);
console.log(`duração:  ${duracao.toFixed(1)} s`);
// Confere se o modelo leu a DIREÇÃO junto com o texto. O sinal disso não é sutil: a
// direção tem ~250 caracteres, então lê-la somaria uns 20 s. Um excesso de poucos
// segundos é só prosódia — silêncio de borda, a pausa das reticências, uma leitura
// mais lenta —, e a primeira versão desta checagem gritava em toda fala curta por
// medir só a razão. Agora há um piso fixo para a borda e a folga é absoluta.
const RITMO_CAR_POR_S = 13; // observado em pt-BR nas 30 falas da bateria
const BORDA_S = 0.9; // silêncio inicial/final + pausa das reticências
const esperado = BORDA_S + opcoes.texto.length / RITMO_CAR_POR_S;
if (duracao > esperado + 3) {
  console.log(
    `ATENÇÃO: ${duracao.toFixed(1)} s para ${opcoes.texto.length} caracteres ` +
      `(esperado ~${esperado.toFixed(1)} s). O modelo pode ter lido a direção em voz alta.`,
  );
}
console.log(`\n${arquivo}`);
