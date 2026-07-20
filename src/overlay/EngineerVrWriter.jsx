import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import useCareerStore from "../stores/useCareerStore";
import { RADIO_VR_W, RADIO_VR_H, drawRadioCard } from "./radioCanvas";
import { useBreakdownFeed } from "./useBreakdownFeed";

// Escritor do RÁDIO DA EQUIPE no VR: quando uma quebra é anunciada, desenha o card
// num canvas e manda os pixels (~10 Hz) pro backend, que os coloca no 2º mapeamento
// (`Local\iRacerEngineerFrame`) lido pela OpenXR API layer — um quad independente da
// torre. Fora da janela da mensagem, manda um frame TRANSPARENTE (o quad some).
//
// Componente invisível: só alimenta o overlay do VR. Roda só dentro do Tauri.

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const HOLD_MS = 6000; // quanto a mensagem fica no ar
const FADE_MS = 450; // fade-out no fim

export default function EngineerVrWriter() {
  const careerId = useCareerStore((s) => s.careerId);
  const message = useBreakdownFeed(careerId);
  const activeRef = useRef({ msg: null, until: 0 });

  // Nova quebra → arma a janela de exibição.
  useEffect(() => {
    if (!message) return;
    activeRef.current = { msg: message, until: Date.now() + HOLD_MS };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [message?.id]);

  useEffect(() => {
    if (!IN_TAURI) return undefined;

    const canvas = document.createElement("canvas");
    canvas.width = RADIO_VR_W;
    canvas.height = RADIO_VR_H;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    let lastKey = null; // evita reenviar o mesmo frame ocioso à toa

    const timer = setInterval(async () => {
      const { msg, until } = activeRef.current;
      const now = Date.now();
      let toDraw = null;
      if (msg && now < until) {
        const remain = until - now;
        const alpha = remain < FADE_MS ? remain / FADE_MS : 1;
        toDraw = { ...msg, alpha };
      }

      // Quando ocioso, só manda a limpeza UMA vez (não fica cuspindo transparente).
      const key = toDraw ? `${toDraw.id}:${Math.round(toDraw.alpha * 12)}` : "idle";
      if (key === "idle" && lastKey === "idle") return;
      lastKey = key;

      try {
        drawRadioCard(ctx, toDraw);
        const img = ctx.getImageData(0, 0, RADIO_VR_W, RADIO_VR_H);
        await invoke("vr_engineer_write_frame", new Uint8Array(img.data.buffer));
      } catch {
        // Falha ao escrever um frame não é fatal: tenta de novo no próximo tick.
      }
    }, 100);

    return () => clearInterval(timer);
  }, []);

  return null;
}
