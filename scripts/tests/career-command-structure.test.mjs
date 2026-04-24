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

test("career command types live in a dedicated sibling module", async () => {
  await assert.doesNotReject(() =>
    access(path.join(projectRoot, "src-tauri/src/commands/career_types.rs")),
  );

  const commandsModSource = await readProjectFile("src-tauri/src/commands/mod.rs");
  const careerSource = await readProjectFile("src-tauri/src/commands/career.rs");
  const careerTypesSource = await readProjectFile("src-tauri/src/commands/career_types.rs");

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
