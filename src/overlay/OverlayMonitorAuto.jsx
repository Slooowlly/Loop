import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import useCareerStore from "../stores/useCareerStore";
import { useOverlayData } from "./useOverlayData";

// Controlador do overlay de MONITOR (roda na janela PRINCIPAL). Só LÓGICA — não
// renderiza nada. Não há mais travar/destravar: a torre é sempre arrastável no
// hover (o vigia de cursor no backend cuida disso). Aqui a gente:
//   • mostra/esconde a JANELA conforme há sessão ao vivo;
//   • guarda o MODO (completo/mini/escondido — o ciclo do olho) e o reflete no overlay;
//   • liga o vigia de hover enquanto a janela está visível.
//
// A JANELA aparece sempre que há sessão (o olho decide completo/mini vs nub) — se
// gateássemos pelo modo, escondido ficaria sem como voltar (nada pra passar o mouse).

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const MODE_KEY = "overlayMonitorMode"; // "full" | "mini" | "hidden"
const LEGACY_ENABLED_KEY = "overlayMonitorEnabled"; // booleano do esquema antigo
const VALID_MODES = new Set(["full", "mini", "hidden"]);

function loadMode() {
  try {
    const raw = localStorage.getItem(MODE_KEY);
    if (raw && VALID_MODES.has(raw)) return raw;
    // Migração do esquema antigo (booleano visível/escondido).
    const legacy = localStorage.getItem(LEGACY_ENABLED_KEY);
    if (legacy != null) return legacy === "false" ? "hidden" : "full";
  } catch {
    /* storage indisponível */
  }
  return "full"; // padrão: completo (opt-out, não opt-in)
}

// Compat: o overlay pode mandar o modo string OU o booleano antigo `on`.
function payloadToMode(p) {
  if (p && typeof p.mode === "string" && VALID_MODES.has(p.mode)) return p.mode;
  if (p && typeof p.on === "boolean") return p.on ? "full" : "hidden";
  return "full";
}

export default function OverlayMonitorAuto() {
  const careerId = useCareerStore((s) => s.careerId);
  const data = useOverlayData(careerId, { intervalMs: 1000 });
  const [mode, setMode] = useState(loadMode);
  const [demo, setDemo] = useState(false); // demo do rádio ligado (Configurações)?
  const towerShownRef = useRef(false);
  const engShownRef = useRef(false);

  const live = Boolean(careerId) && Boolean(data); // iRacing com sessão ativa
  const shouldShow = live;

  // Poll do estado do demo (fonte no backend) — decide se a janela do rádio aparece
  // mesmo sem corrida, pra você achar/posicionar o overlay quando quiser.
  useEffect(() => {
    if (!IN_TAURI) return undefined;
    let stopped = false;
    const tick = () =>
      invoke("overlay_demo_enabled")
        .then((v) => {
          if (!stopped) setDemo(Boolean(v));
        })
        .catch(() => {});
    tick();
    const t = setInterval(tick, 1500);
    return () => {
      stopped = true;
      clearInterval(t);
    };
  }, []);

  // TORRE: só com dados ao vivo. Ao mostrar, sincroniza o modo no overlay.
  useEffect(() => {
    if (!IN_TAURI) return;
    if (live) {
      towerShownRef.current = true;
      invoke("overlay_window_show", { careerId: careerId || "setup" }).catch(() => {});
      emitTo("overlay", "overlay-enabled", { mode }).catch(() => {});
    } else if (towerShownRef.current) {
      towerShownRef.current = false;
      invoke("overlay_window_hide").catch(() => {});
    }
  }, [live, careerId, mode]);

  // RÁDIO: ao vivo OU em demo (assim dá pra ver/posicionar o card sem esperar quebra).
  // O vigia de hover (que captura o mouse pra ARRASTAR) só liga no DEMO — assim o card
  // é reposicionável enquanto você ajusta, mas em CORRIDA REAL fica 100% clique-atravessa
  // (o mouse vai pro iRacing, não trava a interface do jogo por baixo do card).
  useEffect(() => {
    if (!IN_TAURI) return;
    const showRadio = live || demo;
    invoke("engineer_set_hover_watch", { active: demo }).catch(() => {});
    if (showRadio) {
      engShownRef.current = true;
      invoke("engineer_window_show").catch(() => {});
    } else if (engShownRef.current) {
      engShownRef.current = false;
      invoke("engineer_window_hide").catch(() => {});
    }
  }, [live, demo]);

  // Vigia de hover: ativo enquanto a torre está visível (torre OU nub).
  useEffect(() => {
    if (!IN_TAURI) return;
    invoke("overlay_set_hover_watch", { active: shouldShow }).catch(() => {});
  }, [shouldShow]);

  // Persiste o MODO e reflete no overlay sempre que muda.
  useEffect(() => {
    try {
      localStorage.setItem(MODE_KEY, mode);
    } catch {
      /* sem persistência */
    }
    if (IN_TAURI) emitTo("overlay", "overlay-enabled", { mode }).catch(() => {});
  }, [mode]);

  // O olho (👁) na torre cicla o modo. Idempotente: aplica o payload.
  useEffect(() => {
    if (!IN_TAURI) return undefined;
    let un;
    listen("overlay-toggle-enabled", (e) => setMode(payloadToMode(e.payload)))
      .then((f) => {
        un = f;
      })
      .catch(() => {});
    return () => un && un();
  }, []);

  return null; // sem UI — arrastar/ocultar moram no próprio overlay (hover)
}
