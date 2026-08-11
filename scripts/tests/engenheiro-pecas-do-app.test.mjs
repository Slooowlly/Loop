// Guard estrutural: as peças que quem TOCA é o app, e não o renderizador do Rust.
//
// Oito falas do acervo não respondem a pergunta nenhuma. A `nao_sei` é a desistência, as
// `espera*` cobrem o tempo do modelo, as `foco*` e a `radio_ruim` são o freio do rádio —
// todas decididas *depois* do renderizador, na camada de push-to-talk em JavaScript. O Rust
// as declara em `PECAS_DE_OUTRA_CAMADA` para o teste de alcance dele não as chamar de órfãs.
//
// Isso cria o mesmo acoplamento entre dois arquivos que não se conhecem que já existe nas
// peças emprestadas do spotter, e com o mesmo modo de falha: renomear uma chave de um lado
// deixa o outro pedindo um `.opus` que não existe, e o engenheiro fica MUDO — sem erro, sem
// log, indistinguível de o jogador não ter apertado o botão.
//
// O guard confere os DOIS sentidos. Chave que o JS cita e o Rust não gera é fala que nunca
// vai sair; chave que o Rust isenta e ninguém cita é isenção morta — e uma isenção morta
// afrouxa o teste de alcance do Rust exatamente onde ele deveria estar apertado.

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const FALA_RS = path.join(raiz, "src-tauri", "src", "engenheiro", "fala.rs");
const ASSETS = path.join(raiz, "src", "assets", "engenheiro");
// Os dois arquivos do app que nomeiam peça por string literal.
const FONTES_JS = [
  path.join(raiz, "src", "lib", "pttEngenheiro.js"),
  path.join(raiz, "src", "lib", "pttFreio.js"),
];

/** As chaves declaradas em `PECAS_DE_OUTRA_CAMADA`, lidas do Rust como texto. */
function isentadasNoRust() {
  const fonte = fs.readFileSync(FALA_RS, "utf8");
  const bloco = fonte.match(/pub const PECAS_DE_OUTRA_CAMADA:\s*\[&str;\s*(\d+)\]\s*=\s*\[([^\]]+)\]/);
  assert.ok(bloco, "não achei PECAS_DE_OUTRA_CAMADA em fala.rs — o guard perdeu o alvo");
  const chaves = [...bloco[2].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
  // O piso desta extração não é um número escolhido a dedo: o próprio Rust declara o tamanho
  // do array em `[&str; N]`, e o compilador já garante que ele bate com o conteúdo. Conferir
  // contra o N é o piso EXATO, e ele acompanha o crescimento da lista sozinho.
  //
  // Sem isso, uma mudança de forma que fizesse o `matchAll` casar menos chaves — aspas cruas
  // virando `concat!`, um item por linha virando um item comentado — deixaria os quatro casos
  // deste arquivo iterando uma lista curta, ou vazia, e passando verdes.
  assert.equal(
    chaves.length,
    Number(bloco[1]),
    `PECAS_DE_OUTRA_CAMADA declara ${bloco[1]} peças e a extração leu ${chaves.length} — ` +
      "a forma do array mudou e o guard passaria a conferir menos do que existe.",
  );
  return chaves;
}

/** Toda string literal citada pelos arquivos de push-to-talk. */
function citadasNoApp() {
  const citadas = new Set();
  for (const arquivo of FONTES_JS) {
    const fonte = fs.readFileSync(arquivo, "utf8");
    for (const m of fonte.matchAll(/"([a-z][a-z0-9_]*)"/g)) citadas.add(m[1]);
  }
  // Piso de extração: um dos dois arquivos ser renomeado ou trocar aspas por crase faria a
  // varredura devolver pouco ou nada, e o guard passaria comparando com um conjunto vazio.
  assert.ok(
    citadas.size >= 25,
    `só ${citadas.size} strings lidas dos arquivos de push-to-talk (piso 25) — a extração furou`,
  );
  return citadas;
}

