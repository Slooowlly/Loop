import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emitTo } from "@tauri-apps/api/event";

// Controle do overlay de MONITOR na janela principal: um botão que liga o "modo
// mover" (destrava a janela do overlay pra você arrastar) e trava de volta.
//
// A janela do overlay é clique-atravessa por padrão (o mouse vai pro jogo); só no
// modo mover ela vira interativa. Como no modo travado você não consegue clicar
// nela, o botão vive AQUI, na janela principal (alt-tab pro Loop pra mexer).

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export default function OverlayMonitorControls() {
  const [moving, setMoving] = useState(false);
  if (!IN_TAURI) return null;

  const toggle = async () => {
    const next = !moving;
    setMoving(next);
    try {
      await invoke("overlay_window_set_interactive", { interactive: next });
    } catch {
      /* comando indisponível */
    }
    // Avisa a janela do overlay pra mostrar/esconder a camada de arrasto.
    emitTo("overlay", "overlay-move-mode", { on: next }).catch(() => {});
  };

  return (
    <button
      onClick={toggle}
      title="Destrava o overlay do monitor pra você arrastar; trave de novo quando terminar"
      style={{
        position: "fixed",
        right: 8,
        bottom: 72,
        zIndex: 9999,
        background: moving ? "rgba(88,166,255,0.20)" : "rgba(0,0,0,0.6)",
        color: moving ? "#79c0ff" : "#9aa4ad",
        border: "1px solid " + (moving ? "rgba(88,166,255,0.6)" : "rgba(255,255,255,0.12)"),
        font: "11px monospace",
        padding: "4px 8px",
        borderRadius: 6,
        cursor: "pointer",
      }}
    >
      {moving ? "🔒 travar overlay" : "🔓 mover overlay"}
    </button>
  );
}
