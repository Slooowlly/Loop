// Onde o Rust escreve. UMA política, para o desenvolvimento, o release e o CI apontarem
// para o mesmo cache.
//
// Chegamos a ter três: o `.cargo/config.toml` do desenvolvimento, o `C:/dev/loop-target`
// que o release cravava no código, e o `src-tauri/target` que nasce sozinho quando alguém
// roda o cargo da RAIZ com `--manifest-path` — o cargo procura o `.cargo/config.toml` a
// partir do diretório atual, então de fora do crate a config do projeto simplesmente não
// existe e o padrão volta a valer. Três caches do mesmo crate, dezenas de GB cada, e todo
// build que trocava de caminho recompilava tudo do zero.
//
// A política, nesta ordem:
//   1. `CARGO_TARGET_DIR` explícito no ambiente vence. É o que o CI usa.
//   2. Sem ele, vale o que o cargo resolve DE DENTRO do crate — o mesmo mecanismo do
//      desenvolvimento, sem caminho de máquina escrito aqui.
//
// Quem pergunta é o `cargo metadata`, e não um parser nosso do config.toml: ele já
// conhece a hierarquia de configs, o perfil e as variáveis de ambiente, e responde em
// segundos porque não compila nada.
import path from "path";
import { spawnSync } from "child_process";

/// O leitor de verdade. Separado para o teste poder injetar um dublê e não depender de
/// ter o toolchain de Rust instalado.
export function cargoMetadata(dirCrate) {
  // Sem `shell: true`: o cargo é um executável de verdade, não um .cmd como o npm, e passar
  // argumentos por shell só rende o aviso de depreciação do Node.
  const r = spawnSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], {
    cwd: dirCrate,
    encoding: "utf8",
  });
  if (r.error) return { ok: false, motivo: r.error.message };
  if (r.status !== 0) {
    return { ok: false, motivo: (r.stderr || `cargo metadata saiu com ${r.status}`).trim() };
  }
  try {
    return { ok: true, targetDirectory: JSON.parse(r.stdout).target_directory };
  } catch (e) {
    return { ok: false, motivo: `a saída do cargo metadata não é JSON: ${e.message}` };
  }
}

/// Devolve `{ caminho, origem }` ou `{ erro }`. Nunca lança: quem chama é um script de
/// release que precisa morrer com mensagem própria.
export function resolverTargetDir({
  dirCrate = "src-tauri",
  env = process.env,
  metadata = cargoMetadata,
} = {}) {
  const explicito = (env.CARGO_TARGET_DIR ?? "").trim();
  if (explicito) {
    return { caminho: path.resolve(explicito), origem: "CARGO_TARGET_DIR" };
  }

  const meta = metadata(dirCrate);
  if (!meta?.ok) {
    return {
      erro:
        `Não consegui descobrir o target-dir do cargo a partir de ${dirCrate}/: ${meta?.motivo}\n` +
        `  Rode o cargo uma vez de dentro de ${dirCrate}/ para ver o erro completo,\n` +
        `  ou defina CARGO_TARGET_DIR apontando para o mesmo diretório do build de desenvolvimento.`,
    };
  }
  const alvo = (meta.targetDirectory ?? "").trim();
  if (!alvo) {
    return {
      erro:
        `O cargo metadata respondeu sem target_directory (rodando em ${dirCrate}/).\n` +
        `  Defina CARGO_TARGET_DIR para seguir.`,
    };
  }
  return { caminho: path.resolve(alvo), origem: `${dirCrate}/.cargo/config.toml` };
}

/// Onde o bundler do Tauri deixa o instalador NSIS, derivado do target da política acima.
///
/// Existe para o `make-update-manifest.mjs` não ter o caminho escrito no código. O padrão dele
/// era `src-tauri/target/release/bundle/nsis` cru — literalmente o target ACIDENTAL, aquele que
/// nasce quando alguém roda o cargo da raiz com `--manifest-path` e que nenhum build de release
/// usa. Rodado à mão sem `--bundle`, o script ia olhar numa pasta que ou não existe (erro, o
/// caso bom) ou guarda o bundle de um experimento antigo — e aí ele lê o `.sig` de um
/// instalador que não é o que acabou de ser assinado, e o manifesto publicado aponta para um
/// binário que ninguém vai subir.
///
/// Devolve `{ caminho, origem }` ou `{ erro }`, como o `resolverTargetDir`.
export function bundleNsisPadrao(opcoes = {}) {
  const alvo = resolverTargetDir(opcoes);
  if (alvo.erro) return alvo;
  return {
    caminho: path.join(alvo.caminho, "release", "bundle", "nsis"),
    origem: alvo.origem,
  };
}
