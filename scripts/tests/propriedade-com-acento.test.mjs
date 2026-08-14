// Nenhum acesso a propriedade com acento no nome, em `src/`.
//
// O achado B88: `nextRaceBriefing.js` lia `driver.posição_campeonato`, e o campo que o Rust
// manda é `posicao_campeonato`. JavaScript aceita acento em identificador, então o acesso é
// LEGAL: não estoura, não avisa, devolve `undefined` e o `?? 0` logo ao lado transforma o
// engano em número. O termo da posição no campeonato entrava zerado para todo mundo e a
// escolha da frase de expectativa perdeu um dos seis fatores sem que nada acusasse.
//
// A fronteira Rust↔React é toda em snake_case sem acento (os DTOs de `career_types.rs` e as
// colunas do SQLite), então propriedade acentuada em `src/` é sempre erro de digitação. O
// guard vale para leitura e escrita; o que ele não cobre é acesso por colchete com string
// montada, que nenhuma tela faz hoje.
//
// Fora do alcance: arquivo de teste, onde a chave errada é o próprio assunto do caso (ver
// `src/pages/tabs/nextRaceBriefing.test.js`), e qualquer coisa fora de `src/`.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

const ALCANCE = ["src"];

const ACENTOS = "áàâãäéèêëíìîïóòôõöúùûüçñÁÀÂÃÄÉÈÊËÍÌÎÏÓÒÔÕÖÚÙÛÜÇÑ";

/// `.identificador` em que o identificador tem ao menos um acento. O ponto vem colado nos dois
/// lados: à esquerda no que está sendo acessado (nome, `)` ou `]`), à direita no nome da
/// propriedade. É isso que separa acesso a propriedade de ponto final de frase, que sempre tem
/// espaço depois.
const PROPRIEDADE_ACENTUADA = new RegExp(
  `[\\w$)\\]]\\.[A-Za-z_$][\\w$]*[${ACENTOS}][\\w$]*`,
);

/// A linha sem literal de string e sem comentário de fim de linha. Prosa em português é onde o
/// acento é legítimo, e uma chave de i18n como `t("menu.configurações")` não é acesso a
/// propriedade. Aproximação por linha: template literal de várias linhas escapa, e nenhum
/// acesso a propriedade mora dentro de um.
function codigoNu(linha) {
  let saida = "";
  let aspas = null;
  for (let i = 0; i < linha.length; i += 1) {
    const c = linha[i];
    if (aspas) {
      if (c === "\\") i += 1;
      else if (c === aspas) aspas = null;
      continue;
    }
    if (c === '"' || c === "'" || c === "`") {
      aspas = c;
      continue;
    }
    if (c === "/" && (linha[i + 1] === "/" || linha[i + 1] === "*")) break;
    saida += c;
  }
  return saida;
}

/// Todo `.js`/`.jsx` de um diretório, recursivo, sem os arquivos de teste.
function fontes(dir) {
  const achados = [];
  if (!fs.existsSync(path.join(raiz, dir))) return achados;
  const varrer = (relativo) => {
    for (const item of fs.readdirSync(path.join(raiz, relativo), { withFileTypes: true })) {
      const rel = `${relativo}/${item.name}`;
      if (item.isDirectory()) {
        varrer(rel);
      } else if (/\.jsx?$/.test(item.name) && !/\.test\.jsx?$/.test(item.name)) {
        achados.push(rel);
      }
    }
  };
  varrer(dir);
  return achados;
}

test("nenhuma propriedade com acento no nome em src/", () => {
  const ofensas = [];
  for (const dir of ALCANCE) {
    for (const arquivo of fontes(dir)) {
      const linhas = fs.readFileSync(path.join(raiz, arquivo), "utf8").split(/\r?\n/);
      linhas.forEach((linha, i) => {
        if (PROPRIEDADE_ACENTUADA.test(codigoNu(linha))) {
          ofensas.push(`${arquivo}:${i + 1}  ${linha.trim()}`);
        }
      });
    }
  }
  assert.deepEqual(
    ofensas,
    [],
    [
      "acesso a propriedade com acento no nome:",
      ...ofensas,
      "",
      "A ponte com o Rust é snake_case sem acento. O acesso acentuado é legal em JS e devolve",
      "`undefined` calado — confira o nome do campo no DTO de `commands/career_types.rs`.",
    ].join("\n"),
  );
});

// Um guard que não enxerga nada passa verde para sempre.
test("o guard enxerga os arquivos que diz cobrir", () => {
  const total = ALCANCE.flatMap(fontes);
  assert.ok(total.length >= 100, `só ${total.length} arquivos varridos — a varredura furou`);
  assert.ok(
    total.includes("src/pages/tabs/nextRaceBriefing.js"),
    "nextRaceBriefing.js precisa estar no alcance: é a origem do achado B88",
  );
});

test("a regex reconhece o padrão e poupa o que é legítimo", () => {
  for (const amostra of [
    "driver.posição_campeonato ?? 0",
    "const n = piloto.pontuação;",
    "obj.média = 1;",
    "const [a] = x; a[0].posição;",
  ]) {
    assert.ok(PROPRIEDADE_ACENTUADA.test(codigoNu(amostra)), `deveria pegar: ${amostra}`);
  }
  for (const amostra of [
    "driver.posicao_campeonato ?? 0",
    't("menu.configurações")',
    "const x = 1.5;",
    "// o topo do eixo é a liderança. Marcá-lo custaria uma linha",
    "  precisa. Só o chevron da gaveta fica reservado. */}",
    "lista.map((x) => x.nome)",
  ]) {
    assert.ok(!PROPRIEDADE_ACENTUADA.test(codigoNu(amostra)), `não deveria pegar: ${amostra}`);
  }
});
