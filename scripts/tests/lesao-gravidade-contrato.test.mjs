// A gravidade da lesão tem DUAS grafias, e a graça está em elas não se misturarem.
//
// Até 11/08/2026 havia uma só. O Rust guardava `InjuryType` no banco como texto em português
// ("Leve", "Moderada", "Grave", "Critica"), mandava ESSE MESMO texto pela ponte, e o frontend
// o traduzia para chave de i18n comparando string por string. O modo de falha era mudo e
// parecia uma correção de qualidade: alguém acentuava "Critica" para "Crítica" no Rust — que
// é o que a regra de acentuação do projeto pede em toda copy — e o mapa do frontend deixava
// de casar. A gravidade sumia da tela sem erro em lugar nenhum.
//
// Hoje as duas grafias são separadas por construção:
//
//   • `InjuryType::as_str` é a grafia do BANCO. Continua em português e sem acento, porque é
//     valor de coluna gravado desde a primeira migração — acentuá-lo exigiria migrar saves.
//     Só o Rust a vê.
//   • `InjuryType::chave` (e o serde) é a grafia do FIO: "light"/"moderate"/"severe"/
//     "critical". É o que o React recebe e usa como sufixo de chave i18n.
//
// Este guard prende as duas pontas: que a grafia do banco não seja acentuada por engano, que
// o conjunto de chaves do fio seja o MESMO nos dois lados, e que cada chave resolva num texto
// de verdade nos dois locales. E cobra que o mapa antigo não volte.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const ler = (p) => fs.readFileSync(path.join(raiz, p), "utf8");

const ENUM_RS = "src-tauri/src/models/enums/piloto.rs";

/// As chaves de FIO que `InjuryType::chave` devolve, na ordem em que aparecem.
function chavesDoRust() {
  const rs = ler(ENUM_RS);
  // O corpo de `chave`, e só ele — `as_str` tem a mesma forma de `match` logo acima.
  const corpo = /pub fn chave\(&self\) -> &'static str \{([\s\S]*?)\n    \}/.exec(rs);
  assert.ok(corpo, `InjuryType::chave sumiu de ${ENUM_RS}`);
  const chaves = [...corpo[1].matchAll(/InjuryType::(\w+) => "([\w]+)",/g)].map(([, , chave]) => chave);
  assert.equal(chaves.length, 4, `${chaves.length} chaves extraídas de InjuryType::chave — a extração furou`);
  return chaves;
}

/// O conjunto `INJURY_SEVERITIES` declarado num arquivo do frontend.
function chavesDoFront(arquivo) {
  const js = ler(arquivo);
  const bloco = /const INJURY_SEVERITIES = new Set\(\[([^\]]*)\]\)/.exec(js);
  assert.ok(bloco, `INJURY_SEVERITIES sumiu de ${arquivo}`);
  return [...bloco[1].matchAll(/"([\w]+)"/g)].map(([, chave]) => chave);
}

/// Os arquivos do frontend que decidem alguma coisa a partir da gravidade, e o conjunto de
/// chaves que cada um reconhece.
const CONSUMIDORES = [
  ["src/components/race/raceFactsContext.js", chavesDoFront],
  ["src/components/driver/DriverRankingRow.jsx", chavesDoFront],
];

test("o conjunto de chaves do fio é o mesmo no Rust e em cada consumidor do frontend", () => {
  const doRust = chavesDoRust();
  for (const [arquivo, extrair] of CONSUMIDORES) {
    assert.deepEqual(
      [...extrair(arquivo)].sort(),
      [...doRust].sort(),
      `${arquivo} reconhece um conjunto de gravidades diferente do que o Rust emite.\n\n` +
        `Quem manda é InjuryType::chave em ${ENUM_RS}. Gravidade fora da lista do frontend ` +
        `cai no ramo de "valor estranho" e some da tela sem erro nenhum.`,
    );
  }
});

