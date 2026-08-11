// A tabela `races` está morta, e este guard é o que a mantém assim.
//
// `races` e `calendar` descrevem o MESMO conceito — uma corrida de uma temporada. Por um
// tempo o código escreveu nas duas, e duas fontes de verdade para o mesmo fato é o tipo de
// coisa que só cobra a conta muito depois: alguém escreve uma consulta nova, escolhe a
// tabela errada, e o número sai plausível e errado.
//
// A escolha está feita: `calendar` é a fonte de verdade, `race_results.race_id` aponta para
// `calendar.id`, e o último `INSERT INTO races` de produção saiu em 05/05/2026. A tabela
// continua declarada na baseline porque o `DROP` apagaria dado de save em campo sem volta
// (ver o comentário em `db/migrations/baseline.rs`), mas nenhuma linha de produção pode
// voltar a tocá-la.
//
// O guard lê o código-fonte como texto e falha se `races` reaparecer FORA de: a declaração
// da baseline, fixtures de teste (arquivos e blocos `#[cfg(test)]`) e este próprio arquivo.
// Nome de tabela não é tipo, então nada além disto avisa que a escolha foi desfeita.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const fonteRust = path.join(raiz, "src-tauri", "src");

/// Todo `.rs` sob src-tauri/src, menos os que são só teste.
function arquivosDeProducao(dir) {
  const achados = [];
  for (const entrada of fs.readdirSync(dir, { withFileTypes: true })) {
    const caminho = path.join(dir, entrada.name);
    if (entrada.isDirectory()) {
      // Diretórios `tests/` inteiros são fixture por definição.
      if (entrada.name === "tests") continue;
      achados.push(...arquivosDeProducao(caminho));
      continue;
    }
    if (!entrada.name.endsWith(".rs")) continue;
    // Arquivos que são só teste, pelo nome ou pelo caminho.
    if (entrada.name.startsWith("tests_") || entrada.name.endsWith("_tests.rs")) continue;
    achados.push(caminho);
  }
  return achados;
}

/// Corta o primeiro `#[cfg(test)] mod ... { ... }` em diante.
///
/// Grosseiro de propósito: o bloco de teste é sempre o último do arquivo no padrão deste
/// repositório, então cortar dali para baixo tira exatamente as fixtures e nada de
/// produção. Errar para o lado de cortar demais só torna o guard mais silencioso, nunca
/// mais barulhento à toa.
function semBlocoDeTeste(texto) {
  const marca = texto.indexOf("#[cfg(test)]");
  return marca === -1 ? texto : texto.slice(0, marca);
}

/// Tira as linhas de comentário — de Rust (`//`) e de SQL (`--`).
///
/// Sem isto o guard acusa a si mesmo: o comentário da baseline que EXPLICA a decisão cita
/// o `INSERT INTO races` que foi removido.
function semComentarios(texto) {
  return texto
    .split("\n")
    .filter((linha) => {
      const limpa = linha.trimStart();
      return !limpa.startsWith("//") && !limpa.startsWith("--");
    })
    .join("\n");
}

/// As menções à tabela `races` — nome isolado, para não pegar `race_results` e afins.
function mencoesATabelaRaces(texto) {
  const padrao = /\b(?:FROM|INTO|UPDATE|JOIN|TABLE)\s+races\b/gi;
  return [...texto.matchAll(padrao)].map(([m]) => m.trim());
}

test("nenhum código de produção lê ou escreve na tabela morta `races`", () => {
  const baseline = path.join(fonteRust, "db", "migrations", "baseline.rs");
  const infratores = [];

  for (const arquivo of arquivosDeProducao(fonteRust)) {
    const texto = semComentarios(semBlocoDeTeste(fs.readFileSync(arquivo, "utf8")));
    let mencoes = mencoesATabelaRaces(texto);

    if (arquivo === baseline) {
      // A baseline declara a tabela — é o único lugar onde ela pode aparecer, e só como
      // declaração (`CREATE TABLE ... races`).
      mencoes = mencoes.filter((m) => !/^TABLE\s+races$/i.test(m));
    }

    if (mencoes.length > 0) {
      infratores.push(`${path.relative(raiz, arquivo)}: ${mencoes.join(", ")}`);
    }
  }

  assert.deepEqual(
    infratores,
    [],
    "a tabela `races` está morta e `calendar` é a fonte de verdade; consulta nova de corrida " +
      "usa `calendar`. Se a decisão mudou, mude o comentário da baseline junto:\n" +
      infratores.join("\n"),
  );
});

test("a baseline continua declarando `races` com o aviso de tabela morta", () => {
  const baseline = fs.readFileSync(
    path.join(fonteRust, "db", "migrations", "baseline.rs"),
    "utf8",
  );

  assert.ok(
    /CREATE TABLE IF NOT EXISTS races/.test(baseline),
    "`races` sumiu da baseline — se foi DROPada de propósito, este guard inteiro pode sair",
  );
  assert.ok(
    /`races` está MORTA/.test(baseline),
    "o aviso que explica por que `races` continua declarada sumiu da baseline",
  );
});
