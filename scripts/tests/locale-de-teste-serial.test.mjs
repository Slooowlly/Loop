// Todo teste Rust que troca de idioma roda em série.
//
// O `rust-i18n` guarda o locale num estático GLOBAL DO PROCESSO. O `cargo test` roda os testes
// em paralelo, numa thread pool. Um teste que chama `set_locale("en-US")` muda o idioma
// debaixo de qualquer outro teste que esteja no meio de uma asserção sobre prosa em português.
//
// O modo de falha é o pior tipo: INTERMITENTE e à distância. O teste que quebra não é o que
// trocou o idioma — é um vizinho, escolhido pelo escalonador, que passa quando você roda ele
// sozinho e falha no CI uma vez a cada tantas. Depois de perder uma tarde atrás disso, a
// resposta é sempre a mesma: faltou `#[serial]` em algum lugar.
//
// A regra vivia só no CLAUDE.md, e a vistoria de 10/08/2026 apontou que nada a garantia. Este
// guard é a garantia. Ele foi escrito num momento em que a disciplina estava em 100% (65
// chamadas de `set_locale` medidas em 11/08/2026, zero sem `#[serial]`), que é exatamente
// quando um guard assim vale: ele não pede saneamento de nada, só impede a primeira
// reincidência.
//
// A troca de idioma nem sempre é literal. Dois módulos a embrulham num ajudante de uma linha
// (`fn pt()`, `fn baseline_pt()`) e os testes chamam o ajudante — 19 dos call sites de hoje.
// Procurar só por `set_locale(` deixaria esses 19 de fora, e são justamente os mais fáceis de
// copiar para um teste novo, porque a chamada não parece ter nada a ver com idioma. Por isso o
// guard descobre os ajudantes primeiro e passa a tratá-los como a chamada crua.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

/// Todo `.rs` do crate.
function fontesRust() {
  const achados = [];
  const varrer = (dir) => {
    for (const item of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, item.name);
      if (item.isDirectory()) varrer(p);
      else if (item.name.endsWith(".rs")) achados.push(p);
    }
  };
  varrer(path.join(raiz, "src-tauri", "src"));
  return achados;
}

/**
 * Os ajudantes de teste DESTE arquivo que trocam o idioma por dentro — `fn pt() {
 * set_locale(...) }`.
 *
 * A descoberta é por arquivo, e não global, por um motivo medido: `lib.rs` tem uma `pub fn
 * run()` que troca o locale de verdade, e `driving_style.rs` tem uma `fn run()` de teste que
 * monta um acumulador. Um conjunto global de nomes acusaria as seis chamadas de `run(` de lá
 * como troca de idioma sem `#[serial]`. Ajudante é coisa de vizinhança: os dois que existem
 * hoje (`pt`, `baseline_pt`) são chamados do próprio arquivo onde nasceram.
 *
 * A varredura pega também a `run()` do `lib.rs`, que não é ajudante de teste — mas ela TROCA o
 * idioma, então cobrar `#[serial]` de um teste que a chame é o resultado certo pelo caminho
 * torto. O que importa é o recorte por arquivo, que impede o homônimo de outro módulo de ser
 * acusado.
 */
