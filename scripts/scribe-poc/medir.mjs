#!/usr/bin/env node
// MEDIÇÃO DO SCRIBE — a perna do push-to-talk que ficou sem número.
//
// A POC de voz mediu a Cloud TTS (mediana de 1,1 s, plana em relação ao tamanho do texto) e
// deixou duas etapas por medir. Depois que o desenho mudou — a maioria das perguntas passou a
// ser respondida por peças pré-gravadas, sem Gemini nenhum — o Scribe deixou de ser uma etapa
// entre cinco e virou **a única coisa no caminho de ~80% das perguntas**. É a medição que
// decide se o caminho barato é rápido de verdade.
//
// ## O que se mede aqui, e o que NÃO se mede
//
// A LATÊNCIA é medida com validade total: o tempo de uma chamada não sabe se o áudio veio de
// um microfone ou de um sintetizador.
//
// A ACURÁCIA, não. O áudio é gerado pela mesma Cloud TTS do engenheiro — voz de estúdio, sem
// motor, sem headset, sem ruído de cockpit. Qualquer taxa de acerto aqui é um TETO, não uma
// previsão. O que este eixo testa de fato é o NOSSO lado: se o classificador de intenção erra
// com áudio limpo, o problema está na tabela de termos e nenhum microfone melhor conserta.
//
// Por isso cada pergunta é transcrita DUAS vezes — limpa e passada pela cadeia de rádio. A
// cadeia corta em 3,2 kHz, satura e comprime; não é ruído de motor, mas é a degradação que um
// rádio de equipe impõe, e a diferença entre as duas colunas é o primeiro sinal de quão
// sensível o Scribe é a áudio sujo.
//
// ## Parâmetros que trabalham contra nós por padrão
//
// - `tag_audio_events` vem LIGADO e marca risadas, passos e afins. Numa pergunta de três
//   segundos isso é processamento puro sem uso nenhum.
// - `timestamps_granularity` vem em `word`. Não usamos carimbo de tempo em lugar nenhum.
//
// Os dois são desligados aqui. `--padroes` mede com os padrões deles, para saber quanto
// custam — é a diferença que justifica (ou não) manter a configuração explícita no servidor.
//
// Uso:
//   node scripts/scribe-poc/medir.mjs
//   node scripts/scribe-poc/medir.mjs --modelo scribe_v1 --repetir 2
//   node scripts/scribe-poc/medir.mjs --so-limpo --padroes

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { aplicarRadio, escreverWav, lerWav } from "../tts-poc/filtro-radio.mjs";

const TTS_ENDPOINT = "https://texttospeech.googleapis.com/v1/text:synthesize";
const SCRIBE_ENDPOINT = "https://api.elevenlabs.io/v1/speech-to-text";
const VOZ = "pt-BR-Chirp3-HD-Algenib";
const TAXA = 24000;
const DIR = path.join("docs", "scribe-poc");
const DIR_AUDIO = path.join(DIR, "audio");

// AS PERGUNTAS. Uma por intenção que o `engenheiro::intencao` conhece, mais as ambíguas que
// já quebraram a tabela uma vez. `intencao` é o que o classificador DEVE devolver — é o
// gabarito do terceiro eixo da medição.
//
// O comprimento varia de propósito (de 3 a 12 palavras): é o eixo que responde se a latência
// do Scribe escala com a duração do clipe, como a do Gemini TTS escalava com o texto.
const PERGUNTAS = [
  { chave: "posicao", intencao: "Posicao", texto: "Em que posição eu estou?" },
  { chave: "frente", intencao: "Frente", texto: "Qual o gap pro carro da frente?" },
  { chave: "atras", intencao: "Atras", texto: "Quem está atrás de mim?" },
  { chave: "restante", intencao: "Restante", texto: "Quantas voltas ainda faltam?" },
  { chave: "combustivel", intencao: "Combustivel", texto: "Como está o combustível?" },
  { chave: "ritmo", intencao: "Ritmo", texto: "Como está o meu ritmo?" },
  { chave: "carro", intencao: "Carro", texto: "O carro aguenta até o fim?" },
  { chave: "bandeira", intencao: "Bandeira", texto: "Tem bandeira amarela na pista?" },
  { chave: "pista", intencao: "Pista", texto: "Vai chover nessa corrida?" },
  { chave: "pneu", intencao: "Pneu", texto: "Que pneu eu estou usando?" },
  { chave: "geral", intencao: "Geral", texto: "E aí, como estamos?" },
  // As ambíguas — cada uma já derrubou a tabela de termos ou quase.
  {
    chave: "pneu_do_vizinho",
    intencao: "Pneu",
    texto: "O carro da frente ainda está com pneu de seco?",
  },
  {
    chave: "combustivel_com_voltas",
    intencao: "Combustivel",
    texto: "Quantas voltas ainda dá o combustível?",
  },
  {
    chave: "frente_longa",
    intencao: "Frente",
    texto: "Quanto tempo eu tenho pro carro da frente e dá pra alcançar ele?",
  },
  {
    chave: "restante_longa",
    intencao: "Restante",
    texto: "Me fala quantas voltas ainda faltam pro fim dessa corrida por favor.",
  },
];

