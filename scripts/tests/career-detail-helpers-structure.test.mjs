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

test("career driver-detail helpers live in a dedicated sibling module", async () => {
  await assert.doesNotReject(() =>
    access(path.join(projectRoot, "src-tauri/src/commands/career_detail.rs")),
  );

  const commandsModSource = await readProjectFile("src-tauri/src/commands/mod.rs");
  const careerSource = await readProjectFile("src-tauri/src/commands/career.rs");
  const careerDetailSource = await readProjectFile("src-tauri/src/commands/career_detail.rs");

  assert.match(
    commandsModSource,
    /pub mod career_detail;/,
    "expected the commands module to expose the new career_detail sibling module",
  );
  assert.match(
    careerSource,
    /use crate::commands::career_detail::build_driver_detail_payload;/,
    "expected career.rs to delegate driver-detail payload building to career_detail.rs",
  );
  assert.match(
    careerDetailSource,
    /pub\(crate\) fn build_driver_detail_payload\(/,
    "expected career_detail.rs to define the extracted driver-detail payload builder",
  );
  assert.match(
    careerDetailSource,
    /leitura_tecnica:\s*build_driver_technical_read_block\(driver\)/,
    "expected driver detail payload to provide backend-owned technical readings",
  );
  assert.match(
    careerDetailSource,
    /mercado:\s*Some\(build_driver_market_block\([\s\S]*driver,[\s\S]*contract,[\s\S]*team,[\s\S]*season\.numero,[\s\S]*\)\)/,
    "expected driver detail payload to fill market data instead of leaving it disconnected",
  );
  assert.match(
    careerDetailSource,
    /if driver\.stats_carreira\.corridas == 0 && results\.is_empty\(\) \{[\s\S]*veredito:\s*"Estreante"\.to_string\(\),[\s\S]*tom:\s*"info"\.to_string\(\)/,
    "expected backend summary to expose a distinct rookie verdict when no history exists",
  );
  assert.match(
    careerDetailSource,
    /let recent_form_source:[\s\S]*\.take\(10\)[\s\S]*ultimas_10:/,
    "expected driver detail form to expose the last 10 races for the recent-form chart",
  );
  assert.doesNotMatch(
    careerSource,
    /fn convert_tags\(/,
    "expected convert_tags to stop being defined inline in career.rs",
  );
  assert.doesNotMatch(
    careerSource,
    /fn build_driver_profile_block\(/,
    "expected build_driver_profile_block to stop being defined inline in career.rs",
  );
  assert.doesNotMatch(
    careerSource,
    /fn build_driver_performance_block\(/,
    "expected build_driver_performance_block to stop being defined inline in career.rs",
  );
});
