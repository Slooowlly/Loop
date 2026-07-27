/**
 * Lógica central do auto-update (sem React). Encapsula o plugin do Tauri e a
 * seleção de canal (estável x beta).
 *
 * CANAIS: o plugin não deixa trocar o endpoint em runtime — só o parâmetro
 * `target` (que substitui `{{target}}` na URL do endpoint, ver tauri.conf.json).
 * Então cada canal é um `target` diferente, apontando pra um manifesto próprio:
 *   estável → windows-x86_64      → .../updates/windows-x86_64/latest.json
 *   beta    → windows-x86_64-beta → .../updates/windows-x86_64-beta/latest.json
 * O app é Windows-only (iRacing), por isso o target é fixo aqui.
 */

import { estaNoTauri } from "./tauri";

export const CHANNELS = {
  stable: "windows-x86_64",
  beta: "windows-x86_64-beta",
};

export function isTauri() {
  return estaNoTauri();
}

/**
 * Checa o canal pedido. Retorna o objeto Update (com .version, .body, etc.) se
 * houver versão nova, ou null se já está atualizado / fora do Tauri.
 * Nunca lança: erro de rede vira null (com log), pra não travar a UI.
 */
export async function checkChannel(channel = "stable") {
  if (!estaNoTauri()) return null;
  const target = CHANNELS[channel] ?? CHANNELS.stable;
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    return await check({ target });
  } catch (err) {
    console.warn(`[updater] check(${channel}) falhou:`, err);
    return null;
  }
}

/**
 * Baixa + instala o update e reinicia o app. `onProgress(fraction|null)` recebe
 * 0..1 quando o tamanho é conhecido, ou null (indeterminado) enquanto não é.
 */
export async function downloadInstallRelaunch(update, onProgress) {
  let total = 0;
  let received = 0;
  await update.downloadAndInstall((event) => {
    switch (event.event) {
      case "Started":
        total = event.data?.contentLength ?? 0;
        onProgress?.(total ? 0 : null);
        break;
      case "Progress":
        received += event.data?.chunkLength ?? 0;
        onProgress?.(total ? Math.min(received / total, 1) : null);
        break;
      case "Finished":
        onProgress?.(1);
        break;
      default:
        break;
    }
  });
  const { relaunch } = await import("@tauri-apps/plugin-process");
  await relaunch();
}
