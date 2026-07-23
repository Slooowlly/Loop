// Gera src/utils/trackCountries.js a partir da fonte Rust (constants/tracks.rs).
// Chaveado pelo NOME DO LOCAL (parte do `nome` antes de " - "), que é exatamente o
// `track_name` que o calendário guarda (ver split_track_name em calendar/mod.rs).
// Assim o país resolve no frontend por track_name, sem depender de rebuild do backend.
// Rodar: node scripts/gen-track-countries.mjs
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const SRC = join(here, "..", "src-tauri", "src", "constants", "tracks.rs");
const OUT = join(here, "..", "src", "utils", "trackCountries.js");

const rust = readFileSync(SRC, "utf8");
const map = {};
const addKey = (key, pais) => {
  const k = (key ?? "").trim();
  if (k && !(k in map)) map[k] = pais;
};

// Pass A: `nome` completo → também gera a chave do LOCAL (parte antes de " - "),
// que é o que o calendário guarda em track_name (split_track_name).
const reNome = /\bnome:\s*"([^"]+)"[\s\S]*?\bpais:\s*"([^"]+)"/g;
let match;
while ((match = reNome.exec(rust)) !== null) {
  const [, nome, pais] = match;
  addKey(nome, pais);
  addKey(nome.split(" - ")[0], pais);
}

// Pass B: `nome_curto` → robustez para saves que guardaram o nome curto.
const reCurto = /\bnome_curto:\s*"([^"]+)"[\s\S]*?\bpais:\s*"([^"]+)"/g;
while ((match = reCurto.exec(rust)) !== null) {
  addKey(match[1], match[2]);
}

const entries = Object.entries(map).sort((a, b) => a[0].localeCompare(b[0], "pt"));
const body = entries.map(([venue, pais]) => `  ${JSON.stringify(venue)}: ${JSON.stringify(pais)},`).join("\n");
const file = `// GERADO por scripts/gen-track-countries.mjs — não editar à mão.
// Mapa nome→país (com bandeira), de src-tauri/src/constants/tracks.rs. Inclui nome
// completo, nome do local (antes de " - ") e nome curto, para casar com o track_name
// que o calendário guardar (ver split_track_name em calendar/mod.rs).
export const TRACK_COUNTRIES = {
${body}
};
`;

writeFileSync(OUT, file, "utf8");
console.log(`✓ ${entries.length} chaves de nome → ${OUT}`);
