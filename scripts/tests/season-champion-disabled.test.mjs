import test from "node:test";
import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

async function read(relativePath) {
  return readFile(resolve(root, relativePath), "utf8");
}

// O store de carreira virou fachada + slices em src/stores/career/. Ler só o
// arquivo raiz deixaria o guard cego: bastaria ressuscitar o modo campeão dentro
// de um slice pra passar despercebido. Então varremos a fachada e todos os slices.
async function readCareerStoreSources() {
  const sources = [
    { path: "src/stores/useCareerStore.js", code: await read("src/stores/useCareerStore.js") },
  ];
  const sliceDir = "src/stores/career";
  const entries = await readdir(resolve(root, sliceDir), { withFileTypes: true });
  for (const entry of entries) {
    if (!entry.isFile() || !/\.jsx?$/.test(entry.name)) continue;
    const path = `${sliceDir}/${entry.name}`;
    sources.push({ path, code: await read(path) });
  }
  return sources;
}

test("keeps the demo champion overlay dormant in this version", async () => {
  const [app, settings, dashboard, storeSources] = await Promise.all([
    read("src/App.jsx"),
    read("src/pages/Settings.jsx"),
    read("src/pages/Dashboard.jsx"),
    readCareerStoreSources(),
  ]);

  // Sanidade do próprio guard: se os slices sumirem, a varredura precisa gritar
  // em vez de passar por ter lido só a fachada de poucas linhas.
  assert.ok(
    storeSources.length > 1,
    "esperava a fachada do store mais os slices em src/stores/career/",
  );

  assert.doesNotMatch(app, /SeasonChampionOverlay/);
  assert.doesNotMatch(settings, /debugForceSeasonEndChampion|Forçar fim de temporada/);
  assert.doesNotMatch(
    dashboard,
    /showChampionOverlay|pendingDashboardTab|consumePendingDashboardTab/,
  );

  for (const { path, code } of storeSources) {
    assert.doesNotMatch(
      code,
      /debugForceSeasonEndChampion|pendingDashboardTab|consumePendingDashboardTab|afterCloseTab/,
      `${path} reintroduziu o fluxo de fim de temporada desativado`,
    );
  }

  const storeCode = storeSources.map((source) => source.code).join("\n");
  assert.match(storeCode, /showChampionOverlay\s*:/);
  assert.match(storeCode, /hideChampionOverlay\s*:/);
});
