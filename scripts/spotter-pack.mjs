#!/usr/bin/env node
// Gera o PACOTE DE VOZ do spotter: uma fala por chave de evento.
//
// É o primeiro pacote de verdade a sair da POC (ver docs/tts-poc-latencia.md), e
// segue as decisões que ela custou a estabelecer:
//
//   • Cloud TTS com OAuth2 do ADC (chave de API é recusada) e `x-goog-user-project`.
//   • Chirp 3 HD Algenib, 24 kHz — a voz escolhida na audição.
//   • Silêncio das bordas APARADO. Aqui isso não é estética, é latência: meio
//     segundo de silêncio de cabeça é meio segundo de atraso num aviso que só vale
//     enquanto o carro ainda está do lado.
//   • Cadeia de rádio BAKED IN. Estas falas nunca são coladas umas nas outras — cada
//     uma sai sozinha —, então não há a razão que obrigava a filtrar depois de
//     juntar, e filtrar na geração deixa o app com um `<audio>` e nada mais.
//
// O nome do arquivo é a CHAVE do evento (`iracing_sdk::spotter`). Sem carimbo de
// tempo: o app importa por nome, e um pacote que muda de nome a cada geração não
// serviria de pacote.
//
// Grava WAV, que é o MASTER e fica fora do git. O app carrega o Opus — depois de gerar, rode
// `node scripts/audio-para-opus.mjs`. Ver o cabeçalho de `engenheiro-pack.mjs` para o porquê.
//
// Uso:
//   node scripts/spotter-pack.mjs             # só o que falta
//   node scripts/spotter-pack.mjs --refazer   # regera tudo
//   node scripts/spotter-pack.mjs --sem-radio # grava a voz limpa (para comparar)

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { aparar, aplicarRadio, buracoInterno, escreverWav, lerWav, pico } from "./tts-poc/filtro-radio.mjs";
import { FALAS } from "./spotter-falas.mjs";

const ENDPOINT = "https://texttospeech.googleapis.com/v1/text:synthesize";
const IDIOMA = "pt-BR";
const VOZ = "pt-BR-Chirp3-HD-Algenib";
const TAXA = 24000;
const DESTINO = path.join("src", "assets", "spotter");


// Uma fala do spotter é curta. Bem mais longa que isto significa que o modelo
// resolveu interpretar em vez de anunciar, e vale conferir de ouvido.
const DURACAO_SUSPEITA_S = 2.0;

function lerArgumentos(argv) {
  const o = { refazer: false, radio: true, projeto: process.env.GCP_PROJECT || "" };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === "--refazer") o.refazer = true;
    else if (a === "--sem-radio") o.radio = false;
    else if (a === "--projeto") o.projeto = argv[(i += 1)];
  }
  return o;
}

function gcloud(args, erro) {
  try {
    return execFileSync("gcloud", args, { encoding: "utf8", shell: true }).trim();
  } catch {
    console.error(erro);
    process.exit(1);
  }
}

const opcoes = lerArgumentos(process.argv);
const acesso =
  process.env.GCP_TOKEN?.trim() ||
  gcloud(
    ["auth", "application-default", "print-access-token"],
    "Sem credencial. Rode uma vez:\n" +
      "  gcloud auth application-default login\n" +
      "  gcloud services enable texttospeech.googleapis.com",
  );
const projeto =
  opcoes.projeto || gcloud(["config", "get-value", "project"], "Projeto de cota não resolvido. Passe --projeto <id>.");

fs.mkdirSync(DESTINO, { recursive: true });

/// Quantas vezes insistir num pedido recusado, e quanto esperar entre as tentativas.
///
/// A moderação de conteúdo do Google recusa frase inofensiva de forma NÃO determinística:
/// medido na POC, `Livre.` apanhou em 5 de 9 tentativas com o texto idêntico. Como é
/// aleatório, insistir com a mesma frase funciona — não há o que reescrever, e reescrever
/// seria deixar o moderador decidir a redação do produto.
///
/// A espera cresce porque o outro motivo de recusa é cota (429), e aí insistir de imediato
/// só gasta tentativa. Cinco tentativas com 0,5 s, 1 s, 2 s, 4 s cobrem os dois casos e
/// somam menos de 8 s no pior caso de um arquivo.
const TENTATIVAS = 5;
const ESPERA_BASE_MS = 500;

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));

