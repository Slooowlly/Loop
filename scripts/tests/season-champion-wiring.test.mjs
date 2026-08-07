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
// arquivo raiz deixaria o guard cego: bastaria mexer no fluxo do campeão dentro
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

// Este guard substituiu o `season-champion-disabled`: o pop-up saiu do estado
// dormente (dados de exemplo) e agora é alimentado pelo backend. O que ele protege
// mudou de "não montar" para "montar ligado nos dados reais".
test("wires the season champion overlay to real backend data", async () => {
  const [app, dashboard, overlay, storeSources] = await Promise.all([
    read("src/App.jsx"),
    read("src/pages/Dashboard.jsx"),
    read("src/components/season/SeasonChampionOverlay.jsx"),
    readCareerStoreSources(),
  ]);

  // Sanidade do próprio guard: se os slices sumirem, a varredura precisa gritar
  // em vez de passar por ter lido só a fachada de poucas linhas.
  assert.ok(
    storeSources.length > 1,
    "esperava a fachada do store mais os slices em src/stores/career/",
  );

  // Host global — sem isto o pop-up nunca aparece, por mais que o store tenha payload.
  assert.match(app, /<SeasonChampionOverlay \/>/);

  // O gatilho de fim de campeonato vive no pós-corrida do Dashboard.
  assert.match(dashboard, /loadSeasonChampionOverlay/);

  const storeCode = storeSources.map((source) => source.code).join("\n");
  assert.match(storeCode, /showChampionOverlay\s*:/);
  assert.match(storeCode, /hideChampionOverlay\s*:/);
  assert.match(storeCode, /loadSeasonChampionOverlay\s*:/);
  assert.match(storeCode, /get_season_champion_payload/);

  // Dados de exemplo não voltam: o pop-up só desenha o que o backend mandou.
  assert.doesNotMatch(overlay, /\bDEMO\b/);
  assert.doesNotMatch(overlay, /demo\s*:\s*true/);
  assert.doesNotMatch(storeCode, /demo\s*:\s*true/);
  // Prosa em português no JSX quebraria o i18n — o arquivo perdeu o i18n-ignore-file
  // justamente porque não tem mais texto fixo.
  assert.doesNotMatch(overlay, /i18n-ignore-file/);

  // Fluxo antigo (removido em 2026-07) não pode ressuscitar de carona.
  for (const { path, code } of storeSources) {
    assert.doesNotMatch(
      code,
      /debugForceSeasonEndChampion|pendingDashboardTab|consumePendingDashboardTab|afterCloseTab/,
      `${path} reintroduziu o fluxo de fim de temporada removido`,
    );
  }
});
