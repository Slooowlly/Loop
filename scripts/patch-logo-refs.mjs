// Troca .png -> .webp APENAS para logos (categorias + recortadas, logos de time,
// e as duas fontes em source-images/Categorias). Escopado por pasta:
// NUNCA toca flags (bandeiras/flag-icons), texturas (Fuel/Pneu) ou pistas.
// Uso: node scripts/patch-logo-refs.mjs           (dry)
//      node scripts/patch-logo-refs.mjs --write    (aplica)
import fs from 'fs';

const write = process.argv.includes('--write');

// varre src/ por arquivos .js/.jsx
function walk(d, o = []) {
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = `${d}/${e.name}`;
    if (e.isDirectory()) walk(p, o);
    else if (/\.(js|jsx)$/.test(e.name)) o.push(p);
  }
  return o;
}

const RULES = [
  [/(\/utilities\/categorias\/[^"'`]*?)\.png/g, '$1.webp'],          // categorias + recortadas (public)
  [/(TimesNormalized\/[^"'`]*?)\.png/g, '$1.webp'],                   // logos de time
  [/(source-images\/Categorias\/[^"'`]*?)\.png/g, '$1.webp'],         // fontes de categoria
];

let grand = 0;
for (const f of walk('src')) {
  const src = fs.readFileSync(f, 'utf8');
  const lines = src.split('\n');
  let count = 0;
  const out = lines.map((line, i) => {
    let nl = line;
    for (const [re, rep] of RULES) nl = nl.replace(re, rep);
    if (nl !== line) {
      count++;
      console.log(`  ${f}:${i + 1}`);
      console.log(`    - ${line.trim()}`);
      console.log(`    + ${nl.trim()}`);
    }
    return nl;
  });
  if (count) {
    if (write) fs.writeFileSync(f, out.join('\n'));
    grand += count;
  }
}
console.log(`\nTotal: ${grand} linha(s)${write ? ' ESCRITAS' : ' (dry — rode com --write)'}`);
