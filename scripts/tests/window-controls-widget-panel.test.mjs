import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

test("window controls drawer defines a delayed secondary widget panel", async () => {
  const drawerSource = await readFile(
    path.join(projectRoot, "src/components/layout/WindowControlsDrawer.jsx"),
    "utf8",
  );

  assert.match(
    drawerSource,
    /const widgetItems = \[/,
    "expected a local widget list for the secondary panel",
  );
  assert.match(
    drawerSource,
    /const \[showWidgets, setShowWidgets\] = useState\(false\);/,
    "expected a dedicated state for the delayed widget panel",
  );
  assert.match(
    drawerSource,
    /setTimeout\(\(\) => \{\s*setShowWidgets\(true\);/s,
    "expected the widget panel to open after a delay",
  );
  assert.match(
    drawerSource,
    /,\s*500\);/,
    "expected the widget panel delay to be 500ms",
  );
  assert.match(
    drawerSource,
    /widgetItems\.map\(\(widget\) =>/,
    "expected the secondary panel to render from the widget list",
  );
  assert.match(
    drawerSource,
    /left-1\/2 top-\[84px\][\s\S]*-translate-x-1\/2/,
    "expected the widget panel to be centered below the main tray",
  );
  assert.match(
    drawerSource,
    /top-\[84px\]/,
    "expected extra vertical space for the iRacerApp text block",
  );
  assert.match(
    drawerSource,
    /pointer-events-none mt-0 w-full text-center/,
    "expected the iRacerApp text block to stay directly attached to the main tray",
  );
});
