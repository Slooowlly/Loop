import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emitTo } from "@tauri-apps/api/event";
import useCareerStore from "../stores/useCareerStore";
import { useOverlayData } from "./useOverlayData";

// Controlador do overlay de MONITOR (roda na janela PRINCIPAL). Dono ÚNICO da
// visibilidade da janela de overlay. Dois controles:
//   • LIGADO/DESLIGADO (persistido): se desligado, o overlay NUNCA aparece, nem em
//     corrida. É o "não quero" do usuário.
//   • MOVER: destrava a janela pra arrastar; força ela a aparecer mesmo desligado
//     (pra posicionar/pré-ver). Ao TRAVAR, se estiver desligado, ela some de vez.
//
// Visibilidade = moveMode || (ligado && sessão ao vivo).

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
  const [moveMode, setMoveMode] = useState(false);
  const [enabled, setEnabled] = useState(loadEnabled);
  const shownRef = useRef(false);

  const live = Boolean(careerId) && Boolean(data);
  const shouldShow = moveMode || (enabled && live);

  // Visibilidade (fonte única). Reexibe ao mudar careerId pra atualizar de quem o
  // overlay puxa os dados (o show grava o careerId ativo que a janela lê).
  useEffect(() => {
    if (!IN_TAURI) return;
    if (shouldShow) {
      shownRef.current = true;
      invoke("overlay_window_show", { careerId: careerId || "setup" }).catch(() => {});
    } else if (shownRef.current) {
      shownRef.current = false;
      invoke("overlay_window_hide").catch(() => {});
    }
  }, [shouldShow, careerId]);

  const toggleMove = async () => {
    const next = !moveMode;
    setMoveMode(next);
    try {
      await invoke("overlay_window_set_interactive", { interactive: next });
    } catch {
      /* comando indisponível */
    }
    emitTo("overlay", "overlay-move-mode", { on: next }).catch(() => {});
  };

  const toggleEnabled = () => {
    const next = !enabled;
    setEnabled(next);
    try {
      localStorage.setItem(ENABLED_KEY, String(next));
    } catch {
      /* sem persistência */
    }
  };

  if (!IN_TAURI) return null;

  const btn = (extra) => ({
    position: "fixed",
    right: 8,
    zIndex: 9999,
    font: "11px monospace",
    padding: "4px 8px",
    borderRadius: 6,
    cursor: "pointer",
    ...extra,
  });

  return (
    <>
      {/* Liga/desliga o overlay de monitor (persistido). */}
      <button
        onClick={toggleEnabled}
        title="Liga ou desliga o overlay do monitor. Desligado, ele nunca aparece — nem em corrida."
        style={btn({
          bottom: 104,
          background: enabled ? "rgba(63,185,80,0.16)" : "rgba(0,0,0,0.6)",
          color: enabled ? "#7ee787" : "#9aa4ad",
          border: "1px solid " + (enabled ? "rgba(63,185,80,0.5)" : "rgba(255,255,255,0.12)"),
        })}
      >
        {enabled ? "🖥️ overlay: ligado" : "🚫 overlay: desligado"}
      </button>

      {/* Mover/posicionar (força aparecer mesmo desligado). */}
      <button
        onClick={toggleMove}
        title="Destrava o overlay pra arrastar (mostra a janela mesmo desligado/sem sessão); trave quando terminar"
        style={btn({
          bottom: 72,
          background: moveMode ? "rgba(88,166,255,0.20)" : "rgba(0,0,0,0.6)",
          color: moveMode ? "#79c0ff" : "#9aa4ad",
          border: "1px solid " + (moveMode ? "rgba(88,166,255,0.6)" : "rgba(255,255,255,0.12)"),
        })}
      >
        {moveMode ? "🔒 travar overlay" : "🔓 mover overlay"}
      </button>
    </>
  );
}
