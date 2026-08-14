// Schema permanente nasce nas migrações, nunca em `db/queries/`.
//
// Nove tabelas do Loop nasceram fora das migrações: a camada de query criava a sua própria
// tabela com um `ensure_table` de `CREATE TABLE IF NOT EXISTS` no começo de cada função.
// Funciona, e é justamente por isso que se espalhou. O que se perde:
//
//   1. o schema-ouro não enxerga a tabela, então ela muda de forma sem nenhum teste acusar;
//   2. o DDL vive em duas cópias textuais (a da query e a da baseline) e as duas divergem
//      em silêncio — foi assim que `team_car.unit_seed` passou a existir de um lado só;
//   3. `ALTER TABLE` guardado por PRAGMA dentro do `ensure_table` faz mutação de schema
//      fora do caminho versionado, em ordem imprevisível.
//
// A v62 puxou as nove para dentro das migrações e deixou uma regra: o DDL mora numa
// constante `pub(crate) const DDL_*` por módulo de query, e a migração executa EXATAMENTE
// essa constante. Sobra UM motivo legítimo para o `ensure_table` continuar existindo, e ele
// está escrito no cabeçalho de cada módulo: as conexões de teste in-memory não rodam
// migração, e reaplicar o mesmo DDL de forma idempotente é o que as segura.
//
// Este guard lê o código como texto e prende as três coisas que sustentam a regra: o DDL só
// existe dentro da constante, a constante é executada pela migração, e a camada de query não
// muta schema.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const dirQueries = path.join(raiz, "src-tauri", "src", "db", "queries");
const arquivoMigracoes = path.join(raiz, "src-tauri", "src", "db", "migrations.rs");

/// Todo `.rs` sob `db/queries`, menos os que são só teste (diretórios `tests/` e arquivos
/// `tests_*.rs`): fixture de teste pode montar a tabela que quiser à mão.
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

/// Corta o primeiro `#[cfg(test)]` em diante — mesma heurística dos outros guards deste
/// diretório: o bloco de teste é o último do arquivo no padrão do repositório.
function semBlocoDeTeste(texto) {
  const marca = texto.indexOf("#[cfg(test)]");
  return marca === -1 ? texto : texto.slice(0, marca);
}

function semComentarios(texto) {
  return texto
    .split("\n")
    .filter((linha) => {
      const limpa = linha.trimStart();
      return !limpa.startsWith("//") && !limpa.startsWith("--");
    })
    .join("\n");
}

/// Os nomes das constantes `DDL_*` declaradas no arquivo.
function constantesDdl(texto) {
  return [...texto.matchAll(/const\s+(DDL_[A-Z0-9_]+)\s*:\s*&str\s*=/g)].map(([, nome]) => nome);
}

