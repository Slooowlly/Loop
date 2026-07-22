import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
import { listen, emitTo } from "@tauri-apps/api/event";
import {
  VR_W,
  VR_H,
  SUPERSAMPLE,
  PANEL_W,
  SESSION_H,
  drawTower,
  preloadAssets,
  towerContentHeight,
} from "./towerCanvas";
import { useOverlayData } from "./useOverlayData";

// Vista AO VIVO do overlay de MONITOR (a janela transparente por cima do iRacing).
// Roda dentro da janela `#overlay`, um webview separado (sem o store do app), então
// descobre a carreira ativa por poll (`overlay_active_career`).
//
// INTERAÇÃO (simples, sem travar/destravar):
//   • A torre é SEMPRE arrastável — mas o mouse só a alcança quando está EM CIMA
//     dela. O backend (vigia de cursor) torna a janela interativa no hover e volta
//     a clique-atravessa ao sair, então fora da torre o mouse vai pro iRacing.
//   • O arraste é INVISÍVEL até o hover: aí aparece um leve realce e o cursor vira
//     "setas pra todos os lados" (o move do Windows). Segura e arrasta pra posicionar.
//   • Olho (👁): fade in/out no hover, no topo — oculta a torre pra um "nub" mínimo
//     no canto, clicável pra trazer de volta.
//
// A posição é salva/restaurada (localStorage). Ocultar pede ao app (dono do estado)
// via `overlay-toggle-enabled`.

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const POS_KEY = "overlayMonitorPos"; // { x, y } físicos da janela
const TOWER_W = VR_W / SUPERSAMPLE; // 512 CSS px = largura da janela
// Caixa de hover do nub (um pouco maior que o visual, pra o mouse pegar fácil).
const NUB_W = 78;
const NUB_H = 34;