function lerArgumentos(argv) {
  const o = {
    modelo: "scribe_v2",
    repetir: 1,
    projeto: process.env.GCP_PROJECT || "",
    radio: true,
    limpo: true,
    padroes: false,
    regerar: false,
  };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    const proximo = () => argv[(i += 1)];
    if (a === "--modelo") o.modelo = proximo();
    else if (a === "--repetir") o.repetir = Number(proximo());
    else if (a === "--projeto") o.projeto = proximo();
    else if (a === "--so-limpo") o.radio = false;
    else if (a === "--so-radio") o.limpo = false;
    else if (a === "--padroes") o.padroes = true;
    else if (a === "--regerar") o.regerar = true;
  }
  return o;
}

/// A chave, do ambiente ou do arquivo — nunca de argumento de linha de comando, que fica no
/// histórico do shell e na lista de processos.
function chaveScribe() {
  const doAmbiente = process.env.ELEVENLABS_API_KEY?.trim();
  if (doAmbiente) return doAmbiente;
  const arquivo = path.join(
    process.env.APPDATA || process.env.HOME || ".",
    "elevenlabs_key.txt",
  );
  if (fs.existsSync(arquivo)) return fs.readFileSync(arquivo, "utf8").trim();
  console.error(
    "Sem chave da ElevenLabs. Defina ELEVENLABS_API_KEY no ambiente ou crie\n" +
      `  ${arquivo}`,
  );
  process.exit(1);
}

function tokenGoogle() {
  if (process.env.GCP_TOKEN) return process.env.GCP_TOKEN.trim();
  return execFileSync("gcloud", ["auth", "application-default", "print-access-token"], {
    encoding: "utf8",
    shell: true,
  }).trim();
}

function projetoDeCota(informado) {
  if (informado) return informado;
  try {
    return execFileSync("gcloud", ["config", "get-value", "project"], {
      encoding: "utf8",
      shell: true,
    }).trim();
  } catch {
    return "";
  }
}

/// Gera o clipe da pergunta com a MESMA voz do engenheiro. Reaproveita o arquivo se já
/// existir: a geração é determinística o bastante para a medição e cada regeração gasta cota
/// à toa. `--regerar` força.
async function gerarClipe(texto, chave, acesso, projeto, regerar) {
  const destino = path.join(DIR_AUDIO, `${chave}.wav`);
  if (fs.existsSync(destino) && !regerar) return destino;
  const resposta = await fetch(TTS_ENDPOINT, {
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
  if (!resposta.ok) {
    console.error(`  ✗ TTS ${chave}: HTTP ${resposta.status} — ${(await resposta.text()).slice(0, 200)}`);
    return null;
  }
  const { audioContent } = await resposta.json();
  fs.writeFileSync(destino, Buffer.from(audioContent, "base64"));
  return destino;
}

/// A versão suja: mesma cadeia de rádio do engenheiro, aplicada à pergunta. Aproxima a
/// compressão de um microfone de headset — não substitui ruído de motor, mas é o que dá para
/// simular sem gravar de verdade.
function versaoRadio(origem) {
  const destino = origem.replace(/\.wav$/, "-radio.wav");
  if (fs.existsSync(destino)) return destino;
  const { amostras, taxa } = lerWav(origem);
  escreverWav(destino, aplicarRadio(amostras, taxa), taxa);
  return destino;
}

/// Uma transcrição, cronometrada. O tempo é o da requisição inteira: o endpoint em lote não
/// tem streaming, então "primeiro byte" e "texto pronto" são o mesmo instante — e é esse
/// instante que o orçamento do push-to-talk paga.
async function transcrever(caminho, chave, modelo, padroes) {
  const form = new FormData();
  form.append("model_id", modelo);
  form.append("file", new Blob([fs.readFileSync(caminho)]), path.basename(caminho));
  // Declarar o idioma tira do modelo o trabalho de detectá-lo. Com clipes de três segundos a
  // detecção automática é justamente onde ela é menos confiável.
  form.append("language_code", "por");
  if (!padroes) {
    form.append("tag_audio_events", "false");
    form.append("timestamps_granularity", "none");
    form.append("diarize", "false");
  }

  const t0 = performance.now();
  const resposta = await fetch(SCRIBE_ENDPOINT, {
    method: "POST",
    headers: { "xi-api-key": chave },
    body: form,
  });
  const ms = Math.round(performance.now() - t0);
  if (!resposta.ok) {
    return { erro: `HTTP ${resposta.status}: ${(await resposta.text()).slice(0, 300)}`, ms };
  }
  const corpo = await resposta.json();
  return { texto: (corpo.text ?? "").trim(), ms, idioma: corpo.language_code };
}

/// Normaliza para comparar: minúsculas, sem acento, sem pontuação. É a MESMA normalização do
/// `engenheiro::intencao` — comparar de outro jeito mediria uma diferença que o classificador
/// nunca vai ver.
function normalizar(texto) {
  return texto
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^\p{L}\p{N}\s]/gu, "")
    .replace(/\s+/g, " ")
    .trim();
}

