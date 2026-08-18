// Teto de `.unwrap()` / `.expect(` no Rust que entra no build do jogador.
//
// ## O que este guard protege
//
// `unwrap`/`expect` que falha vira PÂNICO, e pânico não passa pelo `diagnostico_registrar`:
// não sobra linha no `loop.log`, que é o arquivo que o jogador sabe anexar. É a mesma dor que
// o `catch-vazio-no-caminho-de-corrida` ataca do lado JS, com o agravante de que aqui a
// thread morre. O caminho certo é propagar `Result` e deixar a casca do comando traduzir.
//
// A árvore hoje está disciplinada: os `unwrap` que sobraram no caminho de produção têm guarda
// imediatamente acima (`if pts.len() < 2 { return None }`, `is_finite()` filtrado antes do
// sort, `else` de um `is_none()`), ou são leitura de tabela de constante, `lock()` de mutex e
// `position()` em enum exaustivo. O guard existe para que isso continue sendo verdade por
// decisão, e não por sorte: `unwrap` novo em produção estoura o teto e obriga a subir o número
// à mão, no mesmo commit, com o motivo escrito.
//
// ## A armadilha de medição, que é o motivo deste arquivo existir
//
// Contar `unwrap` com `grep -v` em nome de arquivo dá um número ERRADO POR 17x. Medido em
// 18/08/2026: a varredura ingênua devolveu 1797, e o número real é 37. As três fugas:
//
//   1. `#[cfg(test)] mod tests { ... }` NO RODAPÉ do arquivo de produção. É onde moram quase
//      6 mil ocorrências. O arquivo se chama `market_proposals.rs` e passa em qualquer filtro
//      por nome, e as suas funções públicas devolvem `Result<_, DbError>` sem um `unwrap`.
//   2. `#[cfg(all(test, windows))]`, que uma regex de `#[cfg(test)]` literal não pega. Vive em
//      `commands/vr_layer.rs`.
//   3. `#[cfg(test)] mod X;` na DECLARAÇÃO do módulo, que tira do build do jogador a subárvore
//      inteira sem que nenhum arquivo dela tenha nome de teste. São cinco: `sim_stats` (o
//      harness de medição, 44 ocorrências), `car::breakdown_sim` (o Monte Carlo de quebra),
//      `public_presence::medicao`, `db::migrations::schema_ouro` e `engenheiro::medicao_radio`.
//
// A fuga 3 é a razão de o alcance ser DERIVADO em vez de uma lista mantida à mão. Harness de
// medição novo entra atrás de `#[cfg(test)]` e sai do alcance sozinho; harness que perder o
// gate entra no alcance e estoura o teto, que é exatamente o aviso que se quer.
//
// ## Fora do alcance de propósito
//
// `panic!`, `unreachable!` e `todo!` não entram. São escritos de propósito e lidos como tal;
// o modo de falha que este guard persegue é o `unwrap` que alguém achou que não podia falhar.
//
// `unwrap_or`, `unwrap_or_else` e `unwrap_or_default` não entram porque não entram em pânico.
// A regex exige `()` colado, então eles ficam de fora sem exceção escrita.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const ALCANCE = "src-tauri/src";

/// Quantas ocorrências existem hoje no Rust que entra no build do jogador. Medido em
/// 18/08/2026, quando o guard entrou.
///
/// SUBIR é decisão consciente: um `unwrap` novo em produção precisa do motivo no assunto do
/// commit. BAIXAR é mecânico e bem-vindo, e o teste `o teto não ficou defasado` avisa quando a
/// folga passa de 5.
const TETO = 35;

