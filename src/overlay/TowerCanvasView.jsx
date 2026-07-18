import { useEffect, useRef } from "react";
import { OVERLAY_MOCK } from "./overlayMockData";
import { VR_W, VR_H, SUPERSAMPLE, drawTower, preloadAssets } from "./towerCanvas";
import { DEFAULT_THEME } from "./towerThemes";

// A torre para o overlay de MONITOR — desenhada pelo MESMO `drawTower` que
// alimenta o VR. Uma fonte de verdade, zero divergência. `theme` escolhe a pele.
export default function TowerCanvasView({ data = OVERLAY_MOCK, theme = DEFAULT_THEME, style }) {
  const ref = useRef(null);

  useEffect(() => {
    const ctx = ref.current.getContext("2d", { willReadFrequently: true });
    let cancelled = false;
    (async () => {
      const assets = await preloadAssets(data);
      if (!cancelled) drawTower(ctx, data, assets, theme);
    })();
    return () => {
      cancelled = true;
    };
  }, [data, theme]);

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
