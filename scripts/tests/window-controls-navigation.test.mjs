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

// O atalho 🏠 do drawer foi aposentado: quem sai da carreira hoje é o PauseMenu
// (tecla Esc ou o botão Loop no canto superior esquerdo), com o LeaveToMenuModal.
// Restam aqui as garantias que continuam valendo pro drawer.
test("window controls drawer keeps the save-aware exit flow", async () => {
  const drawerSource = await readProjectFile("src/components/layout/WindowControlsDrawer.jsx");

  assert.match(drawerSource, /useLocation/, "expected route awareness in the drawer");
  assert.match(drawerSource, /flushSave/, "expected the drawer to offer a save-before-exit path");
  assert.match(drawerSource, /SaveConfirmModal/, "expected the drawer to confirm leaving the career");
});

test("the pause menu owns the route back to the main menu", async () => {
  const pauseSource = await readProjectFile("src/components/layout/PauseMenu.jsx");

  assert.match(pauseSource, /useNavigate/, "expected the pause menu to navigate away");
  assert.match(pauseSource, /"Escape"/, "expected Esc to open the pause menu");
  assert.match(pauseSource, /LeaveToMenuModal/, "expected a confirmation before leaving the career");
});

test("window controls drawer becomes global and dashboard menu button is removed", async () => {
  const appSource = await readProjectFile("src/App.jsx");
  const layoutSource = await readProjectFile("src/components/layout/MainLayout.jsx");
  const headerSource = await readProjectFile("src/components/layout/Header.jsx");

  assert.match(
    appSource,
    /import WindowControlsDrawer from "\.\/components\/layout\/WindowControlsDrawer";/,
    "expected App.jsx to import the global drawer",
  );
  assert.match(
    appSource,
    /<WindowControlsDrawer \/>/,
    "expected App.jsx to render the global drawer",
  );
  assert.doesNotMatch(
    layoutSource,
    /<WindowControlsDrawer \/>/,
    "expected MainLayout.jsx not to render a duplicate drawer",
  );
  assert.doesNotMatch(
    headerSource,
    /Voltar ao menu/,
    "expected the redundant dashboard menu button to be removed",
  );
});
