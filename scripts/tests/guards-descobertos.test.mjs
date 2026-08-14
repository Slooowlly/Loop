// A suíte de guards precisa ser DESCOBERTA, e a descoberta precisa falhar alto quando não
// descobre. Enquanto o `test:structure` era `node --test "scripts/tests/*.test.mjs"`, quem
// resolvia o asterisco dependia de onde o comando rodava: o bash expande antes, o `cmd.exe`
// do `npm run` no Windows não, e o resolvedor de glob do runner do Node só existe a partir do
// v21 — o CI estava no v20.
//
// O primeiro teste daqui é a prova de que o modo de falha é silencioso: glob sem casamento sai
// com código ZERO e zero testes. Os outros travam a forma que substituiu isso.
import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { listarGuards, PISO, DIR_GUARDS } from "../rodar-guards.mjs";

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const pkg = JSON.parse(readFileSync(join(RAIZ, "package.json"), "utf8"));
const ci = readFileSync(join(RAIZ, ".github", "workflows", "ci.yml"), "utf8");
const runner = readFileSync(join(RAIZ, "scripts", "rodar-guards.mjs"), "utf8");

test("glob sem casamento passa VERDE no node --test — é este o buraco", () => {
  // Sem o `NODE_TEST_CONTEXT` herdado: com ele, o filho se acha subprocesso deste runner e
  // troca de reporter, deixando a saída vazia. O que se quer medir é o que o CI veria.
  const env = { ...process.env };
  delete env.NODE_TEST_CONTEXT;
  const r = spawnSync(
    process.execPath,
    ["--test", join(DIR_GUARDS, "nao-existe-guard-nenhum-*.test.mjs")],
    { cwd: RAIZ, encoding: "utf8", env },
  );
  assert.equal(r.status, 0, "se um dia isto virar erro, o glob deixou de ser armadilha");
  assert.match(r.stdout, /tests 0/, "esperava zero testes executados");
  assert.doesNotMatch(r.stdout, /fail [1-9]/, "esperava zero falhas — o silêncio é o problema");
});

test("a descoberta é readdir, e acha todos os guards do diretório", () => {
  const achados = listarGuards();
  assert.ok(
    achados.includes(join(DIR_GUARDS, "guards-descobertos.test.mjs")),
    "o próprio arquivo deste teste ficou fora da descoberta",
  );
  assert.ok(
    achados.includes(join(DIR_GUARDS, "text-encoding-sanity.test.mjs")),
    "guard conhecido ficou fora da descoberta",
  );
  assert.equal(new Set(achados).size, achados.length, "a lista tem repetido");
});

test("o piso pega a suíte encolhendo", () => {
  const achados = listarGuards();
  assert.ok(
    achados.length >= PISO,
    `${achados.length} guards no disco contra um piso de ${PISO} — ou a varredura quebrou, ` +
      "ou alguém apagou guard sem baixar o piso no mesmo commit",
  );
  assert.ok(PISO > 0, "piso zero não trava nada");
});

test("o runner passa arquivo por arquivo, sem glob no meio", () => {
  assert.match(
    runner,
    /"--test",\s*\.\.\.alvos/,
    "o runner precisa entregar a lista explícita ao node --test",
  );
  assert.doesNotMatch(
    runner.replace(/^\s*\/\/.*$/gmu, ""),
    /\*\.test\.mjs/,
    "voltou um glob para dentro do runner",
  );
});

test("o package.json chama o runner, e nenhum script volta ao glob", () => {
  assert.match(
    pkg.scripts["test:structure"],
    /rodar-guards\.mjs/,
    "o test:structure precisa passar pelo runner que conta os guards",
  );
  for (const [nome, comando] of Object.entries(pkg.scripts)) {
    assert.doesNotMatch(
      comando,
      /node --test .*\*/u,
      `o script "${nome}" voltou a entregar glob ao node --test`,
    );
  }
});

test("o CI não roda a mesma suíte duas vezes por PR de branch interna", () => {
  // B105. Com `on: push:` sem filtro, o mesmo commit disparava dois runs num PR de branch
  // interna: um por `refs/heads/<branch>` e outro por `refs/pull/N/merge`. Refs diferentes,
  // então nem a concurrency cancelava. O `push` fica na main; o resto entra pelo PR.
  // O arquivo está em CRLF no disco: sem normalizar, todo `\n` desta leitura erra por um byte.
  const yaml = ci.replace(/\r\n/gu, "\n");
  const gatilhos = /^on:\n((?:[ \t].*\n|\n)*)/mu.exec(yaml)?.[1];
  assert.ok(gatilhos, "não achei o bloco `on:` do workflow");

  const push = /^ {2}push:\n((?:\s{4}.*\n)*)/mu.exec(gatilhos)?.[1];
  assert.ok(push, "o CI precisa continuar rodando em push");
  assert.match(push, /branches:/u, "o push voltou a valer para QUALQUER branch");
  assert.match(push, /-\s*main\b/u, "o push precisa continuar cobrindo a main");

  // Sem filtro de branch no pull_request: é ele que cobre branch interna E fork.
  assert.match(gatilhos, /^ {2}pull_request:\s*$/mu, "o gatilho de pull_request some ou ganhou filtro");
});

test("o Node do CI é o mesmo que o repositório exige", () => {
  const exigido = pkg.engines?.node;
  assert.ok(exigido, "o package.json precisa declarar engines.node");
  const piso = Number(/(\d+)/u.exec(exigido)?.[1]);

  const noCi = /node-version:\s*(\d+)/u.exec(ci);
  assert.ok(noCi, "o CI precisa fixar a maior do Node explicitamente");
  assert.ok(
    Number(noCi[1]) >= piso,
    `o CI está no Node ${noCi[1]} e o package.json exige >=${piso} — foi essa defasagem que ` +
      "quebrou o npm ci com o lock escrito por um npm mais novo",
  );
});
