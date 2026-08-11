// Guarda o MÓDULO DE TIPOS da carreira: os DTOs serde que cruzam a ponte Rust/React vivem em
// `career_types/`, separados da lógica e da casca de comando.
//
// O irmão `career-command-shell-structure.test.mjs` vigia a OUTRA metade: a casca fina de
// `#[tauri::command]`. Os dois se chamavam `career-command-structure` e
// `career-commands-structure` — um "s" de diferença, o que fazia editar o arquivo errado ser
// o desfecho provável. Renomeados em 11/08/2026 para o que cada um de fato vigia.

import test from "node:test";
import assert from "node:assert/strict";
import { access, readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

async function readProjectFile(relativePath) {
  return readFile(path.join(projectRoot, relativePath), "utf8");
}

// `career_types.rs` virou fachada: os DTOs vivem nos irmaos de `career_types/`.
// O que importa e que eles estejam fora de `career.rs`, nao em qual arquivo do
// modulo cairam — entao a suite le o modulo inteiro.
async function readCareerTypesModule() {
  const dir = "src-tauri/src/commands/career_types";
  const arquivos = (await readdir(path.join(projectRoot, dir))).filter((nome) => nome.endsWith(".rs"));
  assert.ok(arquivos.length > 0, `nenhum submodulo .rs encontrado em ${dir}`);
  const partes = await Promise.all(arquivos.map((nome) => readProjectFile(path.posix.join(dir, nome))));
  return partes.join("\n");
}

test("career command types live in a dedicated sibling module", async () => {
  await assert.doesNotReject(() =>
    access(path.join(projectRoot, "src-tauri/src/commands/career_types.rs")),
  );

  const commandsModSource = await readProjectFile("src-tauri/src/commands/mod.rs");
  const careerSource = await readProjectFile("src-tauri/src/commands/career.rs");
  const careerTypesFacade = await readProjectFile("src-tauri/src/commands/career_types.rs");
  const careerTypesSource = await readCareerTypesModule();

  assert.match(
    commandsModSource,
    /pub mod career_types;/,
    "expected the commands module to expose the new career_types sibling module",
  );
  assert.match(
    careerSource,
    /use crate::commands::career_types::\{/,
    "expected career.rs to import DTOs from the new sibling module",
  );
  assert.match(
    careerTypesFacade,
    /pub use \w+::\*;/,
    "expected career_types.rs to stay a facade re-exporting its submodules",
  );
  assert.match(
    careerTypesSource,
    /pub struct CreateCareerInput/,
    "expected CreateCareerInput to move into career_types.rs",
  );
  assert.match(
    careerTypesSource,
    /pub struct DriverDetail/,
    "expected DriverDetail to move into career_types.rs",
  );
  assert.match(
    careerTypesSource,
    /pub struct TeamStanding/,
    "expected TeamStanding to move into career_types.rs",
  );
  assert.doesNotMatch(
    careerSource,
    /pub struct CreateCareerInput/,
    "expected CreateCareerInput to stop being defined inline in career.rs",
  );
  assert.doesNotMatch(
    careerSource,
    /pub struct DriverDetail/,
    "expected DriverDetail to stop being defined inline in career.rs",
  );
  assert.doesNotMatch(
    careerSource,
    /pub struct TeamStanding/,
    "expected TeamStanding to stop being defined inline in career.rs",
  );
});