/**
 * As chaves que o catálogo do Rust manda gravar — os pares `("chave", "texto")`.
 *
 * O espaço em branco é colapsado antes de casar porque o `rustfmt` quebra o par em quatro
 * linhas assim que ele passa da largura, e foi exatamente o que aconteceu com a
 * `radio_ruim`: um casamento linha a linha a perdia e o guard acusava de fantasma a única
 * peça que estava certa.
 */
function geradasNoRust() {
  const fonte = fs.readFileSync(FALA_RS, "utf8").replace(/\s+/g, " ");
  // O ` ?` depois do parêntese é o que sobra da quebra de linha do `rustfmt` — sem ele o
  // par de quatro linhas continua invisível, agora por um espaço em vez de um `\n`.
  const chaves = new Set([...fonte.matchAll(/\( ?"([a-z][a-z0-9_]*)", "/g)].map((m) => m[1]));
  // O piso mora AQUI, e não dentro de um dos testes: os outros casos também consultam esta
  // função, e uma extração que voltasse vazia os deixaria verdes comparando nada com nada.
  assert.ok(chaves.size >= 20, `só ${chaves.size} chaves lidas de fala.rs (piso 20) — a extração furou`);
  return chaves;
}

test("toda peça isentada é realmente gerada pelo catálogo do Rust", () => {
  // A isenção diz "o renderizador não emite, mas ela EXISTE". Se ela não estiver no
  // catálogo, o gerador nunca a grava e a camada de cima pede um arquivo fantasma.
  const geradas = geradasNoRust();
  assert.ok(geradas.size > 20, `li poucas chaves de fala.rs (${geradas.size})`);
  for (const chave of isentadasNoRust()) {
    assert.ok(
      geradas.has(chave),
      `'${chave}' está em PECAS_DE_OUTRA_CAMADA mas NÃO é gerada pelo catálogo — ` +
        "o app vai pedir um .opus que nunca existiu.",
    );
  }
});

test("toda peça isentada é citada por alguém no app", () => {
  const citadas = citadasNoApp();
  for (const chave of isentadasNoRust()) {
    assert.ok(
      citadas.has(chave),
      `'${chave}' está isentada no Rust como "peça da camada de cima", mas NENHUM arquivo ` +
        "de push-to-talk a cita. Ou ela ficou órfã de verdade, ou o app a renomeou — e no " +
        "segundo caso a fala que o app quer tocar não existe no disco.",
    );
  }
});

test("toda peça citada pelo app está isentada no Rust", () => {
  // O sentido inverso é o que pega o caso mais provável: alguém acrescenta uma fala nova no
  // app (como o freio do rádio foi acrescentado) e esquece de declará-la no Rust. Aí o teste
  // de alcance de lá passa a chamá-la de órfã, e a reação natural é afrouxar aquele teste.
  const isentadas = new Set(isentadasNoRust());
  const geradas = geradasNoRust();
  for (const chave of citadasNoApp()) {
    if (!geradas.has(chave)) continue; // string qualquer do código, não é peça
    assert.ok(
      isentadas.has(chave),
      `o app toca '${chave}', que é uma peça do catálogo mas não está em ` +
        "PECAS_DE_OUTRA_CAMADA. Declare-a lá, senão o teste de alcance do Rust a acusa de órfã.",
    );
  }
});

test("toda peça isentada tem o .opus gravado", () => {
  // Estar na lista não é estar no disco: o pacote pode não ter sido regerado depois de a
  // fala ser acrescentada.
  if (!fs.existsSync(ASSETS)) {
    console.log(`  ${ASSETS} não existe ainda — pulando`);
    return;
  }
  const gravadas = new Set(
    fs
      .readdirSync(ASSETS)
      .filter((f) => f.endsWith(".opus"))
      .map((f) => f.replace(/\.opus$/, "")),
  );
  for (const chave of isentadasNoRust()) {
    assert.ok(
      gravadas.has(chave),
      `'${chave}' está isentada mas ${chave}.opus não está em src/assets/engenheiro/. ` +
        "Rode node scripts/engenheiro-pack.mjs.",
    );
  }
});
