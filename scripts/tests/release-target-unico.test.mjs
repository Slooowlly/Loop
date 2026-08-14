// Um target-dir só. Já foram três — o do .cargo/config.toml, o `C:/dev/loop-target` cravado
// no release e o `src-tauri/target` que o cargo abre sozinho quando é chamado da raiz com
// `--manifest-path`. Cada um é um cache completo do crate, e o release recompilava do zero
// o que o desenvolvimento já tinha compilado.
//
// Estes guards travam a política: CARGO_TARGET_DIR quando explícito, senão o que o cargo
// resolve de dentro do crate. Nenhum caminho de máquina escrito no código.
import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { bundleNsisPadrao, resolverTargetDir } from "../lib/cargo-target.mjs";

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const src = readFileSync(join(RAIZ, "scripts", "release.mjs"), "utf8");
const manifesto = readFileSync(join(RAIZ, "scripts", "make-update-manifest.mjs"), "utf8");
const ci = readFileSync(join(RAIZ, ".github", "workflows", "ci.yml"), "utf8");
const claude = readFileSync(join(RAIZ, "CLAUDE.md"), "utf8");

/// Dublê do `cargo metadata`: o guard não pode depender de ter o toolchain de Rust instalado.
const metadataFake = (targetDirectory) => () => ({ ok: true, targetDirectory });

/// Os guards de texto olham CÓDIGO, e não a prosa que explica por que a forma errada é
/// errada — o comentário que cita `--manifest-path` para proibi-lo é justamente o que
/// mantém a regra viva.
const semComentarios = (texto, marca) =>
  texto
    .split("\n")
    .filter((l) => !l.trimStart().startsWith(marca))
    .join("\n");

/// Só o que está dentro de bloco de código do markdown: é isso que alguém copia e cola.
const blocosDeCodigo = (md) =>
  md
    .split("```")
    .filter((_, i) => i % 2 === 1)
    .join("\n");

test("CARGO_TARGET_DIR explícito vence, e o cargo nem é consultado", () => {
  let consultou = false;
  const r = resolverTargetDir({
    env: { CARGO_TARGET_DIR: "C:/ci/target" },
    metadata: () => {
      consultou = true;
      return { ok: true, targetDirectory: "C:/outro" };
    },
  });
  assert.equal(r.erro, undefined);
  assert.equal(r.caminho, resolve("C:/ci/target"));
  assert.equal(r.origem, "CARGO_TARGET_DIR");
  assert.equal(consultou, false, "consultou o cargo mesmo com a variável explícita");
});

test("sem a variável, vale o que o cargo resolve de dentro do crate", () => {
  const r = resolverTargetDir({
    dirCrate: "src-tauri",
    env: {},
    metadata: metadataFake("C:/cargo-target/iracer"),
  });
  assert.equal(r.caminho, resolve("C:/cargo-target/iracer"));
  assert.match(r.origem, /src-tauri.\.cargo.config\.toml/);
});

test("variável definida e vazia não conta como explícita", () => {
  // `set CARGO_TARGET_DIR=` deixa a variável presente e vazia. Tratar isso como explícito
  // mandaria o build para a raiz do disco.
  const r = resolverTargetDir({ env: { CARGO_TARGET_DIR: "   " }, metadata: metadataFake("C:/x") });
  assert.equal(r.caminho, resolve("C:/x"));
});

test("cargo indisponível vira erro com instrução, e não exceção", () => {
  const r = resolverTargetDir({
    env: {},
    metadata: () => ({ ok: false, motivo: "cargo não encontrado no PATH" }),
  });
  assert.equal(r.caminho, undefined);
  assert.match(r.erro, /cargo não encontrado no PATH/);
  assert.match(r.erro, /CARGO_TARGET_DIR/);
});

test("o bundle padrão sai da política, e não de um caminho escrito no código", () => {
  const r = bundleNsisPadrao({ env: {}, metadata: metadataFake("C:/cargo-target/iracer") });
  assert.equal(r.erro, undefined);
  assert.equal(r.caminho, join(resolve("C:/cargo-target/iracer"), "release", "bundle", "nsis"));

  const comVariavel = bundleNsisPadrao({
    env: { CARGO_TARGET_DIR: "D:/a/Loop/Loop/src-tauri/target" },
    metadata: () => assert.fail("consultou o cargo com a variável explícita"),
  });
  assert.equal(comVariavel.origem, "CARGO_TARGET_DIR");
  assert.match(comVariavel.caminho, /release.bundle.nsis$/);
});

