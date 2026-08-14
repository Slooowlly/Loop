// Release de uma tacada: testes → bump → build assinado → manifesto → upload → verificação.
//
// Substitui a sequência manual (exportar variáveis, buildar, assinar, gerar o
// latest.json, 2× gcloud). Tudo que era copiar-e-colar vira UM comando.
//
// As duas primeiras etapas existem para o release nunca ficar pela metade: a suíte inteira
// roda ANTES de qualquer escrita, e o bump dos três arquivos de versão é transacional — valida
// os três, prova que cada troca alterou o texto e devolve tudo se uma etapa posterior morrer.
//
// Uso:
//   node scripts/release.mjs --bump patch --notes "linha1
//   linha2"                                   ← multi-linha SÓ assim (ou --notes-file)
//   npm run release -- --bump minor --notes "uma linha só"
//   npm run release -- --version 1.0.0 --notes-file notas.txt
//
// ⚠️ `npm run` TRUNCA argumentos multi-linha na primeira quebra (limitação do
// npm, acontece antes deste script rodar). Para changelog de várias linhas use
// `node scripts/release.mjs ...` direto, ou `--notes-file`.
//
// A SENHA da chave vem (nesta ordem):
//   1. env TAURI_SIGNING_PRIVATE_KEY_PASSWORD
//   2. arquivo ~/.tauri/loop-updater-v2.pass   ← recomendado, configure 1 vez
// Assim nada de segredo entra em linha de comando nem em histórico de shell.
import fs from "fs";
import path from "path";
import os from "os";
import { spawnSync } from "child_process";
import { resolverTargetDir } from "./lib/cargo-target.mjs";
import { urlDoInstalador } from "./lib/manifesto-publicado.mjs";

const BUCKET = "loop-updates";
const HOME = os.homedir();
const KEY_PATH = path.join(HOME, ".tauri", "loop-updater-v2.key");
const PASS_PATH = path.join(HOME, ".tauri", "loop-updater-v2.pass");
const CRATE_DIR = "src-tauri";
const TARGETS = { stable: "windows-x86_64", beta: "windows-x86_64-beta" };

const args = process.argv.slice(2);
const flag = (n, d = null) => {
  const i = args.indexOf(n);
  return i >= 0 ? args[i + 1] : d;
};
// O que fazer antes de morrer. Começa vazio porque as primeiras mortes (canal inválido, chave
// ausente) acontecem antes de existir qualquer escrita para desfazer; a etapa do bump troca
// isto pelo rollback dela.
let aoMorrer = () => {};
const die = (msg) => {
  console.error(`\n✗ ${msg}\n`);
  aoMorrer();
  process.exit(1);
};
const step = (n, msg) => console.log(`\n[${n}/8] ${msg}`);

const channel = flag("--channel", "stable");
if (!TARGETS[channel]) die(`Canal inválido: ${channel} (use stable ou beta)`);

// ---------- Segredos ----------
//
// O par chave+senha é o que separa um release publicado de um instalador que o updater
// dos jogadores recusa. Ele é conferido AQUI, antes de qualquer build, porque o Tauri
// pula a assinatura em silêncio quando um dos dois não chega — e descobrir isso depois
// de 6-8 minutos de compilação é o caminho conhecido para um release pela metade.
if (!fs.existsSync(KEY_PATH)) die(`Chave não encontrada: ${KEY_PATH}`);
const chave = fs.readFileSync(KEY_PATH, "utf8").trim();
if (!chave) die(`Chave vazia: ${KEY_PATH}`);

// `!password`, e não `password == null`.
//
// A diferença não é estilo. Uma variável de ambiente DEFINIDA E VAZIA — que é o que sobra
// de um `set TAURI_SIGNING_PRIVATE_KEY_PASSWORD=` ou de um shell que exporta a variável sem
// valor — não é `null`. Com a comparação antiga ela passava pelo primeiro `if`, o arquivo
// nunca era lido, e a senha vazia seguia para o build, que pulava a assinatura sem dizer
// nada. O sintoma aparecia sete minutos depois, na etapa 4, como se a chave estivesse
// errada.
let password = process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD;
if (!password && fs.existsSync(PASS_PATH)) {
  password = fs.readFileSync(PASS_PATH, "utf8").trim();
}
if (!password) {
  die(
    `Senha da chave não encontrada (ou vazia).\n  Configure uma vez (PowerShell):\n` +
      `  Read-Host "Senha da chave" | Set-Content "${PASS_PATH}" -NoNewline`,
  );
}

