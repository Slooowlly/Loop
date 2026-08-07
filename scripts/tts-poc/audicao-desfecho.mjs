#!/usr/bin/env node
// AUDIÇÃO: o DESFECHO da quebra do NOSSO carro, em 2ª pessoa.
//
// Nove redações — três severidades × três variações — antes de comprometer as 108 gravações que
// a família inteira custa (12 peças × 3 severidades × 3 variações).
//
// A pergunta NÃO é a colagem: aqui não há colagem nenhuma, é frase inteira numa tomada só, como
// todo o resto de `peca_propria`. A pergunta é a redação. Três coisas para ouvir:
//
//   1. **A pessoa gramatical.** É o defeito que esta família existe para consertar — o rádio
//      dizia "o piloto um da tal equipe abandonou" sobre o próprio jogador. Se a fala nova não
//      soar inequivocamente sobre o SEU carro, ela não resolveu nada.
//   2. **A escada de gravidade.** "Dá pra seguir" (leve), "vamos ter que parar" (grave) e
//      "acabou por hoje" (abandono) precisam soar diferentes o bastante para o piloto saber o
//      que fazer sem pensar. Se a leve e a grave soarem parecidas, a escada está achatada.
//   3. **O "mas" e o "então" sem vírgula.** A regra da casa proíbe pontuação interna (medido:
//      0,35 s de silêncio dentro da tomada). Estas frases têm coordenação sem vírgula, que é
//      justo o caso onde o modelo pode respirar mesmo assim. O relatório mede o buraco.
//
// Uso: node scripts/tts-poc/audicao-desfecho.mjs

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { aparar, aplicarRadio, buracoInterno, escreverWav, lerWav } from "./filtro-radio.mjs";

const ENDPOINT = "https://texttospeech.googleapis.com/v1/text:synthesize";
const VOZ = "pt-BR-Chirp3-HD-Algenib";
const TAXA = 24000;
const DESTINO = path.join("docs", "tts-poc", "audicao-desfecho");
const ORIGEM = path.join("src", "assets", "engenheiro");

// Uma peça por redação, todas sobre o MOTOR — a peça mais comum e a de nome mais curto, que é o
// pior caso para a frase soar truncada. As outras onze só trocam o sintagma.
const CATALOGO = JSON.parse(fs.readFileSync(path.join("docs", "engenheiro-catalogo.json"), "utf8"));
const ALVOS = CATALOGO.filter(({ chave }) => /^meu_(light|heavy|dnf)_engine_\d$/.test(chave));
if (ALVOS.length !== 9) {
  console.error(`Esperava 9 redações de motor no catálogo, achei ${ALVOS.length}.`);
  console.error("  cargo test --lib despeja_catalogo_para_revisao");
  process.exit(1);
}

// A referência: o AVISO da mesma peça, que já está gravado. É contra ele que o desfecho vai ser
// ouvido em corrida — primeiro "estou ouvindo algo estranho no seu motor", depois o desfecho —,
// então as duas falas precisam soar da mesma pessoa e da mesma conversa.
const REFERENCIA = ["meu_engine_0", "meu_poupar"];

function gcloud(args, erro) {
  try {
    return execFileSync("gcloud", args, { encoding: "utf8", shell: true }).trim();
  } catch {
    console.error(erro);
    process.exit(1);
  }
}

const acesso =
  process.env.GCP_TOKEN?.trim() ||
  gcloud(
    ["auth", "application-default", "print-access-token"],
    "Sem credencial. Rode: gcloud auth application-default login",
  );
const projeto = gcloud(["config", "get-value", "project"], "Projeto de cota não resolvido.");

fs.mkdirSync(DESTINO, { recursive: true });

async function sintetizar(texto) {
  const r = await fetch(ENDPOINT, {
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
  if (!r.ok) throw new Error(`HTTP ${r.status}: ${(await r.text()).slice(0, 300)}`);
  return Buffer.from((await r.json()).audioContent, "base64");
}

/** Peça nova: crua, aparada e com a cadeia de rádio — o mesmo caminho do gerador do pacote. */
async function pecaNova(texto) {
  const tmp = path.join(DESTINO, ".tmp.wav");
  fs.writeFileSync(tmp, await sintetizar(texto));
  const { amostras, taxa } = lerWav(tmp);
  fs.unlinkSync(tmp);
  const cortado = aparar(amostras, taxa);
  return { amostras: aplicarRadio(cortado, taxa), taxa, buraco: buracoInterno(cortado, taxa) };
}

const avisos = [];
const SEVERIDADE = { light: "LEVE  ", heavy: "GRAVE ", dnf: "ABANDONO" };

console.log("── as nove redações ──");
for (const { chave, texto } of ALVOS) {
  const p = await pecaNova(texto);
  escreverWav(path.join(DESTINO, `${chave.replace(/^meu_/, "")}.wav`), p.amostras, p.taxa);
  const sev = SEVERIDADE[/^meu_([a-z]+)_/.exec(chave)[1]] ?? "?";
  console.log(
    `  ${sev}  ${(p.amostras.length / p.taxa).toFixed(2)}s  ` +
      (p.buraco > 0.15 ? `⚠ ${p.buraco.toFixed(2)}s de buraco  ` : "") +
      `"${texto}"`,
  );
  if (p.buraco > 0.15) avisos.push(`${chave}: ${p.buraco.toFixed(2)}s de silêncio interno`);
}

console.log("\n── referência do acervo (a mesma voz, já em produção) ──");
for (const chave of REFERENCIA) {
  const origem = path.join(ORIGEM, `${chave}.wav`);
  if (!fs.existsSync(origem)) {
    console.log(`  ${chave.padEnd(18)} ausente no acervo`);
    continue;
  }
  const { amostras, taxa } = lerWav(origem);
  escreverWav(path.join(DESTINO, `ref-${chave}.wav`), amostras, taxa);
  console.log(`  ${chave.padEnd(18)} ${(amostras.length / taxa).toFixed(2)}s`);
}

if (avisos.length) {
  console.log(`\n⚠ ${avisos.length} aviso(s):`);
  for (const a of avisos) console.log(`   ${a}`);
}
console.log(`\n→ ${DESTINO}`);
