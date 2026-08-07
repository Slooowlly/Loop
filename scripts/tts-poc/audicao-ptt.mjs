#!/usr/bin/env node
// AUDIÇÃO DO PUSH-TO-TALK — a resposta gerada AO VIVO encosta na frase pré-gravada?
//
// Este script existe para responder a única pergunta que pode matar o desenho do PTT
// antes de ele ser construído. O `tts-poc-latencia.md` já registrou que a Chirp 3 é
// generativa e não tem `seed`, e concluiu: a deriva de timbre ficou abaixo do limiar
// perceptivo NUM PACOTE PRÉ-GRAVADO, onde cada tomada é ouvida antes de entrar — mas
// "não bastaria para geração ao vivo".
//
// O PTT é exatamente o caso que aquela frase excluiu, e no pior arranjo possível: uma
// frase de espera CURADA toca, e ~4 s depois uma resposta CRUA, de uma chamada nova,
// sai no mesmo ouvido. Se o engenheiro trocar de pessoa nesse vão, a ilusão morre —
// e morre de um jeito que nenhuma latência mata.
//
// O que se mede aqui, então, são duas coisas de uma vez:
//
// 1. **Timbre.** Cada resposta é colada atrás de uma frase de espera e o par vira um
//    arquivo só. A audição é A/B forçado: mesma pessoa ou duas?
// 2. **Latência.** A POC mediu o Gemini TTS, nunca a Cloud TTS. Sem streaming, o
//    Gemini escalava com o tamanho do texto (3,1 s para curto, 7,1 s para ~111
//    caracteres). Se a Cloud fizer o mesmo, "limite de palavras na resposta" deixa de
//    ser estilo e vira requisito de arquitetura. A tabela no fim responde isso.
//
// ORDEM OBRIGATÓRIA — decodificar → colar → filtrar. As peças são geradas CRUAS, o
// silêncio de borda é aparado, elas são coladas, e só então a cadeia de rádio roda no
// sinal inteiro. Filtrar cada peça antes de colar deixaria o compressor com histórico
// diferente em cada uma, e a emenda ganharia um salto de volume.
//
// Uso:
//   node scripts/tts-poc/audicao-ptt.mjs
//   node scripts/tts-poc/audicao-ptt.mjs --n 4 --pausa 1200
//   node scripts/tts-poc/audicao-ptt.mjs --controle    (mesma frase 3x, deriva pura)

import { execFileSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { aparar, aplicarRadio, buracoInterno, escreverWav, lerWav, pico, rms } from "./filtro-radio.mjs";

const ENDPOINT = "https://texttospeech.googleapis.com/v1/text:synthesize";
const IDIOMA = "pt-BR";
const VOZ = "pt-BR-Chirp3-HD-Algenib";
const TAXA = 24000;
const DIR = path.join("docs", "tts-poc", "audicao-ptt");
const DIR_CRU = path.join(DIR, "cru");

// AS FRASES DE ESPERA. Três, e não uma, pela mesma razão que o lembrete do spotter tem
// variações: a mesma frase toda vez deixa de ser fala e vira bipe de eletrodoméstico.
//
// Todas terminam em reticências DE PROPÓSITO. A POC descobriu que a pontuação manda mais
// na entonação que a voz escolhida, e que é decisão por frase: o `...` deixa a fala morrer
// aberta, como quem vai emendar a informação a seguir — que é literalmente o que estas
// frases fazem. Ponto final aqui fecharia a conversa antes de ela começar.
const ESPERAS = [
  "Ok, deixa eu ver aqui…",
  "Peraí, tô checando…",
  "Deixa eu olhar…",
];

// A DESISTÊNCIA. Quando o encanamento estoura o orçamento de tempo, o engenheiro precisa
// dizer alguma coisa — silêncio depois de um "deixa eu ver" é pior que uma negativa.
// Esta FECHA com ponto: é conclusiva, não emenda nada.
const DESISTENCIA = "Não consegui ver isso agora.";

// AS RESPOSTAS. São o que o Gemini deve produzir: curtas, informativas, voz do engenheiro
// em 2ª pessoa. O comprimento varia de propósito (30 a 130 caracteres) — é o eixo que
// responde se a latência da Cloud TTS escala com o texto, como escalava no Gemini.
//
// O conteúdo é escolhido dentro do que a telemetria REALMENTE entrega. Nada sobre desgaste
// ou temperatura de pneu: o `tire_compound` do iRacing vem sempre 0 e essa família de dado
// não existe para nós. Prometer no teste o que o produto não pode cumprir enviesa a audição.
const RESPOSTAS = [
  "Quinto lugar.",
  "Faltam doze voltas.",
  "Gap de um e dois pro carro da frente.",
  "Bandeira amarela no setor dois. Levanta o pé.",
  "Você ganhou duas posições na largada. Sexto agora.",
  "O carro de trás está a oito décimos e vem vindo forte.",
  "Seu último setor foi o melhor da corrida. Mantém esse ritmo.",
  "Asa dianteira com dano leve. Dá pra levar até o fim, mas evita o meio-fio.",
  "O líder está a vinte e dois segundos. Esquece ele e foca no carro da frente.",
  "Você está a três décimos do seu melhor tempo e o pelotão inteiro caiu de ritmo agora.",
];

function lerArgumentos(argv) {
  // A pausa é o vão entre a espera e a resposta. 700 ms e não os ~4 s reais do produto:
  // o teste aqui é de TIMBRE, e quanto mais perto as duas falas estiverem, mais dura fica
  // a comparação. Se passar colado, passa com folga separado.
  const o = { n: RESPOSTAS.length, pausaMs: 700, projeto: process.env.GCP_PROJECT || "", controle: false };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    const proximo = () => argv[(i += 1)];
    if (a === "--n") o.n = Number(proximo());
    else if (a === "--pausa") o.pausaMs = Number(proximo());
    else if (a === "--projeto") o.projeto = proximo();
    else if (a === "--controle") o.controle = true;
  }
  return o;
}

/** Token do ADC. `GCP_TOKEN` no ambiente tem precedência (evita um processo por chamada). */
function token() {
  if (process.env.GCP_TOKEN) return process.env.GCP_TOKEN.trim();
  try {
    return execFileSync("gcloud", ["auth", "application-default", "print-access-token"], {
      encoding: "utf8",
      shell: true,
    }).trim();
  } catch {
    console.error(
      "Sem credencial. Rode uma vez:\n" +
        "  gcloud auth application-default login\n" +
        "  gcloud services enable texttospeech.googleapis.com",
    );
    process.exit(1);
  }
}

function projetoDeCota(informado) {
  if (informado) return informado;
  try {
    return execFileSync("gcloud", ["config", "get-value", "project"], { encoding: "utf8", shell: true }).trim();
  } catch {
    return "";
  }
}

/**
 * Uma geração CRUA — sem rádio, sem aparar. Devolve o caminho do `.wav`, o tempo de
 * parede da chamada e o hash. O tempo medido é o da requisição inteira: não há streaming
 * neste endpoint, então "primeiro byte" e "áudio pronto" são o mesmo instante, e é esse
 * instante que o orçamento do PTT precisa pagar.
 */
async function gerar(texto, rotulo, acesso, projeto) {
  const t0 = performance.now();
  const resposta = await fetch(ENDPOINT, {
    method: "POST",
    headers: {
      authorization: `Bearer ${acesso}`,
      "x-goog-user-project": projeto,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      input: { text: texto },
      voice: { languageCode: IDIOMA, name: VOZ },
      audioConfig: { audioEncoding: "LINEAR16", sampleRateHertz: TAXA },
    }),
  });
  const ms = Math.round(performance.now() - t0);

  if (!resposta.ok) {
    // A moderação bloqueia texto inocente e de forma NÃO determinística — `Livre.` deu 5
    // bloqueios em 9 tentativas na POC. Falhar a bateria inteira por causa disso apagaria
    // a medição; a peça some com um aviso e as outras seguem.
    console.error(`  ✗ ${rotulo}: HTTP ${resposta.status} — ${(await resposta.text()).slice(0, 200)}`);
    return null;
  }

  const { audioContent } = await resposta.json();
  const wav = Buffer.from(audioContent, "base64"); // LINEAR16 já vem com cabeçalho RIFF
  const caminho = path.join(DIR_CRU, `${rotulo}.wav`);
  fs.writeFileSync(caminho, wav);
  return {
    caminho,
    ms,
    texto,
    caracteres: texto.length,
    sha: crypto.createHash("sha256").update(wav).digest("hex").slice(0, 12),
    segundos: (wav.length - 44) / (TAXA * 2),
  };
}

