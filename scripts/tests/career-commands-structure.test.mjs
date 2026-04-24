import test from "node:test";
import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

async function readProjectFile(relativePath) {
  return readFile(path.join(projectRoot, relativePath), "utf8");
}

test("career tauri commands live in a dedicated command-surface module", async () => {
  await assert.doesNotReject(() =>
    access(path.join(projectRoot, "src-tauri/src/commands/career_commands.rs")),
  );

  const commandsModSource = await readProjectFile("src-tauri/src/commands/mod.rs");
  const careerSource = await readProjectFile("src-tauri/src/commands/career.rs");
  const careerCommandsSource = await readProjectFile("src-tauri/src/commands/career_commands.rs");
  const libSource = await readProjectFile("src-tauri/src/lib.rs");

  assert.match(
    commandsModSource,
    /pub mod career_commands;/,
    "expected the commands module to expose the new career_commands sibling module",
  );
  assert.match(
    careerCommandsSource,
    /#\[tauri::command\][\s\S]*pub async fn create_career\(/,
    "expected create_career to move into career_commands.rs",
  );
  assert.match(
    careerCommandsSource,
    /#\[tauri::command\][\s\S]*pub async fn get_driver_detail\(/,
    "expected get_driver_detail to move into career_commands.rs",
  );
  assert.doesNotMatch(
    careerSource,
    /#\[tauri::command\]/,
    "expected career.rs to stop defining Tauri commands directly",
  );
  assert.match(
    libSource,
    /commands::career_commands::create_career,/,
    "expected invoke_handler to use career_commands::create_career",
  );
  assert.match(
    libSource,
    /commands::career_commands::get_driver_detail,/,
    "expected invoke_handler to use career_commands::get_driver_detail",
  );
  assert.doesNotMatch(
    libSource,
    /commands::career::create_career,/,
    "expected invoke_handler to stop referencing career::create_career",
  );
});
