import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import useCareerStore from "../stores/useCareerStore";
import { VR_W, VR_H, drawTower, preloadAssets } from "./towerCanvas";
import { createTowerAnimator } from "./towerAnimation";
import { createTowerWindow } from "./towerRows";
import { VR_THEME } from "./towerThemes";
import { useOverlayData } from "./useOverlayData";
import { estaNoTauri } from "../lib/tauri";

// Escritor do overlay de VR: desenha a torre com dados AO VIVO da corrida e manda
// os pixels (~10 Hz) pro backend, que os coloca na memória compartilhada lida pela
// OpenXR API layer dentro do iRacing.
//
// Roda só dentro do Tauri. Quando não há sessão ativa no iRacing (ou nenhuma
// carreira carregada), não escreve nada — o overlay simplesmente não aparece.
// É invisível na interface: não renderiza nada, só alimenta o overlay do VR.
//
// TODO(produtização): hoje liga sozinho quando o app abre. Depois vai atrás de um
// toggle nas configs.

const VR_IDLE_MS = 100; // 10 Hz: torre parada, o que ela é quase o tempo todo
const VR_MOVING_MS = 33; // ~30 Hz: só durante o deslize de uma ultrapassagem

export default function OverlayVrWriter() {
  const careerId = useCareerStore((s) => s.careerId);
  // 500 ms, igual ao monitor: a 1 s a troca de posição só era descoberta até um
  // segundo depois de acontecer na pista, e o deslize saía visivelmente fora de hora.
  const data = useOverlayData(careerId, { intervalMs: 500 });
  // A "fonte" que o loop de desenho lê: dados + assets já carregados.
  const sourceRef = useRef({ data: null, assets: null });

  // Sempre que os dados mudam, (re)carrega os assets (logos/pneus) e publica.
  useEffect(() => {
    if (!data) {
      sourceRef.current = { data: null, assets: null };
      return undefined; // sem dados ao vivo: o overlay simplesmente não desenha
    }
    let cancelled = false;
    preloadAssets(data).then((assets) => {
      if (!cancelled) sourceRef.current = { data, assets };
    });
    return () => {
      cancelled = true;
    };
  }, [data]);

  // Loop de desenho + escrita, com DEGRAU DE TAXA (independente da taxa dos dados).
  //
  // Cada quadro aqui custa muito mais que na janela do monitor: além de redesenhar a
  // torre, faz `getImageData` de 1024×2048 (8 MB) e manda tudo por IPC pro Rust. Manter
  // 30 Hz o tempo todo pra animar ultrapassagens que duram 260 ms seria triplicar o
  // pipeline mais caro do overlay pra ganhar quase nada — a torre fica parada a maior
  // parte da corrida. Então: 10 Hz parado, ~30 Hz só enquanto há linha em movimento.
  //
  // É um laço que se reagenda (não `setInterval`) por dois motivos: a taxa muda a cada
  // volta do laço, e o `await` da escrita só solta o próximo tick depois de terminar —
  // se o backend engasgar, os quadros não se empilham.
  useEffect(() => {
    if (!estaNoTauri()) return undefined;

    const canvas = document.createElement("canvas");
    canvas.width = VR_W;
    canvas.height = VR_H;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    const anim = createTowerAnimator();
    const win = createTowerWindow(); // a do VR é só dele (ver createTowerWindow)

    let stopped = false;
    let timer = null;

    const tick = async () => {
      const { data: d, assets } = sourceRef.current;
      if (d && assets) {
        try {
          drawTower(ctx, d, assets, VR_THEME, {
            anim,
            now: performance.now(),
            sections: { window: win },
          });
          const img = ctx.getImageData(0, 0, VR_W, VR_H);
          await invoke("vr_overlay_write_frame", new Uint8Array(img.data.buffer));
        } catch {
          // Falha ao escrever um frame não é fatal: tenta de novo no próximo tick.
        }
      }
      if (stopped) return;
      // Depois do desenho: o `drawTower` já sincronizou o animador, então agora a
      // resposta sobre movimento vale pro quadro que vem.
      timer = setTimeout(tick, anim.hasMotion(performance.now()) ? VR_MOVING_MS : VR_IDLE_MS);
    };
    tick();

    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, []);

  // Componente invisível: só alimenta o overlay do VR, sem UI na tela.
  return null;
}
