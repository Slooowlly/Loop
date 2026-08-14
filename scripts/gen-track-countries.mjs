// Gera src/utils/trackCountries.js a partir da fonte Rust (constants/tracks/dados.rs).
// Chaveado pelo NOME DO LOCAL (parte do `nome` antes de " - "), que é exatamente o
// `track_name` que o calendário guarda (ver split_track_name em calendar/montagem.rs).
// Assim o país resolve no frontend por track_name, sem depender de rebuild do backend.
//
// A FONTE é `constants/tracks/dados.rs`, e não `constants/tracks.rs`. O `tracks.rs` virou
// fachada de módulo (`mod dados; pub use ...`) e não tem mais nenhuma entrada de pista —
// ler dele produzia um mapa VAZIO, gravado por cima do bom, sem nenhum erro. É por isso que
// `extrairMapa` morre quando a extração não bate com o que o arquivo declara: um gerador de
// mapa vazio precisa falhar alto, porque o sintoma no jogo é só bandeira que sumiu.
//
// Rodar: node scripts/gen-track-countries.mjs
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
export const SRC = join(here, "..", "src-tauri", "src", "constants", "tracks", "dados.rs");
const OUT = join(here, "..", "src", "utils", "trackCountries.js");

/// Piso absoluto de pistas no catálogo. O catálogo tem 107 entradas em 11/08/2026 e só cresce;
/// qualquer coisa muito abaixo disso significa que o arquivo mudou de forma e a extração pegou
/// um pedaço, não o todo. Fica bem abaixo do número real de propósito: o piso é contra ruína,
/// não contra a curadoria mexer no catálogo.
const PISO_DE_PISTAS = 60;

/** Extrai o mapa nome→país do texto do `dados.rs`. Lança quando a extração não fecha. */
export function extrairMapa(rust) {
  const map = {};
  const addKey = (key, pais) => {
    const k = (key ?? "").trim();
    if (k && !(k in map)) map[k] = pais;
  };

  // Quantas pistas o arquivo DECLARA. É contra este número que a extração é conferida —
  // regex que para de casar por mudança de formatação some em silêncio, e o mapa parcial é
  // tão ruim quanto o vazio.
  const declaradas = (rust.match(/\bnome:\s*"/g) ?? []).length;
  if (declaradas < PISO_DE_PISTAS) {
    throw new Error(
      `A fonte declara ${declaradas} pista(s), abaixo do piso de ${PISO_DE_PISTAS}. ` +
        "O catálogo mudou de arquivo ou de forma — conserte a fonte antes de gerar.",
    );
  }

  // Pass A: `nome` completo → também gera a chave do LOCAL (parte antes de " - "),
  // que é o que o calendário guarda em track_name (split_track_name).
  const reNome = /\bnome:\s*"([^"]+)"[\s\S]*?\bpais:\s*"([^"]+)"/g;
  let match;
  let pares = 0;
  while ((match = reNome.exec(rust)) !== null) {
    const [, nome, pais] = match;
    addKey(nome, pais);
    addKey(nome.split(" - ")[0], pais);
    pares += 1;
  }
  if (pares !== declaradas) {
    throw new Error(
      `${declaradas} pista(s) declarada(s) mas só ${pares} par(es) nome→país extraído(s). ` +
        "Alguma entrada perdeu o campo `pais`, ou os campos trocaram de ordem.",
    );
  }

  // Pass B: `nome_curto` → robustez para saves que guardaram o nome curto.
  const reCurto = /\bnome_curto:\s*"([^"]+)"[\s\S]*?\bpais:\s*"([^"]+)"/g;
  while ((match = reCurto.exec(rust)) !== null) {
    addKey(match[1], match[2]);
  }

  // Cada pista contribui com o nome completo, então o mapa nunca pode ter menos chaves que
  // pistas. Menos que isso é bug de extração, não catálogo pequeno.
  const chaves = Object.keys(map).length;
  if (chaves < declaradas) {
    throw new Error(`Mapa com ${chaves} chave(s) para ${declaradas} pista(s) — extração incompleta.`);
  }
  return map;
}

/** Monta o texto do módulo JS gerado. */
export function montarArquivo(map) {
  const entries = Object.entries(map).sort((a, b) => a[0].localeCompare(b[0], "pt"));
  const body = entries.map(([venue, pais]) => `  ${JSON.stringify(venue)}: ${JSON.stringify(pais)},`).join("\n");
  return `// GERADO por scripts/gen-track-countries.mjs — não editar à mão.
// Mapa nome→país (com bandeira), de src-tauri/src/constants/tracks/dados.rs. Inclui nome
// completo, nome do local (antes de " - ") e nome curto, para casar com o track_name
// que o calendário guardar (ver split_track_name em calendar/montagem.rs).
export const TRACK_COUNTRIES = {
${body}
};
`;
}

// Só grava quando chamado como CLI. Importado (pelo teste), o módulo é só as funções.
const chamadoDireto = process.argv[1]?.endsWith("gen-track-countries.mjs");
if (chamadoDireto) {
  let map;
  try {
    map = extrairMapa(readFileSync(SRC, "utf8"));
  } catch (e) {
    console.error(`✗ ${SRC}\n  ${e.message}\n  Nada foi gravado em ${OUT}.`);
    process.exit(1);
  }
  writeFileSync(OUT, montarArquivo(map), "utf8");
  console.log(`✓ ${Object.keys(map).length} chaves de nome → ${OUT}`);
}