/// As tabelas criadas no arquivo, com o nome da constante em que cada `CREATE` está — ou
/// `null` quando o `CREATE` está solto, fora de qualquer constante.
function tabelasCriadas(texto) {
  const achados = [];
  const padrao = /CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?([A-Za-z0-9_]+)/gi;
  const constantes = [...texto.matchAll(/const\s+(DDL_[A-Z0-9_]+)\s*:\s*&str\s*=\s*"/g)];

  for (const encontro of texto.matchAll(padrao)) {
    const posicao = encontro.index;
    // A constante que "contém" o CREATE é a última declarada antes dele cujo literal de
    // string ainda não fechou. Fecho do literal = a próxima `";` depois da abertura.
    let dona = null;
    for (const c of constantes) {
      const abre = c.index + c[0].length;
      if (abre > posicao) break;
      const fecha = texto.indexOf('";', abre);
      if (fecha === -1 || fecha > posicao) dona = c[1];
    }
    achados.push({ tabela: encontro[1], constante: dona });
  }
  return achados;
}

test("todo CREATE TABLE de `db/queries` mora numa constante DDL_*", () => {
  const soltos = [];
  for (const arquivo of arquivosDeProducao(dirQueries)) {
    const texto = semComentarios(semBlocoDeTeste(fs.readFileSync(arquivo, "utf8")));
    for (const { tabela, constante } of tabelasCriadas(texto)) {
      if (constante === null) {
        soltos.push(`${path.relative(raiz, arquivo)}: CREATE TABLE ${tabela}`);
      }
    }
  }

  assert.deepEqual(
    soltos,
    [],
    "DDL solto na camada de query: o schema fica invisível para o schema-ouro e a cópia " +
      "textual divergiu antes (`team_car.unit_seed`). Ponha o DDL numa `pub(crate) const " +
      "DDL_*` e execute essa MESMA constante na migração:\n" + soltos.join("\n"),
  );
});

test("toda constante DDL_* de `db/queries` é executada por uma migração", () => {
  const migracoes = fs.readFileSync(arquivoMigracoes, "utf8");
  const orfas = [];

  for (const arquivo of arquivosDeProducao(dirQueries)) {
    const texto = semComentarios(semBlocoDeTeste(fs.readFileSync(arquivo, "utf8")));
    for (const nome of constantesDdl(texto)) {
      // A migração referencia por caminho (`queries::team_car::DDL_TEAM_CAR`); basta o
      // nome da constante aparecer em `migrations.rs`.
      if (!migracoes.includes(nome)) {
        orfas.push(`${path.relative(raiz, arquivo)}: ${nome}`);
      }
    }
  }

  assert.deepEqual(
    orfas,
    [],
    "constante DDL que nenhuma migração executa: a tabela nasce só quando alguém chama a " +
      "query, e o schema-ouro nunca a vê. Registre o DDL numa migração nova (ver a v62):\n" +
      orfas.join("\n"),
  );
});

test("a camada de query não faz ALTER TABLE cru", () => {
  const infratores = [];
  for (const arquivo of arquivosDeProducao(dirQueries)) {
    const texto = semComentarios(semBlocoDeTeste(fs.readFileSync(arquivo, "utf8")));
    if (/ALTER\s+TABLE/i.test(texto)) {
      infratores.push(path.relative(raiz, arquivo));
    }
  }

  assert.deepEqual(
    infratores,
    [],
    "`ALTER TABLE` escrito à mão na camada de query. Coluna tardia entra por " +
      "`add_column_if_missing`, que é guardado por PRAGMA e não engole falha de disco:\n" +
      infratores.join("\n"),
  );
});

/// Duas colunas tardias continuam entrando por `ALTER` do lado da query, e não é descuido:
/// `team_car.unit_seed` e `ai_race_story.teams_json` chegaram aos saves em campo por esse
/// caminho, antes de existir migração para elas. `team_car` ainda é declarada pela baseline,
/// que é retrato congelado da v53 — o guard `ddl_das_queries_bate_com_o_da_baseline_nas_
/// tabelas_duplicadas` exige que a constante e a baseline produzam a mesma tabela, então a
/// coluna não pode entrar no `CREATE`. E o `v62_preserva_o_que_o_ensure_table_ja_tinha_criado`
/// prova que tirar o `ALTER` daqui derruba a leitura num save que ainda não migrou.
///
/// O que este guard prende, então, não é a ausência do `ALTER`: é a PARIDADE. Todo `ALTER`
/// da camada de query tem de ter gêmeo em `migrations.rs`, na mesma tabela e coluna. Sem
/// isso a coluna existe só para quem tocou a query, e o schema-ouro nunca a vê.
test("todo add_column_if_missing da query tem gêmeo numa migração", () => {
  const migracoes = fs.readFileSync(arquivoMigracoes, "utf8");
  // `add_column_if_missing(conn, "tabela", "coluna", "TIPO")` — em uma linha ou quebrado
  // pelo rustfmt, daí o `\s*` entre os argumentos.
  const padrao = /add_column_if_missing\(\s*conn\s*,\s*"([A-Za-z0-9_]+)"\s*,\s*"([A-Za-z0-9_]+)"/g;
  const naMigracao = new Set(
    [...migracoes.matchAll(padrao)].map(([, tabela, coluna]) => `${tabela}.${coluna}`),
  );

  const orfas = [];
  for (const arquivo of arquivosDeProducao(dirQueries)) {
    const texto = semComentarios(semBlocoDeTeste(fs.readFileSync(arquivo, "utf8")));
    for (const [, tabela, coluna] of texto.matchAll(padrao)) {
      if (!naMigracao.has(`${tabela}.${coluna}`)) {
        orfas.push(`${path.relative(raiz, arquivo)}: ${tabela}.${coluna}`);
      }
    }
  }

  assert.deepEqual(
    orfas,
    [],
    "coluna que a query acrescenta e nenhuma migração acrescenta: ela existe só para quem " +
      "tocou essa query, o schema-ouro não a vê, e um save que nunca chamou a função fica " +
      "sem ela. Acrescente o mesmo `add_column_if_missing` numa migração nova:\n" +
      orfas.join("\n"),
  );
});

test("cada `ensure_table` continua declarando o motivo de existir", () => {
  const semMotivo = [];
  for (const arquivo of arquivosDeProducao(dirQueries)) {
    const bruto = fs.readFileSync(arquivo, "utf8");
    if (!/fn ensure_[a-z_]*table\b/.test(bruto)) continue;
    // O motivo legítimo é um só: conexão de teste in-memory não roda migração. Exigimos
    // que ele esteja escrito, para que um `ensure_*` novo sem motivo apareça na revisão.
    if (!/n[ãa]o\s+migram|que\s+n[ãa]o\s+rodam?\s+migra|sem\s+migra/i.test(bruto)) {
      semMotivo.push(path.relative(raiz, arquivo));
    }
  }

  assert.deepEqual(
    semMotivo,
    [],
    "`ensure_*table` sem o motivo escrito. O único motivo aceito é reaplicar o DDL para " +
      "conexões de teste in-memory que não migram; qualquer outro é schema nascendo fora " +
      "das migrações:\n" + semMotivo.join("\n"),
  );
});
