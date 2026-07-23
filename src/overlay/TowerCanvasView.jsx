import { useEffect, useRef } from "react";
import { OVERLAY_MOCK } from "./overlayMockData";
import { VR_W, VR_H, SUPERSAMPLE, MINI_SECTION_OPTS, drawTower, preloadAssets } from "./towerCanvas";
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

  useEffect(() => {
    const ctx = ref.current.getContext("2d", { willReadFrequently: true });
    let cancelled = false;
    let raf = null;
    (async () => {
      const assets = await preloadAssets(data); // carrega uma vez por data
      if (cancelled) return;
      const opts = { compact, sections: compact ? MINI_SECTION_OPTS : undefined };
      const draw = () => {
        drawTower(ctx, data, assets, theme, opts);
        // `animate` redesenha a cada frame → o piscar (alerta/flash) pulsa de verdade.
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
