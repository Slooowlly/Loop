// A Chirp 3 devolve 200 OK com silêncio dentro. Medido gerando `"e,"`: 0,20 s com pico 0,008,
// de forma reprodutível. O gerador do engenheiro já media isso e apagava a peça; o do spotter
// gravava primeiro e só depois avisava no console — e o aviso não conserta nada, porque a
// execução seguinte pula a chave pelo "já existe" e o buraco vira permanente.
//
// Estes guards travam as duas coisas: o piso de pico/duração é UM só, em `filtro-radio.mjs`, e
// o pacote do spotter não grava antes de medir.
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import {
  avaliarPeca,
  DURACAO_MINIMA_S,
  PICO_MINIMO,
} from "../tts-poc/filtro-radio.mjs";

const SCRIPTS = join(dirname(fileURLToPath(import.meta.url)), "..");
const TAXA = 24000;

/** Um bloco de amostras com pico e duração cravados. */
function peca(pico, segundos) {
  const x = new Float32Array(Math.round(segundos * TAXA));
  for (let i = 0; i < x.length; i += 1) x[i] = pico * Math.sin((2 * Math.PI * 300 * i) / TAXA);
  if (x.length) x[Math.floor(x.length / 4)] = pico; // garante o pico exato
  return x;
}

test("fala normal passa", () => {
  const r = avaliarPeca(peca(0.95, 1.2), TAXA);
  assert.equal(r.utilizavel, true);
  assert.equal(r.motivo, null);
  assert.ok(Math.abs(r.duracao - 1.2) < 0.01);
});

test("silêncio com 200 OK não passa, mesmo com duração de fala", () => {
  const r = avaliarPeca(peca(0.008, 1.2), TAXA); // o caso medido do `"e,"`
  assert.equal(r.utilizavel, false);
  assert.match(r.motivo, /pico 0\.008/);
});

test("pedaço de vogal não passa, mesmo com pico alto", () => {
  const r = avaliarPeca(peca(0.92, 0.08), TAXA); // a segunda tomada do `"e,"`
  assert.equal(r.utilizavel, false);
});

test("os pisos são os medidos no acervo", () => {
  assert.equal(PICO_MINIMO, 0.2);
  assert.equal(DURACAO_MINIMA_S, 0.2);
  assert.equal(avaliarPeca(peca(PICO_MINIMO, DURACAO_MINIMA_S), TAXA).utilizavel, true);
});

test("os dois geradores de pacote usam o MESMO piso, importado de filtro-radio", () => {
  for (const arquivo of ["spotter-pack.mjs", "engenheiro-pack.mjs"]) {
    const src = readFileSync(join(SCRIPTS, arquivo), "utf8");
    assert.match(src, /avaliarPeca[\s\S]*?from "\.\/tts-poc\/filtro-radio\.mjs"/, `${arquivo} não importa avaliarPeca`);
    assert.doesNotMatch(
      src,
      /^const (PICO_MINIMO|DURACAO_MINIMA_S)\s*=/m,
      `${arquivo} redeclara um piso que já mora em filtro-radio.mjs`,
    );
  }
});

/// A guarda que decide se a tomada vira arquivo. Os dois geradores chegam à mesma resposta por
/// caminhos diferentes: o spotter olha `utilizavel` direto, o engenheiro carrega o veredito no
/// campo `mudo` porque também precisa distinguir fôlego teimoso (que É gravado) de peça muda.
const GUARDA = {
  "spotter-pack.mjs": "if (!r.utilizavel)",
  "engenheiro-pack.mjs": "if (r.mudo)",
};

test("os dois pacotes só gravam a peça depois de medi-la", () => {
  // Era a defesa que só o spotter tinha. No engenheiro o `escreverWav` acontecia dentro de
  // `gerarUmaTomada`, antes da medição, e o apagamento vinha depois — bastava o processo morrer
  // no meio para o WAV inválido ficar no disco e virar permanente pelo "já existe".
  for (const [arquivo, guarda] of Object.entries(GUARDA)) {
    const src = readFileSync(join(SCRIPTS, arquivo), "utf8");
    const gravacoes = [...src.matchAll(/escreverWav\(destino/g)];
    assert.equal(gravacoes.length, 1, `${arquivo}: há mais de um ponto de gravação do asset final`);
    const i = src.indexOf(guarda);
    assert.ok(i > 0, `${arquivo}: sumiu a guarda de peça inutilizável (${guarda})`);
    assert.ok(i < gravacoes[0].index, `${arquivo}: a gravação acontece ANTES da guarda`);

    // E a gravação não pode ter voltado para dentro da tomada: lá ela roda por tentativa, antes
    // de qualquer veredito.
    const tomada = src.indexOf("async function gerarUmaTomada");
    const fimDaTomada = src.indexOf("\n}", tomada);
    assert.ok(
      gravacoes[0].index > fimDaTomada,
      `${arquivo}: o escreverWav voltou para dentro de gerarUmaTomada`,
    );
  }
});

test("os dois pacotes revalidam a peça que já está no disco", () => {
  // O "já existe" pula pelo NOME. Sem releitura, um WAV mudo de rodada antiga fica invisível
  // para sempre, e o único conserto seria alguém desconfiar e apagar o arquivo à mão.
  for (const arquivo of ["spotter-pack.mjs", "engenheiro-pack.mjs"]) {
    const src = readFileSync(join(SCRIPTS, arquivo), "utf8");
    assert.match(
      src,
      /function jaEstaBoa\(destino\)/,
      `${arquivo} não tem a releitura do que já está no disco`,
    );
    assert.match(src, /jaEstaBoa\(destino\)[\s\S]{0,400}utilizavel/, `${arquivo} não usa o veredito de jaEstaBoa`);
    // `existsSync` sozinho decidindo a fila é exatamente o pulo cego que a releitura substitui.
    assert.doesNotMatch(
      src,
      /return opcoes\.refazer \|\| !fs\.existsSync/,
      `${arquivo} voltou a montar a fila só pelo nome do arquivo`,
    );
  }
});

test("a peça muda não sobrevive no disco entre rodadas", () => {
  // O spotter nunca chega a gravar. O engenheiro pode encontrar um arquivo velho ruim que a
  // rodada nova também não conseguiu substituir — e aí ele TEM que sair, senão a revalidação da
  // rodada seguinte é a única barreira e o `audio-para-opus` já converteu no meio do caminho.
  const src = readFileSync(join(SCRIPTS, "engenheiro-pack.mjs"), "utf8");
  const guarda = src.indexOf("if (r.mudo)");
  const trecho = src.slice(guarda, guarda + 600);
  assert.match(trecho, /unlinkSync\(destino\)/, "a peça muda não apaga o arquivo velho do disco");
});