/// Erro por PALAVRA (distância de edição sobre a lista de palavras), que é a métrica que
/// importa aqui: o classificador casa termos, não caracteres. Um acento errado no meio de uma
/// palavra some na normalização; uma palavra trocada derruba a intenção.
function errosDePalavra(esperado, obtido) {
  const a = normalizar(esperado).split(" ").filter(Boolean);
  const b = normalizar(obtido).split(" ").filter(Boolean);
  const d = Array.from({ length: a.length + 1 }, (_, i) =>
    Array.from({ length: b.length + 1 }, (_, j) => (i === 0 ? j : j === 0 ? i : 0)),
  );
  for (let i = 1; i <= a.length; i += 1) {
    for (let j = 1; j <= b.length; j += 1) {
      d[i][j] =
        a[i - 1] === b[j - 1]
          ? d[i - 1][j - 1]
          : 1 + Math.min(d[i - 1][j], d[i][j - 1], d[i - 1][j - 1]);
    }
  }
  return { erros: d[a.length][b.length], palavras: a.length };
}

function percentil(valores, p) {
  if (!valores.length) return 0;
  const o = [...valores].sort((x, y) => x - y);
  return o[Math.min(o.length - 1, Math.floor((p / 100) * o.length))];
}

// ─────────────────────────────────────────────────────────────────────────────

const opcoes = lerArgumentos(process.argv);
const chave = chaveScribe();
const acesso = tokenGoogle();
const projeto = projetoDeCota(opcoes.projeto);
if (!projeto) {
  console.error("Projeto de cota do Google não resolvido. Passe --projeto <id>.");
  process.exit(1);
}
fs.mkdirSync(DIR_AUDIO, { recursive: true });

console.log(`\nModelo ${opcoes.modelo} · ${opcoes.padroes ? "PADRÕES da API" : "sem tags nem timestamps"}`);
console.log("Gerando os clipes com a Cloud TTS (reaproveita os que já existem)…\n");

const variantes = [];
for (const p of PERGUNTAS) {
  const limpo = await gerarClipe(p.texto, p.chave, acesso, projeto, opcoes.regerar);
  if (!limpo) continue;
  const { amostras, taxa } = lerWav(limpo);
  const segundos = amostras.length / taxa;
  if (opcoes.limpo) variantes.push({ ...p, caminho: limpo, sujeira: "limpo", segundos });
  if (opcoes.radio) {
    variantes.push({ ...p, caminho: versaoRadio(limpo), sujeira: "rádio", segundos });
  }
}

