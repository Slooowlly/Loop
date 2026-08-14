#!/usr/bin/env node
// Converte os acervos de voz (engenheiro e spotter) de WAV para Opus.
//
// ## Por que o repo carrega Opus e não WAV
//
// ESTA é a única prosa do repositório que carrega os números do acervo, e ela traz a data da
// medição junto. Contagem copiada para outro comentário envelhece sozinha, e já envelheceu:
// quatro arquivos chegaram a declarar quatro totais diferentes para o mesmo acervo. O total
// autoritativo é derivado, e sai em `docs/engenheiro-catalogo.md`.
//
// Medido em 12/08/2026: 3.943 arquivos. Em PCM 24 kHz mono isso dá 334 MB — num repositório
// de 214 MB, e crescendo a cada fala nova. WAV não delta-comprime: cada regravação de uma
// peça soma o arquivo inteiro ao histórico, para sempre. Opus a 32 kbps entrega o mesmo
// áudio em ~28 MB.
//
// Os WAV continuam sendo os MASTERS e continuam no disco — o `.gitignore` os mantém fora do
// git, não fora do projeto. Quem gera peça nova (`engenheiro-pack.mjs`, `spotter-pack.mjs`)
// escreve WAV como sempre escreveu; este script é o passo que vem depois.
//
// ## Por que 32 kbps não é pouco
//
// O acervo já sai do gerador com a cadeia de rádio gravada dentro: banda estreita, compressão
// pesada, distorção. O material que o Opus tem para descartar já foi descartado antes dele.
// 32 kbps mono é folgado para fala em 24 kHz — o gargalo de qualidade percebida está na cadeia
// de rádio, não no codec.
//
// ## Por que a junta não estala
//
// Opus tem atraso de encoder, e concatenar arquivos codificados em separado costuma render
// clique ou micro-silêncio na emenda. Aqui não: o app decodifica cada peça em `AudioBuffer`
// (`decodeAudioData`) e toca cada uma no seu próprio `BufferSource`, com pausa DELIBERADA de
// 160 ms entre frases (`PAUSA_ENTRE_FRASES_MS` em `src/lib/engenheiroVoz.js`). Nada é emendado
// sample a sample em tempo de execução. As peças que precisam ser contínuas — o tempo de volta
// — já saem FUNDIDAS do gerador, como um arquivo só.
//
// Uso:
//   node scripts/audio-para-opus.mjs              # converte o que falta e o que ficou velho
//   node scripts/audio-para-opus.mjs --forcar     # reconverte tudo

import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import ffmpeg from "ffmpeg-static";

export const ACERVOS = ["src/assets/engenheiro", "src/assets/spotter"];

// Quantas conversões ao mesmo tempo. O ffmpeg em fala curta é dominado por partida de
// processo, não por CPU — subir muito além disso troca ganho por contenção de disco.
const PARALELO = 8;

const BITRATE = "32k";

/// O Opus está em dia com o WAV?
///
/// "Existe" não basta. O gerador de peças (`spotter-pack.mjs --refazer`, `engenheiro-pack.mjs`)
/// REESCREVE o `.wav` no lugar, com o mesmo nome. Com o teste de existência sozinho, a peça
/// regravada ficava com o Opus ANTIGO ao lado para sempre: o script dizia "0 a converter", o
/// commit levava o Opus velho, e o app tocava a fala que foi corrigida justamente por estar
/// errada. O sintoma é o pior tipo — nada falha, e o áudio no jogo é o de antes.
///
/// `--forcar` continua existindo para o caso em que o mtime não conta a história (cópia de
/// arquivo, checkout, troca do bitrate).
export function precisaConverter(entrada, saida, forcar = false) {
  if (forcar) return true;
  if (!fs.existsSync(saida)) return true;
  // `>` estrito: mtimes iguais são o caso normal de uma conversão que acabou de rodar.
  return fs.statSync(entrada).mtimeMs > fs.statSync(saida).mtimeMs;
}