// ---------- Onde o Rust escreve ----------
//
// O release NÃO tem target próprio. Ele usa o mesmo do desenvolvimento, resolvido pela
// política única de scripts/lib/cargo-target.mjs: CARGO_TARGET_DIR quando explícito, senão
// o que o cargo enxerga de dentro de src-tauri/. Havia um `C:/dev/loop-target` cravado
// aqui, e ele era um terceiro cache do mesmo crate: o build de release recompilava do zero
// o que o desenvolvimento já tinha compilado, e nada avisava.
//
// Todo comando que chama cargo daqui roda com o cwd em src-tauri/, e não da raiz com
// `--manifest-path`. Da raiz o cargo não lê o src-tauri/.cargo/config.toml e o target volta
// a ser um `src-tauri/target` novo.
const alvo = resolverTargetDir({ dirCrate: CRATE_DIR });
if (alvo.erro) die(alvo.erro);
const TARGET_DIR = alvo.caminho;
const BUNDLE_DIR = path.join(TARGET_DIR, "release/bundle/nsis");
console.log(`  target do cargo: ${TARGET_DIR}  (${alvo.origem})`);

// ---------- Versão ----------
//
// Três arquivos declaram a versão e os três TÊM que concordar: o `package.json` alimenta o
// `__APP_VERSION__` que aparece no menu, o `tauri.conf.json` é o que o updater compara com a
// instalada, e o `Cargo.toml` carimba o binário. Um deles fora de sincronia produz um release
// que se apresenta com dois números diferentes conforme onde o jogador olha.
const CONF = "src-tauri/tauri.conf.json";
const CARGO = "src-tauri/Cargo.toml";
const PKG = "package.json";

/// A marca textual da versão em cada arquivo, que é o que o bump troca. É por ela que a troca
/// é conferida: um `replace` que não acha o texto devolve a string intacta, em silêncio.
const ARQUIVOS_DE_VERSAO = [
  { caminho: CONF, marca: (v) => `"version": "${v}"` },
  { caminho: PKG, marca: (v) => `"version": "${v}"` },
  { caminho: CARGO, marca: (v) => `version = "${v}"` },
];

function lerVersoes() {
  const cargo = fs.readFileSync(CARGO, "utf8").match(/^version = "([^"]+)"$/m);
  return {
    [CONF]: JSON.parse(fs.readFileSync(CONF, "utf8")).version,
    [PKG]: JSON.parse(fs.readFileSync(PKG, "utf8")).version,
    // Só a linha do `[package]` começa na coluna zero; as das dependências vivem dentro de
    // `{ version = "..." }`, na mesma linha da chave.
    [CARGO]: cargo?.[1] ?? null,
  };
}

const versoes = lerVersoes();
const divergentes = Object.entries(versoes).filter(([, v]) => v !== versoes[CONF]);
if (divergentes.length) {
  die(
    `Os arquivos de versão não concordam:\n` +
      Object.entries(versoes)
        .map(([f, v]) => `    ${f}: ${v ?? "(não achei a linha da versão)"}`)
        .join("\n") +
      `\n  Acerte os três na mão antes de publicar — o bump não tem como adivinhar qual está certo.`,
  );
}
const current = versoes[CONF];

function nextVersion() {
  const explicit = flag("--version");
  if (explicit) return explicit;
  const bump = flag("--bump", "patch");
  const [maj, min, pat] = current.split(".").map(Number);
  if (bump === "major") return `${maj + 1}.0.0`;
  if (bump === "minor") return `${maj}.${min + 1}.0`;
  if (bump === "patch") return `${maj}.${min}.${pat + 1}`;
  die(`--bump inválido: ${bump} (use major, minor ou patch)`);
}
const version = nextVersion();
if (version === current) die(`Versão ${version} é igual à atual — nada a publicar.`);

// Notas do changelog (viram a tela "o que mudou").
const notesFile = flag("--notes-file");
let notes = flag("--notes");
if (notesFile) {
  if (!fs.existsSync(notesFile)) die(`--notes-file não existe: ${notesFile}`);
  notes = fs.readFileSync(notesFile, "utf8").trim();
}
if (!notes) notes = `Loop ${version}`;

const run = (cmd, cmdArgs, { env: extraEnv = {}, cwd } = {}) =>
  spawnSync(cmd, cmdArgs, {
    stdio: "inherit",
    shell: true,
    cwd,
    env: { ...process.env, ...extraEnv },
  });

