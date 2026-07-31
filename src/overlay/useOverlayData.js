import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCategoryColor } from "../utils/categoryColors";
import { estaNoTauri } from "../lib/tauri";

// Dados AO VIVO da torre (`get_overlay_data`, que cruza a telemetria do iRacing com o
// nosso elenco: times/cores/pontos). `null` = nenhuma sessão ativa; o chamador decide o
// que fazer (esconder / mock).
//
// ─── Um poll, três consumidores ───
// Antes cada consumidor tinha o SEU `setInterval` chamando `get_overlay_data`: o
// controlador do monitor a 1 Hz e o escritor de VR a 2 Hz na janela principal, mais a
// janela de overlay a 2 Hz por conta própria. Três leituras independentes do mesmo dado,
// cada uma custando uma query no backend e uma travessia da ponte.
//
// Agora existe UMA fonte e um barramento local:
//   • `useOverlayDataSource(careerId)` — só na janela PRINCIPAL. Faz o único poll,
//     publica no barramento local e repassa por evento pra janela de overlay.
//   • `useOverlayDataRelay()` — só na janela de OVERLAY. Ouve o evento e publica no
//     barramento local dela. Zero poll, zero query.
//   • `useOverlayData(fallback)` — qualquer consumidor, nas duas janelas. Só lê.
//
// O papel é EXPLÍCITO (cada janela chama o seu hook) em vez de inferido da label da
// janela: quem monta o quê já é decidido no `main.jsx`, e sniffar label aqui dentro
// espalharia essa decisão por dois lugares.

// O backend manda a classe só com id/label; a COR vem da fonte única do app
// (categoryColors.js), pra bater com o resto da interface.
function decorate(data) {
  if (!data || !Array.isArray(data.classes)) return null;
  data.classes.forEach((c) => {
    c.color = getCategoryColor(c.id, "#7d8590");
  });
  return data;
}

// ─── Barramento local (por janela) ───
// `ultimo` guarda o valor corrente pra quem se inscrever DEPOIS do primeiro dado já
// nascer com a torre certa, em vez de esperar o próximo tick.
let ultimo = null;
const inscritos = new Set();

function publicar(data) {
  ultimo = data;
  inscritos.forEach((fn) => fn(data));
}

/// Lê o dado corrente do barramento. `fallback` vale enquanto não há sessão ao vivo.
export function useOverlayData(fallback = null) {
  const [data, setData] = useState(ultimo);
  useEffect(() => {
    inscritos.add(setData);
    setData(ultimo); // pega o valor que já existia entre o render e a inscrição
    return () => {
      inscritos.delete(setData);
    };
  }, []);
  return data ?? fallback;
}

/// FONTE (janela principal): o único poll de `get_overlay_data`. Publica local e repassa
/// pra janela de overlay. `onState` é diagnóstico (não re-dispara o efeito).
export function useOverlayDataSource(careerId, { intervalMs = 500, onState } = {}) {
  const onStateRef = useRef(onState);
  useEffect(() => {
    onStateRef.current = onState;
  });

  useEffect(() => {
    if (!estaNoTauri() || !careerId) {
      publicar(null);
      onStateRef.current?.(estaNoTauri() ? "no-career" : "not-tauri");
      return undefined;
    }

    let stopped = false;

    const repassar = (data) => {
      publicar(data);
      // A janela de overlay pode não existir ainda (ou estar oculta) — o evento cai no
      // vazio e não tem problema nenhum.
      emitTo("overlay", "overlay-data", data).catch(() => {});
    };

    const tick = async () => {
      try {
        const live = await invoke("get_overlay_data", { careerId });
        if (stopped) return;
        repassar(live ? decorate(live) : null);
        onStateRef.current?.(live ? "live" : "null");
      } catch (e) {
        if (stopped) return;
        repassar(null);
        onStateRef.current?.("error", e);
      }
    };

    tick();
    const timer = setInterval(tick, intervalMs);
    return () => {
      stopped = true;
      clearInterval(timer);
    };
  }, [careerId, intervalMs]);
}

/// RELÉ (janela de overlay): recebe o dado da principal e publica no barramento local.
/// A cor da categoria é reaplicada aqui porque o evento atravessa a ponte como JSON —
/// o `decorate` da fonte sobrevive (é campo comum), mas reaplicar é barato e garante a
/// mesma fonte de verdade se um dia o payload mudar de forma.
export function useOverlayDataRelay() {
  useEffect(() => {
    if (!estaNoTauri()) return undefined;
    let un = null;
    listen("overlay-data", (e) => publicar(e.payload ? decorate(e.payload) : null))
      .then((fn) => {
        un = fn;
      })
      .catch(() => {});
    return () => {
      if (un) un();
    };
  }, []);
}
