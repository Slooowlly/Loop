import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

// Versao: fonte unica no package.json (espelha tauri.conf.json / Cargo.toml).
const pkg = JSON.parse(
  readFileSync(fileURLToPath(new URL("./package.json", import.meta.url)), "utf8"),
);

// Build: contagem de commits do git (cresce sozinho a cada commit). Fallback 0.
function gitBuildNumber() {
  try {
    return Number(execSync("git rev-list --count HEAD").toString().trim()) || 0;
  } catch {
    return 0;
  }
}

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
    __APP_BUILD__: JSON.stringify(gitBuildNumber()),
  },
  build: {
    // As peças de voz NUNCA são inlinadas, por curtas que sejam.
    //
    // Isto passou a importar quando o acervo virou Opus. Em WAV a peça mais curta tinha ~24 kB
    // e o teto de 4 kB do Vite nunca era atingido; em Opus, 432 das 3.994 peças caem abaixo
    // dele — e viraram 1,68 MB de base64 DENTRO do chunk principal, parseado no startup.
    //
    // O custo não é só o tamanho. O carregamento das peças é deliberadamente preguiçoso e
    // limitado (`src/lib/filaDeCarga.js`): o acervo tem 3.994 arquivos, e a fila existe para
    // que uma resposta do engenheiro não dispare uma avalanche de requisições no meio de uma
    // corrida. Peça inlinada não passa por essa fila — ela já está na memória, sempre, quer o
    // jogador use o rádio ou não.
    assetsInlineLimit: (caminho) =>
      /[\\/]assets[\\/](engenheiro|spotter)[\\/].*\.opus$/.test(caminho) ? false : undefined,
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: "./src/test/setup.js",
    include: ["src/**/*.{test,spec}.{js,jsx,ts,tsx}"],
    exclude: ["scripts/tests/**", "src-tauri/**", "dist/**", "node_modules/**"],
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
