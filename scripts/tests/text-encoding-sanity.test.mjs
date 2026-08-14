// Mojibake no fonte: texto UTF-8 que alguém abriu como Latin-1 e regravou.
//
// A varredura cobre `src`, `src-tauri`, `.claude` e `scripts`. O `scripts/` entrou em
// 12/08/2026, e a ausência dele era um buraco de verdade: os geradores de pacote, o release e
// os próprios guards são escritos em português, passam por editor e por agente, e nada olhava
// o encoding deles. Junto veio o `.mjs`, que é a extensão em que quase todo `scripts/` está —
// sem ela, incluir o diretório não teria mudado nada.
//
// Os padrões e a isenção por linha moram em `scripts/lib/mojibake.mjs`, para o guard poder
// morder uma linha SINTÉTICA em vez de só provar que a árvore está limpa hoje.
import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readdir, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { acharMojibake, linhaSuspeita, MARCADOR_ISENCAO } from "../lib/mojibake.mjs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

const TARGET_DIRECTORIES = ["src", "src-tauri", ".claude", "scripts"];
const TEXT_EXTENSIONS = new Set([
  ".js",
  ".jsx",
  ".mjs",
  ".cjs",
  ".ts",
  ".tsx",
  ".rs",
  ".md",
  ".json",
]);
const SKIPPED_DIRECTORY_NAMES = new Set([
  ".git",
  ".git-backups",
  ".superpowers",
  // Checkouts paralelos de agentes. Sao outra copia do repo: escanea-los faz o
  // guard reclamar de arquivo que nao esta nesta arvore. O Claude Code cria em
  // `.claude/worktrees/`, sem ponto — e `.claude` E escaneado, entao sem esta
  // entrada o guard entra la dentro.
  ".worktrees",
  "worktrees",
  "dist",
  "gen",
  "node_modules",
  "target",
  "target-verify",
]);

/// Uma sequência de mojibake montada por código, e não digitada: escrever o par literal aqui
/// faria este arquivo se acusar na varredura, e a isenção por linha existe para o caso real,
/// não para o guard se poupar.
const c = (...bytes) => String.fromCharCode(...bytes);
const CEDILHA_QUEBRADA = c(0x00c3, 0x00a7); // "ç" em UTF-8 lido como Latin-1
const TRAVESSAO_QUEBRADO = c(0x00e2, 0x0080, 0x0094); // travessão, idem
const BOM_QUEBRADO = c(0x00ef, 0x00bb, 0x00bf);

test("encoding scan skips generated and dependency directories", () => {
  assert.equal(shouldSkipDirectory("src-tauri/target"), true);
  assert.equal(shouldSkipDirectory("src-tauri/target-verify"), true);
  assert.equal(shouldSkipDirectory("dist"), true);
  assert.equal(shouldSkipDirectory("node_modules"), true);
  assert.equal(shouldSkipDirectory(".claude/worktrees/algum-agente"), true);
  assert.equal(shouldSkipDirectory("src/pages"), false);
  assert.equal(shouldSkipDirectory("scripts/tests"), false);
});

function shouldSkipDirectory(relativeDir) {
  return relativeDir
    .split(/[\\/]/u)
    .some((part) => SKIPPED_DIRECTORY_NAMES.has(part));
}

