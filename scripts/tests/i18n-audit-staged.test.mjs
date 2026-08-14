// Dois vãos do auditor de i18n, os dois no caminho que mais importa: o hook de pre-commit.
//
//  1. Ele lia o DISCO. O que vai ser commitado é o blob do índice, e os dois divergem toda vez
//     que alguém dá `git add` e continua editando, ou usa `git add -p`. O hook julgava um texto
//     que ninguém ia commitar.
//  2. Ele pulava a linha inteira quando via `t(`. Linha traduzida pela METADE é o caso mais
//     comum de todos — `<span>{t("titulos")}: {n} vitórias</span>` tem uma chave e uma palavra
//     crua —, e era justamente ela que passava batido.
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { baselineOrfa, mascararTraduzido, scanTexto } from "../i18nAudit.mjs";
import { BASELINE_JS } from "../i18nBaseline.mjs";

const AUDITOR = join(dirname(fileURLToPath(import.meta.url)), "..", "i18nAudit.mjs");

// ─────────────────────────── a máscara ───────────────────────────

test("a chamada de tradução some e a prosa crua fica", () => {
  const m = mascararTraduzido('        <span>{t("titulos")}: {n} vitórias</span>');
  assert.ok(!m.includes('t("titulos")'), "a chamada continua na linha");
  assert.ok(m.includes("vitórias"), "a prosa crua sumiu junto");
});

test("parêntese dentro do literal não fecha a chamada cedo", () => {
  const m = mascararTraduzido('<p>{t("Vencedor (provisório)")} sem tradução</p>');
  assert.ok(!m.includes("provisório"), "a chamada não foi mascarada inteira");
  assert.ok(m.includes("sem tradução"));
});

test("chamada com opções aninhadas é mascarada inteira", () => {
  const m = mascararTraduzido('<p>{t("chave", { n: contar(x) })}</p>');
  assert.ok(!m.includes("contar"), `sobrou pedaço da chamada: ${m}`);
});

test("i18n.t também conta, e uma linha sem tradução passa intacta", () => {
  assert.ok(!mascararTraduzido('const a = i18n.t("x");').includes('"x"'));
  const crua = "<p>Nenhuma equipe disponível</p>";
  assert.equal(mascararTraduzido(crua), crua);
});

test("identificador terminado em t não vira chamada de tradução", () => {
  const linha = "const y = format(x);";
  assert.equal(mascararTraduzido(linha), linha);
});

// ─────────────────────────── a varredura ───────────────────────────

test("linha traduzida pela metade é apontada", () => {
  const achados = scanTexto("src/x.jsx", '<span>{t("titulos")}: {n} vitórias</span>\n');
  assert.equal(achados.length, 1, JSON.stringify(achados));
  assert.match(achados[0].text, /vitórias/);
});

test("linha traduzida por inteiro continua limpa", () => {
  assert.deepEqual(scanTexto("src/x.jsx", '<span>{t("titulos")}: {t("vitorias", { n })}</span>\n'), []);
});

test("i18n-ignore continua valendo na linha meio traduzida", () => {
  const src = '{/* i18n-ignore */}\n<span>{t("titulos")}: {n} vitórias</span>\n';
  assert.deepEqual(scanTexto("src/x.jsx", src), []);
});

// ─────────────────────────── a varredura do .js ───────────────────────────
//
// No `.jsx` a copy tem tag em volta. No `.js` ela é o valor de retorno, o item do array, o
// rótulo do mapa — então lá o alvo é o LITERAL, e o trabalho todo é não confundir prosa com
// código. Os testes abaixo cobrem os dois lados: o que TEM de falhar e o que não pode virar
// falso positivo (comentário, chave de objeto, caminho de import, log de console).

test("prosa nova em .js é apontada", () => {
  const achados = scanTexto("src/x.js", 'export const rotulo = "Nenhuma equipe disponível";\n');
  assert.equal(achados.length, 1, JSON.stringify(achados));
  assert.match(achados[0].text, /Nenhuma equipe disponível/);
});

test("o que é código em .js não vira pendência", () => {
  const casos = [
    'export const rotulo = t("vazio");',
    "// comentário: nenhuma equipe disponível na temporada",
    'const mapa = { "corrida disponível": 1 };',
    'console.warn("Falha ao carregar a temporada:", e);',
    'import x from "./equipe-de-corrida";',
    'const classe = "linha-de-corrida";',
    'const arte = "Autódromo Nazionale Monza.jpg";',
  ];
  for (const linha of casos) {
    assert.deepEqual(scanTexto("src/x.js", `${linha}\n`), [], `falso positivo em: ${linha}`);
  }
});

test("o texto do template literal é normalizado: a interpolação some e o espaço colapsa", () => {
  // O baseline guarda o texto NORMALIZADO. Sem isso, renomear a variável dentro do template
  // mudaria a string e a entrada do baseline viraria órfã sem ninguém ter tocado na copy.
  const achados = scanTexto("src/x.js", "const s = `Volante ${a} · botão ${b}`;\n");
  assert.equal(achados.length, 1, JSON.stringify(achados));
  assert.equal(achados[0].text, "Volante · botão");
});