/** Apara o silêncio de cada peça, cola com pausa, e SÓ ENTÃO aplica o rádio. */
function colar(caminhos, pausaMs, destino) {
  const pecas = caminhos.map((c) => {
    const { amostras, taxa } = lerWav(c);
    const util = aparar(amostras, taxa);
    return { caminho: c, taxa, original: amostras, util, buraco: buracoInterno(util, taxa) };
  });
  const taxa = pecas[0].taxa;
  const pausa = Math.round((pausaMs / 1000) * taxa);
  const total = pecas.reduce((s, p) => s + p.util.length, 0) + pausa * (pecas.length - 1);
  const junto = new Float32Array(total);
  let cursor = 0;
  for (const [i, p] of pecas.entries()) {
    junto.set(p.util, cursor);
    cursor += p.util.length + (i < pecas.length - 1 ? pausa : 0);
  }
  const final = aplicarRadio(junto, taxa);
  escreverWav(destino, final, taxa);
  return { pecas, taxa, duracao: junto.length / taxa, rms: rms(final), pico: pico(final) };
}

function percentil(valores, p) {
  if (!valores.length) return 0;
  const ordenado = [...valores].sort((a, b) => a - b);
  return ordenado[Math.min(ordenado.length - 1, Math.floor((p / 100) * ordenado.length))];
}

// ─────────────────────────────────────────────────────────────────────────────

const opcoes = lerArgumentos(process.argv);
const acesso = token();
const projeto = projetoDeCota(opcoes.projeto);
if (!projeto) {
  console.error("Projeto de cota não resolvido. Passe --projeto <id>.");
  process.exit(1);
}
fs.mkdirSync(DIR_CRU, { recursive: true });

const latencias = [];
const registrar = (g) => {
  if (g) latencias.push({ ms: g.ms, caracteres: g.caracteres, segundos: g.segundos, texto: g.texto });
  return g;
};

// CONTROLE — a mesma frase três vezes. Isola a deriva pura, sem a variável do texto
// diferente: se estas três já soarem como pessoas distintas, o problema não é o PTT.
if (opcoes.controle) {
  console.log(`\nCONTROLE — "${ESPERAS[0]}" três vezes\n`);
  const tomadas = [];
  for (let i = 0; i < 3; i += 1) {
    const g = registrar(await gerar(ESPERAS[0], `controle-t${i + 1}`, acesso, projeto));
    if (g) {
      tomadas.push(g);
      console.log(`  t${i + 1}  ${String(g.ms).padStart(5)} ms  ${g.segundos.toFixed(2)}s  sha ${g.sha}`);
    }
  }
  const unicos = new Set(tomadas.map((t) => t.sha));
  console.log(
    `\n  ${unicos.size === 1 ? "bytes idênticos" : `${unicos.size} saídas distintas em ${tomadas.length}`}` +
      `  ·  durações ${tomadas.map((t) => t.segundos.toFixed(2)).join(" / ")}s`,
  );
  const destino = path.join(DIR, "controle-3-tomadas.wav");
  colar(tomadas.map((t) => t.caminho), opcoes.pausaMs, destino);
  console.log(`  ${destino}\n`);
}

// AS ESPERAS. Geradas uma vez cada — no produto elas são pré-gravadas e curadas, então
// aqui elas fazem o papel de "a tomada boa" contra a qual a resposta ao vivo é comparada.
console.log("\nFRASES DE ESPERA (o pré-gravado, a âncora da comparação)\n");
const esperas = [];
for (const [i, texto] of ESPERAS.entries()) {
  const g = registrar(await gerar(texto, `espera-${i + 1}`, acesso, projeto));
  if (g) {
    esperas.push(g);
    console.log(`  ${String(g.ms).padStart(5)} ms  ${g.segundos.toFixed(2)}s  "${texto}"`);
  }
}
if (!esperas.length) {
  console.error("Nenhuma frase de espera foi gerada — sem âncora não há comparação.");
  process.exit(1);
}