function ajudantesDoArquivo(rel, linhas) {
  const nomes = new Set();
  const sobTeste = (i) =>
    rel.includes("/tests/") || linhas.slice(0, i).some((l) => /#\[cfg\(test\)\]/.test(l));

  linhas.forEach((linha, i) => {
    const m = /^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-z_][a-z0-9_]*)\s*\(/.exec(linha);
    if (!m || !sobTeste(i)) return;
    // O corpo vai até a próxima assinatura de função — folga de sobra para um ajudante, que
    // por definição é curto, e sem precisar contar chaves.
    const fim = linhas.findIndex((l, k) => k > i && /^\s*(pub\s+)?(async\s+)?fn\s/.test(l));
    const corpo = linhas.slice(i, fim === -1 ? linhas.length : fim).join("\n");
    const attrs = atributosAcima(linhas, i);
    if (/#\[test\]|#\[tokio::test/.test(attrs)) return; // é teste, não ajudante
    if (/set_locale\(/.test(corpo)) nomes.add(m[1]);
  });
  return nomes;
}

/// Os atributos `#[...]` imediatamente acima de uma assinatura de função.
function atributosAcima(linhas, indiceDaAssinatura) {
  const attrs = [];
  for (let k = indiceDaAssinatura - 1; k >= 0 && /^\s*#\[/.test(linhas[k]); k--) {
    attrs.unshift(linhas[k]);
  }
  return attrs.join("\n");
}

test("todo teste que chama set_locale está marcado com #[serial]", () => {
  const infratores = [];
  const ajudantes = new Set();
  let chamadas = 0;

  for (const arquivo of fontesRust()) {
    const rel = path.relative(raiz, arquivo).split(path.sep).join("/");
    const linhas = fs.readFileSync(arquivo, "utf8").split("\n");

    // A troca pode vir literal ou pelo nome de um ajudante do próprio arquivo.
    const doArquivo = ajudantesDoArquivo(rel, linhas);
    for (const n of doArquivo) ajudantes.add(`${rel}:${n}`);
    const trocaDeIdioma = new RegExp(
      [`set_locale\\(`, ...[...doArquivo].map((n) => `\\b${n}\\s*\\(`)].join("|"),
    );

    linhas.forEach((linha, i) => {
      if (!trocaDeIdioma.test(linha)) return;
      if (/^\s*(\/\/|\/\*)/.test(linha)) return; // citação em comentário
      if (/^\s*(pub\s+)?(async\s+)?fn\s/.test(linha)) return; // é a declaração do ajudante
      chamadas += 1;

      // Sobe até a assinatura da função que contém a chamada. O teto de 400 linhas é folga
      // para os testes longos de integração de `career/tests` sem varrer o arquivo inteiro.
      for (let j = i; j >= 0 && i - j < 400; j--) {
        if (!/^\s*(pub\s+)?(async\s+)?fn\s/.test(linhas[j]) || j >= i) continue;
        const attrs = atributosAcima(linhas, j);
        const ehTeste = /#\[test\]|#\[tokio::test/.test(attrs);
        if (ehTeste && !/serial/.test(attrs)) {
          infratores.push(`${rel}:${j + 1}  ${linhas[j].trim()}`);
        }
        break;
      }
    });
  }

  // Piso de extração. Se o padrão de chamada mudar (um alias, um ajudante novo com outra
  // forma), a varredura casa zero e este guard passaria vazio para sempre, calado justamente
  // quando o contrato mexeu. Hoje são 85 pontos de troca: 65 chamadas cruas de `set_locale`,
  // 19 pelos dois ajudantes e a `run()` do `lib.rs`.
  assert.ok(
    chamadas >= 60,
    `só ${chamadas} trocas de idioma encontradas (piso 60) — a extração furou`,
  );
  assert.ok(
    ajudantes.size >= 2,
    `só ${ajudantes.size} ajudantes de locale achados (piso 2: pt, baseline_pt) — ` +
      "a descoberta furou e os testes que trocam o idioma pelo ajudante ficariam sem cobrança",
  );

  assert.deepEqual(
    [...new Set(infratores)],
    [],
    `teste que troca o idioma sem #[serial]:\n  ${[...new Set(infratores)].join("\n  ")}\n\n` +
      `O locale do rust-i18n é global do processo. Sem #[serial] este teste muda o idioma ` +
      `debaixo de outro que está asseverando prosa em PT, e a falha aparece longe daqui, ` +
      `de forma intermitente. Some #[serial] (crate serial_test) ao teste.`,
  );
});

test("o crate serial_test continua declarado", () => {
  // O guard acima cobra o atributo; este cobra que o atributo signifique alguma coisa. Um
  // `#[serial]` sem a dependência não compila, mas remover a dependência junto com os
  // atributos passaria despercebido e devolveria o paralelismo em silêncio.
  const cargo = fs.readFileSync(path.join(raiz, "src-tauri", "Cargo.toml"), "utf8");
  assert.match(cargo, /serial_test/, "serial_test sumiu do Cargo.toml");
});