test("bundle velho no target ACIDENTAL nunca é escolhido", () => {
  // O target acidental é o `src-tauri/target` que o cargo abre sozinho quando é chamado da
  // raiz com `--manifest-path`. Nenhum build de release o alimenta, e o instalador que sobra
  // lá é de algum experimento antigo — o pior arquivo possível para um manifesto apontar,
  // porque ele existe, tem `.sig` ao lado e passa por bom.
  const tmp = mkdtempSync(join(os.tmpdir(), "guard-target-"));
  try {
    const acidental = join(tmp, "src-tauri", "target", "release", "bundle", "nsis");
    const daPolitica = join(tmp, "cargo-target", "iracer", "release", "bundle", "nsis");
    mkdirSync(acidental, { recursive: true });
    mkdirSync(daPolitica, { recursive: true });
    writeFileSync(join(acidental, "Loop_0.9.0_x64-setup.exe"), "instalador velho");
    writeFileSync(join(acidental, "Loop_0.9.0_x64-setup.exe.sig"), "assinatura velha");

    const r = bundleNsisPadrao({
      env: {},
      metadata: metadataFake(join(tmp, "cargo-target", "iracer")),
    });
    assert.equal(r.caminho, daPolitica);
    assert.notEqual(r.caminho, acidental);
    assert.doesNotMatch(r.caminho, /src-tauri.target/, "o padrão caiu no target acidental");
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
});

test("sem target resolvido o manifesto morre, em vez de cair num caminho relativo", () => {
  const r = bundleNsisPadrao({ env: {}, metadata: () => ({ ok: false, motivo: "cargo ausente" }) });
  assert.equal(r.caminho, undefined);
  assert.match(r.erro, /cargo ausente/);

  const codigo = semComentarios(manifesto, "//");
  assert.doesNotMatch(
    codigo,
    /src-tauri.target/,
    "voltou o caminho do target acidental cravado no make-update-manifest",
  );
  assert.match(
    codigo,
    /bundleNsisPadrao/,
    "o make-update-manifest precisa herdar a política única de target-dir",
  );
});

test("nenhum caminho de máquina sobrou nos scripts", () => {
  const codigo = semComentarios(src, "//");
  assert.doesNotMatch(codigo, /loop-target/, "voltou o target hardcoded do release");
  assert.doesNotMatch(
    codigo,
    /[A-Za-z]:[\\/](dev|cargo-target)/,
    "voltou um caminho absoluto de máquina no release",
  );
  assert.doesNotMatch(
    codigo,
    /CARGO_TARGET_DIR\s*[:=]/,
    "o release voltou a forçar CARGO_TARGET_DIR em vez de herdar a política",
  );
});

test("o release chama o cargo de dentro do crate, sem --manifest-path", () => {
  const chamada = src.slice(src.indexOf('run("cargo"'), src.indexOf('run("cargo"') + 200);
  assert.ok(chamada.startsWith('run("cargo"'), "sumiu a chamada do cargo test");
  assert.doesNotMatch(chamada, /manifest-path/, "da raiz o cargo ignora o .cargo/config.toml");
  assert.match(chamada, /cwd: CRATE_DIR/, "o cargo test precisa rodar com o cwd no crate");
});

test("a documentação e o CI não ensinam o comando que abre um target novo", () => {
  assert.doesNotMatch(
    blocosDeCodigo(claude),
    /cargo (test|build|clippy)[^\n]*--manifest-path/,
    "CLAUDE.md voltou a documentar o cargo da raiz com --manifest-path",
  );
  assert.match(
    blocosDeCodigo(claude),
    /cd src-tauri[^\n]*cargo test/,
    "CLAUDE.md precisa documentar o cargo test rodando de dentro do crate",
  );
  assert.doesNotMatch(
    semComentarios(ci, "#"),
    /cargo (test|build|clippy)[^\n]*--manifest-path/,
    "o CI voltou a rodar o cargo da raiz com --manifest-path",
  );
  assert.match(ci, /working-directory: src-tauri/, "o CI precisa rodar o cargo de dentro do crate");
  // No CI a variável explícita é a política, e o cache do Swatinem depende dela.
  assert.match(ci, /CARGO_TARGET_DIR:/, "o CI precisa continuar fixando CARGO_TARGET_DIR");
});
