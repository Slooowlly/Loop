#!/usr/bin/env node
// AUDIÇÃO: as falas da classificação, montadas com as peças de produção.
//
// Não gera nada — cola o que já está no acervo com as pausas que o rádio vai usar. As três
// perguntas que ela responde:
//
//   1. **A despedida soa como alguém querendo alguma coisa?** É a única fala do acervo inteiro
//      sem dado nenhum, e é o pedido que originou a família. Se ela soar como locução, falhou.
//   2. **A emenda entre o reconhecimento e a conta.** "Perdeu essa." + "Ainda dá pra mais três."
//      são duas frases inteiras, e a junção é de ponto final (220 ms). Curta demais atropela;
//      longa demais vira duas falas em vez de uma.
//   3. **A que não consola.** "Essa foi embora." + "Era essa. Volta pro box." tem que soar
//      firme sem soar cruel — é a fala que mais depende do tom e menos do texto.
//
// Uso: node scripts/tts-poc/audicao-classificacao.mjs

import fs from "node:fs";
import path from "node:path";

import { pausasDoRadio } from "../../src/lib/pausasDoRadio.js";

const DESTINO = path.join("docs", "tts-poc", "audicao-classificacao");
const ORIGEM = path.join("src", "assets", "engenheiro");

function lerWav(arquivo) {
  const b = fs.readFileSync(arquivo);
  let pos = 12;
  let taxa = 0;
  let dados = null;
  while (pos + 8 <= b.length) {
    const id = b.toString("ascii", pos, pos + 4);
    const n = b.readUInt32LE(pos + 4);
    if (id === "fmt ") taxa = b.readUInt32LE(pos + 12);
    else if (id === "data") dados = b.subarray(pos + 8, pos + 8 + n);
    pos += 8 + n + (n % 2);
  }
  return { taxa, dados };
}

function escreverWav(arquivo, taxa, partes) {
  const dados = Buffer.concat(partes);
  const b = Buffer.alloc(44 + dados.length);
  b.write("RIFF", 0);
  b.writeUInt32LE(36 + dados.length, 4);
  b.write("WAVE", 8);
  b.write("fmt ", 12);
  b.writeUInt32LE(16, 16);
  b.writeUInt16LE(1, 20);
  b.writeUInt16LE(1, 22);
  b.writeUInt32LE(taxa, 24);
  b.writeUInt32LE(taxa * 2, 28);
  b.writeUInt16LE(2, 32);
  b.writeUInt16LE(16, 34);
  b.write("data", 36);
  b.writeUInt32LE(dados.length, 40);
  dados.copy(b, 44);
  fs.writeFileSync(arquivo, b);
}

const MONTAGENS = [
  { nome: "despedida", chaves: ["cl_despedida_0"], nota: "a que só quer alguma coisa" },
  { nome: "despedida-equipe", chaves: ["cl_despedida_1"], nota: "a que invoca a equipe" },
  { nome: "despedida-curta", chaves: ["cl_despedida_2"], nota: "a mais seca — cabe sempre" },
  {
    nome: "volta-perdida-com-tres",
    chaves: ["cl_perdeu_0", "cl_restam_3"],
    nota: "reconhece e conta o que sobra",
  },
  {
    nome: "volta-perdida-sem-tempo",
    chaves: ["cl_perdeu_1", "cl_acabou_0"],
    nota: "a que NÃO consola",
  },
];

fs.mkdirSync(DESTINO, { recursive: true });
for (const { nome, chaves, nota } of MONTAGENS) {
  const pausas = pausasDoRadio(chaves);
  const partes = [];
  let taxa = 0;
  chaves.forEach((c, i) => {
    const w = lerWav(path.join(ORIGEM, `${c}.wav`));
    taxa = w.taxa;
    partes.push(w.dados);
    if (i < chaves.length - 1) {
      partes.push(Buffer.alloc(Math.round(taxa * 2 * (pausas[i] / 1000))));
    }
  });
  escreverWav(path.join(DESTINO, `${nome}.wav`), taxa, partes);
  const dur = partes.reduce((a, p) => a + p.length, 0) / (taxa * 2);
  console.log(
    `  ${nome.padEnd(26)} ${dur.toFixed(2)}s  ${chaves.length} peça(s)` +
      (pausas.length ? `  pausas ${JSON.stringify(pausas)}` : "") +
      `  — ${nota}`,
  );
}
console.log(`\n→ ${DESTINO}`);
