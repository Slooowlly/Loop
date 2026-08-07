// Release de uma tacada: bump → build assinado → manifesto → upload → verificação.
//
// Substitui a sequência manual (exportar variáveis, buildar, assinar, gerar o
// latest.json, 2× gcloud). Tudo que era copiar-e-colar vira UM comando.
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

const BUCKET = "loop-updates";
const HOME = os.homedir();
const KEY_PATH = path.join(HOME, ".tauri", "loop-updater-v2.key");
const PASS_PATH = path.join(HOME, ".tauri", "loop-updater-v2.pass");
const TARGET_DIR = process.env.CARGO_TARGET_DIR || "C:/dev/loop-target";
const BUNDLE_DIR = path.join(TARGET_DIR, "release/bundle/nsis");
const TARGETS = { stable: "windows-x86_64", beta: "windows-x86_64-beta" };

const args = process.argv.slice(2);
const flag = (n, d = null) => {
  const i = args.indexOf(n);
  return i >= 0 ? args[i + 1] : d;
};
const die = (msg) => {
  console.error(`\n✗ ${msg}\n`);
  process.exit(1);
};
const step = (n, msg) => console.log(`\n[${n}/7] ${msg}`);

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

// ---------- Versão ----------
const CONF = "src-tauri/tauri.conf.json";
const CARGO = "src-tauri/Cargo.toml";
const PKG = "package.json";
const current = JSON.parse(fs.readFileSync(CONF, "utf8")).version;

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

step(1, `Versão ${current} → ${version} (canal: ${channel})`);
// Grava nos 3 arquivos que definem a versão (menu, updater e binário).
fs.writeFileSync(
  CONF,
  fs.readFileSync(CONF, "utf8").replace(`"version": "${current}"`, `"version": "${version}"`),
);
fs.writeFileSync(
  PKG,
  fs.readFileSync(PKG, "utf8").replace(`"version": "${current}"`, `"version": "${version}"`),
);
fs.writeFileSync(
  CARGO,
  fs.readFileSync(CARGO, "utf8").replace(`version = "${current}"`, `version = "${version}"`),
);

// ---------- Build assinado ----------
const run = (cmd, cmdArgs, extraEnv = {}) =>
  spawnSync(cmd, cmdArgs, {
    stdio: "inherit",
    shell: true,
    env: { ...process.env, ...extraEnv },
  });

// ---------- API layer do OpenXR ----------
// O `beforeBuildCommand` também compila a layer, então isto é redundante por design:
// aqui ela falha em SEGUNDOS se o toolchain de C++ não estiver de pé, em vez de depois
// de 6-8 minutos de vite + cargo. A layer é o leitor do overlay de VR — sem ela no
// bundle, o VR não existe na máquina do jogador.
step(2, "Compilando a API layer do OpenXR…");
const layer = run("node", ["scripts/build-vr-layer.mjs"]);
if (layer.status !== 0) {
  die("Build da API layer falhou — precisa do Visual Studio 2022 com o workload de C++.");
}

step(3, "Build assinado (leva ~6-8 min)…");
const build = run("npm", ["run", "tauri", "build", "--", "--bundles", "nsis"], {
  CARGO_TARGET_DIR: TARGET_DIR,
  TAURI_SIGNING_PRIVATE_KEY: chave,
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: password,
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
step(4, "Conferindo assinatura…");
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
    TAURI_SIGNING_PRIVATE_KEY: chave,
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: password,
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
step(5, "Gerando manifesto…");
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
step(6, "Publicando no bucket…");
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
step(7, "Verificando no ar…");
const manifestUrl = `https://storage.googleapis.com/${BUCKET}/updates/${TARGETS[channel]}/latest.json`;
const live = await (await fetch(manifestUrl, { cache: "no-store" })).json();
if (live.version !== version) {
  die(`Manifesto publicado diz ${live.version}, esperado ${version}.`);
}
const head = await fetch(live.platforms[TARGETS[channel]].url, { method: "HEAD" });
if (!head.ok) die(`Instalador não acessível (HTTP ${head.status}).`);

console.log(`
✓ ${version} publicada no canal ${channel}

  Download: ${live.platforms[TARGETS[channel]].url}
  Manifesto: ${manifestUrl}
`);
