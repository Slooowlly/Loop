#!/usr/bin/env node
// Cadeia de rádio aplicada a arquivos WAV, fora do navegador.
//
// É a MESMA receita de `src/dev/tts/ttsRadio.js`, que roda no Web Audio dentro do app:
// passa-altas 300 Hz -> passa-baixas 3200 Hz -> presença 1800 Hz -> saturação ->
// ganho -> compressão. Existe aqui para audição: dá para ouvir o efeito num `.wav`
// pronto sem subir o app inteiro, e para gerar o "antes e depois" lado a lado.
//
// NÃO é idêntica ao bit: o `DynamicsCompressor` do Chromium tem curva de joelho e
// look-ahead próprios, e o sobreamostrado 2× do `WaveShaper` usa filtros que a
// especificação não define. O que se reproduz aqui é o SOM, não a amostra. Para a
// decisão de timbre isso basta; se algum dia a diferença importar, a referência é o
// app, não este script.
//
// Uso:
//   node scripts/tts-poc/filtro-radio.mjs docs/tts-poc/audicao-cloud/*.wav
//   node scripts/tts-poc/filtro-radio.mjs --sufixo -radio arquivo.wav

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

// Mesmos valores de ttsRadio.js. Mudou lá, muda aqui.
const CORTE_GRAVE_HZ = 300;
const CORTE_AGUDO_HZ = 3200;
const PRESENCA_HZ = 1800;
const PRESENCA_DB = 6;
const PRESENCA_Q = 1.1;
const Q_CORTE = 0.7;
const SATURACAO_K = 3;
const GANHO = 1.9;
const COMP_LIMIAR_DB = -24;
const COMP_JOELHO_DB = 6;
const COMP_RAZAO = 8;
const COMP_ATAQUE_S = 0.003;
const COMP_RELEASE_S = 0.12;
// Recuperação de nível pós-compressor. Ver a explicação em ttsRadio.js: sem ela a
// cadeia sai três a quatro vezes mais baixa que a voz limpa.
const GANHO_SAIDA = 3.5;

// ---------------------------------------------------------------------------
// WAV
// ---------------------------------------------------------------------------

function lerWav(caminho) {
  const buf = fs.readFileSync(caminho);
  if (buf.toString("ascii", 0, 4) !== "RIFF" || buf.toString("ascii", 8, 12) !== "WAVE") {
    throw new Error(`${caminho} não é WAV`);
  }
  let i = 12;
  let taxa = 24000;
  let canais = 1;
  let bits = 16;
  let corpo = null;
  while (i + 8 <= buf.length) {
    const id = buf.toString("ascii", i, i + 4);
    const tamanho = buf.readUInt32LE(i + 4);
    if (id === "fmt ") {
      canais = buf.readUInt16LE(i + 10);
      taxa = buf.readUInt32LE(i + 12);
      bits = buf.readUInt16LE(i + 22);
    } else if (id === "data") {
      corpo = buf.subarray(i + 8, i + 8 + tamanho);
      break;
    }
    i += 8 + tamanho + (tamanho % 2);
  }
  if (!corpo) throw new Error(`${caminho} sem bloco data`);
  if (bits !== 16) throw new Error(`${caminho}: só 16 bits (veio ${bits})`);

  // Mistura para mono: a cadeia de rádio é monofônica por natureza.
  const totalQuadros = Math.floor(corpo.length / 2 / canais);
  const amostras = new Float32Array(totalQuadros);
  for (let q = 0; q < totalQuadros; q += 1) {
    let soma = 0;
    for (let c = 0; c < canais; c += 1) soma += corpo.readInt16LE((q * canais + c) * 2) / 32768;
    amostras[q] = soma / canais;
  }
  return { amostras, taxa };
}

function escreverWav(caminho, amostras, taxa) {
  const corpo = Buffer.alloc(amostras.length * 2);
  for (let i = 0; i < amostras.length; i += 1) {
    const v = Math.max(-1, Math.min(1, amostras[i]));
    corpo.writeInt16LE(Math.round(v * 32767), i * 2);
  }
  const c = Buffer.alloc(44);
  c.write("RIFF", 0);
  c.writeUInt32LE(36 + corpo.length, 4);
  c.write("WAVE", 8);
  c.write("fmt ", 12);
  c.writeUInt32LE(16, 16);
  c.writeUInt16LE(1, 20);
  c.writeUInt16LE(1, 22);
  c.writeUInt32LE(taxa, 24);
  c.writeUInt32LE(taxa * 2, 28);
  c.writeUInt16LE(2, 32);
  c.writeUInt16LE(16, 34);
  c.write("data", 36);
  c.writeUInt32LE(corpo.length, 40);
  fs.writeFileSync(caminho, Buffer.concat([c, corpo]));
}

