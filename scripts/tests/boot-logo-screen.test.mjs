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

test("app defines a boot screen before the splash route", async () => {
  const appSource = await readProjectFile("src/App.jsx");

  assert.match(
    appSource,
    /import BootLogoScreen from "\.\/pages\/BootLogoScreen";/,
    "expected App.jsx to import the boot logo screen",
  );
  assert.match(
    appSource,
    /<Route path="\/" element={<BootLogoScreen \/>} \/>/,
    "expected the root route to render the boot logo screen",
  );
  assert.match(
    appSource,
    /<Route path="\/splash" element={<SplashScreen \/>} \/>/,
    "expected SplashScreen to move to /splash",
  );
});

test("boot logo screen auto-navigates after 2 seconds", async () => {
  const bootSource = await readProjectFile("src/pages/BootLogoScreen.jsx");

  assert.match(bootSource, /useNavigate/, "expected navigation from the boot screen");
  assert.match(
    bootSource,
    /setTimeout\(\(\) => \{\s*navigate\("\/splash"\);\s*\}, 2000\);/s,
    "expected automatic navigation to /splash after 2000ms",
  );
  assert.match(
    bootSource,
    /logo/i,
    "expected the boot screen to render the app logo asset",
  );
});

test("window controls drawer hides during the boot flow", async () => {
  const drawerSource = await readProjectFile("src/components/layout/WindowControlsDrawer.jsx");
  const splashSource = await readProjectFile("src/pages/SplashScreen.jsx");

  assert.match(
    drawerSource,
    /location\.pathname === "\/" \|\| location\.pathname === "\/splash"/,
    "expected the drawer to hide on the boot flow routes",
  );
  assert.match(
    splashSource,
    /navigate\("\/menu"\)/,
    "expected the splash screen to keep entering the app through /menu",
  );
});