/** Os pares WAV→Opus de um acervo que ainda precisam de conversão. */
export function pendentesDoAcervo(acervo, forcar = false) {
  const wavs = fs.readdirSync(acervo).filter((n) => n.endsWith(".wav"));
  const pendentes = wavs
    .map((n) => ({
      entrada: path.join(acervo, n),
      saida: path.join(acervo, n.replace(/\.wav$/, ".opus")),
    }))
    .filter(({ entrada, saida }) => precisaConverter(entrada, saida, forcar));
  return { wavs: wavs.length, pendentes };
}

function converter(entrada, saida) {
  return new Promise((resolve, reject) => {
    const p = spawn(
      ffmpeg,
      [
        "-hide_banner",
        "-loglevel", "error",
        "-y",
        "-i", entrada,
        "-c:a", "libopus",
        "-b:a", BITRATE,
        // `audio`, não `voip`: a cadeia de rádio é parte do timbre pretendido, e o modo de
        // voz trata artefato de rádio como ruído a suprimir.
        "-application", "audio",
        "-ac", "1",
        saida,
      ],
      { stdio: ["ignore", "ignore", "pipe"] },
    );
    let erro = "";
    p.stderr.on("data", (b) => (erro += b));
    p.on("close", (code) =>
      code === 0 ? resolve() : reject(new Error(`${path.basename(entrada)}: ${erro.trim()}`)),
    );
    p.on("error", reject);
  });
}

async function comFila(itens, limite, tarefa) {
  let i = 0;
  let feitos = 0;
  const falhas = [];
  const trabalhadores = Array.from({ length: Math.min(limite, itens.length) }, async () => {
    while (i < itens.length) {
      const meu = itens[i++];
      try {
        await tarefa(meu);
      } catch (e) {
        falhas.push(e.message);
      }
      feitos++;
      if (feitos % 200 === 0 || feitos === itens.length) {
        process.stdout.write(`\r  ${feitos}/${itens.length}`);
      }
    }
  });
  await Promise.all(trabalhadores);
  process.stdout.write("\n");
  return falhas;
}

async function main(forcar) {
  let totalWav = 0;
  let totalOpus = 0;
  const todasFalhas = [];

  for (const acervo of ACERVOS) {
    if (!fs.existsSync(acervo)) {
      console.log(`${acervo}: não existe, pulando`);
      continue;
    }
    const { wavs, pendentes } = pendentesDoAcervo(acervo, forcar);

    console.log(`${acervo}: ${wavs} WAV, ${pendentes.length} a converter`);
    if (pendentes.length) {
      const falhas = await comFila(pendentes, PARALELO, ({ entrada, saida }) =>
        converter(entrada, saida),
      );
      todasFalhas.push(...falhas);
    }

    for (const n of fs.readdirSync(acervo)) {
      const tam = fs.statSync(path.join(acervo, n)).size;
      if (n.endsWith(".wav")) totalWav += tam;
      else if (n.endsWith(".opus")) totalOpus += tam;
    }
  }

  const mb = (b) => (b / 1048576).toFixed(1);
  console.log(`\nWAV  ${mb(totalWav)} MB`);
  console.log(`Opus ${mb(totalOpus)} MB  (${(totalWav / (totalOpus || 1)).toFixed(1)}x menor)`);

  if (todasFalhas.length) {
    console.error(`\n${todasFalhas.length} falha(s):`);
    for (const f of todasFalhas.slice(0, 20)) console.error(`  ${f}`);
    process.exit(1);
  }
}

// Só converte quando chamado como CLI. Importado (pelo teste), o módulo é só as funções —
// sem esta guarda, `import` deste arquivo dispararia a conversão do acervo inteiro.
if (process.argv[1]?.endsWith("audio-para-opus.mjs")) {
  await main(process.argv.includes("--forcar"));
}