// ---------------------------------------------------------------------------
// Biquads — fórmulas do "Audio EQ Cookbook" (RBJ), as mesmas que a
// especificação do Web Audio manda o BiquadFilterNode usar.
// ---------------------------------------------------------------------------

function coefPassaAltas(f0, Q, fs) {
  const w0 = (2 * Math.PI * f0) / fs;
  const alpha = Math.sin(w0) / (2 * Q);
  const cos = Math.cos(w0);
  return normalizar(
    [(1 + cos) / 2, -(1 + cos), (1 + cos) / 2],
    [1 + alpha, -2 * cos, 1 - alpha],
  );
}

function coefPassaBaixas(f0, Q, fs) {
  const w0 = (2 * Math.PI * f0) / fs;
  const alpha = Math.sin(w0) / (2 * Q);
  const cos = Math.cos(w0);
  return normalizar([(1 - cos) / 2, 1 - cos, (1 - cos) / 2], [1 + alpha, -2 * cos, 1 - alpha]);
}

function coefPresenca(f0, Q, ganhoDb, fs) {
  const A = 10 ** (ganhoDb / 40);
  const w0 = (2 * Math.PI * f0) / fs;
  const alpha = Math.sin(w0) / (2 * Q);
  const cos = Math.cos(w0);
  return normalizar(
    [1 + alpha * A, -2 * cos, 1 - alpha * A],
    [1 + alpha / A, -2 * cos, 1 - alpha / A],
  );
}

function normalizar([b0, b1, b2], [a0, a1, a2]) {
  return { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 };
}

/** Forma direta I, em cascata sobre o sinal inteiro. */
function aplicarBiquad(x, c) {
  const y = new Float32Array(x.length);
  let x1 = 0;
  let x2 = 0;
  let y1 = 0;
  let y2 = 0;
  for (let i = 0; i < x.length; i += 1) {
    const v = c.b0 * x[i] + c.b1 * x1 + c.b2 * x2 - c.a1 * y1 - c.a2 * y2;
    x2 = x1;
    x1 = x[i];
    y2 = y1;
    y1 = v;
    y[i] = v;
  }
  return y;
}

// ---------------------------------------------------------------------------
// Saturação e compressão
// ---------------------------------------------------------------------------

/**
 * Saturação suave com sobreamostragem 2× (interpolação linear na subida, média na
 * descida). O sinal já entra limitado a 3,2 kHz, então os harmônicos que a tanh cria
 * ficam bem abaixo de Nyquist — a sobreamostragem aqui é margem, não necessidade.
 */
function saturar(x, k = SATURACAO_K) {
  const norma = Math.tanh(k);
  const alto = new Float32Array(x.length * 2);
  for (let i = 0; i < x.length; i += 1) {
    const anterior = i > 0 ? x[i - 1] : x[0];
    alto[i * 2] = (anterior + x[i]) / 2;
    alto[i * 2 + 1] = x[i];
  }
  for (let i = 0; i < alto.length; i += 1) alto[i] = Math.tanh(k * alto[i]) / norma;
  const y = new Float32Array(x.length);
  for (let i = 0; i < x.length; i += 1) y[i] = (alto[i * 2] + alto[i * 2 + 1]) / 2;
  return y;
}

/**
 * Limitador macio de amostra, último da cadeia. Abaixo de `t` não toca em nada; acima,
 * curva suave que satura em 1,0.
 *
 * Existe porque o compressor tem ataque de 3 ms e o transiente ESCAPA por baixo dele —
 * medido: com a recuperação de nível, os picos chegavam a 1,7 e recortavam. Baixar a
 * recuperação resolveria o pico e devolveria o problema do volume; o limitador resolve
 * os dois, mexendo só nos poucos milissegundos que passam do teto.
 */
// ATENÇÃO a quem for portar isto para o navegador: aqui a curva é aplicada em unidades
// REAIS de amostra, e passa de ±1 sem problema. O `WaveShaper` do Web Audio não — ele
// indexa a curva sempre pela faixa ±1 da entrada. Lá é preciso um ganho de 1/2 ANTES do
// nó, senão o limitador vira um expansor de 2×. Ver `LIMITADOR_ENTRADA_MAX` em
// `src/lib/filtroRadio.js`, que é onde esse defeito morou.
function limitar(x, t = 0.7, teto = 0.97) {
  const y = new Float32Array(x.length);
  const faixa = teto - t;
  for (let i = 0; i < x.length; i += 1) {
    const a = Math.abs(x[i]);
    y[i] = a <= t ? x[i] : Math.sign(x[i]) * (t + faixa * Math.tanh((a - t) / faixa));
  }
  return y;
}

