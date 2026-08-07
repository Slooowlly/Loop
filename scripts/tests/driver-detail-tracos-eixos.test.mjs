import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

const ler = (relativo) => readFile(path.join(projectRoot, relativo), "utf8");

// O hover de um traço acende o eixo de onde ele veio, e esse laço é um mapa no
// frontend (`TRAIT_AXIS`) entre DUAS listas que moram no Rust: os atributos que
// viram tag (`TAGGED_ATTRS`) e as chaves da leitura técnica (`QUALITY_AXES` /
// `STYLE_AXES`). As palavras dos dois lados não são as mesmas — skill vira
// "ritmo", fitness vira "preparo" —, então nada no compilador nem no vitest
// percebe se uma das pontas for renomeada: o hover simplesmente para de acender,
// em silêncio, num piloto que talvez ninguém abra tão cedo.
//
// Este guard é a costura. Ele falha no minuto em que as listas divergem.
test("cada traço do piloto aponta para um eixo que existe", async () => {
  const [tags, leitura, ficha] = await Promise.all([
    ler("src-tauri/src/models/driver_tags.rs"),
    ler("src-tauri/src/commands/career_detail/leitura.rs"),
    ler("src/components/driver/v2/DriverDetailModalV2.jsx"),
  ]);

  const blocoTags = tags.match(/const TAGGED_ATTRS: &\[&str\] = &\[([\s\S]*?)\];/);
  assert.ok(blocoTags, "expected TAGGED_ATTRS in driver_tags.rs");
  const atributosComTag = [...blocoTags[1].matchAll(/"([a-z_]+)"/g)].map((m) => m[1]);
  assert.ok(atributosComTag.length >= 15, "expected the tagged-attribute list to be non-trivial");

  // `("grupo", "chave", ...)` nas duas tabelas de eixos: interessa só a chave.
  const eixosTecnicos = new Set(
    [...leitura.matchAll(/\(\s*"(?:volta_seca|corrida|condicoes|estilo)",\s*"([a-z_]+)"/g)].map(
      (m) => m[1],
    ),
  );
  assert.ok(eixosTecnicos.size >= 14, "expected the technical axis tables to be non-trivial");

  const blocoMapa = ficha.match(/const TRAIT_AXIS = \{([\s\S]*?)\n\};/);
  assert.ok(blocoMapa, "expected TRAIT_AXIS in DriverDetailModalV2.jsx");
  const mapa = new Map(
    [...blocoMapa[1].matchAll(/^\s*([a-z_]+):\s*"([a-z_:]+)",/gm)].map((m) => [m[1], m[2]]),
  );

  const semEixo = atributosComTag.filter((attr) => !mapa.has(attr));
  assert.deepEqual(
    semEixo,
    [],
    `every tagged attribute needs an entry in TRAIT_AXIS or its chip has a dead hover; missing: ${semEixo.join(", ")}`,
  );

  // Alvos fora da leitura técnica vivem no arco e no estrelato, e cada prefixo
  // tem de bater com o atributo que o bloco correspondente desenha.
  // Só o corpo do CareerArc: `key:` é palavra comum demais no arquivo inteiro, e
  // um alvo do arco casando com o `key` de outro componente deixaria o guard
  // frouxo justamente onde ele precisa apertar.
  const corpoDoArco = ficha.match(/function CareerArc\(\{ arco \}\)[\s\S]*?\n\}\n/);
  assert.ok(corpoDoArco, "expected CareerArc in DriverDetailModalV2.jsx");
  const alvosDoArco = new Set(
    [...corpoDoArco[0].matchAll(/\{\s*key:\s*"([a-z_]+)"/g)].map((m) => `arco:${m[1]}`),
  );
  const alvosDoEstrelato = new Set(
    [...ficha.matchAll(/eixo="(estrelato:[a-z_]+)"/g)].map((m) => m[1]),
  );

  const orfaos = [];
  for (const [attr, alvo] of mapa) {
    const existe = alvo.startsWith("arco:")
      ? alvosDoArco.has(alvo)
      : alvo.startsWith("estrelato:")
        ? alvosDoEstrelato.has(alvo)
        : eixosTecnicos.has(alvo);
    if (!existe) orfaos.push(`${attr} -> ${alvo}`);
  }

  assert.deepEqual(
    orfaos,
    [],
    `TRAIT_AXIS points at axes that no longer exist; orphans: ${orfaos.join(", ")}`,
  );
});
