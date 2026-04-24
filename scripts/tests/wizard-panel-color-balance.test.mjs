import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

async function readProjectFile(relativePath) {
  return readFile(path.join(projectRoot, relativePath), "utf8");
}

test("wizard panel glow uses the monochromatic blue palette", async () => {
  const cssSource = await readProjectFile("src/index.css");

  assert.match(cssSource, /\.wizard-panel::before/, "expected wizard panel overlay to exist");
  assert.doesNotMatch(
    cssSource,
    /rgba\(188,\s*140,\s*255,\s*0\.12\)/,
    "expected wizard panel glow not to use the old violet tone",
  );
});
