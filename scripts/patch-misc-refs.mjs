// Troca .png -> .webp para o lote "misc": texturas (Fuel/Pneu/paper-spread),
// news (magazine-cover), tutorial (steps) e o logo do menu (LOGO NOVA).
// Escopado por prefixo — não toca em mais nada. Processa .js/.jsx/.css.
// Uso: node scripts/patch-misc-refs.mjs [--write]
import fs from 'fs';

const write = process.argv.includes('--write');
function walk(d, o = []) {
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = `${d}/${e.name}`;
    if (e.isDirectory()) walk(p, o);
    else if (/\.(js|jsx|css)$/.test(e.name)) o.push(p);
  }
  return o;
}

const RULES = [
  [/(\/utilities\/textures\/[^"'`)]*?)\.png/g, '$1.webp'],   // paper-spread (CSS url())
  [/(\$\{TX\}[^"'`]*?)\.png/g, '$1.webp'],                    // Fuel/Pneu via template ${TX}
  [/(\/utilities\/news\/[^"'`]*?)\.png/g, '$1.webp'],         // magazine-cover
  [/(\/utilities\/tutorial\/[^"'`]*?)\.png/g, '$1.webp'],     // tutorial steps
  [/(\/utilities\/LOGO%20NOVA)\.png/g, '$1.webp'],            // logo do menu
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
      console.log(`  ${f}:${i + 1}\n    - ${line.trim()}\n    + ${nl.trim()}`);
    }
    return nl;
  });
  if (count && write) fs.writeFileSync(f, out.join('\n'));
  grand += count;
}
console.log(`\nTotal: ${grand} linha(s)${write ? ' ESCRITAS' : ' (dry)'}`);
