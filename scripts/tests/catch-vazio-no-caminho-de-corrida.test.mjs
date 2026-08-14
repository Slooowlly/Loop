// Nenhum `.catch(() => {})` cru no caminho de corrida nem nos stores.
//
// O achado V6.2 da vistoria de 11/08/2026: 77 ocorrências do padrão em `src/`, sem nada que
// distinga "isto pode falhar e não custa nada" de "isto falhou, a feature não vai acontecer e
// ninguém soube". Os dois casos mais caros estavam no caminho principal — a macro de bandeira
// amarela e o modo janela do simulador, ambos pré-requisitos de features que o jogador só usa
// depois, e ambos engolidos na exportação da etapa.
//
// O padrão em si não é o defeito; a AUSÊNCIA DE RASTRO é. O app é uma GUI sem console na
// máquina do jogador, então a falha engolida some para sempre e chega ao suporte como "não
// funciona". A substituição é `bestEffort(promessa, rotulo)` (ou `bestEffortComRetorno`), em
// `src/utils/bestEffort.js`: engole igual para a UI e registra uma linha no `loop.log`, que é o
// arquivo que o jogador já sabe anexar.
//
// O ALCANCE é deliberadamente estreito, e o critério de aceite do achado é este:
//
//   • `src/components/race/` — a tela de próxima corrida, o resultado e a telemetria;
//   • `src/stores/` — preventivo, os slices não têm nenhuma ocorrência em código de produção.
//
// Fora do alcance de propósito, e não por esquecimento:
//
//   • `src/overlay/` — laços de telemetria a 60 Hz. Registrar de lá encheria o log com a mesma
//     linha e afogaria o que ele existe para mostrar. São best-effort assumido, e a decisão de
//     instrumentá-los depende de um canal com amostragem, não deste guard.
//   • `ctx.resume().catch(...)` do Web Audio — a política de autoplay REJEITA por projeto até
//     haver gesto do jogador. Não é falha, é o contrato do navegador.
//   • `p.catch(...)` que só existe para não virar "unhandled rejection" — ali o tratamento de
//     verdade acontece em outro lugar, e o `catch` é higiene do webview.
//   • ARQUIVO DE TESTE — engolir uma rejeição esperada é o próprio assunto do caso. Dois
//     exemplos vivos em `stores/career/seasonSlice.test.js`.
//
// O guard cobre a forma com função-seta vazia, que é a documentada no achado. A forma de bloco
// (`try {} catch {}`) fica de fora: ela aparece em volta de `localStorage`, onde o silêncio é o
// comportamento certo e um rastro por leitura seria ruído.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

/// Os diretórios sob guarda, relativos à raiz do repositório.
const ALCANCE = ["src/components/race", "src/stores"];

/// `.catch(() => {})` e as variações que passam batido numa busca literal: sem espaço,
/// com espaço dentro das chaves, com o erro nomeado e ignorado (`(e) => {}`, `e => {}`).
const CATCH_VAZIO = /\.catch\(\s*(?:\(\s*[\w$]*\s*\)|[\w$]+)\s*=>\s*\{\s*\}\s*\)/;

/// Todo `.js`/`.jsx` de um diretório, recursivo. Arquivo de teste fica de fora — ver o
/// cabeçalho: lá o `catch` vazio é o próprio objeto do caso.
function fontes(dir) {
  const achados = [];
  const absoluto = path.join(raiz, dir);
  if (!fs.existsSync(absoluto)) return achados;
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

test("nenhum `.catch(() => {})` cru em src/components/race/ e src/stores/", () => {
  const ofensas = [];
  for (const dir of ALCANCE) {
    for (const arquivo of fontes(dir)) {
      const linhas = fs.readFileSync(path.join(raiz, arquivo), "utf8").split(/\r?\n/);
      linhas.forEach((linha, i) => {
        if (CATCH_VAZIO.test(linha)) ofensas.push(`${arquivo}:${i + 1}  ${linha.trim()}`);
      });
    }
  }
  assert.deepEqual(
    ofensas,
    [],
    [
      "falha engolida sem rastro no caminho de corrida:",
      ...ofensas,
      "",
      "Troque por `bestEffort(promessa, rotulo)` de src/utils/bestEffort.js — ele engole",
      "igual para a UI e deixa a linha no loop.log. Quando a falha impede uma feature que o",
      "jogador vai usar, use `bestEffortComRetorno` e mostre o estado na tela.",
    ].join("\n"),
  );
});

// Um guard que não enxerga nada passa verde para sempre. Estes dois casos provam que ele
// está lendo arquivo de verdade e que a regex reconhece o padrão que existe para proibir.
test("o guard enxerga os arquivos que diz cobrir", () => {
  const total = ALCANCE.flatMap(fontes);
  assert.ok(
    total.length >= 20,
    `só ${total.length} arquivos varridos no alcance — a varredura furou`,
  );
  assert.ok(
    total.some((f) => f.endsWith("nextrace/useIracingExport.js")),
    "useIracingExport.js precisa estar no alcance: é a origem do achado V6.2",
  );
});

test("a regex reconhece as variações do padrão", () => {
  for (const amostra of [
    "  .catch(() => {});",
    "invoke('x').catch(()=>{})",
    "invoke('x').catch(() => { })",
    "invoke('x').catch((e) => {})",
    "invoke('x').catch(erro => {})",
  ]) {
    assert.ok(CATCH_VAZIO.test(amostra), `deveria pegar: ${amostra}`);
  }
  for (const amostra of [
    ".catch(semDestino)",
    ".catch((e) => registrar(e))",
    ".catch(() => setErro('falhou'))",
    "bestEffort(invoke('x'), 'x')",
  ]) {
    assert.ok(!CATCH_VAZIO.test(amostra), `não deveria pegar: ${amostra}`);
  }
});
