// A etapa 8 do release lê o `latest.json` que ACABOU de subir. Ela roda depois do upload,
// que é o ponto onde o rollback morre — daí a única saída boa quando o manifesto vem torto
// é uma mensagem que diga onde olhar. Um `TypeError` no meio do script não diz.
//
// Estes guards travam as duas coisas: a leitura defensiva do manifesto e a ordem, para a
// validação continuar acontecendo depois do desarme do rollback (e nunca desfazer a árvore
// com instalador já publicado).
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { urlDoInstalador } from "../lib/manifesto-publicado.mjs";

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const src = readFileSync(join(RAIZ, "scripts", "release.mjs"), "utf8");

const CONTEXTO = { alvo: "windows-x86_64-beta", canal: "beta", versao: "0.15.0" };

test("manifesto sem a plataforma do canal vira erro de domínio, não TypeError", () => {
  const live = {
    version: "0.15.0",
    platforms: { "windows-x86_64": { url: "https://exemplo/setup.exe", signature: "x" } },
  };
  const r = urlDoInstalador(live, CONTEXTO);

  assert.equal(r.url, undefined, "não podia devolver URL nenhuma");
  assert.ok(r.erro, "faltou o erro");
  // A mensagem tem que dizer versão, canal e target — é o que decide onde procurar.
  assert.match(r.erro, /0\.15\.0/);
  assert.match(r.erro, /beta/);
  assert.match(r.erro, /windows-x86_64-beta/);
  // E mostrar o que o manifesto TEM, senão o operador abre o arquivo na mão.
  assert.match(r.erro, /windows-x86_64`/);
  // Depois do upload não se desfaz nada localmente.
  assert.match(r.erro, /JÁ foi publicado/);
});

test("manifesto sem o bloco platforms também não estoura", () => {
  const r = urlDoInstalador({ version: "0.15.0", notes: "nada" }, CONTEXTO);
  assert.ok(r.erro);
  assert.match(r.erro, /platforms/);
  assert.match(r.erro, /windows-x86_64-beta/);
});

test("plataforma presente mas sem url utilizável é erro", () => {
  for (const plataforma of [{}, { url: "" }, { url: "   " }, { url: 42 }, { url: null }]) {
    const live = { platforms: { "windows-x86_64-beta": plataforma } };
    const r = urlDoInstalador(live, CONTEXTO);
    assert.ok(r.erro, `passou batido com ${JSON.stringify(plataforma)}`);
    assert.match(r.erro, /url/);
  }
});

test("manifesto que não é objeto não estoura", () => {
  for (const live of [null, undefined, "texto", 7, []]) {
    const r = urlDoInstalador(live, CONTEXTO);
    assert.ok(r.erro, `passou batido com ${JSON.stringify(live) ?? "undefined"}`);
  }
});

test("o caminho feliz devolve a url, sem espaço em volta", () => {
  const live = {
    platforms: { "windows-x86_64-beta": { url: "  https://exemplo/Loop_0.15.0_x64-setup.exe " } },
  };
  const r = urlDoInstalador(live, CONTEXTO);
  assert.equal(r.erro, undefined);
  assert.equal(r.url, "https://exemplo/Loop_0.15.0_x64-setup.exe");
});

test("o release não indexa platforms direto, e valida depois do desarme do rollback", () => {
  // Só o código: o comentário que cita a forma antiga é o que mantém a razão viva.
  const codigo = src
    .split("\n")
    .filter((l) => !l.trimStart().startsWith("//"))
    .join("\n");
  assert.doesNotMatch(
    codigo,
    /live\.platforms\[/,
    "voltou o acesso direto a live.platforms[...] — é ele que produz o TypeError",
  );
  assert.match(src, /urlDoInstalador\(live, \{/, "sumiu a validação do manifesto publicado");

  const desarma = src.indexOf('step(7, "Publicando no bucket');
  const valida = src.indexOf("urlDoInstalador(live");
  assert.ok(desarma > 0 && valida > desarma, "a validação roda antes do upload desarmar o rollback");
});
