// O auditor de captura lê DUAS listas do Rust como texto, por regex:
//
//   • `CANAIS_CURADOS` de `iracing_sdk/canais.rs` — os canais que a leitura procura no SDK;
//   • o `match` de nomes de `iracing_sdk/imp/leitura.rs` — o mapa canal -> campo.
//
// Ler do fonte é o certo, e é o mesmo padrão dos outros guards daqui: uma segunda cópia da
// lista dentro do script envelheceria calada, e um auditor que audita contra lista velha
// aprova o que devia reprovar.
//
// O preço é este guard. As duas extrações dependem da FORMA do código Rust, e as duas falham
// devolvendo vazio — não erro. Um `pub const CANAIS_CURADOS` que vire `static`, ou um braço
// de `match` que passe a escrever num campo intermediário, apagam a conferência sem apagar o
// relatório: o auditor continua imprimindo, bonito, dizendo que está tudo certo porque não
// olhou nada. É a falha mais perigosa que uma ferramenta de diagnóstico pode ter.
//
// Este guard não confere as capturas: elas moram em `%APPDATA%` e não estão no repositório.
// Ele confere que o auditor ainda CONSEGUE ler o que precisa ler.

import assert from "node:assert/strict";
import { test } from "node:test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const AUDITOR = path.join(raiz, "scripts", "captura-auditar.mjs");

/** Roda uma função exportada pelo auditor sem executar o `main` dele. */
async function auditor() {
  return import(`file://${AUDITOR.replace(/\\/g, "/")}`);
}

test("o auditor ainda acha CANAIS_CURADOS em canais.rs", async () => {
  const src = fs.readFileSync(path.join(raiz, "src-tauri", "src", "iracing_sdk", "canais.rs"), "utf8");
  const bloco = /pub const CANAIS_CURADOS:\s*\[&str;\s*\d+\]\s*=\s*\[([\s\S]*?)\];/.exec(src);
  assert.ok(bloco, "a regex do auditor não casa mais com a declaração de CANAIS_CURADOS");
  const nomes = [...bloco[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
  assert.ok(nomes.length > 50, `extraiu só ${nomes.length} canais; a lista tem dezenas`);

  // O tamanho declarado no tipo é a conferência que o próprio Rust já faz. Se ele e a
  // contagem extraída divergirem, quem está errado é a extração.
  const declarado = Number(/pub const CANAIS_CURADOS:\s*\[&str;\s*(\d+)\]/.exec(src)[1]);
  assert.equal(nomes.length, declarado, "a extração não devolveu todos os canais declarados");

  // Uma âncora conhecida: o canal cujo nome errado originou tudo isto.
  assert.ok(nomes.includes("PitRepairLeft"), "PitRepairLeft sumiu da lista curada");
});

test("o auditor ainda acha o mapa canal -> campo em leitura.rs", async () => {
  const src = fs.readFileSync(
    path.join(raiz, "src-tauri", "src", "iracing_sdk", "imp", "leitura.rs"),
    "utf8",
  );
  const pares = [...src.matchAll(/"([A-Za-z0-9_]+)"\s*=>\s*(?:t|car)\.([a-z0-9_]+)\s*=/g)];
  assert.ok(pares.length > 50, `extraiu só ${pares.length} pares; o match tem dezenas de braços`);

  const porCanal = new Map(pares.map((m) => [m[1], m[2]]));
  // As duas âncoras que o relatório usa por nome, uma de cada lado do `match`.
  assert.equal(porCanal.get("PitRepairLeft"), "pit_repair_needed");
  assert.equal(porCanal.get("CarIdxLapDistPct"), "lap_dist_pct");
});

test("os grupos de classificação do auditor não se sobrepõem", async () => {
  // Um campo em dois grupos torna o veredito dependente da ordem dos `if` em
  // `julgarConstante`, que é o jeito silencioso de um alarme deixar de sair.
  const src = fs.readFileSync(AUDITOR, "utf8");
  const grupos = {};
  for (const nome of ["AMBIENTE", "SESSAO", "PILOTAGEM", "CORRIDA"]) {
    const m = new RegExp(`const ${nome} = new Set\\(\\[([\\s\\S]*?)\\]\\);`).exec(src);
    assert.ok(m, `o grupo ${nome} sumiu ou mudou de forma`);
    grupos[nome] = [...m[1].matchAll(/"([^"]+)"/g)].map((x) => x[1]);
  }
  const sentinela = /const SENTINELA = \{([\s\S]*?)\n\};/.exec(src);
  assert.ok(sentinela, "a tabela SENTINELA sumiu ou mudou de forma");
  grupos.SENTINELA = [...sentinela[1].matchAll(/^\s*([a-z0-9_]+):/gm)].map((x) => x[1]);

  const visto = new Map();
  for (const [nome, campos] of Object.entries(grupos)) {
    for (const campo of campos) {
      const antes = visto.get(campo);
      assert.equal(antes, undefined, `\`${campo}\` está em ${antes} e em ${nome} ao mesmo tempo`);
      visto.set(campo, nome);
    }
  }
});
