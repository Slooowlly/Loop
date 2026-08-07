import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

async function readSources() {
  const [drawer, header] = await Promise.all([
    readFile(path.join(projectRoot, "src/components/layout/WindowControlsDrawer.jsx"), "utf8"),
    readFile(
      path.join(projectRoot, "src/components/season/preseason/v2/PreSeasonHeaderV2.jsx"),
      "utf8",
    ),
  ]);
  return { drawer, header };
}

// A gaveta de controles de janela ocupa um retângulo de interação fixo no canto
// superior direito, com z-50 e fundo transparente, aberta ou fechada. Qualquer
// coisa desenhada sob ele recebe zero cliques e zero hover — foi assim que
// "Ver Quem Sai" morreu quando o layout foi para a largura cheia, sem nenhum
// sintoma além do botão parecer quebrado.
test("the preseason header corner holds nothing the drawer could swallow", async () => {
  const { drawer, header } = await readSources();

  assert.match(
    drawer,
    /className="fixed right-\d+ top-\[\d+px\] z-50"/,
    "expected the drawer to stay anchored to the top-right corner",
  );

  const start = header.indexOf('data-testid="preseason-week-panel"');
  assert.ok(start > 0, "expected the header corner block to be identifiable by test id");
  const end = header.indexOf('data-testid="preseason-filter-bar"', start);
  assert.ok(end > start, "expected the filter bar to follow the header corner block");
  const corner = header.slice(start, end);

  for (const [pattern, what] of [
    [/onClick=/, "a click handler"],
    [/<button/, "a button"],
    [/<Tooltip/, "a tooltip"],
  ]) {
    assert.ok(
      !pattern.test(corner),
      `the preseason header corner block declares ${what}. It sits inside the window controls `
        + "drawer's hit area, so the mouse never reaches it — move it out of the corner or drop it.",
    );
  }
});

// O botão é a única ação da tela e precisa ficar no centro DA TELA. Com
// `1fr auto 1fr` isso só vale enquanto as duas laterais medirem igual: um recuo
// só de um lado na linha desloca a coluna do meio junto com ele, e o botão sai
// do centro sem nenhum aviso.
test("the preseason action button stays centred on the screen", async () => {
  const { header } = await readSources();

  const row = header.match(/data-testid="preseason-header-row"\s+className="([^"]*)"/);
  assert.ok(row, "expected the header action row to be identifiable by test id");
  assert.match(
    row[1],
    /grid-cols-\[1fr_auto_1fr\]/,
    "expected the header row to keep equal side tracks around the action column",
  );

  const left = row[1].match(/(?:^|\s)pl-\[(\d+)px\]/);
  const right = row[1].match(/(?:^|\s)pr-\[(\d+)px\]/);
  assert.ok(
    (left?.[1] ?? null) === (right?.[1] ?? null),
    `the header row pads ${left?.[1] ?? 0}px on the left and ${right?.[1] ?? 0}px on the right. `
      + "Asymmetric padding shifts the centre column by half the difference, so the action button "
      + "stops being centred on the screen. Pad inside the side columns instead.",
  );

  const button = header.match(/data-testid="preseason-advance-week"[\s\S]{0,400}?className=\{`([^`]*)`/);
  assert.ok(button, "expected the action button to be identifiable by test id");
  assert.match(
    button[1],
    /justify-self-center/,
    "expected the action button to sit in the centred grid column",
  );
});