test("o baseline libera a FRASE, não o arquivo", () => {
  const arquivo = "src/utils/bestEffort.js";
  const congelada = BASELINE_JS[arquivo][0];

  assert.deepEqual(scanTexto(arquivo, `return "${congelada}";\n`), [], "a frase congelada bloqueou");
  const nova = scanTexto(arquivo, 'return "falha nova sem tradução";\n');
  assert.equal(nova.length, 1, "frase nova passou no arquivo que tem baseline");
});

test("o baseline só fala de .js dentro de src/", () => {
  // A varredura por literal é a do `.js`. Uma chave `.jsx` aqui seria lida pela varredura
  // errada e não liberaria nada — pior, daria a impressão de estar liberando.
  for (const caminho of Object.keys(BASELINE_JS)) {
    assert.ok(
      caminho.startsWith("src/") && caminho.endsWith(".js"),
      `entrada fora do escopo do baseline: ${caminho}`,
    );
  }
});

test("entrada de baseline que o auditor não acha mais é reportada como órfã", () => {
  assert.deepEqual(baselineOrfa(), [], "o baseline atual já tem entrada morta");

  BASELINE_JS["src/utils/bestEffort.js"].push("frase que nunca existiu neste arquivo");
  try {
    const orfas = baselineOrfa();
    assert.equal(orfas.length, 1, JSON.stringify(orfas));
    assert.equal(orfas[0].text, "frase que nunca existiu neste arquivo");
  } finally {
    BASELINE_JS["src/utils/bestEffort.js"].pop();
  }
});

// ─────────────────────────── o modo --staged ───────────────────────────

/// Um repositório de mentira, para o auditor rodar com o `--staged` de verdade. O `ROOT` dele
/// sai do cwd do processo, então basta rodar o CLI de dentro daqui.
function repoDeTeste() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "loop-i18n-"));
  const git = (...args) => execFileSync("git", args, { cwd: dir, stdio: "pipe" });
  git("init", "-q");
  fs.writeFileSync(path.join(dir, "package.json"), '{ "name": "falso", "version": "0.0.0" }\n');
  fs.mkdirSync(path.join(dir, "src"));
  return { dir, git };
}

function auditar(dir) {
  const r = execFileSync("node", [AUDITOR, "--staged"], {
    cwd: dir,
    encoding: "utf8",
    stdio: "pipe",
    // O CLI sai com 1 quando acha pendência; o teste quer a saída, não a exceção.
  });
  return { saida: r, codigo: 0 };
}

function auditarEsperandoFalha(dir) {
  try {
    auditar(dir);
    return null;
  } catch (e) {
    return `${e.stdout ?? ""}${e.stderr ?? ""}`;
  }
}

test("o --staged julga o ÍNDICE, não o disco: crua no índice é bloqueada", () => {
  const { dir, git } = repoDeTeste();
  const alvo = path.join(dir, "src", "Tela.jsx");
  fs.writeFileSync(alvo, "export default () => <p>Nenhuma equipe disponível</p>;\n");
  git("add", "src/Tela.jsx");
  // Depois do add, o disco fica limpo. O que vai ser commitado continua sujo.
  fs.writeFileSync(alvo, 'export default () => <p>{t("vazio")}</p>;\n');

  const saida = auditarEsperandoFalha(dir);
  assert.ok(saida, "o auditor passou lendo o disco em vez do índice");
  assert.match(saida, /src\/Tela\.jsx/);
});

test("o --staged também julga o ÍNDICE do .js", () => {
  const { dir, git } = repoDeTeste();
  const alvo = path.join(dir, "src", "rotulos.js");
  fs.writeFileSync(alvo, 'export const vazio = "Nenhuma equipe disponível";\n');
  git("add", "src/rotulos.js");
  // Mesmo jogo do .jsx: o disco já está limpo, e o que vai ser commitado continua sujo.
  fs.writeFileSync(alvo, 'export const vazio = t("vazio");\n');

  const saida = auditarEsperandoFalha(dir);
  assert.ok(saida, "o .js no stage não foi varrido");
  assert.match(saida, /src\/rotulos\.js/);
});

test("o --staged julga o ÍNDICE, não o disco: crua só no disco não bloqueia", () => {
  const { dir, git } = repoDeTeste();
  const alvo = path.join(dir, "src", "Tela.jsx");
  fs.writeFileSync(alvo, 'export default () => <p>{t("vazio")}</p>;\n');
  git("add", "src/Tela.jsx");
  // Edição posterior, ainda sendo escrita e fora do commit.
  fs.writeFileSync(alvo, "export default () => <p>Rascunho de equipe</p>;\n");

  assert.equal(auditar(dir).codigo, 0, "bloqueou o commit por texto que não está no índice");
});
