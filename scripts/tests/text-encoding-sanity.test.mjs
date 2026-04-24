import test from "node:test";
import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

const TARGET_DIRECTORIES = ["src", "src-tauri", ".claude"];
const TEXT_EXTENSIONS = new Set([".js", ".jsx", ".ts", ".tsx", ".rs", ".md", ".json"]);
const SUSPICIOUS_PATTERNS = [
  /\u00c3[\u0080-\u00bf\u00a0-\u00ff]/u,
  /\u00c2[\u0080-\u00bf\u00a0-\u00ff]/u,
  /\u00e2[\u0080-\u00bf]/u,
  /\u00e2[\u2013\u2014\u2018-\u201e\u2020-\u2022\u2030\u2039\u203a\u0152\u0153\u0160\u0161\u0178\u017d\u017e\u02c6\u02dc\u2122]/u,
  /\u00f0\u0178[\u0080-\u00bf]/u,
  /\u00ef\u00bb\u00bf/u,
];

async function collectFiles(relativeDir) {
  const dir = path.join(projectRoot, relativeDir);
  const entries = await readdir(dir, { withFileTypes: true });
  const files = await Promise.all(
    entries.map(async (entry) => {
      const entryPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        return collectFiles(path.join(relativeDir, entry.name));
      }
      return TEXT_EXTENSIONS.has(path.extname(entry.name).toLowerCase())
        ? [path.join(relativeDir, entry.name)]
        : [];
    }),
  );
  return files.flat();
}

test("key source files stay free from mojibake sequences", async () => {
  const files = (await Promise.all(TARGET_DIRECTORIES.map((dir) => collectFiles(dir)))).flat();
  const hits = [];

  for (const relativeFile of files) {
    const source = await readFile(path.join(projectRoot, relativeFile), "utf8");
    const lines = source.split(/\r?\n/u);

    lines.forEach((line, index) => {
      if (SUSPICIOUS_PATTERNS.some((pattern) => pattern.test(line))) {
        hits.push(`${relativeFile}:${index + 1}: ${line}`);
      }
    });
  }

  assert.deepStrictEqual(
    hits,
    [],
    `expected source files to avoid mojibake sequences:\n${hits.join("\n")}`,
  );
});
