import test from "node:test";
import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

async function readProjectFile(relativePath) {
  return readFile(path.join(projectRoot, relativePath), "utf8");
}

/// Todo `.rs` do crate, com o caminho relativo junto — a asserção precisa NOMEAR o arquivo
/// que reintroduziu o símbolo, senão a falha manda procurar em 900 arquivos.
async function readRustSources() {
  const raiz = path.join(projectRoot, "src-tauri", "src");
  const entradas = await readdir(raiz, { recursive: true, withFileTypes: true });
  const arquivos = entradas.filter((e) => e.isFile() && e.name.endsWith(".rs"));

  return Promise.all(
    arquivos.map(async (e) => {
      const absoluto = path.join(e.parentPath ?? e.path, e.name);
      return {
        relativePath: path.relative(projectRoot, absoluto).split(path.sep).join("/"),
        source: await readFile(absoluto, "utf8"),
      };
    }),
  );
}

test("window controls drawer keeps a fullscreen toggle button", async () => {
  const drawerSource = await readProjectFile("src/components/layout/WindowControlsDrawer.jsx");

  assert.match(
    drawerSource,
    /handleToggleFullscreen/,
    "expected the drawer to keep a fullscreen toggle handler",
  );
  assert.match(
    drawerSource,
    /toggle_fullscreen_window/,
    "expected the drawer to invoke the fullscreen toggle command",
  );
});

test("tauri backend exposes fullscreen toggle commands", async () => {
  const windowCommands = await readProjectFile("src-tauri/src/commands/window.rs");
  const libSource = await readProjectFile("src-tauri/src/lib.rs");

  assert.match(
    windowCommands,
    /pub fn toggle_fullscreen_window/,
    "expected a backend fullscreen toggle command",
  );
  assert.match(
    windowCommands,
    /pub fn get_window_fullscreen/,
    "expected a backend fullscreen state command",
  );
  assert.match(
    libSource,
    /commands::window::toggle_fullscreen_window/,
    "expected the app to register the fullscreen toggle command",
  );
  assert.match(
    libSource,
    /commands::window::get_window_fullscreen/,
    "expected the app to register the fullscreen state command",
  );
});

// Os três helpers de diagnóstico de carreira (`verify_database`, `test_create_driver`,
// `test_list_drivers`) foram REMOVIDOS em 12/08/2026: viviam em `commands/career/lifecycle.rs`
// sob `#[allow(dead_code)]`, sem entrada no `generate_handler!` e sem nenhum `invoke` no
// frontend. Este guard cobrava só que eles não entrassem no handler — com o código apagado,
// essa asserção passou a ser verdadeira por vacuidade e não podia mais falhar. Agora ele cobra
// o fato novo: os nomes não voltam ao crate. Quem quiser o helper de volta escreve um comando
// de verdade, registrado, em vez de ressuscitar a função morta.
test("tauri backend keeps diagnostic career helpers out of the rust crate", async () => {
  const rustSources = await readRustSources();

  // Varredura que não acha arquivo nenhum passa verde e não guarda nada — o mesmo modo de
  // falha que o PISO de `rodar-guards.mjs` existe para pegar.
  assert.ok(
    rustSources.length > 100,
    `a varredura leu só ${rustSources.length} arquivos .rs — o guard não estaria olhando o crate`,
  );

  for (const nome of ["verify_database", "test_create_driver", "test_list_drivers"]) {
    const encontrado = rustSources.filter(({ source }) => source.includes(nome));
    assert.deepEqual(
      encontrado.map(({ relativePath }) => relativePath),
      [],
      `expected ${nome} to stay out of the rust crate`,
    );
  }
});
