import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

test("ResultBadge renders a purple fastest-lap marker when history says the driver had the best lap", async () => {
  const source = await readFile(
    path.join(projectRoot, "src/components/standings/ResultBadge.jsx"),
    "utf8",
  );

  assert.match(
    source,
    /has_fastest_lap/,
    "expected ResultBadge to consume the fastest-lap flag from round history",
  );
  assert.match(
    source,
    /#bc8cff|ROXO|purple|violet/,
    "expected ResultBadge to style the fastest-lap marker with a purple accent",
  );
});
