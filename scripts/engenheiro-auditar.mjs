#!/usr/bin/env node
// Audita o pacote do engenheiro JÁ GRAVADO, procurando as falas que precisam ser regeradas.
//
// O gerador avisa sobre defeito enquanto grava, mas o aviso rola na tela junto com 480 linhas
// e some. Este script lê o que está no disco — que é o que o jogador vai ouvir — e devolve só
// o que está torto, ordenado pela gravidade.
//
// Procura três coisas:
//
// 1. **Fôlego partido.** O modelo dividiu uma frase de uma oração em dois, e sobrou silêncio
//    no meio. É defeito de material: nenhuma edição conserta, só regerar. Como a Chirp 3 não é
//    determinística, a segunda tentativa tem boa chance de sair inteira.
// 2. **Duração fora do esperado.** Muito longa costuma ser o modelo interpretando em vez de
//    ler; muito curta costuma ser fala cortada.
// 3. **Pico perto do teto.** O limitador da cadeia de rádio mira 0,97; bem acima disso
//    sugere que alguma coisa passou por ele.
//
// A coluna que decide é a de ORAÇÕES: uma pausa entre duas frases é prosódia correta, e só
// vira defeito quando a frase tem uma oração só. O gerador não faz essa distinção e por isso
// alarma demais.
//
// Uso:
//   node scripts/engenheiro-auditar.mjs
//   node scripts/engenheiro-auditar.mjs --tudo   # lista todas, não só as suspeitas

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { buracoInterno, lerWav, pico } from "./tts-poc/filtro-radio.mjs";

const CATALOGO = path.join("docs", "engenheiro-catalogo.json");
const DIR = path.join("src", "assets", "engenheiro");

/// Silêncio interno a partir do qual vale reclamar numa frase de UMA oração.
const BURACO_MIN_S = 0.15;
const DURACAO_MAX_S = 4.5;
const DURACAO_MIN_S = 0.5;
const PICO_MAX = 0.99;

const tudo = process.argv.includes("--tudo");

if (!fs.existsSync(CATALOGO)) {
  console.error(`Sem ${CATALOGO}. Rode: cargo test --lib despeja_catalogo_para_revisao`);
  process.exit(1);
}
const catalogo = JSON.parse(fs.readFileSync(CATALOGO, "utf8"));

/// Quantas orações a frase tem. Uma pausa DENTRO de uma oração é o modelo respirando errado;
/// entre duas orações é pontuação sendo respeitada.
function oracoes(texto) {
  return (texto.match(/[.!?]/g) || []).length;
}

const linhas = [];
let ausentes = 0;
for (const { chave, texto } of catalogo) {
  // O WAV, não o Opus. Este auditor mede FORMA DE ONDA — pico e silêncio interno —, e isso
  // exige as amostras PCM dos masters. Auditar o Opus daria a medida do codec somada à da
  // tomada, que é o oposto do que a ferramenta existe para ver. Quem não tem os masters no
  // disco (eles ficam fora do git, ver `audio-para-opus.mjs`) não roda este script.
  const arquivo = path.join(DIR, `${chave}.wav`);
  if (!fs.existsSync(arquivo)) {
    ausentes += 1;
    linhas.push({ chave, texto, ausente: true, gravidade: 3 });
    continue;
  }
  const { amostras, taxa } = lerWav(arquivo);
  const dur = amostras.length / taxa;
  const buraco = buracoInterno(amostras, taxa);
  const p = pico(amostras);
  const n = oracoes(texto);

  const problemas = [];
  let gravidade = 0;
  if (buraco >= BURACO_MIN_S && n <= 1) {
    problemas.push(`fôlego partido: ${buraco.toFixed(2)}s de silêncio numa frase de uma oração`);
    gravidade = Math.max(gravidade, 2);
  }
  if (dur > DURACAO_MAX_S) {
    problemas.push(`${dur.toFixed(2)}s — longa demais`);
    gravidade = Math.max(gravidade, 1);
  }
  if (dur < DURACAO_MIN_S) {
    problemas.push(`${dur.toFixed(2)}s — curta demais, pode estar cortada`);
    gravidade = Math.max(gravidade, 2);
  }
  if (p > PICO_MAX) {
    problemas.push(`pico ${p.toFixed(3)} — acima do teto do limitador`);
    gravidade = Math.max(gravidade, 1);
  }
  linhas.push({ chave, texto, dur, buraco, pico: p, oracoes: n, problemas, gravidade });
}

const suspeitas = linhas.filter((l) => l.gravidade > 0);
const mostrar = tudo ? linhas : suspeitas;
mostrar.sort((a, b) => b.gravidade - a.gravidade || (b.buraco ?? 0) - (a.buraco ?? 0));

console.log(`\n${catalogo.length} falas no catálogo · ${suspeitas.length} suspeita(s)\n`);
for (const l of mostrar) {
  if (l.ausente) {
    console.log(`  ✘  ${l.chave.padEnd(26)} AUSENTE DO DISCO — "${l.texto}"`);
    continue;
  }
  const marca = l.gravidade >= 2 ? "✘" : l.gravidade === 1 ? "⚠" : "·";
  console.log(
    `  ${marca}  ${l.chave.padEnd(26)} ${l.dur.toFixed(2)}s  ` +
      `buraco ${l.buraco.toFixed(2)}s  ${l.oracoes} oração(ões)`,
  );
  console.log(`     "${l.texto}"`);
  for (const p of l.problemas) console.log(`     → ${p}`);
}

const regerar = suspeitas.filter((l) => l.gravidade >= 2).map((l) => l.chave);
if (regerar.length) {
  console.log(`\n  ${regerar.length} para regerar. A Chirp 3 não é determinística, então a`);
  console.log("  segunda tentativa com o MESMO texto costuma sair inteira:\n");
  console.log(`    node scripts/engenheiro-pack.mjs --refazer --so ${regerar[0]}`);
  console.log("\n  Ou apague as suspeitas e rode o gerador sem filtro:\n");
  console.log(
    `    node -e "['${regerar.join("','")}'].forEach(c=>['wav','opus'].forEach(e=>require('fs').rmSync(\`src/assets/engenheiro/\${c}.\${e}\`,{force:true})))"`,
  );
  console.log("    node scripts/engenheiro-pack.mjs");
  console.log("    node scripts/audio-para-opus.mjs   # o app carrega o Opus, não o WAV");
}
if (ausentes) console.error(`\n  ${ausentes} fala(s) do catálogo não estão no disco.`);
