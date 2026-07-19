import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import useCareerStore from "../stores/useCareerStore";
import { useOverlayData } from "./useOverlayData";

// Controlador do overlay de MONITOR (roda na janela PRINCIPAL). Só LÓGICA — não
// renderiza nada. Não há mais travar/destravar: a torre é sempre arrastável no
// hover (o vigia de cursor no backend cuida disso). Aqui a gente:
//   • mostra/esconde a JANELA conforme há sessão ao vivo;
//   • guarda `enabled` (o olho — torre visível ou nub) e o reflete no overlay;
//   • liga o vigia de hover enquanto a janela está visível.
//
// A JANELA aparece sempre que há sessão (o olho decide torre inteira vs nub) — se
// gateássemos por `enabled`, oculto ficaria sem como voltar (nada pra passar o mouse).

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const ENABLED_KEY = "overlayMonitorEnabled";

function loadEnabled() {
  try {
    const raw = localStorage.getItem(ENABLED_KEY);
    if (raw != null) return raw === "true";
  } catch {
    /* storage indisponível */
  }
  return true; // padrão: ligado (opt-out, não opt-in)
}

export default function OverlayMonitorAuto() {
  const careerId = useCareerStore((s) => s.careerId);
  const data = useOverlayData(careerId, { intervalMs: 1000 });
  const [enabled, setEnabled] = useState(loadEnabled);
  const shownRef = useRef(false);

  const live = Boolean(careerId) && Boolean(data); // iRacing com sessão ativa
  const shouldShow = live;

  // Visibilidade da janela (fonte única). Ao mostrar, sincroniza o olho no overlay.
  useEffect(() => {
    if (!IN_TAURI) return;
    if (shouldShow) {
      shownRef.current = true;
      invoke("overlay_window_show", { careerId: careerId || "setup" }).catch(() => {});
      emitTo("overlay", "overlay-enabled", { on: enabled }).catch(() => {});
    } else if (shownRef.current) {
      shownRef.current = false;
      invoke("overlay_window_hide").catch(() => {});
    }
  }, [shouldShow, careerId, enabled]);

  // Vigia de hover: ativo enquanto a janela está visível (torre OU nub).
  useEffect(() => {
    if (!IN_TAURI) return;
    invoke("overlay_set_hover_watch", { active: shouldShow }).catch(() => {});
  }, [shouldShow]);

  // Persiste `enabled` e reflete no olho do overlay sempre que muda.
  useEffect(() => {
    try {
      localStorage.setItem(ENABLED_KEY, String(enabled));
    } catch {
      /* sem persistência */
    }
    if (IN_TAURI) emitTo("overlay", "overlay-enabled", { on: enabled }).catch(() => {});
  }, [enabled]);

  // O olho (👁) na torre pede pra ligar/desligar. Idempotente: aplica o payload.
  useEffect(() => {
    if (!IN_TAURI) return undefined;
    let un;
    listen("overlay-toggle-enabled", (e) => setEnabled(Boolean(e.payload?.on)))
      .then((f) => {
        un = f;
      })
      .catch(() => {});
    return () => un && un();
  }, []);

  return null; // sem UI — arrastar/ocultar moram no próprio overlay (hover)
}
