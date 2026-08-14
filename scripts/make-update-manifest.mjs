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
// --notes vira a tela "o que mudou" (uma linha = um item). Sem --bundle, o caminho
// sai da MESMA política de target-dir do resto (scripts/lib/cargo-target.mjs): a
// variável CARGO_TARGET_DIR quando explícita, senão o que o cargo resolve de dentro
// de src-tauri/. Havia um `src-tauri/target/release/bundle/nsis` cravado aqui, e ele
// era justamente o target ACIDENTAL — o que nasce quando alguém roda o cargo da raiz
// com --manifest-path e que nenhum build de release alimenta. Ver o comentário de
// `bundleNsisPadrao`.
import fs from "fs";
import path from "path";

import { bundleNsisPadrao } from "./lib/cargo-target.mjs";

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

// Sem --bundle, o padrão vem da política de target-dir. Ela pode não responder (cargo fora do
// PATH, por exemplo), e aí o certo é morrer com a instrução — nunca cair num caminho relativo
// de consolo, que é como o target acidental entrava.
let bundleDir = flag("--bundle");
let origemBundle = "--bundle";
if (!bundleDir) {
  const padrao = bundleNsisPadrao({ dirCrate: "src-tauri" });
  if (padrao.erro) {
    console.error(`✗ Sem --bundle e sem conseguir resolver o target do cargo:\n${padrao.erro}`);
    process.exit(1);
  }
  bundleDir = padrao.caminho;
  origemBundle = padrao.origem;
}
if (!fs.existsSync(bundleDir)) {
  console.error(`✗ Pasta do bundle não existe: ${bundleDir}  (origem: ${origemBundle})`);
  console.error(
    "  Rode `npm run tauri build` primeiro, ou passe --bundle <caminho>.",
  );
  process.exit(1);
}

// Windows/NSIS: o updater usa o instalador -setup.exe direto; a assinatura fica
// no .sig ao lado. A pasta guarda builds de VÁRIAS versões (comum, o Tauri não limpa a
// anterior), então o casamento é pela `version` atual e SÓ por ela.
//
// Não há fallback para "qualquer -setup.exe". O fallback existia e era a pior falha
// possível deste script: sem o instalador da versão pedida, ele pegava o da versão
// vizinha, escrevia um manifesto dizendo `version: <nova>` apontando para o binário
// <antigo>, e publicava. O jogador baixava, instalava a versão velha por cima, e o
// updater oferecia a atualização de novo no próximo boot, para sempre. Faltar o
// instalador é erro; publicar o errado é armadilha.
//
// A comparação exige a versão DELIMITADA: `includes("0.14.1")` casa dentro de
// `Loop_0.14.10_x64-setup.exe`, que é exatamente a versão vizinha que não pode passar.
const files = fs.readdirSync(bundleDir);
const versaoDelimitada = new RegExp(
  `(?:^|[^0-9.])${version.replace(/\./g, "\\.")}(?:[^0-9.]|$)`,
);
const pickSetup = (re) => files.find((f) => versaoDelimitada.test(f) && re.test(f));
const setup = pickSetup(/-setup\.exe$/i);
const sig = pickSetup(/-setup\.exe\.sig$/i);
if (!setup || !sig) {
  const outras = files.filter((f) => /-setup\.exe$/i.test(f));
  console.error(`✗ Não achei o setup.exe + .sig da versão ${version} em ${bundleDir}`);
  if (outras.length) {
    console.error(`  Há instaladores de OUTRAS versões: ${outras.join(", ")}`);
    console.error("  Nenhum deles serve — o manifesto publicaria a versão errada.");
  }
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