// ---------- Testes ----------
//
// Antes de qualquer escrita. Um release que sobe com a suíte vermelha custa muito mais caro que
// um release que não sai: o instalador já está no bucket, o manifesto já mandou os jogadores
// atualizarem, e desfazer é publicar de novo. Aqui a suíte vermelha custa alguns minutos e
// NENHUM arquivo tocado — a versão dos três arquivos ainda é a atual quando isto falha.
step(1, "Testes (JS + Rust) antes de tocar em qualquer arquivo…");

// `cargo test` exige o `dist/` construído: `tauri::generate_context!` embute os assets em tempo
// de compilação, e sem eles o crate nem compila. O build do frontend é rápido e o `tauri build`
// da etapa 4 o refaz de qualquer jeito, então rodá-lo aqui não custa nada além de segundos.
if (run("npm", ["run", "build"]).status !== 0) {
  die("Build do frontend falhou — a suíte de Rust nem chega a rodar sem o dist/.");
}
if (run("npm", ["run", "test:all"]).status !== 0) {
  die("Testes de JS falharam. Nada foi bumpado nem publicado.");
}
// Com o cwd em src-tauri/ e sem `--manifest-path`: é assim que o cargo lê o
// src-tauri/.cargo/config.toml e cai no MESMO target que a etapa 4 vai usar, então o que
// compila aqui já serve lá. Da raiz, com `--manifest-path`, ele ignoraria a config e abriria
// um `src-tauri/target` novo — recompilando tudo e brigando pelo lock com outra sessão.
if (run("cargo", ["test"], { cwd: CRATE_DIR }).status !== 0) {
  die("Testes de Rust falharam. Nada foi bumpado nem publicado.");
}

// ---------- Bump ----------
//
// TRANSACIONAL, e não por elegância. O bump é a primeira escrita do release e tudo depois dele
// pode falhar: o build de 8 minutos, a assinatura intermitente, o upload, a verificação. Quando
// isso acontecia, a árvore ficava com a versão nova nos três arquivos e NADA publicado — e a
// execução seguinte partia de um `current` que nunca existiu para ninguém, então o próximo bump
// pulava um número e o guard de "igual à atual" deixava de proteger.
//
// São três garantias, nesta ordem: valida os três arquivos ANTES de escrever em qualquer um,
// prova que cada troca mudou o texto, e devolve tudo ao estado original se uma etapa posterior
// morrer.
step(2, `Versão ${current} → ${version} (canal: ${channel})`);

const original = new Map();
let bumpAplicado = false;

function desfazerBump() {
  if (!bumpAplicado) return;
  bumpAplicado = false;
  for (const [caminho, texto] of original) fs.writeFileSync(caminho, texto);
  console.error(`  ↩ versão devolvida para ${current} nos 3 arquivos — nada ficou pela metade.`);
}

// Passo 1: lê e confere os três. Uma ocorrência, exatamente — zero significa que o arquivo não
// tem a marca esperada (formatação mudou) e o `replace` viraria um no-op silencioso; mais de
// uma significa que a troca acertaria também alguma linha que não é a versão do pacote.
const trocas = [];
for (const { caminho, marca } of ARQUIVOS_DE_VERSAO) {
  const antes = fs.readFileSync(caminho, "utf8");
  const alvo = marca(current);
  const ocorrencias = antes.split(alvo).length - 1;
  if (ocorrencias !== 1) {
    die(
      `${caminho}: achei ${ocorrencias} ocorrência(s) de \`${alvo}\`, esperava exatamente 1.\n` +
        `  O bump não vai adivinhar onde trocar. Nada foi tocado.`,
    );
  }
  original.set(caminho, antes);
  trocas.push({ caminho, antes, depois: antes.replace(alvo, marca(version)) });
}

// Passo 2: escreve, provando que cada arquivo de fato mudou.
for (const { caminho, antes, depois } of trocas) {
  if (depois === antes) {
    desfazerBump();
    die(`${caminho}: a troca de versão não alterou o texto. Nada foi publicado.`);
  }
  fs.writeFileSync(caminho, depois);
  bumpAplicado = true;
}
// Daqui em diante, qualquer `die` devolve os três arquivos ao estado anterior.
aoMorrer = desfazerBump;