const desistencia = registrar(await gerar(DESISTENCIA, "desistencia", acesso, projeto));
if (desistencia) console.log(`  ${String(desistencia.ms).padStart(5)} ms  ${desistencia.segundos.toFixed(2)}s  "${DESISTENCIA}"  (desistência)`);

// AS RESPOSTAS, cada uma colada atrás de uma espera. O rodízio das esperas é o mesmo do
// produto, e serve à audição: ouvir a mesma âncora dez vezes seguidas anestesia o ouvido
// justamente para a diferença que se quer detectar.
console.log("\nRESPOSTAS AO VIVO (cada uma colada atrás de uma espera)\n");
const pares = [];
for (let i = 0; i < Math.min(opcoes.n, RESPOSTAS.length); i += 1) {
  const rotulo = String(i + 1).padStart(2, "0");
  const g = registrar(await gerar(RESPOSTAS[i], `resposta-${rotulo}`, acesso, projeto));
  if (!g) continue;
  const espera = esperas[i % esperas.length];
  const destino = path.join(DIR, `ptt-${rotulo}.wav`);
  const junto = colar([espera.caminho, g.caminho], opcoes.pausaMs, destino);
  const alarme = junto.pecas.find((p) => p.buraco) ? "  ⚠ buraco interno" : "";
  pares.push({ destino, resposta: g, espera });
  console.log(
    `  ${rotulo}  ${String(g.ms).padStart(5)} ms  ${String(g.caracteres).padStart(3)} car  ` +
      `${junto.duracao.toFixed(2)}s total  sha ${g.sha}${alarme}\n` +
      `      "${espera.texto}"  +  "${g.texto}"\n` +
      `      ${destino}`,
  );
}

// ─── A TABELA QUE DECIDE ──────────────────────────────────────────────────────

const ms = latencias.map((l) => l.ms);
console.log("\n" + "─".repeat(78));
console.log("\nLATÊNCIA DA CLOUD TTS (chamada inteira — não há streaming neste endpoint)\n");
console.log(
  `  n ${String(ms.length).padStart(3)}   melhor ${percentil(ms, 0)} ms   mediana ${percentil(ms, 50)} ms   ` +
    `P90 ${percentil(ms, 90)} ms   pior ${percentil(ms, 100)} ms`,
);

// Escala com o texto? Foi o que matou a geração sem streaming no Gemini (3,1 s no curto,
// 10,1 s na narrativa). Se a Cloud repetir a curva, o teto de palavras da resposta vira
// requisito de latência; se for plana, o Gemini pode escrever à vontade.
const curtas = latencias.filter((l) => l.caracteres <= 40);
const longas = latencias.filter((l) => l.caracteres >= 70);
if (curtas.length && longas.length) {
  const mediaC = Math.round(curtas.reduce((s, l) => s + l.ms, 0) / curtas.length);
  const mediaL = Math.round(longas.reduce((s, l) => s + l.ms, 0) / longas.length);
  console.log(
    `\n  até 40 car. (n ${curtas.length}): ${mediaC} ms de média\n` +
      `  70+  car. (n ${longas.length}): ${mediaL} ms de média\n` +
      `  fator ${(mediaL / mediaC).toFixed(2)}× — ${mediaL / mediaC > 1.5 ? "ESCALA com o texto: limitar o tamanho da resposta é requisito de latência." : "praticamente PLANA: o tamanho da resposta não é o gargalo."}`,
  );
}

console.log("\nORÇAMENTO DO PTT — a TTS é só a última das três etapas:");
console.log(`  Scribe (?) + Gemini (?) + Cloud TTS (${percentil(ms, 50)} ms medido) = o total ainda não medido.`);
console.log(`\nOs pares para ouvir estão em ${DIR}. A pergunta é uma só: mesma pessoa nas duas falas?\n`);
