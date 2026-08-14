// O release escreve a versão em três arquivos ANTES de buildar — não tem como ser diferente, o
// binário carimba o número em tempo de compilação. Isso torna todo o resto do script uma janela
// em que a árvore pode ficar com a versão nova e o bucket com nada: build de 8 minutos,
// assinatura intermitente, dois uploads, verificação em rede.
//
// Estes guards travam a forma que fecha essa janela: suíte verde antes da primeira escrita,
// troca conferida uma a uma, rollback armado.
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const src = readFileSync(join(RAIZ, "scripts", "release.mjs"), "utf8");

/// Índice da primeira escrita em arquivo do repositório. As notas do changelog vão para um
/// temporário em `os.tmpdir()` e não contam — o que importa é quando a ÁRVORE muda.
const primeiraEscrita = src.indexOf("fs.writeFileSync(caminho, depois)");

test("a suíte inteira roda antes da primeira escrita", () => {
  const js = src.indexOf('"test:all"');
  const rust = src.indexOf('run("cargo"');
  assert.ok(js > 0, "sumiu a chamada do npm run test:all");
  assert.ok(rust > 0, "sumiu a chamada do cargo test");
  assert.ok(primeiraEscrita > 0, "sumiu a escrita do bump");
  assert.ok(js < primeiraEscrita, "os testes de JS rodam depois do bump");
  assert.ok(rust < primeiraEscrita, "os testes de Rust rodam depois do bump");
});

// A política de target-dir tem guard próprio em release-target-unico.test.mjs.

test("o bump valida ocorrência única antes de trocar", () => {
  assert.match(src, /ocorrencias !== 1/);
  assert.ok(src.indexOf("ocorrencias !== 1") < primeiraEscrita, "a validação vem depois da escrita");
});

test("cada troca é provada, e a versão é reconferida depois", () => {
  assert.match(src, /if \(depois === antes\)/, "sumiu a prova de que o replace mudou o texto");
  assert.match(src, /const depoisDoBump = lerVersoes\(\)/, "sumiu a reconferência dos três arquivos");
});

test("o rollback fica armado assim que a escrita acontece", () => {
  const arma = src.indexOf("aoMorrer = desfazerBump");
  assert.ok(arma > primeiraEscrita, "o rollback não é armado depois da escrita");
  assert.match(src, /function desfazerBump\(\)/);
  // E o `die` tem que passar por ele, senão o rollback nunca é chamado.
  assert.match(src, /const die = \(msg\) => \{[\s\S]{0,200}aoMorrer\(\)/);
});

test("depois que o upload começa, o rollback é desarmado", () => {
  const upload = src.indexOf('step(7, "Publicando no bucket');
  const desarma = src.indexOf("aoMorrer = () => {", upload);
  assert.ok(upload > 0 && desarma > upload, "o rollback continua armado durante o upload");
});

test("os três arquivos de versão concordam hoje", () => {
  const conf = JSON.parse(readFileSync(join(RAIZ, "src-tauri", "tauri.conf.json"), "utf8")).version;
  const pkg = JSON.parse(readFileSync(join(RAIZ, "package.json"), "utf8")).version;
  const cargo = readFileSync(join(RAIZ, "src-tauri", "Cargo.toml"), "utf8").match(/^version = "([^"]+)"$/m);
  assert.equal(pkg, conf, "package.json e tauri.conf.json divergem");
  assert.equal(cargo?.[1], conf, "Cargo.toml e tauri.conf.json divergem");
});
