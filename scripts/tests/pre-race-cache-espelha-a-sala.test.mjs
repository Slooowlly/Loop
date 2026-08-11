// O pré-carregamento da etapa precisa buscar tudo o que a Sala de Estratégia busca.
//
// Dois caminhos leem o mesmo retrato estático da pré-corrida: o cache que roda durante a
// animação do calendário (`stores/career/preRaceFetch.js`, consumido pelo
// `preRaceCacheSlice`) e a Sala ao montar (`components/race/nextrace/useBriefingData.js`).
//
// O modo de falha é silencioso e foi apontado na vistoria de 10/08/2026: a Sala passa a
// pedir um comando novo, o cache continua "completo" aos olhos de quem o escreveu, e o
// flash template→IA que o cache existe para evitar volta sem nenhum erro no console.
//
// Este guard não obriga as listas a serem iguais — o cache pode buscar A MAIS. O que ele
// proíbe é a Sala pedir algo que o cache não busca.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const ler = (p) => fs.readFileSync(path.join(raiz, p), "utf8");

/// Nomes de comando passados a `invoke("...")` num arquivo.
function invokesDe(caminho) {
  const fonte = ler(caminho);
  const nomes = [...fonte.matchAll(/invoke\(\s*["'`]([a-z0-9_]+)["'`]/g)].map(([, n]) => n);
  assert.ok(
    nomes.length > 0,
    `nenhum invoke extraído de ${caminho} — a extração furou e o guard passaria vazio`,
  );
  return new Set(nomes);
}

/// A lista declarada em `COMANDOS_DA_PRE_CORRIDA`.
function listaDeclarada() {
  const fonte = ler("src/stores/career/preRaceFetch.js");
  const bloco = /COMANDOS_DA_PRE_CORRIDA\s*=\s*\[([\s\S]*?)\]/.exec(fonte);
  assert.ok(bloco, "não achei COMANDOS_DA_PRE_CORRIDA em src/stores/career/preRaceFetch.js");
  const nomes = [...bloco[1].matchAll(/["']([a-z0-9_]+)["']/g)].map(([, n]) => n);
  assert.ok(nomes.length >= 3, `só ${nomes.length} comandos na lista — a extração furou`);
  return new Set(nomes);
}

test("a lista declarada bate com o que o módulo de busca realmente invoca", () => {
  const declarados = listaDeclarada();
  const invocados = invokesDe("src/stores/career/preRaceFetch.js");

  assert.deepEqual(
    [...invocados].sort(),
    [...declarados].sort(),
    "COMANDOS_DA_PRE_CORRIDA é a documentação do módulo; se ela mente, o guard abaixo " +
      "compara contra uma lista errada",
  );
});

// Comandos que a Sala lê e o pré-carregamento NÃO busca, cada um com o motivo. A lista é
// dívida registrada, não permissão aberta: enquanto um nome está aqui, aquele pedaço da
// Sala continua chegando depois da tela, com o flash que o cache existe para evitar.
//
// Vazia desde 11/08/2026. `get_grid_breakdown_risk` e `get_weekend_modifiers` moravam
// aqui porque alimentavam estado LOCAL do hook, fora do `preRaceStandings`. Hoje o retrato
// inteiro vai para o store e a Sala o lê de lá, então os dois entraram no cache.
const FORA_DO_CACHE_COM_MOTIVO = new Map([]);

test("a Sala de Estratégia não pede nenhum comando que o cache deixe de buscar", () => {
  const doCache = listaDeclarada();
  const daSala = invokesDe("src/components/race/nextrace/useBriefingData.js");

  // A Sala também dispara comandos que não fazem parte do retrato estático (geração de
  // IA sob demanda, por exemplo). Só os de leitura de estado da etapa entram na conta.
  const somenteLeitura = [...daSala].filter((nome) => nome.startsWith("get_"));

  const faltando = somenteLeitura.filter(
    (nome) => !doCache.has(nome) && !FORA_DO_CACHE_COM_MOTIVO.has(nome),
  );
  assert.deepEqual(
    faltando,
    [],
    `a Sala de Estratégia lê ${faltando.join(", ")} e o pré-carregamento não busca. ` +
      "Some com o comando em src/stores/career/preRaceFetch.js ou registre a exceção com " +
      "motivo em FORA_DO_CACHE_COM_MOTIVO — senão a Sala abre com flash, sem erro visível.",
  );
});

test("a lista de exceções não guarda nome que a Sala parou de usar", () => {
  const daSala = invokesDe("src/components/race/nextrace/useBriefingData.js");

  const orfaos = [...FORA_DO_CACHE_COM_MOTIVO.keys()].filter((nome) => !daSala.has(nome));
  assert.deepEqual(
    orfaos,
    [],
    `${orfaos.join(", ")} está na lista de exceções e a Sala não pede mais. Dívida quitada: ` +
      "tire da lista.",
  );
});

test("a Sala de Estratégia usa o módulo compartilhado em vez de invocar por conta própria", () => {
  const sala = ler("src/components/race/nextrace/useBriefingData.js");

  assert.match(
    sala,
    /buscarDadosDaPreCorrida/,
    "a Sala precisa consumir o módulo compartilhado; sem isso os testes acima passam vazios " +
      "(nenhum get_ na Sala para comparar) e o espelho manual volta sem ninguém notar",
  );

  const leiturasNaSala = [...sala.matchAll(/invoke\(\s*["'`](get_[a-z0-9_]+)["'`]/g)].map(
    ([, n]) => n,
  );
  assert.deepEqual(
    leiturasNaSala,
    [],
    `a Sala voltou a invocar ${leiturasNaSala.join(", ")} direto — essas leituras ` +
      "pertencem ao preRaceFetch.js, senão o pré-carregamento fica cego para elas",
  );
});

test("o slice de cache usa o módulo compartilhado em vez de invocar por conta própria", () => {
  const slice = ler("src/stores/career/preRaceCacheSlice.js");

  assert.match(
    slice,
    /buscarDadosDaPreCorrida/,
    "o slice precisa consumir o módulo compartilhado; invocar direto recria o espelho manual",
  );

  const invocadosNoSlice = [...slice.matchAll(/invoke\(\s*["'`]([a-z0-9_]+)["'`]/g)].map(
    ([, n]) => n,
  );
  const leiturasDeEstado = invocadosNoSlice.filter((nome) => nome.startsWith("get_"));
  assert.deepEqual(
    leiturasDeEstado,
    [],
    `o slice voltou a invocar ${leiturasDeEstado.join(", ")} direto — essas leituras ` +
      "pertencem ao preRaceFetch.js",
  );
});
