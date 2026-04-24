import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

test("window controls drawer uses compact icon sizing", async () => {
  const drawerSource = await readFile(
    path.join(projectRoot, "src/components/layout/WindowControlsDrawer.jsx"),
    "utf8",
  );

  assert.match(
    drawerSource,
    /flex h-9 w-9 items-center justify-center/,
    "expected smaller tray buttons",
  );
  assert.match(
    drawerSource,
    /text-\[11px\].*&minus;/s,
    "expected a smaller minimize icon",
  );
  assert.match(
    drawerSource,
    /text-\[12px\].*\{isFullscreen \? /s,
    "expected a smaller fullscreen icon",
  );
  assert.match(
    drawerSource,
    /text-\[14px\].*&times;/s,
    "expected a smaller close icon",
  );
});
