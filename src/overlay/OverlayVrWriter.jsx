import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import useCareerStore from "../stores/useCareerStore";
import { VR_W, VR_H, drawTower, preloadAssets } from "./towerCanvas";
import { VR_THEME } from "./towerThemes";
import { useOverlayData } from "./useOverlayData";

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

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export default function OverlayVrWriter() {
  const careerId = useCareerStore((s) => s.careerId);
  const data = useOverlayData(careerId, { intervalMs: 1000 });
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

  // Loop de desenho + escrita a 10 Hz (independente da taxa dos dados).
  useEffect(() => {
    if (!IN_TAURI) return undefined;

    const canvas = document.createElement("canvas");
    canvas.width = VR_W;
    canvas.height = VR_H;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });

    const timer = setInterval(async () => {
      const { data: d, assets } = sourceRef.current;
      if (!d || !assets) return; // nada a mostrar
      try {
        drawTower(ctx, d, assets, VR_THEME);
        const img = ctx.getImageData(0, 0, VR_W, VR_H);
        await invoke("vr_overlay_write_frame", new Uint8Array(img.data.buffer));
      } catch {
        // Falha ao escrever um frame não é fatal: tenta de novo no próximo tick.
      }
    }, 100);

    return () => clearInterval(timer);
  }, []);

  // Componente invisível: só alimenta o overlay do VR, sem UI na tela.
  return null;
}
