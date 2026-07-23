// Gera o `latest.json` do auto-update a partir dos artefatos do build.
//
// O updater do Tauri baixa este JSON, compara `version` com a instalada e, se for
// mais nova, baixa o instalador da `url` e valida o `signature` contra a pubkey
// do tauri.conf.json. A assinatura TEM que ser o conteúdo EXATO do arquivo .sig —
// este script copia pra você não errar isso na mão.
//
// CANAIS: o app troca de canal pelo parâmetro `target` do plugin, que vira o
// segmento `{{target}}` da URL do endpoint (ver tauri.conf.json e src/lib/updater.js):
//   estável → updates/windows-x86_64/latest.json
//   beta    → updates/windows-x86_64-beta/latest.json
//
// Uso (rode DEPOIS de `npm run tauri build`):
//   node scripts/make-update-manifest.mjs [--channel stable|beta] \
//        [--bundle "<pasta com setup.exe + .sig>"] \
//        [--notes "linha1\nlinha2"  |  --notes-file caminho.txt]
//
// --notes vira a tela "o que mudou" (uma linha = um item). Sem --bundle, procura
// em src-tauri/target/release/bundle/nsis (você builda fora do OneDrive com
// CARGO_TARGET_DIR — nesse caso passe --bundle apontando pro nsis de lá).
import fs from "fs";
import path from "path";

const BUCKET = "loop-updates"; // troque se usar outro nome de bucket
const TARGETS = { stable: "windows-x86_64", beta: "windows-x86_64-beta" };

const args = process.argv.slice(2);
const flag = (name, def = null) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : def;
};

const channel = flag("--channel", "stable");
if (!TARGETS[channel]) {
  console.error(`✗ Canal inválido: ${channel} (use stable ou beta)`);
  process.exit(1);
}
const target = TARGETS[channel];

// Versão canônica: lê do tauri.conf.json (fonte da verdade do app empacotado).
const conf = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
const version = conf.version;

// Notas do changelog: --notes-file tem prioridade sobre --notes.
const notesFile = flag("--notes-file");
let notes = flag("--notes", `Loop ${version}`);
if (notesFile) {
  if (!fs.existsSync(notesFile)) {
    console.error(`✗ --notes-file não existe: ${notesFile}`);
    process.exit(1);
  }
  notes = fs.readFileSync(notesFile, "utf8").trim();
}

const bundleDir = flag("--bundle") ?? "src-tauri/target/release/bundle/nsis";
if (!fs.existsSync(bundleDir)) {
  console.error(`✗ Pasta do bundle não existe: ${bundleDir}`);
  console.error(
    "  Rode `npm run tauri build` primeiro, ou passe --bundle <caminho>.",
  );
  process.exit(1);
}

// Windows/NSIS: o updater usa o instalador -setup.exe direto; a assinatura fica
// no .sig ao lado. Se a pasta tiver builds de VÁRIAS versões (comum, o Tauri não
// limpa a anterior), casamos pela `version` atual — senão pegaríamos o exe errado.
const files = fs.readdirSync(bundleDir);
const pickSetup = (re) =>
  files.find((f) => f.includes(version) && re.test(f)) ??
  files.find((f) => re.test(f));
const setup = pickSetup(/-setup\.exe$/i);
const sig = pickSetup(/-setup\.exe\.sig$/i);
if (!setup || !sig) {
  console.error(`✗ Não achei setup.exe + .sig em ${bundleDir}`);
  console.error(`  Arquivos: ${files.join(", ") || "(vazio)"}`);
  console.error("  Confirme que createUpdaterArtifacts=true e o build assinou.");
  process.exit(1);
}

const signature = fs.readFileSync(path.join(bundleDir, sig), "utf8").trim();
const url = `https://storage.googleapis.com/${BUCKET}/downloads/${encodeURIComponent(setup)}`;
const entry = { signature, url };

// platforms: a chave tem que bater com o `target` que o app usou no check().
// No beta incluímos as DUAS chaves (hedge) apontando pro mesmo build, pra não
// depender do detalhe interno de qual chave o plugin procura.
const platforms =
  channel === "beta"
    ? { "windows-x86_64-beta": entry, "windows-x86_64": entry }
    : { "windows-x86_64": entry };

const manifest = {
  version,
  notes,
  // Sem Date.now() hardcoded: usa a hora do build. RFC 3339.
  pub_date: new Date().toISOString(),
  platforms,
};

const outPath = path.join(bundleDir, "latest.json");
fs.writeFileSync(outPath, JSON.stringify(manifest, null, 2) + "\n");

const remoteManifest = `gs://${BUCKET}/updates/${target}/latest.json`;
console.log(`✓ Gerado ${outPath}  (canal=${channel} · v${version})`);
console.log("\nSuba os 2 arquivos pro bucket público:\n");
console.log(`  gsutil cp "${path.join(bundleDir, setup)}" gs://${BUCKET}/downloads/`);
console.log(
  `  gsutil -h "Cache-Control:public, max-age=60" cp "${outPath}" ${remoteManifest}`,
);
if (channel === "beta") {
  console.log(
    "\n(beta: só quem clicar 5× na versão no menu vê esta atualização.)",
  );
}
