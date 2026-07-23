import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

async function read(relativePath) {
  return readFile(resolve(root, relativePath), "utf8");
}

test("keeps the demo champion overlay dormant in this version", async () => {
  const [app, settings, dashboard, store] = await Promise.all([
    read("src/App.jsx"),
    read("src/pages/Settings.jsx"),
    read("src/pages/Dashboard.jsx"),
    read("src/stores/useCareerStore.js"),
  ]);

  assert.doesNotMatch(app, /SeasonChampionOverlay/);
  assert.doesNotMatch(settings, /debugForceSeasonEndChampion|Forçar fim de temporada/);
  assert.doesNotMatch(
    dashboard,
    /showChampionOverlay|pendingDashboardTab|consumePendingDashboardTab/,
  );
  assert.doesNotMatch(
    store,
    /debugForceSeasonEndChampion|pendingDashboardTab|consumePendingDashboardTab|afterCloseTab/,
  );
  assert.match(store, /showChampionOverlay\s*:/);
  assert.match(store, /hideChampionOverlay\s*:/);
});