/** Compressor de joelho suave, com envelope de ataque/release na REDUÇÃO de ganho. */
function comprimir(x, fs) {
  const y = new Float32Array(x.length);
  const ataque = Math.exp(-1 / (COMP_ATAQUE_S * fs));
  const release = Math.exp(-1 / (COMP_RELEASE_S * fs));
  let reducaoDb = 0;

  for (let i = 0; i < x.length; i += 1) {
    const nivelDb = 20 * Math.log10(Math.max(Math.abs(x[i]), 1e-6));
    const acima = nivelDb - COMP_LIMIAR_DB;

    let alvoDb;
    if (2 * acima < -COMP_JOELHO_DB) {
      alvoDb = 0;
    } else if (2 * Math.abs(acima) <= COMP_JOELHO_DB) {
      // Dentro do joelho: transição quadrática, como manda a especificação.
      const t = acima + COMP_JOELHO_DB / 2;
      alvoDb = ((1 / COMP_RAZAO - 1) * t * t) / (2 * COMP_JOELHO_DB);
    } else {
      alvoDb = acima * (1 / COMP_RAZAO - 1);
    }

    // Reduzir mais é "atacar"; voltar é "soltar".
    const coef = alvoDb < reducaoDb ? ataque : release;
    reducaoDb = coef * reducaoDb + (1 - coef) * alvoDb;
    y[i] = x[i] * 10 ** (reducaoDb / 20);
  }
  return y;
}

// ---------------------------------------------------------------------------

export function aplicarRadio(amostras, fs) {
  let s = aplicarBiquad(amostras, coefPassaAltas(CORTE_GRAVE_HZ, Q_CORTE, fs));
  s = aplicarBiquad(s, coefPassaBaixas(CORTE_AGUDO_HZ, Q_CORTE, fs));
  s = aplicarBiquad(s, coefPresenca(PRESENCA_HZ, PRESENCA_Q, PRESENCA_DB, fs));
  s = saturar(s);
  // Ganho ANTES do compressor, como em ttsRadio.js: com ele depois, nada segura o pico.
  const g = new Float32Array(s.length);
  for (let i = 0; i < s.length; i += 1) g[i] = s[i] * GANHO;
  const c = comprimir(g, fs);
  for (let i = 0; i < c.length; i += 1) c[i] *= GANHO_SAIDA;
  return limitar(c);
}

function pico(x) {
  let p = 0;
  for (const v of x) p = Math.max(p, Math.abs(v));
  return p;
}

function rms(x) {
  let soma = 0;
  for (const v of x) soma += v * v;
  return Math.sqrt(soma / x.length);
}

export { lerWav, escreverWav, pico, rms };

// ─────────────────────────── APARAR SILÊNCIO ───────────────────────────
// Mora aqui porque tem DOIS clientes — a colagem de peças (`juntar.mjs`) e o gerador
// do pacote do spotter — e a calibragem abaixo custou medição para acertar. Duas
// cópias divergindo seria o pior dos mundos: a colagem soaria diferente do pacote.

// Janela de análise. O corte NÃO pode olhar amostra a amostra: o silêncio do Chirp 3
// não é zero, é um chiado baixo cujos picos isolados passam de -45 dBFS. Medido: em
// "Volta em", 0,6 s de silêncio de cabeça sobreviviam intactos a um limiar de pico,
// porque bastava uma amostra de ruído para o trecho contar como fala. Energia média
// numa janela curta separa chiado de voz; pico não separa.
const JANELA_MS = 10;
// Limiar RELATIVO ao pico da própria peça. Absoluto não serve porque cada geração sai
// num nível diferente — "um," saiu 10 dB mais baixo que "um trinta e dois,".
const QUEDA_DB = 32;
// Piso absoluto, para peças que sejam só ruído não virarem "fala" por comparação.
const PISO_DB = -50;
// Margem em torno da fala. Cortar na primeira janela acima do limiar come o ataque da
// consoante; 30 ms de folga devolvem o "D" de "Daniel".
const MARGEM_MS = 30;
// Rampa anti-clique nas pontas de cada peça.
const RAMPA_MS = 5;

