// Guard estrutural: os nomes de campo que atravessam a ponte Rust -> JS no push-to-talk.
//
// Este teste existe por causa de um defeito que já aconteceu. A resposta falada volta do
// servidor num campo `audio_b64`, o Rust a deserializa numa struct com esse nome, e o
// Tauri a serializa de volta para o front — com o nome **como está escrito no Rust**. A
// ponte converte camelCase para snake_case na IDA (os argumentos do comando), e não
// converte nada na VOLTA. O orquestrador estava lendo `audioB64`.
//
// O modo de falha é o pior que existe neste recurso: `undefined` chega ao decodificador
// de áudio, a decodificação estoura, e o engenheiro cai na desistência. Nenhum erro,
// nenhum log — o jogador só ouve "não consegui ver isso agora" para toda pergunta que
// passar pelo modelo, e não há nada na tela que diga por quê.
//
// A mesma struct é usada nas duas pontas (deserializa do servidor, serializa para o
// front), então conferir o Rust contra o JS cobre as duas travessias de uma vez. O que
// este guard NÃO alcança é o servidor: ele vive noutro repositório, e o contrato com ele
// está escrito na documentação de `commands/ptt_voz.rs`.

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const PTT_VOZ_RS = path.join(raiz, "src-tauri", "src", "commands", "ptt_voz.rs");
const ORQUESTRADOR_JS = path.join(raiz, "src", "lib", "pttEngenheiro.js");

/** Os campos públicos de `FalaSintetizada`, lidos do Rust como texto. */
function camposDaFala() {
  const fonte = fs.readFileSync(PTT_VOZ_RS, "utf8");
  const bloco = fonte.match(/pub struct FalaSintetizada \{([\s\S]*?)\n\}/);
  assert.ok(bloco, "não achei `pub struct FalaSintetizada` em ptt_voz.rs — o guard perdeu o alvo");
  return [...bloco[1].matchAll(/pub (\w+):/g)].map((m) => m[1]);
}

test("a struct da fala não ganhou serde rename sem o front saber", () => {
  const fonte = fs.readFileSync(PTT_VOZ_RS, "utf8");
  const antes = fonte.slice(0, fonte.indexOf("pub struct FalaSintetizada"));
  const derive = antes.slice(antes.lastIndexOf("#[derive"));
  assert.ok(
    !/rename_all/.test(derive),
    "FalaSintetizada ganhou um `rename_all`: os nomes que chegam ao front mudaram, e o " +
      "orquestrador continua lendo os antigos. Atualize pttEngenheiro.js junto.",
  );
});

test("o orquestrador lê exatamente os campos que o Rust entrega", () => {
  const campos = camposDaFala();
  assert.deepEqual(
    campos.sort(),
    ["audio_b64", "mime", "texto"],
    "os campos de FalaSintetizada mudaram; confira o que pttEngenheiro.js lê de `falada`",
  );

  const js = fs.readFileSync(ORQUESTRADOR_JS, "utf8");
  const lidos = [...js.matchAll(/falada\.(\w+)/g)].map((m) => m[1]);
  assert.ok(lidos.length > 0, "o orquestrador não lê nada de `falada` — a resposta do modelo sumiu");
  for (const campo of lidos) {
    assert.ok(
      campos.includes(campo),
      `pttEngenheiro.js lê \`falada.${campo}\`, que não existe em FalaSintetizada ` +
        `(campos: ${campos.join(", ")}). Chegaria undefined, e o engenheiro emudeceria em silêncio.`,
    );
  }
  // O áudio é o que importa: sem ele não há voz nenhuma, e um `texto` lido sozinho
  // passaria despercebido no teste acima.
  assert.ok(
    lidos.includes("audio_b64"),
    "o orquestrador não lê `falada.audio_b64` — o caminho do modelo não tem como falar",
  );
});