// Conferência final pelo mesmo leitor que validou a entrada: os três têm que sair concordando
// na versão NOVA. Se um deles não saiu, é agora que se descobre, com a árvore ainda recuperável.
const depoisDoBump = lerVersoes();
if (Object.values(depoisDoBump).some((v) => v !== version)) {
  desfazerBump();
  die(
    `Depois do bump os arquivos não concordam em ${version}:\n` +
      Object.entries(depoisDoBump)
        .map(([f, v]) => `    ${f}: ${v ?? "(não achei a linha da versão)"}`)
        .join("\n"),
  );
}

// ---------- Build assinado ----------

// ---------- API layer do OpenXR ----------
// O `beforeBuildCommand` também compila a layer, então isto é redundante por design:
// aqui ela falha em SEGUNDOS se o toolchain de C++ não estiver de pé, em vez de depois
// de 6-8 minutos de vite + cargo. A layer é o leitor do overlay de VR — sem ela no
// bundle, o VR não existe na máquina do jogador.
step(3, "Compilando a API layer do OpenXR…");
const layer = run("node", ["scripts/build-vr-layer.mjs"]);
if (layer.status !== 0) {
  die("Build da API layer falhou — precisa do Visual Studio 2022 com o workload de C++.");
}

step(4, "Build assinado (leva ~6-8 min)…");
// Sem CARGO_TARGET_DIR forçado: a CLI do Tauri chama o cargo de dentro de src-tauri/, então
// ela cai sozinha no mesmo target que a etapa 1 usou e que o TARGET_DIR acima aponta.
const build = run("npm", ["run", "tauri", "build", "--", "--bundles", "nsis"], {
  env: {
    TAURI_SIGNING_PRIVATE_KEY: chave,
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: password,
  },
});
if (build.status !== 0) die("Build falhou (veja o erro acima).");

// ---------- Guarda da assinatura ----------
//
// O Tauri PULA a assinatura em silêncio se a senha/chave não chegam — foi o que quebrou
// TRÊS releases (0.13.0, 0.13.3 e 0.14.0; nos dois primeiros o `.sig` foi gerado à mão
// depois, e os carimbos de hora no diretório de bundle ainda mostram isso).
//
// A checagem de segredos lá em cima cobre a causa que sabemos nomear. Mas em 0.14.0 a
// falha aconteceu com chave e senha SADIAS — o mesmo comando, no mesmo disco, assinou
// normalmente vinte minutos depois. Ou seja: é intermitente, e não há causa raiz para
// consertar por enquanto.
//
// Então a etapa deixa de ser um veredito e passa a ser uma RECUPERAÇÃO. Assinar o
// instalador já construído é barato (segundos, sem rebuild) e produz exatamente o mesmo
// `.sig` que o bundler produziria — a assinatura é do arquivo, não do processo que a
// pediu. Morrer aqui, em troca, custava os 6-8 minutos de build de novo e deixava o
// release pela metade: versão bumpada nos três arquivos, nada publicado.
//
// O aviso é alto de propósito. Autocura silenciosa esconderia a intermitência justamente
// enquanto ela ainda não tem explicação.
step(5, "Conferindo assinatura…");
const setupName = `Loop_${version}_x64-setup.exe`;
const setupPath = path.join(BUNDLE_DIR, setupName);
const sigPath = `${setupPath}.sig`;
if (!fs.existsSync(setupPath)) die(`Instalador não gerado: ${setupPath}`);
if (!fs.existsSync(sigPath)) {
  console.warn(
    `\n      ⚠ O BUILD NÃO ASSINOU (${setupName}.sig não existe).\n` +
      `        Chave e senha estavam presentes, então isto é a falha intermitente\n` +
      `        conhecida do bundler. Assinando o instalador agora, sem rebuildar.\n`,
  );
  const assina = run("npx", ["tauri", "signer", "sign", `"${setupPath}"`], {
    env: {
      TAURI_SIGNING_PRIVATE_KEY: chave,
      TAURI_SIGNING_PRIVATE_KEY_PASSWORD: password,
    },
  });
  if (assina.status !== 0 || !fs.existsSync(sigPath)) {
    die(
      `Assinatura manual também falhou (${sigPath}).\n` +
        `  Sem .sig o auto-update não funciona — nada foi publicado.\n` +
        `  Tente à mão:\n` +
        `  npx tauri signer sign -f "${KEY_PATH}" -p '<senha>' "${setupPath}"`,
    );
  }
  console.log(`      recuperado — ${setupName}.sig assinado fora do bundler`);
} else {
  console.log(`      ok — ${setupName}.sig`);
}

