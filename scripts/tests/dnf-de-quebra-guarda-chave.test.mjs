// Abandono POR QUEBRA nunca persiste só a frase traduzida — a chave de i18n vai junto.
//
// O bug que este guard fecha (12/08/2026): as 99 frases de `car/breakdown.rs::problem_text`
// foram para o i18n e o histórico estruturado da quebra (`race_breakdowns`, v52) passou a
// re-renderizar pela trinca `(part, problem, severity)`. O motivo do abandono no resultado
// (`race_results.dnf_reason`) ficou de fora: ele continuava gravado como a prosa já renderizada
// no idioma ativo na hora da corrida. Carreira jogada em pt-BR e lida em en-US mostrava as duas
// metades da MESMA quebra em idiomas diferentes, lado a lado na mesma tela.
//
// A v68 acrescentou `race_results.dnf_reason_key`. O par frase + chave só serve junto: a chave
// sozinha deixa sem texto o save cuja peça esta versão não conhece mais, e a prosa sozinha é o
// congelamento que se está corrigindo. Por isso os dois campos têm um ponto único de escrita,
// `RaceDriverResult::marcar_dnf_de_quebra`, e por isso este guard existe: para que o próximo
// caminho de quebra que apareça não volte a escrever `dnf_reason` sozinho.
//
// O que o guard NÃO faz, de propósito:
//
//   • não conta linhas nem ocorrências — número envelhece no primeiro caminho novo;
//   • não toca em abandono que NÃO é quebra (batida, erro de pilotagem, pane do catálogo de
//     incidentes). Esses continuam inteiramente pelo `dnf_reason`, sem chave, e é correto: não
//     existe estrutura semântica por trás deles para reconstruir texto;
//   • não exige nada de save antigo. Linha anterior à v68 fica com a chave `NULL` e mostra o
//     texto de então.
//
// O critério é de FORMA: atribuir `dnf_reason` a partir de um rótulo de quebra
// (`.label`, `problem_label()`, `problem_text(`) obriga a chave a aparecer perto — seja pela
// atribuição de `dnf_reason_key`, seja pela chamada de `marcar_dnf_de_quebra`.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const FONTE_RUST = "src-tauri/src";

/// Atribuição do motivo do abandono: `x.dnf_reason = ...` (nunca o campo num literal de
/// struct, que é `dnf_reason: ...` e é inicialização, não carimbo).
const ATRIBUI_MOTIVO = /\bdnf_reason\s*=\s*/;

/// O motivo veio do sistema de QUEBRA? É o rótulo da peça que denuncia — nas três formas em
/// que ele aparece no código.
const ROTULO_DE_QUEBRA = /\.label\b|problem_label\s*\(|problem_text\s*\(/;

/// A chave acompanhou? Direto, ou pelo ponto único de escrita.
const CARIMBA_CHAVE = /\bdnf_reason_key\s*=|marcar_dnf_de_quebra\s*\(/;

/// Quantas linhas ao redor contam como "perto". Três cobre o par escrito em linhas seguidas
/// com um comentário no meio, e não chega a alcançar o próximo bloco.
const JANELA = 3;

/// Todo `.rs` sob um diretório, recursivo.
function fontes(dir) {
  const achados = [];
  const absoluto = path.join(raiz, dir);
  if (!fs.existsSync(absoluto)) return achados;
  const varrer = (relativo) => {
    for (const item of fs.readdirSync(path.join(raiz, relativo), { withFileTypes: true })) {
      const rel = `${relativo}/${item.name}`;
      if (item.isDirectory()) varrer(rel);
      else if (item.name.endsWith(".rs")) achados.push(rel);
    }
  };
  varrer(dir);
  return achados;
}

test("motivo de abandono por quebra carimba a chave junto da frase", () => {
  const infracoes = [];

  for (const arquivo of fontes(FONTE_RUST)) {
    const linhas = fs.readFileSync(path.join(raiz, arquivo), "utf8").split(/\r?\n/);
    linhas.forEach((linha, i) => {
      if (!ATRIBUI_MOTIVO.test(linha)) return;
      if (!ROTULO_DE_QUEBRA.test(linha)) return;
      const inicio = Math.max(0, i - JANELA);
      const vizinhanca = linhas.slice(inicio, i + JANELA + 1).join("\n");
      if (CARIMBA_CHAVE.test(vizinhanca)) return;
      infracoes.push(`${arquivo}:${i + 1}: ${linha.trim()}`);
    });
  }

  assert.deepEqual(
    infracoes,
    [],
    "abandono por quebra gravando só a frase traduzida — o motivo vai congelar no idioma da " +
      "corrida. Use `RaceDriverResult::marcar_dnf_de_quebra(label, chave)`, que escreve o par:\n" +
      infracoes.join("\n"),
  );
});

test("a coluna da chave continua no INSERT e no SELECT de race_results", () => {
  // O par só chega ao save se a coluna estiver nas duas pontas. Tirar uma delas não quebraria
  // compilação nenhuma: o INSERT calaria a chave e a leitura voltaria a mostrar a prosa
  // congelada, que é exatamente o sintoma de antes da v68.
  const escrita = fs.readFileSync(path.join(raiz, "src-tauri/src/db/queries/races.rs"), "utf8");
  assert.match(
    escrita,
    /dnf_reason_key/,
    "o INSERT de race_results deixou de gravar `dnf_reason_key`",
  );

  const leitura = fs.readFileSync(
    path.join(raiz, "src-tauri/src/db/queries/race_history/rodadas.rs"),
    "utf8",
  );
  assert.match(
    leitura,
    /rr\.dnf_reason_key/,
    "`get_event_results` deixou de ler `dnf_reason_key` — o motivo volta a sair congelado",
  );
});