export default function OverlayLiveView() {
  const { t } = useTranslation();
  const [careerId, setCareerId] = useState(null);
  const [enabled, setEnabled] = useState(true); // torre visível (olho)?
  const [hover, setHover] = useState(false); // mouse sobre a torre/nub
  const canvasRef = useRef(null);
  const sourceRef = useRef({ data: null, assets: null });
  const enabledRef = useRef(true); // o loop de desenho lê daqui (sem recriar o timer)

  useEffect(() => {
    enabledRef.current = enabled;
  }, [enabled]);

  // Poll da carreira ativa (o app grava ao mostrar o overlay).
  useEffect(() => {
    if (!IN_TAURI) return undefined;
    let stopped = false;
    const poll = async () => {
      try {
        const id = await invoke("overlay_active_career");
        if (!stopped) setCareerId(id ?? null);
      } catch {
        /* ponte ainda não pronta */
      }
    };
    poll();
    const t = setInterval(poll, 1000);
    return () => {
      stopped = true;
      clearInterval(t);
    };
  }, []);

  // Setup da JANELA: restaura posição, persiste ao mover, e ouve o app (olho) e o
  // vigia de cursor (hover).
  useEffect(() => {
    if (!IN_TAURI) return undefined;
    const w = getCurrentWindow();
    const cleanups = [];
    (async () => {
      try {
        const raw = localStorage.getItem(POS_KEY);
        if (raw) {
          const { x, y } = JSON.parse(raw);
          if (Number.isFinite(x) && Number.isFinite(y)) {
            await w.setPosition(new PhysicalPosition(x, y));
          }
        }
      } catch {
        /* sem posição salva */
      }
      try {
        cleanups.push(
          await w.onMoved(({ payload }) => {
            try {
              localStorage.setItem(POS_KEY, JSON.stringify({ x: payload.x, y: payload.y }));
            } catch {
              /* storage indisponível */
            }
          }),
        );
      } catch {
        /* onMoved indisponível */
      }
      try {
        cleanups.push(await listen("overlay-enabled", (e) => setEnabled(Boolean(e.payload?.on))));
        cleanups.push(await listen("overlay-hover", (e) => setHover(Boolean(e.payload))));
      } catch {
        /* listen indisponível */
      }
    })();
    return () => cleanups.forEach((fn) => fn && fn());
  }, []);

  const data = useOverlayData(careerId, { intervalMs: 1000 });

  // Recarrega assets (logos/pneus) só quando os dados mudam de verdade.
  useEffect(() => {
    if (!data) {
      sourceRef.current = { data: null, assets: null };
      return undefined;
    }
    let cancelled = false;
    preloadAssets(data).then((assets) => {
      if (!cancelled) sourceRef.current = { data, assets };
    });
    return () => {
      cancelled = true;
    };
  }, [data]);

  // Reporta ao vigia a caixa que conta como "em cima da torre": a torre inteira
  // quando visível, ou só o nub quando oculta.
  useEffect(() => {
    if (!IN_TAURI) return;
    const width = enabled && data ? TOWER_W : NUB_W;
    const height = enabled && data ? towerContentHeight(data) : NUB_H;
    invoke("overlay_set_hover_rect", { width, height }).catch(() => {});
  }, [enabled, data]);

  // Loop de desenho ~10 Hz. Oculto (olho OFF) → limpa (mostra só o nub em HTML).
  useEffect(() => {
    const ctx = canvasRef.current?.getContext("2d");
    if (!ctx) return undefined;
    const timer = setInterval(() => {
      const { data: d, assets } = sourceRef.current;
      if (d && assets && enabledRef.current) {
        drawTower(ctx, d, assets);
      } else {
        ctx.setTransform(1, 0, 0, 1, 0, 0);
        ctx.clearRect(0, 0, VR_W, VR_H);
      }
    }, 100);
    return () => clearInterval(timer);
  }, []);

  // Ações.
  const toggleEnabled = () =>
    setEnabled((cur) => {
      const next = !cur;
      emitTo("main", "overlay-toggle-enabled", { on: next }).catch(() => {});
      return next;
    });
  const startDrag = () =>
    getCurrentWindow()
      .startDragging()
      .catch(() => {});
  const stop = (e) => e.stopPropagation();

  const towerH = enabled && data ? towerContentHeight(data) : 0;

  return (
    <>
      <canvas
        ref={canvasRef}
        width={VR_W}
        height={VR_H}
        style={{
          position: "fixed",
          top: 0,
          left: 0,
          width: TOWER_W,
          height: VR_H / SUPERSAMPLE,
        }}
      />

      {enabled && data && (
        <>
          {/* Camada de arraste sobre a torre: totalmente invisível — só o cursor de
              setas indica que dá pra arrastar. Segurar e arrastar move a janela. */}
          <div
            onMouseDown={startDrag}
            style={{
              position: "fixed",
              top: 0,
              left: 0,
              width: TOWER_W,
              height: towerH,
              cursor: "move", // "setas pra todos os lados" (SIZEALL do Windows)
              background: "transparent",
              zIndex: 9,
            }}
          />

          {/* Olho: único elemento no hover — centrado no PAINEL (horizontal) e no
              CORPO abaixo do cabeçalho (vertical). Bem grande, sem fundo, translúcido,
              com sombra leve. Fade in/out. Oculta a torre. */}
          <button
            onMouseDown={stop}
            onClick={toggleEnabled}
            title={t("overlay.liveView.hideTower")}
            style={{
              position: "fixed",
              top: (SESSION_H + towerH) / 2, // meio do corpo (rows), sem contar o header
              left: PANEL_W / 2, // centro do painel (não da janela de 512)
              transform: "translate(-50%, -50%)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              width: 88,
              height: 88,
              background: "transparent",
              border: "none",
              cursor: "pointer",
              fontSize: 60,
              lineHeight: 1,
              filter: "drop-shadow(0 2px 4px rgba(0,0,0,0.7))",
              opacity: hover ? 0.5 : 0,
              pointerEvents: hover ? "auto" : "none",
              transition: "opacity 0.14s ease",
              zIndex: 10,
            }}
          >
            👁
          </button>
        </>
      )}

      {/* OCULTA (olho OFF): nub mínimo no canto — clicável (via hover) pra voltar. */}
      {!enabled && (
        <button
          onMouseDown={stop}
          onClick={toggleEnabled}
          title={t("overlay.liveView.showTower")}
          style={{
            position: "fixed",
            top: 5,
            left: 5,
            display: "flex",
            alignItems: "center",
            gap: 5,
            height: 24,
            padding: "0 8px",
            background: hover ? "rgba(13,17,23,0.95)" : "rgba(13,17,23,0.6)",
            border: "1px solid " + (hover ? "rgba(88,166,255,0.5)" : "rgba(255,255,255,0.14)"),
            borderRadius: 7,
            color: "#9aa4ad",
            cursor: "pointer",
            font: "600 11px system-ui, sans-serif",
            transition: "background 0.14s ease, border-color 0.14s ease",
            zIndex: 10,
          }}
        >
          👁 <span style={{ opacity: 0.8 }}>Standings</span>
        </button>
      )}
    </>
  );
}
