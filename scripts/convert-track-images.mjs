// Converte os mapas de pista PNG -> WebP (público, embarca no app).
// WebP q82 preserva alfa; qualidade visualmente idêntica, ~10-20x menor.
// Uso:
//   node scripts/convert-track-images.mjs --dry   (só relatório, não escreve)
//   node scripts/convert-track-images.mjs         (converte e apaga o .png)
//   node scripts/convert-track-images.mjs --keep  (converte mas mantém o .png)
import fs from 'fs';
import path from 'path';
import sharp from 'sharp';

const DIR = 'public/utilities/tracks';
const QUALITY = 82;
const dry = process.argv.includes('--dry');
const keep = process.argv.includes('--keep');

function walk(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (path.extname(e.name).toLowerCase() === '.png') out.push(p);
  }
  return out;
}

const files = walk(DIR);
if (!files.length) { console.log('Nenhum PNG em', DIR); process.exit(0); }

const kb = b => (b / 1024).toFixed(0);
let before = 0, after = 0, n = 0;
const rows = [];

for (const src of files) {
  const dst = src.replace(/\.png$/i, '.webp');
  const oldSize = fs.statSync(src).size;
  before += oldSize;
  if (dry) {
    rows.push([path.basename(src), kb(oldSize), '(dry)', '']);
    continue;
  }
  const buf = await sharp(src).webp({ quality: QUALITY, effort: 6 }).toBuffer();
  fs.writeFileSync(dst, buf);
  const newSize = buf.length;
  after += newSize;
  n++;
  if (!keep) fs.unlinkSync(src);
  const pct = ((1 - newSize / oldSize) * 100).toFixed(0);
  rows.push([path.basename(dst), kb(oldSize), kb(newSize), `-${pct}%`]);
}

console.log(`\n${dry ? 'DRY RUN — ' : ''}Pasta: ${DIR}  |  Qualidade WebP: ${QUALITY}\n`);
console.log('arquivo'.padEnd(46), 'antes(KB)'.padStart(10), 'depois(KB)'.padStart(11), 'ganho'.padStart(7));
console.log('-'.repeat(78));
for (const [name, o, nw, pct] of rows) {
  console.log(name.slice(0, 45).padEnd(46), o.padStart(10), String(nw).padStart(11), pct.padStart(7));
}
console.log('-'.repeat(78));
if (dry) {
  console.log(`${files.length} PNG somando ${(before / 1048576).toFixed(1)} MB (rode sem --dry pra converter)`);
} else {
  console.log(`Convertidos: ${n}  |  ${(before / 1048576).toFixed(1)} MB -> ${(after / 1048576).toFixed(1)} MB  |  economia: ${((1 - after / before) * 100).toFixed(1)}% (${((before - after) / 1048576).toFixed(1)} MB)`);
  if (!keep) console.log('PNGs originais removidos (recuperáveis via git).');
}
