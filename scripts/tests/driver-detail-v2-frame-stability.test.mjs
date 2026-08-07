import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

// A troca de aba na ficha v2 custa 5–15ms de React e 281 nós de DOM. O que
// pesava eram as IMAGENS: as artes de equipe vinham em 768x512 para slots de
// 36x24, e o navegador decodifica e aplica o halo na resolução de origem — 58ms
// por repintura, pagos a cada troca de aba. As artes foram reduzidas a 384x256
// (ver scripts/downscale-team-logos.mjs); estas duas linhas são o resto da
// limpeza, pequenas e fáceis de desfazer sem querer.
test("driver detail v2 confines tab switching to the content area", async () => {
  const source = await readFile(
    path.join(projectRoot, "src/components/driver/v2/DriverDetailModalV2.jsx"),
    "utf8",
  );

  assert.match(
    source,
    /ref=\{contentRef\}[\s\S]*\[contain:layout_paint\]/,
    "expected the scrollable content area to confine layout and paint to itself",
  );
  assert.match(
    source,
    /data-testid=\{`driver-detail-tab-\$\{id\}`\}[\s\S]*transition-colors duration-150/,
    "expected the tab pills to transition only colors instead of the 300ms `transition: all` of transition-glass",
  );
});

// A arte de equipe não pode voltar a ser servida em tamanho de cartaz: é o slot
// mais repetido da interface (listas, dossiês, escada de categorias) e o custo
// aparece em toda repintura, não só no primeiro carregamento.
test("team logo art stays close to the size it is drawn at", async () => {
  const dir = path.join(projectRoot, "src/assets/utilities/source-images/TimesNormalized");
  const { readdir, readFile: read } = await import("node:fs/promises");

  async function walk(current, out = []) {
    for (const entry of await readdir(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) await walk(full, out);
      else if (/\.webp$/i.test(entry.name)) out.push(full);
    }
    return out;
  }

  // Largura do VP8X/VP8L/VP8 direto do cabeçalho — evita depender do sharp num
  // teste que só precisa saber se alguém regrediu o tamanho da arte.
  function largura(buffer) {
    const fmt = buffer.toString("ascii", 12, 16);
    if (fmt === "VP8X") return 1 + buffer.readUIntLE(24, 3);
    if (fmt === "VP8L") return (buffer.readUInt32LE(21) & 0x3fff) + 1;
    if (fmt === "VP8 ") {
      const start = buffer.indexOf(Buffer.from([0x9d, 0x01, 0x2a]));
      return buffer.readUInt16LE(start + 3) & 0x3fff;
    }
    return 0;
  }

  const arquivos = await walk(dir);
  assert.ok(arquivos.length > 0, "expected the team logo art folder to exist");

  const grandes = [];
  for (const arquivo of arquivos) {
    if (largura(await read(arquivo)) > 384) grandes.push(path.basename(arquivo));
  }

  assert.deepEqual(
    grandes,
    [],
    `expected every team logo to be at most 384px wide (the hero slot is 168 CSS px); oversized: ${grandes.join(", ")}`,
  );
});
