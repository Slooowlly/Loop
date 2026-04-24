import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

test("window controls drawer keeps a dedicated hover target separate from the visual tray", async () => {
  const drawerSource = await readFile(
    path.join(projectRoot, "src/components/layout/WindowControlsDrawer.jsx"),
    "utf8",
  );

  assert.match(
    drawerSource,
    /className="relative h-\[390px\] w-\[148px\]"/,
    "expected a narrower interaction wrapper around the tray",
  );
  assert.match(
    drawerSource,
    /data-testid="window-controls-hover-target"/,
    "expected a dedicated hover target outside the visible tray",
  );
  assert.match(
    drawerSource,
    /className=\{\[\s*"absolute right-0 top-\[8px\] z-20 flex h-10 w-10 items-center justify-center rounded-xl/,
    "expected the hover target to stay compact and positioned beside the tray",
  );
  assert.match(
    drawerSource,
    /className="relative z-10 ml-auto w-\[132px\]"/,
    "expected the visual tray to be isolated from the hover zone",
  );
});
