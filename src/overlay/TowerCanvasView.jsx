import { useEffect, useRef } from "react";
import { OVERLAY_MOCK } from "./overlayMockData";
import { VR_W, VR_H, SUPERSAMPLE, MINI_SECTION_OPTS, drawTower, preloadAssets } from "./towerCanvas";
import { createTowerAnimator } from "./towerAnimation";
import { createTowerWindow } from "./towerRows";
import { DEFAULT_THEME } from "./towerThemes";

// A torre para o overlay de MONITOR — desenhada pelo MESMO `drawTower` que
// alimenta o VR. Uma fonte de verdade, zero divergência. `theme` escolhe a pele;
// `compact` escolhe a versão MINI (painel estreito, menos carros).
export default function TowerCanvasView({
  data = OVERLAY_MOCK,
  theme = DEFAULT_THEME,
  style,
  animate = false,
  compact = false,
}) {
  const ref = useRef(null);
  // Sobrevive à troca de `data`: é o que dá continuidade ao deslize quando as
  // posições mudam (recriar por render zeraria a animação a cada ultrapassagem).
  const animRef = useRef(null);
  if (animate && !animRef.current) animRef.current = createTowerAnimator();
  // Idem: a janela solta só faz sentido com o tempo passando, então acompanha o
  // `animate`. Sem ele, a torre estática segue com a janela grudada de sempre.
  const windowRef = useRef(null);
  if (animate && !windowRef.current) windowRef.current = createTowerWindow();

  useEffect(() => {
    const ctx = ref.current.getContext("2d", { willReadFrequently: true });
    let cancelled = false;
    let raf = null;
    (async () => {
      const assets = await preloadAssets(data); // carrega uma vez por data
      if (cancelled) return;
      const opts = {
        compact,
        sections: {
          ...(compact ? MINI_SECTION_OPTS : {}),
          ...(windowRef.current ? { window: windowRef.current } : {}),
        },
      };
      // O relógio precisa ser o MESMO do `requestAnimationFrame` (performance.now).
      // Este primeiro desenho é o que enxerga o layout novo quando `data` muda — se
      // ele passasse um `now` de outra régua (ou 0), o deslize começaria fora de
      // escala e a linha saltaria direto pro destino.
      const draw = (now = performance.now()) => {
        drawTower(ctx, data, assets, theme, { ...opts, anim: animRef.current, now });
        // `animate` redesenha a cada frame → o piscar (alerta/flash) pulsa de verdade
        // e o deslize das linhas roda.
        if (animate) raf = requestAnimationFrame(draw);
      };
      draw();
    })();
    return () => {
      cancelled = true;
      if (raf) cancelAnimationFrame(raf);
    };
  }, [data, theme, animate, compact]);

  // Buffer em 2× (VR_W×VR_H) pra nitidez; DISPLAY no tamanho lógico, senão o
  // canvas apareceria com o dobro do tamanho no monitor. `style` do chamador vence.
  return (
    <canvas
      ref={ref}
      width={VR_W}
      height={VR_H}
      style={{ width: VR_W / SUPERSAMPLE, height: VR_H / SUPERSAMPLE, ...style }}
    />
  );
}
