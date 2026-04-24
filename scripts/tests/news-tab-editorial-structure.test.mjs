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

test("dashboard navigation promotes News in the main tab bar while keeping the dedicated race briefing flow", async () => {
  const dashboardSource = await readProjectFile("src/pages/Dashboard.jsx");
  const navSource = await readProjectFile("src/components/layout/TabNavigation.jsx");

  assert.match(dashboardSource, /NewsTab/, "expected Dashboard to render NewsTab");
  assert.match(
    dashboardSource,
    /NextRaceTab/,
    "expected Dashboard to keep the dedicated next race briefing flow outside the main tab bar",
  );
  assert.match(
    dashboardSource,
    /showRaceBriefing/,
    "expected Dashboard to guard the dedicated race briefing with a store flag",
  );
  assert.match(navSource, /label:\s*"Notícias"/, "expected top nav to include Notícias");
  assert.doesNotMatch(
    navSource,
    /Proxima Corrida/,
    "expected top nav to remove Proxima Corrida",
  );
});

test("news tab renders the editorial section hierarchy in order", async () => {
  const source = await readProjectFile("src/pages/tabs/NewsTab.jsx");
  const drawersSource = await readProjectFile("src/pages/tabs/NewsScopeDrawers.jsx");

  const heroIndex = source.indexOf('data-news-section="hero"');
  const mainReaderIndex = source.indexOf('data-news-section="main-reader"');
  const dashboardIndex = drawersSource.indexOf('data-news-section="dashboard"');

  assert.notEqual(heroIndex, -1, 'expected NewsTab to include data-news-section="hero"');
  assert.notEqual(
    mainReaderIndex,
    -1,
    'expected NewsTab to include data-news-section="main-reader"',
  );
  assert.ok(
    mainReaderIndex > heroIndex,
    'expected the main reader section to appear after the hero wrapper',
  );
  assert.notEqual(
    dashboardIndex,
    -1,
    'expected NewsScopeDrawers to expose data-news-section="dashboard"',
  );
  assert.match(
    source,
    /<section data-news-section="hero"[\s\S]*<NewsScopeDrawers[\s\S]*<\/section>[\s\S]*<section data-news-section="main-reader"/,
    "expected the hero wrapper to host the drawer dashboard before the main reader section",
  );

  assert.equal(
    source.includes('data-news-section="cover"'),
    false,
    "expected NewsTab to drop the old cover section",
  );
  assert.equal(
    source.includes('data-news-section="feed"'),
    false,
    "expected NewsTab to drop the old feed section",
  );
  assert.match(
    drawersSource,
    /renderPrimaryFilters/,
    "expected the drawer dashboard to host the top-level primary filters",
  );
  assert.match(
    drawersSource,
    /renderContextualFilters/,
    "expected the drawer dashboard to host the contextual filter pills",
  );
});

test("news tab exposes the rankings special scope and delegates filter definitions to the shared helpers", async () => {
  const source = await readProjectFile("src/pages/tabs/NewsTab.jsx");
  const drawersSource = await readProjectFile("src/pages/tabs/NewsScopeDrawers.jsx");

  assert.match(
    drawersSource,
    /const SPECIAL_SCOPE_LABEL = "Rankings"/,
    "expected the special editorial scope to be labeled as Rankings",
  );
  assert.match(
    source,
    /buildFallbackPrimaryFilters/,
    "expected NewsTab to derive primary filters through the shared helper layer",
  );
  assert.doesNotMatch(
    source,
    /Expectativas/,
    "expected NewsTab not to hard-code the removed Expectativas filter in the surface",
  );
});
