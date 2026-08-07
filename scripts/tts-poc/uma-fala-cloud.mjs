#!/usr/bin/env node
// UMA fala pela Cloud Text-to-Speech — o outro caminho do Google, para comparar.
//
// Diferenças que importam em relação ao Gemini TTS (`uma-fala.mjs`):
//
// - **Autenticação.** A Cloud TTS RECUSA chave de API ("API keys are not supported by
//   this API"); exige OAuth2. Aqui isso vem do Application Default Credentials do
//   gcloud. Também exige um projeto de cota, mandado no header `x-goog-user-project` —
//   sem ele a chamada morre em 403 apontando para o projeto do próprio SDK.
// - **Sem direção por prompt.** Famílias convencionais (Standard/WaveNet/Neural2) não
//   interpretam instrução de atuação; o controle é SSML. Por isso não há `--direcao`.
// - **pt-BR explícito.** O idioma é parâmetro (`languageCode`), não inferido do texto,
//   e cada voz declara o gênero — os dois buracos que nos custaram caro no Gemini.
// - **Determinismo.** É a razão de este arquivo existir: as famílias convencionais
//   não são generativas, então a mesma entrada deve devolver os mesmos bytes.
//   `--repetir N` gera N vezes e compara os hashes, que é como se prova isso.
//
// Uso:
//   node scripts/tts-poc/uma-fala-cloud.mjs --voz pt-BR-Neural2-B --texto "Esquerda..."
//   node scripts/tts-poc/uma-fala-cloud.mjs --voz pt-BR-Chirp3-HD-Algenib --repetir 3
//   node scripts/tts-poc/uma-fala-cloud.mjs --ssml "<speak>Esquerda<break time='300ms'/></speak>"

import { execFileSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { processarArquivo, pico, rms } from "./filtro-radio.mjs";

const ENDPOINT = "https://texttospeech.googleapis.com/v1/text:synthesize";
const IDIOMA = "pt-BR";
const TAXA = 24000; // igual à do Gemini TTS, para a comparação ser justa

function lerArgumentos(argv) {
  const o = {
    voz: "pt-BR-Neural2-B",
    texto: "Daniel, o carro da frente está perdendo tempo no segundo setor. Continue pressionando, estamos nos aproximando.",
    ssml: null,
    repetir: 1,
    projeto: process.env.GCP_PROJECT || "",
    velocidade: 1.0,
    tom: 0,
    // Nome curto no arquivo. Sem isso um pack vira dez arquivos indistinguíveis,
    // separados só por segundos no carimbo de tempo.
    rotulo: "",
    // A cadeia de rádio sai LIGADA. O som do produto é o filtrado — a voz limpa é
    // material intermediário, e ouvir o intermediário para decidir leva a decidir
    // errado. O `.wav` cru continua gravado ao lado, para reprocessar sem regerar.
    radio: true,
  };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    const proximo = () => argv[(i += 1)];
    if (a === "--voz") o.voz = proximo();
    else if (a === "--texto") o.texto = proximo();
    else if (a === "--ssml") o.ssml = proximo();
    else if (a === "--repetir") o.repetir = Number(proximo());
    else if (a === "--projeto") o.projeto = proximo();
    else if (a === "--velocidade") o.velocidade = Number(proximo());
    else if (a === "--tom") o.tom = Number(proximo());
    else if (a === "--rotulo") o.rotulo = proximo();
    else if (a === "--sem-radio") o.radio = false;
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

/** Projeto de cota: sem ele o ADC de usuário cai em 403 no projeto do próprio SDK. */
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

const opcoes = lerArgumentos(process.argv);
const acesso = token();
const projeto = projetoDeCota(opcoes.projeto);
if (!projeto) {
  console.error("Projeto de cota não resolvido. Passe --projeto <id>.");
  process.exit(1);
}

const dir = path.join("docs", "tts-poc", "audicao-cloud");
fs.mkdirSync(dir, { recursive: true });

const hashes = [];
for (let i = 0; i < opcoes.repetir; i += 1) {
  const t0 = performance.now();
  const resposta = await fetch(ENDPOINT, {
    method: "POST",
    headers: {
      authorization: `Bearer ${acesso}`,
      "x-goog-user-project": projeto,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      input: opcoes.ssml ? { ssml: opcoes.ssml } : { text: opcoes.texto },
      voice: { languageCode: IDIOMA, name: opcoes.voz },
      audioConfig: {
        audioEncoding: "LINEAR16",
        sampleRateHertz: TAXA,
        speakingRate: opcoes.velocidade,
        pitch: opcoes.tom,
      },
    }),
  });

  if (!resposta.ok) {
    console.error(`HTTP ${resposta.status}: ${(await resposta.text()).slice(0, 700)}`);
    process.exit(1);
  }

  const { audioContent } = await resposta.json();
  // LINEAR16 já vem com cabeçalho RIFF — diferente do Gemini, que devolve PCM cru.
  const wav = Buffer.from(audioContent, "base64");
  const ms = Math.round(performance.now() - t0);
  const hash = crypto.createHash("sha256").update(wav).digest("hex").slice(0, 16);
  hashes.push(hash);

  const carimbo = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+$/, "");
  const sufixo = opcoes.repetir > 1 ? `-t${i + 1}` : "";
  const rotulo = opcoes.rotulo ? `-${opcoes.rotulo}` : "";
  const arquivo = path.join(dir, `${carimbo}${rotulo}-${opcoes.voz}${sufixo}.wav`);
  fs.writeFileSync(arquivo, wav);

  const corpo = wav.length - 44;
  let destaque = arquivo;
  let marcaRadio = "";
  if (opcoes.radio) {
    const { destino, antes, depois } = processarArquivo(arquivo);
    destaque = destino;
    marcaRadio = ` radio(rms ${rms(antes).toFixed(3)}→${rms(depois).toFixed(3)}, pico ${pico(depois).toFixed(2)})`;
  }
  console.log(
    `${String(i + 1).padStart(2)}/${opcoes.repetir}  ${String(ms).padStart(5)} ms  ` +
      `${(corpo / (TAXA * 2)).toFixed(2)}s  sha ${hash}${marcaRadio}  ${destaque}`,
  );
}

if (opcoes.repetir > 1) {
  const unicos = new Set(hashes);
  console.log(
    unicos.size === 1
      ? `\nDETERMINÍSTICA: ${opcoes.repetir} gerações, bytes idênticos.`
      : `\nNÃO determinística: ${unicos.size} saídas distintas em ${opcoes.repetir} gerações.`,
  );
}
