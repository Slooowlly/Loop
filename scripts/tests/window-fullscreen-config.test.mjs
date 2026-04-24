import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");
const tauriConfigPath = path.join(projectRoot, "src-tauri", "tauri.conf.json");

test("main window uses native fullscreen instead of maximized mode", async () => {
  const rawConfig = await readFile(tauriConfigPath, "utf8");
  const config = JSON.parse(rawConfig);
  const mainWindow = config?.app?.windows?.[0];

  assert.ok(mainWindow, "expected a main Tauri window configuration");
  assert.equal(
    mainWindow.fullscreen,
    true,
    "expected the main window to open in native fullscreen",
  );
  assert.equal(
    mainWindow.maximized,
    false,
    "expected the main window not to rely on maximized mode",
  );
});