/** Energia média por janela, em dB. */
function envelope(x, taxa) {
  const jan = Math.max(1, Math.round((JANELA_MS / 1000) * taxa));
  const db = [];
  for (let i = 0; i < x.length; i += jan) {
    let soma = 0;
    const ate = Math.min(i + jan, x.length);
    for (let j = i; j < ate; j += 1) soma += x[j] * x[j];
    db.push(20 * Math.log10(Math.max(Math.sqrt(soma / (ate - i)), 1e-9)));
  }
  return { db, jan };
}

/** Índices do primeiro e do último instante com sinal, já com a margem aplicada. */
export function faixaUtil(x, taxa) {
  const { db, jan } = envelope(x, taxa);
  const topo = Math.max(...db);
  const limiar = Math.max(topo - QUEDA_DB, PISO_DB);
  let a = 0;
  let b = db.length - 1;
  while (a < db.length && db[a] < limiar) a += 1;
  while (b > a && db[b] < limiar) b -= 1;
  if (a >= b) return { inicio: 0, fim: x.length - 1 }; // peça toda silenciosa: preserva
  const margem = Math.round((MARGEM_MS / 1000) * taxa);
  return {
    inicio: Math.max(0, a * jan - margem),
    fim: Math.min(x.length - 1, (b + 1) * jan - 1 + margem),
  };
}

/** Corta o silêncio das bordas e aplica a rampa anti-clique. */
export function aparar(x, taxa) {
  const { inicio, fim } = faixaUtil(x, taxa);
  const corte = x.slice(inicio, fim + 1);
  const rampa = Math.min(Math.round((RAMPA_MS / 1000) * taxa), Math.floor(corte.length / 2));
  for (let i = 0; i < rampa; i += 1) {
    const g = i / rampa;
    corte[i] *= g;
    corte[corte.length - 1 - i] *= g;
  }
  return corte;
}

/**
 * Silêncio DENTRO da peça, depois de aparada. Uma peça curta não deveria ter nenhum —
 * quando tem, o modelo dividiu a fala em dois fôlegos ou emendou algo que não foi
 * pedido, e nenhuma colagem conserta isso. É defeito de material, e o único conserto é
 * regerar a peça. Vale gritar, senão vira um buraco inexplicável no meio da frase.
 */
export function buracoInterno(x, taxa) {
  const { db, jan } = envelope(x, taxa);
  const limiar = Math.max(Math.max(...db) - QUEDA_DB, PISO_DB);
  const minimoJanelas = Math.round(200 / JANELA_MS); // 200 ms
  let corrida = 0;
  let maior = 0;
  for (const v of db) {
    corrida = v < limiar ? corrida + 1 : 0;
    maior = Math.max(maior, corrida);
  }
  return maior >= minimoJanelas ? (maior * jan) / taxa : 0;
}

/** Lê um WAV, aplica a cadeia e grava o irmão com sufixo. Devolve o caminho gravado. */
export function processarArquivo(entrada, sufixo = "-radio") {
  const { amostras, taxa } = lerWav(entrada);
  const saida = aplicarRadio(amostras, taxa);
  const destino = path.join(
    path.dirname(entrada),
    `${path.basename(entrada, ".wav")}${sufixo}.wav`,
  );
  escreverWav(destino, saida, taxa);
  return { destino, antes: amostras, depois: saida };
}

// Só roda como CLI quando invocado direto. Sem esta guarda, `import` deste módulo
// dispararia a leitura de argv de quem importou.
const chamadoDireto =
  process.argv[1] && import.meta.url === new URL(`file://${process.argv[1].replace(/\\/g, "/")}`).href;

if (chamadoDireto) {
  const args = process.argv.slice(2);
  let sufixo = "-radio";
  const arquivos = [];
  for (let i = 0; i < args.length; i += 1) {
    if (args[i] === "--sufixo") sufixo = args[(i += 1)];
    else arquivos.push(args[i]);
  }

  if (arquivos.length === 0) {
    console.error("Passe um ou mais arquivos .wav.");
    process.exit(1);
  }

  for (const entrada of arquivos) {
    const { destino, antes, depois } = processarArquivo(entrada, sufixo);
    const clipes = Array.from(depois).filter((v) => Math.abs(v) >= 0.999).length;
    console.log(
      `${path.basename(entrada).padEnd(46)} pico ${pico(antes).toFixed(2)}→${pico(depois).toFixed(2)}  ` +
        `rms ${rms(antes).toFixed(3)}→${rms(depois).toFixed(3)}  ` +
        `${clipes ? `CLIPE em ${clipes} amostras` : "sem clipe"}  ${path.basename(destino)}`,
    );
  }
}