/// O par que entra em pânico. `unwrap_or*` fica de fora porque exige `()` colado.
const PANICO = /\.unwrap\(\)|\.expect\(/g;

/// `#[cfg(test)]` e `#[cfg(all(test, ...))]`, nas duas posições em que aparecem: em cima de um
/// `mod tests { ... }` inline e em cima de um `mod X;` que declara subárvore.
const CFG_TESTE = /^\s*#\[cfg\(\s*(?:test\b|all\s*\(\s*test\b)/;

/// Todo `.rs` sob o alcance, em caminho relativo à raiz e com barra normal.
function todosOsFontes() {
  const achados = [];
  const varrer = (relativo) => {
    for (const item of fs.readdirSync(path.join(raiz, relativo), { withFileTypes: true })) {
      const rel = `${relativo}/${item.name}`;
      if (item.isDirectory()) varrer(rel);
      else if (item.name.endsWith(".rs")) achados.push(rel);
    }
  };
  varrer(ALCANCE);
  return achados;
}

/// Marca as linhas que caem dentro de um bloco `#[cfg(test)] ... { }`, contando chaves.
/// É a fuga 1 do cabeçalho, e vale por arquivo.
function linhasDeTeste(linhas) {
  const dentro = new Array(linhas.length).fill(false);
  for (let i = 0; i < linhas.length; i += 1) {
    if (!CFG_TESTE.test(linhas[i])) continue;
    let profundidade = 0;
    let abriu = false;
    let j = i;
    for (; j < linhas.length; j += 1) {
      profundidade +=
        (linhas[j].match(/\{/g) || []).length - (linhas[j].match(/\}/g) || []).length;
      if (linhas[j].includes("{")) abriu = true;
      dentro[j] = true;
      if (abriu && profundidade <= 0) break;
      // `mod X;` sem chave: a marcação vale só para esta linha e a do atributo.
      if (!abriu && /^\s*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+[\w]+\s*;/.test(linhas[j])) break;
    }
    i = j;
  }
  return dentro;
}

/// O diretório em que um `mod X;` declarado dentro de `arquivo` procura o filho.
/// `a/b/mod.rs` e `lib.rs` declaram irmãos do próprio diretório; `a/b.rs` declara em `a/b/`.
function diretorioDeModulo(arquivo) {
  const base = path.posix.basename(arquivo);
  const dir = path.posix.dirname(arquivo);
  if (base === "mod.rs" || base === "lib.rs" || base === "main.rs") return dir;
  return `${dir}/${base.slice(0, -3)}`;
}

/// Os prefixos de caminho que o build do jogador NÃO compila, derivados dos `#[cfg(test)]
/// mod X;` espalhados pela árvore. É a fuga 3 do cabeçalho.
function subarvoresDeTeste(fontes) {
  const fora = [];
  for (const arquivo of fontes) {
    const linhas = fs.readFileSync(path.join(raiz, arquivo), "utf8").split(/\r?\n/);
    linhas.forEach((linha, i) => {
      if (!CFG_TESTE.test(linha)) return;
      const seguinte = linhas[i + 1] ?? "";
      const m = seguinte.match(/^\s*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+([\w]+)\s*;/);
      if (!m) return;
      const base = `${diretorioDeModulo(arquivo)}/${m[1]}`;
      fora.push(`${base}.rs`, `${base}/`);
    });
  }
  return fora;
}

/// Arquivo cujo nome já o declara teste. Cobre `career/tests/lifecycle.rs`,
/// `queries/tests_projecoes.rs` e `foo_test.rs`.
function nomeDeTeste(arquivo) {
  const base = path.posix.basename(arquivo);
  return (
    arquivo.includes("/tests/") ||
    base === "tests.rs" ||
    base.startsWith("tests_") ||
    base.includes("_test")
  );
}

/// A contagem por arquivo do que entra no build do jogador.
function contarProducao() {
  const fontes = todosOsFontes();
  const fora = subarvoresDeTeste(fontes);
  const porArquivo = new Map();
  let total = 0;

  for (const arquivo of fontes) {
    if (nomeDeTeste(arquivo)) continue;
    if (fora.some((p) => (p.endsWith("/") ? arquivo.startsWith(p) : arquivo === p))) continue;

    const linhas = fs.readFileSync(path.join(raiz, arquivo), "utf8").split(/\r?\n/);
    const dentro = linhasDeTeste(linhas);
    let n = 0;
    linhas.forEach((linha, i) => {
      if (dentro[i]) return;
      n += (linha.match(PANICO) || []).length;
    });
    if (n > 0) {
      porArquivo.set(arquivo, n);
      total += n;
    }
  }
  return { total, porArquivo, varridos: fontes.length, fora };
}

test("o Rust de produção não passa do teto de unwrap/expect", () => {
  const { total, porArquivo } = contarProducao();
  const detalhe = [...porArquivo.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([f, n]) => `  ${String(n).padStart(3)}  ${f}`);

  assert.ok(
    total <= TETO,
    [
      `${total} ocorrências de .unwrap()/.expect( no Rust de produção, e o teto é ${TETO}.`,
      "",
      ...detalhe,
      "",
      "Pânico em produção mata a thread e NÃO deixa linha no loop.log, então a falha chega",
      "ao suporte como “não funciona”. Propague `Result` e deixe a casca do comando traduzir.",
      "",
      "Quando o unwrap novo é provadamente seguro (guarda imediatamente acima, enum exaustivo,",
      `tabela de constante), suba o TETO para ${total} em scripts/tests/${path.posix.basename(
        "unwrap-no-caminho-de-producao.test.mjs",
      )}`,
      "no mesmo commit, com o motivo no assunto.",
    ].join("\n"),
  );
});

test("o teto não ficou defasado", () => {
  const { total } = contarProducao();
  assert.ok(
    TETO - total <= 5,
    `o teto é ${TETO} e a árvore tem ${total}: baixe o TETO para ${total} e feche a folga.`,
  );
});

// Um guard que não enxerga nada passa verde para sempre. Os três casos abaixo provam que a
// varredura lê arquivo de verdade, que o gate de módulo é resolvido e que o descascador de
// bloco funciona — as três fugas do cabeçalho, uma a uma.

test("a varredura enxerga a árvore Rust inteira", () => {
  const { varridos } = contarProducao();
  assert.ok(varridos >= 400, `só ${varridos} arquivos .rs varridos — a descoberta furou`);
});

test("o gate de módulo tira o harness de medição do alcance", () => {
  const { fora, porArquivo } = contarProducao();
  // `#[cfg(test)] mod sim_stats;` no lib.rs. Se este par sair do ar, os 44 unwrap do harness
  // entram no alcance e o teto estoura sem nenhum unwrap novo ter sido escrito.
  assert.ok(
    fora.includes("src-tauri/src/sim_stats/"),
    "sim_stats/ precisa ser resolvido como subárvore de teste (é `#[cfg(test)] mod sim_stats;`)",
  );
  for (const gated of [
    "src-tauri/src/sim_stats/snapshots.rs",
    "src-tauri/src/public_presence/medicao.rs",
    "src-tauri/src/db/migrations/schema_ouro.rs",
    "src-tauri/src/engenheiro/medicao_radio.rs",
  ]) {
    assert.ok(!porArquivo.has(gated), `${gated} está atrás de #[cfg(test)] e não deveria contar`);
  }
});

test("o descascador reconhece as duas formas de #[cfg(test)]", () => {
  for (const amostra of ["#[cfg(test)]", "    #[cfg(all(test, windows))]", "#[cfg(all(test,unix))]"]) {
    assert.ok(CFG_TESTE.test(amostra), `deveria pegar: ${amostra}`);
  }
  for (const amostra of ["#[cfg(windows)]", "#[cfg(debug_assertions)]", "#[cfg(not(test))]"]) {
    assert.ok(!CFG_TESTE.test(amostra), `não deveria pegar: ${amostra}`);
  }

  const arquivo = [
    "fn producao() { a.unwrap(); }",
    "#[cfg(test)]",
    "mod tests {",
    "    fn caso() { b.unwrap(); }",
    "}",
    "fn depois() { c.unwrap(); }",
  ];
  const dentro = linhasDeTeste(arquivo);
  assert.deepEqual(
    arquivo.filter((_, i) => !dentro[i]),
    ["fn producao() { a.unwrap(); }", "fn depois() { c.unwrap(); }"],
    "o bloco inline de teste tem de sair, e o código depois dele tem de voltar",
  );
});

test("o resolvedor de módulo acha o filho nos dois formatos de pai", () => {
  assert.equal(diretorioDeModulo("src-tauri/src/lib.rs"), "src-tauri/src");
  assert.equal(diretorioDeModulo("src-tauri/src/engenheiro.rs"), "src-tauri/src/engenheiro");
  assert.equal(diretorioDeModulo("src-tauri/src/public_presence/mod.rs"), "src-tauri/src/public_presence");
});
