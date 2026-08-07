// Guard estrutural: `listen` do Tauri registrado dentro de um efeito é sempre removível.
//
// O padrão que este teste protege é uma armadilha de tempo, e ela já mordeu:
//
//     useEffect(() => {
//       const limpezas = [];
//       (async () => { limpezas.push(await listen("evento", ...)); })();
//       return () => limpezas.forEach((f) => f());     // <- roda ANTES do await terminar
//     }, []);
//
// `listen` é assíncrono e a limpeza do efeito é síncrona. Se a desmontagem acontecer antes
// de a promessa resolver — o que o StrictMode do React provoca DE PROPÓSITO em dev, e o
// que uma navegação rápida provoca em produção —, `limpezas` ainda está vazio quando a
// limpeza roda. O ouvinte nasce depois, órfão: vivo, e sem ninguém que saiba removê-lo.
//
// O sintoma não parece um vazamento. No push-to-talk ele apareceu como cada toque no botão
// virando DUAS perguntas: dois `apertar`, dois `soltar`, e um deles registrado como toque
// acidental. Nada no console, nenhum erro — só o engenheiro fazendo tudo em dobro.
//
// O conserto é uma bandeira: se a limpeza já rodou, desregistra na hora em vez de guardar.
// É esse `morto`/`cancelado` que o guard procura.

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const SRC = path.join(raiz, "src");

function arquivosComListenAssincrono() {
  const achados = [];
  const visitar = (dir) => {
    for (const entrada of fs.readdirSync(dir, { withFileTypes: true })) {
      const alvo = path.join(dir, entrada.name);
      if (entrada.isDirectory()) {
        visitar(alvo);
        continue;
      }
      if (!/\.(js|jsx)$/.test(entrada.name)) continue;
      if (/\.test\.(js|jsx)$/.test(entrada.name)) continue;
      const texto = fs.readFileSync(alvo, "utf8");
      // Só interessa o `listen` do Tauri esperado dentro de um efeito. `addEventListener`
      // do DOM é síncrono e não tem este problema.
      if (!/from\s+["']@tauri-apps\/api\/event["']/.test(texto)) continue;
      if (!/await\s+listen\s*\(/.test(texto)) continue;
      // Só dentro de EFEITO. `listen` numa função solta (o `ttsRunner` da POC) não tem
      // limpeza para correr contra — o problema é a desmontagem, não a espera.
      if (!/useEffect\s*\(/.test(texto)) continue;
      achados.push({ arquivo: path.relative(raiz, alvo), texto });
    }
  };
  visitar(SRC);
  return achados;
}

test("quem espera por listen num efeito sabe desregistrar o que chegou tarde", () => {
  const arquivos = arquivosComListenAssincrono();
  assert.ok(
    arquivos.length > 0,
    "ninguém mais usa `await listen(...)` — ou o guard perdeu o alvo, ou os eventos sumiram",
  );
  for (const { arquivo, texto } of arquivos) {
    // A bandeira pode ter qualquer nome; o que importa é existir uma marca de
    // "já desmontei" que a continuação assíncrona consulte antes de guardar a limpeza.
    const temBandeira = /\blet\s+(morto|cancelado|desmontado|parado)\b/.test(texto);
    assert.ok(
      temBandeira,
      `${arquivo} dá await em \`listen\` dentro de um efeito sem bandeira de desmontagem. ` +
        "Se o componente sair antes de a promessa resolver, o ouvinte fica órfão e os " +
        "eventos passam a chegar em dobro. Use `let morto = false` e desregistre na hora.",
    );
    // A bandeira só serve se a limpeza a levantar.
    assert.ok(
      /return\s*\(\)\s*=>\s*\{[\s\S]{0,200}?(morto|cancelado|desmontado|parado)\s*=\s*true/.test(
        texto,
      ),
      `${arquivo} tem a bandeira mas a limpeza do efeito não a levanta — ela nunca vira true`,
    );
  }
});
