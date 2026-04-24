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

test("entry flow defines shared visual primitives", async () => {
  const cssSource = await readProjectFile("src/index.css");

  assert.match(cssSource, /\.entry-shell\s*\{/, "expected a shared entry shell class");
  assert.match(cssSource, /\.entry-backdrop\s*\{/, "expected a shared entry backdrop class");
  assert.match(cssSource, /\.entry-panel\s*\{/, "expected a shared entry panel class");
});

test("boot, splash, and menu reuse the same branded entry system", async () => {
  const bootSource = await readProjectFile("src/pages/BootLogoScreen.jsx");
  const splashSource = await readProjectFile("src/pages/SplashScreen.jsx");
  const menuSource = await readProjectFile("src/pages/MainMenu.jsx");

  assert.match(bootSource, /entry-shell/, "expected boot screen to use the shared shell");
  assert.match(splashSource, /entry-shell/, "expected splash screen to use the shared shell");
  assert.match(menuSource, /entry-shell/, "expected main menu to use the shared shell");

  assert.match(splashSource, /logo-nova\.png/, "expected splash screen to reuse the new logo");
  assert.match(menuSource, /logo-nova\.png/, "expected main menu to reuse the new logo");
  assert.match(menuSource, /entry-panel/, "expected main menu actions to live inside a branded panel");
});
