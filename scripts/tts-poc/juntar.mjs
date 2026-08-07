#!/usr/bin/env node
// Junta vários WAV num só, para testar fala montada por PEDAÇOS.
//
// A pergunta que este script existe para responder: dá para gravar o nome do piloto
// separado do resto da frase e colar os dois na hora? Se der, a biblioteca deixa de
// ser "uma gravação por nome × por frase" e vira "os nomes + as frases", que é a
// diferença entre milhares de arquivos e dezenas.
//
// Dois problemas atrapalham a colagem ingênua, e os dois são tratados aqui:
//
// 1. **Silêncio de borda.** Todo TTS entrega a fala embrulhada em silêncio — meio
//    segundo na frente, outro tanto no fim. Concatenar cru soma as duas bordas e abre
//    um buraco de ~1 s no meio da frase. Por isso o corte por limiar de energia.
// 2. **Emenda audível.** Cortar rente ao zero cria um degrau na forma de onda, que o
//    ouvido escuta como clique. Por isso a rampa curta de entrada e saída em cada peça.
//
// O que este script NÃO conserta é prosódia: o pedaço gravado sozinho tem a entoação
// de quem começa e termina uma frase. Isso não é defeito de colagem, é do material —
// e é exatamente o que a audição precisa julgar.
//
// Uso:
//   node scripts/tts-poc/juntar.mjs nome.wav resto.wav --saida junto.wav
//   node scripts/tts-poc/juntar.mjs a.wav b.wav --pausa 120 --sem-radio

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import {
  aparar,
  aplicarRadio,
  buracoInterno,
  escreverWav,
  lerWav,
  pico,
  rms,
} from "./filtro-radio.mjs";

function lerArgumentos(argv) {
  const o = { arquivos: [], saida: "", pausaMs: 90, radio: true, aparar: true };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    const proximo = () => argv[(i += 1)];
    if (a === "--saida") o.saida = proximo();
    else if (a === "--pausa") o.pausaMs = Number(proximo());
    else if (a === "--sem-radio") o.radio = false;
    else if (a === "--sem-aparar") o.aparar = false;
    else o.arquivos.push(a);
  }
  return o;
}

const opcoes = lerArgumentos(process.argv);
if (opcoes.arquivos.length < 2) {
  console.error("Passe ao menos dois arquivos .wav.");
  process.exit(1);
}

const pecas = opcoes.arquivos.map((caminho) => {
  const { amostras, taxa } = lerWav(caminho);
  const util = opcoes.aparar ? aparar(amostras, taxa) : amostras;
  const vao = buracoInterno(util, taxa);
  return {
    caminho,
    taxa,
    original: amostras,
    util,
    buraco: vao ? `  ATENÇÃO: ${vao.toFixed(2)}s de silêncio NO MEIO da peça` : "",
  };
});

const taxa = pecas[0].taxa;
const divergente = pecas.find((p) => p.taxa !== taxa);
if (divergente) {
  console.error(`Taxas diferentes: ${taxa} Hz e ${divergente.taxa} Hz em ${divergente.caminho}.`);
  process.exit(1);
}

const pausa = Math.round((opcoes.pausaMs / 1000) * taxa);
const total =
  pecas.reduce((s, p) => s + p.util.length, 0) + pausa * (pecas.length - 1);
const junto = new Float32Array(total);
let cursor = 0;
for (const [i, p] of pecas.entries()) {
  junto.set(p.util, cursor);
  cursor += p.util.length + (i < pecas.length - 1 ? pausa : 0);
}

const saida =
  opcoes.saida ||
  path.join(path.dirname(pecas[0].caminho), `${path.basename(pecas[0].caminho, ".wav")}-junto.wav`);
fs.mkdirSync(path.dirname(saida), { recursive: true });
const final = opcoes.radio ? aplicarRadio(junto, taxa) : junto;
escreverWav(saida, final, taxa);

const seg = (n) => (n / taxa).toFixed(2);
for (const p of pecas) {
  console.log(
    `${path.basename(p.caminho).padEnd(60)} ${seg(p.original.length)}s → ${seg(p.util.length)}s ` +
      `(cortou ${seg(p.original.length - p.util.length)}s de silêncio)${p.buraco}`,
  );
}
console.log(
  `\npausa ${opcoes.pausaMs} ms · total ${seg(junto.length)}s · ` +
    `rms ${rms(final).toFixed(3)} · pico ${pico(final).toFixed(2)} · ` +
    `${opcoes.radio ? "com rádio" : "sem rádio"}`,
);
console.log(saida);