async function sintetizar(texto) {
  let ultima;
  for (let tentativa = 1; tentativa <= TENTATIVAS; tentativa += 1) {
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
    if (resposta.ok) {
      const { audioContent } = await resposta.json();
      return Buffer.from(audioContent, "base64"); // LINEAR16 já vem com cabeçalho RIFF
    }
    const corpo = (await resposta.text()).slice(0, 500);
    ultima = `HTTP ${resposta.status}: ${corpo}`;
    // A regra é INSISTIR POR PADRÃO, e desistir só nos três status em que insistir é
    // provadamente inútil: 401 (a API recusa chave de API — precisa de OAuth2), 403
    // (falta o `x-goog-user-project`) e 404 (endpoint ou voz que não existe). Todos os
    // três estão medidos em `docs/tts-poc-latencia.md` e nenhum melhora com repetição.
    //
    // O importante é o que NÃO está nessa lista: a recusa da moderação. O doc registra a
    // mensagem ("sensitive words that violate the Prohibited Use policy") e não o status,
    // e foi medida no Gemini TTS enquanto isto aqui fala com a Cloud TTS. Como o status é
    // desconhecido, listar quem se repete seria apostar — e uma aposta errada aqui derruba
    // exatamente o caso para o qual esta função existe.
    if (resposta.status === 401 || resposta.status === 403 || resposta.status === 404) {
      throw new Error(ultima);
    }
    if (tentativa < TENTATIVAS) {
      const moderacao = /sensitive words|Prohibited Use/i.test(corpo);
      process.stdout.write(
        `   ↻  ${moderacao ? "moderação recusou" : `recusa ${resposta.status}`}` +
          ` na tentativa ${tentativa}, insistindo com o MESMO texto\n`,
      );
      await dormir(ESPERA_BASE_MS * 2 ** (tentativa - 1));
    }
  }
  throw new Error(`${TENTATIVAS} tentativas recusadas. Última: ${ultima}`);
}

const avisos = [];
const falhas = [];
let gerados = 0;

for (const [chave, texto] of Object.entries(FALAS)) {
  const destino = path.join(DESTINO, `${chave}.wav`);
  if (!opcoes.refazer && fs.existsSync(destino)) {
    console.log(`   ·  ${chave.padEnd(14)} já existe`);
    continue;
  }

  const t0 = performance.now();
  // Uma fala que não sai não pode derrubar as outras. Antes, a exceção subia e a rodada
  // morria no meio — com o agravante de que as JÁ GERADAS ficavam no disco, então a
  // execução seguinte as pulava pelo `já existe` e o buraco virava invisível. O resumo do
  // fim é que fecha a conta.
  let bruto;
  try {
    bruto = await sintetizar(texto);
  } catch (e) {
    falhas.push({ chave, texto, motivo: String(e.message || e).split("\n")[0] });
    console.log(`   ✘  ${chave.padEnd(14)} NÃO SAIU — ${String(e.message || e).slice(0, 120)}`);
    continue;
  }
  const ms = Math.round(performance.now() - t0);

  // Grava, lê de volta pelo mesmo leitor que o resto da POC usa e processa.
  const temporario = path.join(DESTINO, `.${chave}.cru.wav`);
  fs.writeFileSync(temporario, bruto);
  const { amostras, taxa } = lerWav(temporario);
  fs.unlinkSync(temporario);

  const antes = amostras.length / taxa;
  const cortado = aparar(amostras, taxa);
  const final = opcoes.radio ? aplicarRadio(cortado, taxa) : cortado;
  escreverWav(destino, final, taxa);

  const depois = final.length / taxa;
  const buraco = buracoInterno(cortado, taxa);
  if (buraco > 0) {
    avisos.push(`${chave}: ${buraco.toFixed(2)}s de silêncio DENTRO da fala — o modelo partiu em dois fôlegos`);
  }
  if (depois > DURACAO_SUSPEITA_S) {
    avisos.push(`${chave}: ${depois.toFixed(2)}s é longo demais para um aviso de spotter`);
  }

  gerados += 1;
  console.log(
    `  ${String(gerados).padStart(2)}  ${chave.padEnd(14)} ${String(ms).padStart(5)} ms  ` +
      `${antes.toFixed(2)}s → ${depois.toFixed(2)}s  pico ${pico(final).toFixed(2)}  ${destino}`,
  );
}

const bytes = Object.keys(FALAS)
  .map((c) => path.join(DESTINO, `${c}.wav`))
  .filter((p) => fs.existsSync(p))
  .reduce((soma, p) => soma + fs.statSync(p).size, 0);
console.log(`\n${Object.keys(FALAS).length} falas, ${(bytes / 1024).toFixed(0)} KB no total.`);

for (const aviso of avisos) console.warn(`  ⚠  ${aviso}`);

// O que não saiu, no fim e em voz alta. Um pacote incompleto não avisa sozinho: o detector
// emite a chave, a camada de voz não acha o arquivo, e o sintoma na pista é silêncio — que
// é indistinguível de "não havia o que falar". Saída diferente de zero para que o release
// não empacote um pacote com buraco achando que deu certo.
if (falhas.length) {
  console.error(`\n  ${falhas.length} fala(s) NÃO gerada(s):`);
  for (const f of falhas) console.error(`    ✘  ${f.chave.padEnd(14)} "${f.texto}"  —  ${f.motivo}`);
  console.error(
    `\n  Rode de novo: as que saíram são puladas pelo "já existe" e só estas serão tentadas.\n` +
      `  A moderação é aleatória, então repetir com o MESMO texto costuma resolver.`,
  );
  process.exitCode = 1;
}