async function collectFiles(relativeDir) {
  if (shouldSkipDirectory(relativeDir)) {
    return [];
  }

  const dir = path.join(projectRoot, relativeDir);
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch (error) {
    if (error?.code === "ENOENT") {
      return [];
    }
    throw error;
  }

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

/// Varre uma raiz qualquer com as MESMAS regras da varredura de verdade. Existe para o teste
/// sintético poder apontar o guard para um diretório temporário: escrever o arquivo com
/// mojibake dentro da árvore de verdade seria uma corrida com a varredura que roda ao lado.
async function varrer(raiz, diretorios) {
  const hits = [];
  for (const relativeDir of diretorios) {
    for (const relativeFile of await collectFilesEm(raiz, relativeDir)) {
      const fonte = await readFile(path.join(raiz, relativeFile), "utf8");
      for (const { linha, texto } of acharMojibake(fonte)) {
        hits.push(`${relativeFile}:${linha}: ${texto}`);
      }
    }
  }
  return hits;
}

async function collectFilesEm(raiz, relativeDir) {
  if (raiz === projectRoot) return collectFiles(relativeDir);
  const entries = await readdir(path.join(raiz, relativeDir), { withFileTypes: true });
  const files = await Promise.all(
    entries.map(async (entry) =>
      entry.isDirectory()
        ? collectFilesEm(raiz, path.join(relativeDir, entry.name))
        : TEXT_EXTENSIONS.has(path.extname(entry.name).toLowerCase())
          ? [path.join(relativeDir, entry.name)]
          : [],
    ),
  );
  return files.flat();
}

test("o detector morde as três formas de mojibake que já apareceram", () => {
  assert.equal(linhaSuspeita(`const t = "situa${CEDILHA_QUEBRADA}${c(0x00c3, 0x00a3)}o";`), true);
  assert.equal(linhaSuspeita(`// nota ${TRAVESSAO_QUEBRADO} e o resto da frase`), true);
  assert.equal(linhaSuspeita(`${BOM_QUEBRADO}import fs from "node:fs";`), true);
});

test("acentuação legítima e emoji legítimo NÃO acusam", () => {
  assert.equal(linhaSuspeita('const t = "situação, coração, ãõç";'), false);
  assert.equal(linhaSuspeita("// bandeira quadriculada: 🏁 e o resto"), false);
});

test("classe de maiúscula acentuada não é mojibake, e o padrão sabe disso", () => {
  // Era o falso positivo do guard: neste tipo de classe o A-til encosta no O-til, e a versão
  // larga do padrão aceitava qualquer byte alto depois do líder. Byte de continuação de UTF-8
  // é só 0x80-0xBF, então a sequência nem podia ter vindo de decodificação errada. Sem esta
  // correção, toda regex de nome próprio do repositório precisaria de isenção.
  assert.equal(linhaSuspeita("const CLASSE = /[A-ZÁÉÍÓÚÂÊÔÃÕÇ]/u;"), false);
  assert.equal(linhaSuspeita('const A = "áàâãäéèêëíìîïóòôõöúùûüçñÁÀÂÃÄÉÈÊËÍÌÎÏÓÒÔÕÖÚÙÛÜÇÑ";'), false);
});

test("a isenção é por LINHA, e só na linha que a pede", () => {
  // Nenhum arquivo da árvore precisa dela hoje, e é assim que tem de ser: a isenção existe
  // para o caso raro em que um texto correto imita a assinatura, e não para calar o guard num
  // arquivo inteiro. Testada aqui para não apodrecer sem consumidor.
  const linha = `const x = "${CEDILHA_QUEBRADA}";`;
  assert.equal(linhaSuspeita(linha), true);
  assert.equal(linhaSuspeita(`${linha} // ${MARCADOR_ISENCAO}: motivo escrito aqui`), false);
});

test("mojibake plantado num .mjs de scripts é pego pela varredura", async () => {
  const raiz = await mkdtemp(path.join(os.tmpdir(), "guard-encoding-"));
  try {
    await writeFile(
      path.join(raiz, "scripts-falso.mjs"),
      `// gerador de pacote\nconst rotulo = "corre${CEDILHA_QUEBRADA}${c(0x00c3, 0x00a3)}o";\n`,
      "utf8",
    );
    // Um arquivo de extensão fora da lista no MESMO diretório: prova que o corte é por
    // extensão e que o `.mjs` é o que faz `scripts/` valer alguma coisa.
    await writeFile(path.join(raiz, "ruido.wav"), `${CEDILHA_QUEBRADA}`, "utf8");

    const hits = await varrer(raiz, ["."]);
    assert.equal(hits.length, 1, `esperava um único achado, veio:\n${hits.join("\n")}`);
    assert.match(hits[0], /scripts-falso\.mjs:2:/);
  } finally {
    await rm(raiz, { recursive: true, force: true });
  }
});

test("a varredura de verdade alcança os scripts em .mjs", async () => {
  const arquivos = await collectFiles("scripts");
  assert.ok(
    arquivos.includes(path.join("scripts", "engenheiro-pack.mjs")),
    "o gerador do engenheiro ficou fora da varredura",
  );
  assert.ok(
    arquivos.includes(path.join("scripts", "lib", "cargo-target.mjs")),
    "os módulos de scripts/lib ficaram fora da varredura",
  );
  assert.ok(arquivos.length > 50, `só ${arquivos.length} arquivos em scripts/ — a varredura encolheu`);
});

test("key source files stay free from mojibake sequences", async () => {
  const hits = await varrer(projectRoot, TARGET_DIRECTORIES);

  assert.deepStrictEqual(
    hits,
    [],
    `expected source files to avoid mojibake sequences:\n${hits.join("\n")}`,
  );
});
