import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { VR_W, VR_H, SUPERSAMPLE, drawTower, preloadAssets } from "./towerCanvas";
import { useOverlayData } from "./useOverlayData";

// Vista AO VIVO do overlay de MONITOR (a janela transparente por cima do iRacing).
// Roda dentro da janela `#overlay`, que é um webview separado: ela NÃO tem o store
// do app, então descobre de qual carreira puxar dados perguntando ao backend
// (`overlay_active_career`, setado pelo app quando manda mostrar o overlay).
//
// Desenha o MESMO `drawTower` do VR num canvas visível. Sem dados (sem sessão) não
// pinta nada — a janela fica 100% transparente até a corrida começar.
//
// POSIÇÃO: normalmente a janela é clique-atravessa (o mouse vai pro jogo). O app
// pode ligar o "modo mover" (evento `overlay-move-mode`): aí a janela vira
// interativa e esta view mostra uma camada de arrasto. A posição é salva e
// restaurada (localStorage), então fica onde você deixou.

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const POS_KEY = "overlayMonitorPos"; // { x, y } físicos da janela

export default function OverlayLiveView() {
  const [careerId, setCareerId] = useState(null);
  const [moveMode, setMoveMode] = useState(false);
  const canvasRef = useRef(null);
  // Fonte que o loop de desenho lê: dados + assets já carregados (evita recarregar
  // logos a cada frame). Espelha o padrão do OverlayVrWriter.
  const sourceRef = useRef({ data: null, assets: null });

  // A janela vive desde o boot (oculta); faz POLL da carreira ativa pra pegar o
  // careerId quando o app manda mostrar o overlay (e soltá-lo quando a sessão sai).
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

  // Setup da JANELA: restaura a posição salva, persiste ao mover, e ouve o app
  // ligar/desligar o "modo mover".
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
        cleanups.push(
          await listen("overlay-move-mode", (e) => setMoveMode(Boolean(e.payload?.on))),
        );
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

  // Loop de desenho ~10 Hz (independente da taxa dos dados). Sem fonte → limpa
  // (mantém a janela transparente entre sessões).
  useEffect(() => {
    const ctx = canvasRef.current?.getContext("2d");
    if (!ctx) return undefined;
    const timer = setInterval(() => {
      const { data: d, assets } = sourceRef.current;
      if (d && assets) {
        drawTower(ctx, d, assets);
      } else {
        ctx.setTransform(1, 0, 0, 1, 0, 0);
        ctx.clearRect(0, 0, VR_W, VR_H);
      }
    }, 100);
    return () => clearInterval(timer);
  }, []);

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
          width: VR_W / SUPERSAMPLE,
          height: VR_H / SUPERSAMPLE,
        }}
      />
      {/* Modo mover: camada de arrasto sobre a janela toda. */}
      {moveMode && (
        <div
          onMouseDown={() => {
            getCurrentWindow()
              .startDragging()
              .catch(() => {});
          }}
          style={{
            position: "fixed",
            inset: 0,
            cursor: "move",
            background: "transparent",
            display: "flex",
            alignItems: "flex-start",
            justifyContent: "center",
            zIndex: 10,
          }}
        >
          <div
            style={{
              marginTop: 8,
              background: "rgba(13,17,23,0.9)",
              color: "#79c0ff",
              border: "1px solid rgba(88,166,255,0.5)",
              font: "600 11px system-ui, sans-serif",
              padding: "4px 10px",
              borderRadius: 8,
              pointerEvents: "none",
              whiteSpace: "nowrap",
              boxShadow: "0 2px 10px rgba(0,0,0,0.5)",
            }}
          >
            ✋ arraste pra posicionar · trave no app quando terminar
          </div>
        </div>
      )}
    </>
  );
}