const linhas = [];
for (let volta = 0; volta < opcoes.repetir; volta += 1) {
  for (const v of variantes) {
    const r = await transcrever(v.caminho, chave, opcoes.modelo, opcoes.padroes);
    if (r.erro) {
      console.log(`  ✗ ${v.chave} (${v.sujeira}): ${r.erro}`);
      continue;
    }
    const { erros, palavras } = errosDePalavra(v.texto, r.texto);
    const exato = normalizar(v.texto) === normalizar(r.texto);
    // Os dois textos são nomeados à mão, e não por espalhamento: `v.texto` (o que foi dito)
    // e `r.texto` (o que voltou) têm o mesmo nome, e um `{...v, ...r}` faria o segundo
    // apagar o primeiro em silêncio — a medição passaria a comparar a transcrição com ela
    // mesma e reportaria 100% de acerto para sempre.
    linhas.push({
      chave: v.chave,
      sujeira: v.sujeira,
      segundos: v.segundos,
      intencao: v.intencao,
      esperado: v.texto,
      transcrito: r.texto,
      idioma: r.idioma,
      ms: r.ms,
      erros,
      palavras,
      exato,
    });
    const marca = exato ? "✓" : `≠ ${erros}/${palavras}`;
    console.log(
      `  ${String(r.ms).padStart(5)} ms  ${v.segundos.toFixed(1)}s  ${v.sujeira.padEnd(6)} ` +
        `${marca.padEnd(8)} ${v.chave.padEnd(22)} "${r.texto}"`,
    );
  }
}

// ─── Os números ──────────────────────────────────────────────────────────────

const ms = linhas.map((l) => l.ms);
console.log("\n" + "─".repeat(78));
console.log("\nLATÊNCIA DO SCRIBE (chamada inteira — o endpoint em lote não faz streaming)\n");
console.log(
  `  n ${String(ms.length).padStart(3)}   melhor ${percentil(ms, 0)} ms   mediana ${percentil(ms, 50)} ms   ` +
    `P90 ${percentil(ms, 90)} ms   pior ${percentil(ms, 100)} ms`,
);

const curtos = linhas.filter((l) => l.segundos <= 2.2);
const longos = linhas.filter((l) => l.segundos > 3.0);
if (curtos.length && longos.length) {
  const media = (xs) => Math.round(xs.reduce((s, l) => s + l.ms, 0) / xs.length);
  const mc = media(curtos);
  const ml = media(longos);
  console.log(
    `\n  clipes até 2,2 s (n ${curtos.length}): ${mc} ms\n` +
      `  clipes de 3 s+   (n ${longos.length}): ${ml} ms\n` +
      `  fator ${(ml / mc).toFixed(2)}× — ${ml / mc > 1.5 ? "ESCALA com a duração: perguntas longas custam caro." : "praticamente PLANA: falar mais não custa mais."}`,
  );
}

for (const sujeira of ["limpo", "rádio"]) {
  const grupo = linhas.filter((l) => l.sujeira === sujeira);
  if (!grupo.length) continue;
  const exatos = grupo.filter((l) => l.exato).length;
  const erros = grupo.reduce((s, l) => s + l.erros, 0);
  const palavras = grupo.reduce((s, l) => s + l.palavras, 0);
  console.log(
    `\nTRANSCRIÇÃO (${sujeira}): ${exatos}/${grupo.length} exatas · ` +
      `${erros} erros em ${palavras} palavras (${((erros / palavras) * 100).toFixed(1)}% WER)`,
  );
  for (const l of grupo.filter((x) => !x.exato)) {
    console.log(`    ${l.chave}`);
    console.log(`      dito:    "${l.esperado}"`);
    console.log(`      ouvido:  "${l.transcrito}"`);
  }
}

// O gabarito para o terceiro eixo: as transcrições vão para um JSON que o teste Rust lê e
// passa pelo `engenheiro::classificar`. O classificador mora em Rust e é ele que decide o
// caminho barato — reimplementá-lo aqui em JS só para o teste criaria uma segunda verdade.
const saidaJson = path.join(DIR, "transcricoes.json");
fs.writeFileSync(
  saidaJson,
  JSON.stringify(
    linhas.map((l) => ({
      chave: l.chave,
      sujeira: l.sujeira,
      esperado: l.esperado,
      transcrito: l.transcrito,
      intencao_esperada: l.intencao,
      ms: l.ms,
    })),
    null,
    2,
  ),
  "utf8",
);
console.log(`\nTranscrições em ${saidaJson}`);
console.log("Rode o teste Rust de classificação para fechar o terceiro eixo:");
console.log("  cargo test --lib intencao_sobrevive_a_transcricao -- --nocapture\n");
