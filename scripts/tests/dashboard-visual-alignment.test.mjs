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

test("app defines shared standard background primitives", async () => {
  const cssSource = await readProjectFile("src/index.css");

  assert.match(cssSource, /\.app-shell\s*\{/, "expected a shared app shell class");
  assert.match(cssSource, /\.app-backdrop\s*\{/, "expected a shared app backdrop class");
});

test("dashboard, load save, and new career use the branded standard background", async () => {
  const layoutSource = await readProjectFile("src/components/layout/MainLayout.jsx");
  const loadSaveSource = await readProjectFile("src/pages/LoadSave.jsx");
  const newCareerSource = await readProjectFile("src/pages/NewCareer.jsx");

  assert.match(layoutSource, /app-shell/, "expected MainLayout to use the shared app shell");
  assert.match(layoutSource, /app-backdrop/, "expected MainLayout to render the shared app backdrop");
  assert.match(loadSaveSource, /app-shell/, "expected LoadSave to use the shared app shell");
  assert.match(loadSaveSource, /app-backdrop/, "expected LoadSave to render the shared app backdrop");
  assert.match(newCareerSource, /app-shell/, "expected NewCareer to use the shared app shell");
  assert.match(newCareerSource, /app-backdrop/, "expected NewCareer to render the shared app backdrop");
});
