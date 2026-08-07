// Guard estrutural: quem abre o microfone abre o ESCOLHIDO, não o padrão do Windows.
//
// Este teste nasce de um defeito relatado em uso. Três telas abrem o microfone — as
// configurações, o painel de teste e o componente que roda durante a corrida —, e cada
// uma chamava `armar()` por conta própria. Duas delas chamavam sem dispositivo nenhum, o
// que abre o padrão do Windows. Num rig de VR o padrão costuma ser o áudio virtual do
// headset, e não o microfone: o jogador escolhia o USB numa tela, e a corrida gravava
// pela placa errada.
//
// O modo de falha é o de sempre neste recurso — silencioso. Não há erro, não há log; o
// medidor fica em zero e o engenheiro não responde, e nada na tela liga uma coisa à
// outra. Por isso o guard é sobre a CHAMADA, e não sobre o resultado: `armar()` sem
// `deviceId` é a assinatura do defeito, e ela é visível no texto.
//
// A exceção é o próprio `microfone.js`, que é quem implementa o argumento.

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const SRC = path.join(raiz, "src");

/// Arquivos que IMPORTAM o módulo do microfone e chamam `armar(` — varridos como texto,
/// para um chamador novo cair no guard sozinho. Uma lista fixa só protegeria os três que
/// já existem.
///
/// O filtro pelo import não é zelo: `armar` é uma palavra comum em código português, e a
/// primeira versão deste guard acusou um `armar()` local de um tooltip de dossiê que não
/// tem nada com áudio.
///
/// Os COMENTÁRIOS saem antes da varredura, pela mesma razão levada um passo adiante. Este
/// código é comentado em prosa portuguesa, e "enquanto o microfone falhar ao armar (o `catch`
/// abaixo devolve...)" casa com `armar\s*\(` tão bem quanto uma chamada de verdade — o guard
/// acusou exatamente essa frase em `EngenheiroPttAuto.jsx`, onde a chamada logo abaixo passa
/// o `deviceId` corretamente. Um guard que acusa prosa é pior que nenhum: ensina a ignorá-lo.
function semComentarios(texto) {
  // Preserva a contagem de linhas trocando por vazio, não removendo — a mensagem de erro do
  // guard cita trecho, e um deslocamento de linha mandaria quem lê para o lugar errado.
  return texto
    .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "))
    .replace(/(^|[^:])\/\/[^\n]*/g, (m, antes) => antes + " ".repeat(m.length - antes.length));
}

function chamadores() {
  const achados = [];
  const visitar = (dir) => {
    for (const entrada of fs.readdirSync(dir, { withFileTypes: true })) {
      const alvo = path.join(dir, entrada.name);
      if (entrada.isDirectory()) {
        visitar(alvo);
        continue;
      }
      if (!/\.(js|jsx)$/.test(entrada.name)) continue;
      if (entrada.name.endsWith(".test.js") || entrada.name.endsWith(".test.jsx")) continue;
      // O módulo do microfone DEFINE `armar`; não é chamador dele.
      if (alvo.endsWith(path.join("lib", "microfone.js"))) continue;
      const texto = semComentarios(fs.readFileSync(alvo, "utf8"));
      if (!/from\s+["'][^"']*lib\/microfone["']/.test(texto)) continue;
      const chamadas = [...texto.matchAll(/(?:microfone\.)?\barmar\s*\(([^)]*)\)/g)];
      if (chamadas.length) {
        achados.push({ arquivo: path.relative(raiz, alvo), chamadas: chamadas.map((m) => m[1]) });
      }
    }
  };
  visitar(SRC);
  return achados;
}

test("toda tela que abre o microfone passa o dispositivo escolhido", () => {
  const arquivos = chamadores();
  assert.ok(
    arquivos.length >= 2,
    "ninguém mais chama `armar()` — ou o guard perdeu o alvo, ou o microfone parou de abrir",
  );
  for (const { arquivo, chamadas } of arquivos) {
    for (const args of chamadas) {
      assert.ok(
        /deviceId/.test(args),
        `${arquivo} chama \`armar(${args.trim()})\` sem deviceId: abriria o padrão do ` +
          "Windows por cima da escolha do jogador. Passe `{ deviceId: lerMicSalvo() }`.",
      );
    }
  }
});

test("a escolha do microfone é persistida, e não só aplicada", () => {
  // Aplicar sem gravar foi o defeito original: a troca valia enquanto a tela de
  // configurações estivesse aberta, e a corrida — que abre o microfone noutro componente,
  // depois — voltava ao padrão.
  const config = fs.readFileSync(path.join(SRC, "lib", "pttConfig.js"), "utf8");
  for (const fn of ["lerMicSalvo", "salvarMic"]) {
    assert.ok(
      new RegExp(`export function ${fn}\\b`).test(config),
      `pttConfig.js perdeu \`${fn}\` — a escolha do microfone deixou de sobreviver à tela`,
    );
  }

  const settings = fs.readFileSync(
    path.join(SRC, "components", "iracing", "PttEngenheiroSettings.jsx"),
    "utf8",
  );
  assert.ok(
    /salvarMic\(/.test(settings),
    "as configurações trocam o microfone sem gravar a escolha: na corrida ela some",
  );
});
