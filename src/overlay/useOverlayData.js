import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCategoryColor } from "../utils/categoryColors";

// Puxa os dados AO VIVO da torre do backend (`get_overlay_data`), que cruza a
// telemetria do iRacing com o nosso elenco (times/cores/pontos). Retorna null
// quando não há sessão ativa — o chamador decide o que fazer (esconder / mock).

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// O backend manda a classe só com id/label; a COR vem da fonte única do app
// (categoryColors.js), pra bater com o resto da interface.
function decorate(data) {
  if (!data || !Array.isArray(data.classes)) return null;
  data.classes.forEach((c) => {
    c.color = getCategoryColor(c.id, "#7d8590");
  });
  return data;
}

export function useOverlayData(careerId, { intervalMs = 500, fallback = null, onState } = {}) {
  const [data, setData] = useState(fallback);
  // Callback de diagnóstico (sem re-disparar o efeito quando a identidade muda).
  const onStateRef = useRef(onState);
  useEffect(() => {
    onStateRef.current = onState;
  });

  useEffect(() => {
    if (!IN_TAURI || !careerId) {
      setData(fallback);
      onStateRef.current?.(IN_TAURI ? "no-career" : "not-tauri");
      return undefined;
    }

    let stopped = false;
    let timer = null;

    const tick = async () => {
      try {
        const live = await invoke("get_overlay_data", { careerId });
        if (!stopped) {
          setData(live ? decorate(live) : fallback);
          onStateRef.current?.(live ? "live" : "null");
        }
      } catch (e) {
        if (!stopped) {
          setData(fallback);
          onStateRef.current?.("error", e);
        }
      }
    };

    tick();
    timer = setInterval(tick, intervalMs);
    return () => {
      stopped = true;
      if (timer) clearInterval(timer);
    };
  }, [careerId, intervalMs, fallback]);

  return data;
}