// A assinatura existe, mas ela é do instalador QUE ESTÁ AQUI? Um `.sig` sobrando de uma
// tentativa anterior passaria na checagem de existência e publicaria um manifesto que o
// updater recusa — o diretório de bundle guarda os artefatos de todas as versões, e já
// houve um `.sig` órfão sem o `.exe` correspondente.
if (fs.statSync(sigPath).mtimeMs < fs.statSync(setupPath).mtimeMs) {
  die(
    `O ${setupName}.sig é MAIS VELHO que o instalador.\n` +
      `  Ele assina outro arquivo, e o updater vai recusar o update.\n` +
      `  Apague o .sig e rode a assinatura de novo:\n` +
      `  npx tauri signer sign -f "${KEY_PATH}" -p '<senha>' "${setupPath}"`,
  );
}

// ---------- Manifesto ----------
step(6, "Gerando manifesto…");
// As notas vão por ARQUIVO, não por argumento: passar texto multi-linha pelo
// shell viraria "\n" literal dentro do manifesto (e a tela "o que mudou"
// mostraria a barra invertida em vez de quebrar a linha).
const notesTmp = path.join(os.tmpdir(), `loop-release-notes-${version}.txt`);
fs.writeFileSync(notesTmp, notes, "utf8");
const man = run("node", [
  "scripts/make-update-manifest.mjs",
  "--channel", channel,
  "--bundle", `"${BUNDLE_DIR}"`,
  "--notes-file", `"${notesTmp}"`,
]);
fs.rmSync(notesTmp, { force: true });
if (man.status !== 0) die("Falha ao gerar o manifesto.");

// ---------- Upload ----------
//
// Aqui acaba o rollback. Até esta linha, morrer significa "não saiu nada" e devolver a versão
// aos três arquivos é a verdade. A partir do primeiro `cp`, alguma coisa PODE estar no bucket —
// e árvore dizendo 0.14.0 com instalador 0.14.1 publicado é pior que a versão bumpada: some o
// único registro local de qual número já foi ao ar.
step(7, "Publicando no bucket…");
aoMorrer = () => {
  console.error(
    `  ⚠ A versão ${version} FICA nos três arquivos: o upload já tinha começado.\n` +
      `    Confira o bucket antes de rodar de novo — pode haver instalador publicado sem manifesto.`,
  );
};
const manifestPath = path.join(BUNDLE_DIR, "latest.json");
const up1 = run("gcloud", ["storage", "cp", `"${setupPath}"`, `gs://${BUCKET}/downloads/`]);
if (up1.status !== 0) die("Upload do instalador falhou.");
const up2 = run("gcloud", [
  "storage", "cp",
  "--cache-control=\"public, max-age=60\"",
  `"${manifestPath}"`,
  `gs://${BUCKET}/updates/${TARGETS[channel]}/latest.json`,
]);
if (up2.status !== 0) die("Upload do manifesto falhou.");

// ---------- Verificação ----------
step(8, "Verificando no ar…");
const manifestUrl = `https://storage.googleapis.com/${BUCKET}/updates/${TARGETS[channel]}/latest.json`;
// `fetch` que estoura a rede LANÇA em vez de devolver resposta, e uma exceção solta aqui
// terminaria o script com stack trace e sem o aviso de que o release já está no ar.
let live;
try {
  live = await (await fetch(manifestUrl, { cache: "no-store" })).json();
} catch (e) {
  die(`Não consegui ler o manifesto publicado (${manifestUrl}): ${e.message}`);
}
if (live.version !== version) {
  die(`Manifesto publicado diz ${live.version}, esperado ${version}.`);
}
// A estrutura do manifesto é conferida ANTES de ler a URL. Um `live.platforms[target].url`
// direto estourava `TypeError` quando o manifesto subia sem a plataforma do canal — no meio
// da etapa 8, com o instalador já no bucket, e sem dizer qual canal olhar. Aqui isso é erro
// de domínio com versão, canal e target na mensagem. O rollback continua desarmado desde a
// etapa 7: nada na árvore é desfeito depois que o upload aconteceu.
const publicado = urlDoInstalador(live, {
  alvo: TARGETS[channel],
  canal: channel,
  versao: version,
});
if (publicado.erro) die(publicado.erro);

let head;
try {
  head = await fetch(publicado.url, { method: "HEAD" });
} catch (e) {
  die(`Não consegui alcançar o instalador publicado: ${e.message}`);
}
if (!head.ok) die(`Instalador não acessível (HTTP ${head.status}).`);

console.log(`
✓ ${version} publicada no canal ${channel}

  Download: ${publicado.url}
  Manifesto: ${manifestUrl}
`);
