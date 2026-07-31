import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { estaNoTauri } from "../lib/tauri";

// Estado do pipeline de overlay, resolvido pelo backend em `overlay_pipeline_flags`:
//   • vrOverlay   — decisão EFETIVA: desenhar os painéis de VR agora?
//   • monitorInVr — com VR ativo, abrir TAMBÉM as janelas de overlay no monitor
//   • simInVr     — o iRacing está em VR agora (a API layer avisa; ver vr_layer.rs)
//   • vrMode      — "auto" | "on" | "off": o que está configurado, não a decisão
//
// A REGRA (auto segue a detecção, on/off forçam) mora no Rust de propósito: é uma regra
// só, num lugar só, e o front não a reimplementa — aqui `vrOverlay` já vem decidido.
//
// Poll de 2 s em vez de evento: o custo é um invoke que lê um atômico e tenta abrir um
// evento nomeado. Um poll ralo evita todo o aparato de emitir/assinar evento — e como
// "auto" depende de um sinal EXTERNO (o sim abrindo em VR), alguma forma de sondagem
// teria que existir de qualquer jeito.
//
// Fora do Tauri (vite puro) devolve tudo `false`: não há ponte pra escrever frame nenhum.
const PARADO = { vrOverlay: false, monitorInVr: false, simInVr: false, vrMode: "auto" };

export function useOverlayFlags({ intervalMs = 2000 } = {}) {
  const [flags, setFlags] = useState(PARADO);

  useEffect(() => {
    if (!estaNoTauri()) return undefined;
    let stopped = false;
    const tick = () =>
      invoke("overlay_pipeline_flags")
        .then((f) => {
          if (stopped) return;
          const novo = {
            vrOverlay: Boolean(f?.vr_overlay),
            monitorInVr: Boolean(f?.monitor_in_vr),
            simInVr: Boolean(f?.sim_in_vr),
            vrMode: f?.vr_mode ?? "auto",
          };
          // Só troca a referência quando algo MUDA: o objeto é dependência de efeitos
          // lá em cima, e um novo a cada 2 s remontaria os laços de desenho.
          setFlags((prev) =>
            prev.vrOverlay === novo.vrOverlay &&
            prev.monitorInVr === novo.monitorInVr &&
            prev.simInVr === novo.simInVr &&
            prev.vrMode === novo.vrMode
              ? prev
              : novo,
          );
        })
        .catch(() => {});
    tick();
    const timer = setInterval(tick, intervalMs);
    return () => {
      stopped = true;
      clearInterval(timer);
    };
  }, [intervalMs]);

  return flags;
}
