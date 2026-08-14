// `temporada_inicio` e `temporada_fim` são colunas TEXT, e SQL que as trata como número
// precisa de `CAST(... AS INTEGER)`.
//
// As duas colunas nascem `TEXT NOT NULL` em `db/migrations/baseline.rs`, mesmo carregando
// número. Em TEXT o SQLite compara lexicograficamente, e aí `'10' < '9'` e `'26' < '9'`.
// O estrago é silencioso: o `ORDER BY temporada_fim DESC` que escolhe o "contrato mais
// recente" do jogador passa a devolver a temporada 9 assim que a carreira chega na 10.
//
// A afinidade contamina até o parâmetro ligado: coluna TEXT contra `?1` inteiro empurra o
// parâmetro para TEXT. Numa igualdade isso acerta enquanto os dois lados escreverem o
// número igual, e é só nisso que a igualdade sem CAST se apoia — `'09'` gravado no banco
// já não bate com o inteiro 9.
//
// Este guard lê o SQL como texto e cobra o CAST em toda leitura numérica das duas colunas:
// `ORDER BY`, `<`, `>`, `<=`, `>=`, `BETWEEN`, `MIN()` e `MAX()`, mais a igualdade. O que
// fica de fora, por não ser leitura numérica: lista de colunas de `SELECT`/`INSERT`, DDL e
// o lado esquerdo de um `UPDATE ... SET`.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const dirFontes = path.join(raiz, "src-tauri", "src");

const COLUNAS = /temporada_(?:inicio|fim)\b/g;

/// Todo `.rs` sob `src-tauri/src`, menos o que é só teste: fixture de teste monta e consulta
/// a tabela que quiser, inclusive para provar justamente o comportamento em TEXT.
function arquivosDeProducao(dir) {
  const achados = [];
  for (const entrada of fs.readdirSync(dir, { withFileTypes: true })) {
    const caminho = path.join(dir, entrada.name);
    if (entrada.isDirectory()) {
      if (entrada.name === "tests") continue;
      achados.push(...arquivosDeProducao(caminho));
      continue;
    }
    if (!entrada.name.endsWith(".rs")) continue;
    if (entrada.name.startsWith("tests_")) continue;
    achados.push(caminho);
  }
  return achados;
}

/// Corta o primeiro `#[cfg(test)]` em diante — mesma heurística dos outros guards daqui.
function semBlocoDeTeste(texto) {
  const marca = texto.indexOf("#[cfg(test)]");
  return marca === -1 ? texto : texto.slice(0, marca);
}

/// Só o conteúdo dos literais de string, que é onde o SQL mora. Fora deles, um
/// `contract.temporada_fim < season` é comparação de `i32` em Rust e está correta.
function literaisDeString(texto) {
  const encontrados = [];
  const padrao = /"(?:[^"\\]|\\[\s\S])*"/g;
  let achado;
  while ((achado = padrao.exec(texto)) !== null) {
    encontrados.push({ conteudo: achado[0], deslocamento: achado.index });
  }
  return encontrados;
}

function linhaDe(texto, indice) {
  return texto.slice(0, indice).split("\n").length;
}

