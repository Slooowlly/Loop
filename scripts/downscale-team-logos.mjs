// Reduz as 102 artes de equipe para o tamanho em que elas de fato aparecem.
//
// As artes estavam em 768x512. O maior slot que o TeamLogoMark desenha é o
// `hero`, 168x112 CSS px — ou seja, a arte vinha com ~21x mais pixels do que a
// tela usa no caso pequeno (36x24) e ~4.6x no maior. Isso não é um detalhe de
// peso de arquivo: o navegador DECODIFICA a imagem no tamanho de origem e, pior,
// aplica o `drop-shadow` do halo nessa mesma resolução.
//
// Medido em Chromium, 14 logos desenhados a 36x24:
//
//                        768x512   384x256
//   1a vez (decode+halo)   168ms      40ms
//   em cache (com halo)     58ms       9ms
//   em cache (sem halo)     15ms       8ms
//
// Os 58ms em cache eram o custo por REPINTURA — pagos toda vez que a aba do
// dossiê troca, para sempre. Com a arte menor o halo deixa de custar (9 vs 8).
// Para efeito de comparação, o React resolve a troca de aba em 5-15ms: eram as
// imagens, e não o código, que dominavam a conta.
//
// 384x256 é o alvo porque cobre o `hero` (168x112) até DPR 2 sem perder nitidez.
// Descer mais só ajudaria os slots pequenos, e o ganho já estaria no ruído.
//
// Uso:
//   node scripts/downscale-team-logos.mjs            (DRY: só o relatório)
//   node scripts/downscale-team-logos.mjs --write     (regrava as artes no lugar)
import sharp from "sharp";
import fs from "fs";
import path from "path";

const DIR = "src/assets/utilities/source-images/TimesNormalized";
const LARGURA = 384;
const ALTURA = 256;
const QUALIDADE = 92;

const write = process.argv.includes("--write");

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (/\.webp$/i.test(entry.name)) out.push(full);
  }
  return out;
}

const arquivos = walk(DIR);
let antes = 0;
let depois = 0;
let convertidos = 0;
let intactos = 0;

for (const arquivo of arquivos) {
  const original = fs.readFileSync(arquivo);
  const meta = await sharp(original).metadata();
  antes += original.length;

  if (meta.width <= LARGURA && meta.height <= ALTURA) {
    depois += original.length;
    intactos += 1;
    continue;
  }

  // `fit: inside` preserva a proporção — nenhuma arte é cortada nem esticada, e
  // as que já fogem do 3:2 continuam fugindo do mesmo jeito.
  const menor = await sharp(original)
    .resize(LARGURA, ALTURA, { fit: "inside", withoutEnlargement: true })
    .webp({ quality: QUALIDADE, alphaQuality: 100 })
    .toBuffer();

  depois += menor.length;
  convertidos += 1;
  if (write) fs.writeFileSync(arquivo, menor);
}

const mb = (bytes) => (bytes / 1048576).toFixed(2) + "MB";
console.log(`${arquivos.length} artes · ${convertidos} reduzidas · ${intactos} já pequenas`);
console.log(`${mb(antes)} -> ${mb(depois)}`);
if (!write) console.log("DRY RUN — rode com --write para gravar.");
