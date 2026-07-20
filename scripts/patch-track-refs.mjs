// Troca referências .png -> .webp APENAS para imagens de pista.
// Duas regras, ambas escopadas em "tracks" (nunca toca em /utilities/categorias/):
//   1) caminhos ".../utilities/tracks/<nome>.png"
//   2) entradas de tabela  file: "<nome>.png"   (só pistas usam essa chave)
// Uso: node scripts/patch-track-refs.mjs        (dry, só mostra)
//      node scripts/patch-track-refs.mjs --write (aplica)
import fs from 'fs';

const write = process.argv.includes('--write');
const FILES = [
  'src/utils/trackImages.js',
  'src/components/layout/Header.jsx',
  'src/pages/tabs/CalendarTab.jsx',
  'src/pages/tabs/CalendarTab.test.jsx',
  'src/components/layout/Header.test.jsx',
];

const R1 = /(\/utilities\/tracks\/[^\n"'`]*?)\.png/g;   // caminhos de pista
const R2 = /(file:\s*"[^"\n]*?)\.png"/g;                 // entradas file: "x.png"

let grandTotal = 0;
for (const f of FILES) {
  let src;
  try { src = fs.readFileSync(f, 'utf8'); } catch { console.log(`(pulado, não existe) ${f}`); continue; }
  const lines = src.split('\n');
  let count = 0;
  const out = lines.map((line, i) => {
    let nl = line.replace(R1, '$1.webp').replace(R2, '$1.webp"');
    if (nl !== line) {
      count++;
      console.log(`  ${f}:${i + 1}`);
      console.log(`    - ${line.trim()}`);
      console.log(`    + ${nl.trim()}`);
    }
    return nl;
  });
  if (count && write) fs.writeFileSync(f, out.join('\n'));
  console.log(`${count ? '✓' : '·'} ${f} — ${count} linha(s)${count && !write ? ' (dry)' : ''}\n`);
  grandTotal += count;
}
console.log(`Total: ${grandTotal} linha(s)${write ? ' ESCRITAS' : ' (dry — rode com --write pra aplicar)'}`);