/// Classifica uma ocorrência da coluna dentro de um literal. Devolve o motivo da reprovação,
/// ou `null` quando o uso é seguro.
function motivoDeReprovacao(literal, inicio, fim) {
  const antes = literal.slice(Math.max(0, inicio - 120), inicio);
  const depois = literal.slice(fim, fim + 40);

  // Já protegida: `CAST(temporada_fim AS INTEGER)`, com ou sem alias de tabela.
  if (/CAST\(\s*(?:\w+\.)?$/i.test(antes)) return null;

  // Lado esquerdo de atribuição: `UPDATE ... SET temporada_fim = ?1`. Escrita, não leitura.
  if (/\bSET\s+(?:\w+\.)?$/i.test(antes)) return null;

  if (/^\s*(?:<=|>=|<>|<|>)/.test(depois)) {
    return "desigualdade sem CAST";
  }
  if (/^\s*=[^=]/.test(depois)) {
    return "igualdade sem CAST";
  }
  if (/^\s+BETWEEN\b/i.test(depois)) {
    return "BETWEEN sem CAST";
  }
  if (/\b(?:MIN|MAX)\(\s*(?:\w+\.)?$/i.test(antes)) {
    return "MIN/MAX sem CAST";
  }
  // Dentro da cláusula de ordenação: nada entre o `ORDER BY` e a coluna fecha a cláusula.
  if (/\bORDER\s+BY\b(?:(?!\bLIMIT\b|\bSELECT\b)[\s\S])*$/i.test(antes)) {
    return "ORDER BY sem CAST";
  }
  return null;
}

function varrer() {
  const achados = [];
  for (const arquivo of arquivosDeProducao(dirFontes)) {
    const bruto = fs.readFileSync(arquivo, "utf8");
    const texto = semBlocoDeTeste(bruto);
    for (const { conteudo, deslocamento } of literaisDeString(texto)) {
      COLUNAS.lastIndex = 0;
      let ocorrencia;
      while ((ocorrencia = COLUNAS.exec(conteudo)) !== null) {
        const motivo = motivoDeReprovacao(
          conteudo,
          ocorrencia.index,
          ocorrencia.index + ocorrencia[0].length,
        );
        if (!motivo) continue;
        achados.push({
          arquivo: path.relative(raiz, arquivo).replaceAll("\\", "/"),
          linha: linhaDe(texto, deslocamento + ocorrencia.index),
          coluna: ocorrencia[0],
          motivo,
        });
      }
    }
  }
  return achados;
}

test("SQL que lê temporada_inicio/temporada_fim como número passa por CAST(... AS INTEGER)", () => {
  const achados = varrer();
  const relato = achados
    .map((a) => `  ${a.arquivo}:${a.linha} — ${a.coluna}: ${a.motivo}`)
    .join("\n");
  assert.equal(
    achados.length,
    0,
    `As duas colunas são TEXT: comparar ou ordenar sem CAST põe a temporada 9 na frente da 10.\n${relato}`,
  );
});

// O guard só vale se ele conseguir enxergar o problema. Os casos abaixo passam SQL sintético
// pela mesma classificação, para o dia em que a heurística for afrouxada por acidente.
test("o classificador reprova as formas numéricas sem CAST", () => {
  const casos = [
    "SELECT categoria FROM contracts WHERE piloto_id = ?1 ORDER BY temporada_fim DESC LIMIT 1",
    "SELECT id FROM contracts WHERE temporada_fim < ?1",
    "SELECT id FROM contracts WHERE temporada_inicio >= ?1",
    "SELECT id FROM contracts WHERE temporada_inicio = ?1",
    "SELECT MAX(temporada_fim) FROM contracts",
    "SELECT id FROM contracts WHERE temporada_inicio BETWEEN ?1 AND ?2",
  ];
  for (const sql of casos) {
    COLUNAS.lastIndex = 0;
    const ocorrencia = COLUNAS.exec(sql);
    const motivo = motivoDeReprovacao(
      sql,
      ocorrencia.index,
      ocorrencia.index + ocorrencia[0].length,
    );
    assert.ok(motivo, `deveria reprovar: ${sql}`);
  }
});

test("o classificador aprova o CAST, a lista de colunas, o DDL e o SET", () => {
  const casos = [
    "SELECT categoria FROM contracts ORDER BY CAST(temporada_fim AS INTEGER) DESC LIMIT 1",
    "SELECT id FROM contracts c WHERE CAST(c.temporada_inicio AS INTEGER) <= ?1",
    "SELECT MAX(CAST(temporada_fim AS INTEGER)) FROM contracts",
    "SELECT id, temporada_inicio, temporada_fim FROM contracts",
    "INSERT INTO contracts (id, temporada_inicio, temporada_fim) VALUES (?1, ?2, ?3)",
    "temporada_inicio  TEXT NOT NULL,",
    "UPDATE contracts SET temporada_fim = ?1 WHERE id = ?2",
  ];
  for (const sql of casos) {
    COLUNAS.lastIndex = 0;
    let ocorrencia;
    while ((ocorrencia = COLUNAS.exec(sql)) !== null) {
      const motivo = motivoDeReprovacao(
        sql,
        ocorrencia.index,
        ocorrencia.index + ocorrencia[0].length,
      );
      assert.equal(motivo, null, `não deveria reprovar: ${sql} (${motivo})`);
    }
  }
});
