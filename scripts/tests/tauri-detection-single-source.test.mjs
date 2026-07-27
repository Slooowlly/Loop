import test from "node:test";
import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

// A deteccao "estou dentro do shell do Tauri?" ja foi copiada em 11 arquivos (10
// deles no overlay, a parte mais dificil de testar a mao). Como ela e o gate que
// decide se um `invoke` acontece ou se o codigo vira no-op, uma copia divergente
// falha em silencio. Agora mora so em src/lib/tauri.js — este guard e o que
// impede a 12a copia de nascer.

const FONTE_UNICA = "src/lib/tauri.js";

async function arquivosDeFonte(dirRelativo) {
  const encontrados = [];
  const varrer = async (rel) => {
    const entradas = await readdir(path.join(projectRoot, rel), { withFileTypes: true });
    for (const entrada of entradas) {
      const filho = path.posix.join(rel, entrada.name);
      if (entrada.isDirectory()) {
        await varrer(filho);
      } else if (/\.(js|jsx)$/.test(entrada.name)) {
        encontrados.push(filho);
      }
    }
  };
  await varrer(dirRelativo);
  return encontrados;
}

test("tauri detection lives in a single canonical module", async () => {
  const fonte = await readFile(path.join(projectRoot, FONTE_UNICA), "utf8");

  assert.match(
    fonte,
    /export function estaNoTauri\(\)/,
    `expected ${FONTE_UNICA} to export the canonical estaNoTauri() helper`,
  );
  assert.match(
    fonte,
    /"__TAURI_INTERNALS__" in window/,
    // `window.__TAURI__` so existe com app.withGlobalTauri no tauri.conf.json,
    // que este projeto nao liga — `__TAURI_INTERNALS__` e o global sempre
    // injetado pelo Tauri v2.
    `expected ${FONTE_UNICA} to detect Tauri v2 via __TAURI_INTERNALS__`,
  );
});

test("no module redefines the Tauri detection locally", async () => {
  const arquivos = await arquivosDeFonte("src");
  const infratores = [];

  for (const rel of arquivos) {
    if (rel === FONTE_UNICA) continue;
    const conteudo = await readFile(path.join(projectRoot, rel), "utf8");
    if (/__TAURI_INTERNALS__|__TAURI__/.test(conteudo) || /\bIN_TAURI\b/.test(conteudo)) {
      infratores.push(rel);
    }
  }

  assert.deepEqual(
    infratores,
    [],
    `estes arquivos redefinem a deteccao do Tauri; importe estaNoTauri() de ${FONTE_UNICA}`,
  );
});
