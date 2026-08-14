// Nenhum comando de depuração sobrevive ao build de release.
//
// São cinco, e quatro deles escrevem no save do jogador: `debug_skip_to_season_finale`
// simula as etapas pendentes, `debug_prepare_market_scenario` rescinde o contrato e força
// classificação e fama por SQL cru, `debug_force_player_poach_offer` grava a oferta no
// plano de pré-temporada e `debug_stamp_player_championship` carimba a posição no arquivo
// da temporada. O quinto, `debug_poaching_auctions`, roda em transação com rollback.
//
// O portão anterior era um `if cfg!(debug_assertions)` no corpo da casca: o comando
// continuava REGISTRADO no `invoke_handler` do release e só recusava em tempo de execução.
// Hoje o gate é `#[cfg(debug_assertions)]` na cadeia inteira — módulo, lógica, casca e
// registro — e no release o nome não existe na ponte. `tauri::generate_handler!` aceita
// atributos externos em cada entrada e os repassa ao braço do `match` (ver
// `tauri-macros/src/command/handler.rs`), então o `cfg` na lista funciona.
//
// Este guard existe porque a regressão é silenciosa dos dois lados: tirar um `cfg` compila,
// passa no `cargo test` (que roda em debug, onde tudo está ligado) e só aparece num build
// assinado que já foi para a mão do jogador.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const ler = (p) => fs.readFileSync(path.join(raiz, p), "utf8");

const CFG = "#[cfg(debug_assertions)]";

// Os comandos de depuração que existem hoje. A lista é congelada de propósito: um comando
// novo tem de passar por aqui, e a passagem é a hora de conferir se ele também foi gateado.
const COMANDOS_DE_DEBUG = [
  "debug_force_player_poach_offer",
  "debug_poaching_auctions",
  "debug_prepare_market_scenario",
  "debug_skip_to_season_finale",
  "debug_stamp_player_championship",
];

/// O bloco de dentro do `generate_handler![...]`, em linhas.
function linhasDoHandler() {
  const lib = ler("src-tauri/src/lib.rs");
  const bloco = /invoke_handler\(tauri::generate_handler!\[([\s\S]*?)\n\s*\]\)/.exec(lib);
  assert.ok(bloco, "não achei o generate_handler! em src-tauri/src/lib.rs");
  return bloco[1].split("\n");
}

/// A linha de atributo/código anterior, pulando comentário e linha em branco. É assim que
/// o `cfg` se lê no arquivo: pode haver um bloco de comentário entre ele e a entrada.
function anteriorSignificativa(linhas, indice) {
  for (let i = indice - 1; i >= 0; i -= 1) {
    const linha = linhas[i].trim();
    if (linha === "" || linha.startsWith("//")) continue;
    return linha;
  }
  return "";
}

test("todo comando de depuração registrado no handler está atrás de cfg(debug_assertions)", () => {
  const linhas = linhasDoHandler();
  const registrados = [];
  const semGate = [];

  linhas.forEach((linha, i) => {
    const m = /^\s*(?:[a-z0-9_]+::)*([a-z0-9_]+),\s*$/.exec(linha);
    if (!m || !m[1].startsWith("debug_")) return;
    registrados.push(m[1]);
    if (anteriorSignificativa(linhas, i) !== CFG) semGate.push(m[1]);
  });

  assert.deepEqual(
    semGate,
    [],
    `comando(s) de depuração registrados SEM ${CFG} em src-tauri/src/lib.rs:\n  ${semGate.join(
      "\n  ",
    )}\n\nSem o atributo o comando fica invocável por devtools num build de release, e ` +
      `eles escrevem no save.`,
  );
  assert.deepEqual(
    registrados.sort(),
    [...COMANDOS_DE_DEBUG].sort(),
    "o inventário de comandos de depuração mudou — atualize COMANDOS_DE_DEBUG neste guard " +
      "depois de conferir que o comando novo também está gateado na cadeia inteira.",
  );
});

test("a casca #[tauri::command] de cada comando de depuração está atrás de cfg", () => {
  const texto = ler("src-tauri/src/commands/career_commands.rs");
  const linhas = texto.split("\n");
  const semGate = [];

  for (const nome of COMANDOS_DE_DEBUG) {
    const i = linhas.findIndex((l) => new RegExp(`^pub (async )?fn ${nome}\\b`).test(l.trim()));
    assert.notEqual(i, -1, `não achei a casca de ${nome} em career_commands.rs`);
    // Bloco de atributos imediatamente acima da assinatura.
    const atributos = [];
    for (let j = i - 1; j >= 0 && linhas[j].trim().startsWith("#["); j -= 1) {
      atributos.push(linhas[j].trim());
    }
    assert.ok(
      atributos.includes("#[tauri::command]"),
      `${nome} deixou de ser #[tauri::command] — este guard precisa ser revisto`,
    );
    if (!atributos.includes(CFG)) semGate.push(nome);
  }

  assert.deepEqual(
    semGate,
    [],
    `casca(s) de comando de depuração sem ${CFG} em career_commands.rs:\n  ${semGate.join("\n  ")}`,
  );
});

test("o módulo de depuração e seus exports saem do binário de release", () => {
  const career = ler("src-tauri/src/commands/career.rs");

  assert.match(
    career,
    new RegExp(`${escapar(CFG)}\\s*\\n#\\[path = "career/debug\\.rs"\\]\\s*\\nmod debug;`),
    `o módulo de depuração precisa de ${CFG} em commands/career.rs — sem isso a lógica que ` +
      "escreve SQL cru no save continua compilada no release, mesmo sem comando exposto.",
  );
  assert.match(
    career,
    new RegExp(`${escapar(CFG)}\\s*\\npub\\(crate\\) use debug::\\{`),
    `os exports de debug:: precisam de ${CFG} em commands/career.rs`,
  );
});

test("nenhum portão de depuração ficou só em tempo de execução", () => {
  // Só código: o comentário do portão CITA o gate antigo para explicar por que ele saiu.
  const codigo = ler("src-tauri/src/commands/career_commands.rs")
    .split("\n")
    .filter((l) => !l.trim().startsWith("//"))
    .join("\n");
  assert.doesNotMatch(
    codigo,
    /if cfg!\(debug_assertions\)/,
    "sobrou um gate de RUNTIME em career_commands.rs. `cfg!(...)` é uma expressão booleana: " +
      "o comando continua registrado no handler do release e só recusa quando chamado. " +
      "O gate correto é o atributo #[cfg(debug_assertions)], que apaga o comando da ponte.",
  );
});

function escapar(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
