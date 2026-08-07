// Guard estrutural: a permissão de microfone continua concedida, e igual em todas as janelas.
//
// O push-to-talk depende de uma linha de configuração que não se parece com permissão
// nenhuma: `--use-fake-ui-for-media-stream` em `additionalBrowserArgs`. O nome engana — ele
// não falsifica o dispositivo, ele ACEITA automaticamente o pedido de permissão do
// `getUserMedia`. Sem ele o WebView2 abre um diálogo próprio, que é impossível de responder
// por cima de um overlay transparente e click-through, e pior ainda dentro do VR.
//
// São duas armadilhas separadas, e nenhuma delas dá erro visível:
//
//  1. **Sumir com a chave.** O microfone para de abrir, e a única pista é um diálogo do
//     WebView2 que o jogador de VR nem enxerga.
//  2. **Divergir entre janelas.** O WebView2 cria UM ambiente por processo, com os argumentos
//     da primeira janela. Pedir argumentos diferentes na segunda faz a criação dela falhar —
//     o overlay simplesmente não abre. Por isso a string tem de ser idêntica nas três, e não
//     só "presente em todas".
//
// A terceira armadilha é o padrão do wry: ele passa
// `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection` por conta própria, e quem
// define `additionalBrowserArgs` SUBSTITUI esse padrão em vez de somar a ele. Esquecer de
// repetir a lista reativaria a interface fora do processo do WebView2 no app inteiro.

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const CONF = path.join(raiz, "src-tauri", "tauri.conf.json");

/// O que o wry passa sozinho quando ninguém define nada. Repetir é obrigação de quem define.
const PADRAO_DO_WRY = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";
/// A chave da permissão.
const ACEITA_MIDIA = "--use-fake-ui-for-media-stream";

function janelas() {
  const conf = JSON.parse(fs.readFileSync(CONF, "utf8"));
  const lista = conf.app?.windows ?? [];
  assert.ok(lista.length >= 3, "esperava as três webviews (principal, overlay, engineer)");
  return lista;
}

test("toda janela concede microfone e mantém o padrão do wry", () => {
  for (const j of janelas()) {
    const rotulo = j.label ?? "main";
    const args = j.additionalBrowserArgs;
    assert.ok(
      typeof args === "string",
      `a janela "${rotulo}" não define additionalBrowserArgs — o microfone abriria um diálogo nela`,
    );
    assert.ok(
      args.includes(ACEITA_MIDIA),
      `a janela "${rotulo}" perdeu ${ACEITA_MIDIA} — o push-to-talk para de abrir o microfone`,
    );
    assert.ok(
      args.includes(PADRAO_DO_WRY),
      `a janela "${rotulo}" define additionalBrowserArgs sem repetir o padrão do wry (${PADRAO_DO_WRY}); definir SUBSTITUI o padrão, não soma`,
    );
  }
});

test("as janelas pedem exatamente os mesmos argumentos", () => {
  const porArgumento = new Map();
  for (const j of janelas()) {
    const rotulo = j.label ?? "main";
    const args = j.additionalBrowserArgs ?? "(ausente)";
    if (!porArgumento.has(args)) porArgumento.set(args, []);
    porArgumento.get(args).push(rotulo);
  }
  assert.equal(
    porArgumento.size,
    1,
    "as janelas pedem argumentos diferentes ao WebView2, e ele só cria UM ambiente por " +
      `processo — a segunda janela falharia ao abrir. Grupos: ${JSON.stringify([...porArgumento.entries()])}`,
  );
});