test("o corte de gravidade SÉRIA é o mesmo nos dois lados", () => {
  // O selo 🚑 da classificação. No Rust é `InjuryType::e_seria`; no frontend, o Set do
  // DriverStandingsTable. Divergir aqui não apaga nada da tela — troca o selo em silêncio,
  // que é pior de notar.
  const rs = ler(ENUM_RS);
  const corpo = /pub fn e_seria\(&self\) -> bool \{([\s\S]*?)\n    \}/.exec(rs);
  assert.ok(corpo, `InjuryType::e_seria sumiu de ${ENUM_RS}`);
  const variantesSerias = [...corpo[1].matchAll(/InjuryType::(\w+)/g)].map(([, v]) => v);

  // Traduz variante → chave de fio pelo próprio corpo de `chave`, sem tabela paralela.
  const porVariante = Object.fromEntries(
    [...(/pub fn chave\(&self\) -> &'static str \{([\s\S]*?)\n    \}/.exec(rs) ?? ["", ""])[1].matchAll(
      /InjuryType::(\w+) => "([\w]+)",/g,
    )].map(([, variante, chave]) => [variante, chave]),
  );
  const seriasDoRust = variantesSerias.map((v) => porVariante[v]).filter(Boolean);

  const js = ler("src/components/standings/DriverStandingsTable.jsx");
  const bloco = /const SEVERE_INJURY_TYPES = new Set\(\[([^\]]*)\]\)/.exec(js);
  assert.ok(bloco, "SEVERE_INJURY_TYPES sumiu de DriverStandingsTable.jsx");
  const seriasDoFront = [...bloco[1].matchAll(/"([\w]+)"/g)].map(([, c]) => c);

  assert.deepEqual(
    [...seriasDoFront].sort(),
    [...seriasDoRust].sort(),
    "o corte de lesão séria divergiu entre InjuryType::e_seria e SEVERE_INJURY_TYPES",
  );
});

test("cada chave de gravidade resolve num texto nos dois locales", () => {
  // A chave é o outro lado da tradução. Chave sem entrada devolve a própria chave crua, e o
  // jogador lê "severe" no meio de uma frase em português.
  const locales = {
    "pt-BR": JSON.parse(ler("src/i18n/locales/pt-BR/common.json")),
    "en-US": JSON.parse(ler("src/i18n/locales/en-US/common.json")),
  };
  // Os prefixos em uso: a prosa interpolada da prévia, o rótulo do tooltip do ranking e a
  // gravidade do aviso de lesão da ficha do piloto.
  const PREFIXOS = [
    "raceContext.facts.injurySeverity",
    "globalDrivers.row.injurySeverity",
    "driverDetail.injury.severityValue",
  ];

  for (const prefixo of PREFIXOS) {
    for (const [locale, json] of Object.entries(locales)) {
      for (const chave of chavesDoRust()) {
        const caminho = `${prefixo}.${chave}`.split(".");
        let no = json;
        for (const parte of caminho) no = no?.[parte];
        assert.ok(
          typeof no === "string" && no.length > 0,
          `${locale} não tem a chave ${caminho.join(".")}`,
        );
      }
    }
  }
});

test("a grafia do BANCO continua sem acento, e só o Rust a vê", () => {
  // "Critica" é o valor gravado na coluna `injuries.type` desde a primeira migração; acentuá-lo
  // é uma correção de copy que aqui é quebra de contrato de dado, e que ainda exigiria migrar
  // os saves existentes. A diferença para antes é que agora ela não sai do backend.
  const rs = ler(ENUM_RS);
  assert.match(rs, /InjuryType::Critica => "Critica",/, "o Rust mudou o texto da gravidade crítica");

  for (const [arquivo] of CONSUMIDORES) {
    // Sem os comentários: eles CITAM a grafia antiga de propósito, para explicar por que ela
    // saiu. O que não pode voltar é código comparando por ela.
    const codigo = ler(arquivo).replace(/^\s*\/\/.*$/gm, "");
    assert.equal(
      /"(Leve|Moderada|Grave|Critica)"/.test(codigo),
      false,
      `${arquivo} voltou a comparar a gravidade pela grafia do BANCO — é a chave de fio ` +
        `("light"/"moderate"/"severe"/"critical") que atravessa a ponte.`,
    );
  }
});
