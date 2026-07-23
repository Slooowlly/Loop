// Converte logos PNG -> WebP com REDE DE SEGURANÇA por imagem.
// Para cada logo: codifica q90, mede SSIM contra o original (luma compositada
// sobre cinza p/ respeitar o alfa). Se SSIM >= LIMIAR -> usa q90 (leve);
// senão -> cai pra WebP lossless (pixel-idêntico). Assim nunca degrada de verdade.
//
// Uso:
//   node scripts/convert-logos.mjs           (DRY: só relatório + distribuição)
//   node scripts/convert-logos.mjs --write    (grava .webp ao lado; mantém .png)
//   [--th 0.985]  limiar SSIM  (padrão 0.985)
import sharp from 'sharp';
import fs from 'fs';
import path from 'path';

const DEFAULT_DIRS = [
  'src/assets/utilities/source-images/TimesNormalized',
  'public/utilities/categorias',
  'src/assets/utilities/source-images/Categorias',
];
const write = process.argv.includes('--write');
const thArg = process.argv.indexOf('--th');
const TH = thArg >= 0 ? parseFloat(process.argv[thArg + 1]) : 0.985;
// roots vindos da linha de comando (arquivos .png ou pastas); senão usa o padrão
const cliRoots = process.argv.slice(2).filter((a) => !a.startsWith('--') && a !== String(TH));
const DIRS = cliRoots.length ? cliRoots : DEFAULT_DIRS;

function walk(d, o = []) {
  if (!fs.existsSync(d)) return o;
  const st = fs.statSync(d);
  if (st.isFile()) { if (/\.png$/i.test(d)) o.push(d); return o; }
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name);
    if (e.isDirectory()) walk(p, o);
    else if (/\.png$/i.test(e.name)) o.push(p);
  }
  return o;
}

// luma compositada sobre cinza (128), respeitando alfa
function toLuma(data, w, h) {
  const n = w * h;
  const out = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    const r = data[i * 4], g = data[i * 4 + 1], b = data[i * 4 + 2], a = data[i * 4 + 3] / 255;
    const R = r * a + 128 * (1 - a), G = g * a + 128 * (1 - a), B = b * a + 128 * (1 - a);
    out[i] = 0.299 * R + 0.587 * G + 0.114 * B;
  }
  return out;
}

// SSIM médio em janelas 8x8 (média), canal luma
function ssim(x, y, w, h) {
  const C1 = (0.01 * 255) ** 2, C2 = (0.03 * 255) ** 2, W = 8;
  let acc = 0, cnt = 0;
  for (let by = 0; by + W <= h; by += W) {
    for (let bx = 0; bx + W <= w; bx += W) {
      let mx = 0, my = 0;
      for (let j = 0; j < W; j++) for (let i = 0; i < W; i++) {
        const idx = (by + j) * w + (bx + i); mx += x[idx]; my += y[idx];
      }
      const N = W * W; mx /= N; my /= N;
      let vx = 0, vy = 0, cxy = 0;
      for (let j = 0; j < W; j++) for (let i = 0; i < W; i++) {
        const idx = (by + j) * w + (bx + i);
        const dx = x[idx] - mx, dy = y[idx] - my;
        vx += dx * dx; vy += dy * dy; cxy += dx * dy;
      }
      vx /= N - 1; vy /= N - 1; cxy /= N - 1;
      const s = ((2 * mx * my + C1) * (2 * cxy + C2)) / ((mx * mx + my * my + C1) * (vx + vy + C2));
      acc += s; cnt++;
    }
  }
  return cnt ? acc / cnt : 1;
}

const all = DIRS.flatMap((d) => walk(d));
if (!all.length) { console.log('Nenhum PNG encontrado nas pastas de logo.'); process.exit(0); }

const kb = b => (b / 1024).toFixed(0);
let totOrig = 0, totNew = 0, nQ90 = 0, nLossless = 0;
const fallbacks = [];
const buckets = { '>=0.999': 0, '0.995-0.999': 0, '0.985-0.995': 0, '<0.985': 0 };

for (const src of all) {
  const orig = fs.statSync(src).size;
  totOrig += orig;
  const { data: oData, info } = await sharp(src).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  const { width: w, height: h } = info;
  const q90 = await sharp(src).webp({ quality: 90, effort: 6 }).toBuffer();
  const { data: cData } = await sharp(q90).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  const s = ssim(toLuma(oData, w, h), toLuma(cData, w, h), w, h);

  if (s >= 0.999) buckets['>=0.999']++;
  else if (s >= 0.995) buckets['0.995-0.999']++;
  else if (s >= 0.985) buckets['0.985-0.995']++;
  else buckets['<0.985']++;

  let chosen, buf, mode;
  if (s >= TH) { chosen = q90; mode = 'q90'; nQ90++; }
  else {
    buf = await sharp(src).webp({ lossless: true, effort: 6 }).toBuffer();
    chosen = buf; mode = 'LOSSLESS'; nLossless++;
    fallbacks.push([src, s.toFixed(4)]);
  }
  totNew += chosen.length;
  if (write) fs.writeFileSync(src.replace(/\.png$/i, '.webp'), chosen);
}

console.log(`\n${write ? '' : 'DRY — '}Logos: ${all.length} arquivos  |  limiar SSIM p/ q90: ${TH}\n`);
console.log('Distribuição de qualidade (SSIM do q90 vs original):');
for (const [k, v] of Object.entries(buckets)) console.log(`  ${k.padEnd(14)} ${v}`);
console.log(`\nEscolha: q90=${nQ90}  |  lossless(rede acionada)=${nLossless}`);
if (fallbacks.length) {
  console.log('  Caíram pra lossless:');
  fallbacks.forEach(([f, s]) => console.log(`    SSIM ${s}  ${f}`));
}
console.log(`\nTamanho: ${(totOrig / 1048576).toFixed(1)} MB -> ${(totNew / 1048576).toFixed(1)} MB  (-${((1 - totNew / totOrig) * 100).toFixed(1)}%, economia ${((totOrig - totNew) / 1048576).toFixed(1)} MB)`);
console.log(write ? '\n.webp gravados (PNGs mantidos — remoção é passo separado).' : '\nRode com --write pra gravar.');
