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

test("tauri backend keeps diagnostic career helpers out of the production command surface", async () => {
  const libSource = await readProjectFile("src-tauri/src/lib.rs");

  assert.doesNotMatch(
    libSource,
    /commands::career::verify_database/,
    "expected verify_database to stay out of the production invoke handler",
  );
  assert.doesNotMatch(
    libSource,
    /commands::career::test_create_driver/,
    "expected test_create_driver to stay out of the production invoke handler",
  );
  assert.doesNotMatch(
    libSource,
    /commands::career::test_list_drivers/,
    "expected test_list_drivers to stay out of the production invoke handler",
  );
});
