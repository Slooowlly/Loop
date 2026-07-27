import { useEffect, useState } from "react";
import TowerCanvasView from "./TowerCanvasView";
import OverlayLiveView from "./OverlayLiveView";
import { OVERLAY_MOCK, OVERLAY_MOCK_QUALY } from "./overlayMockData";

// Troca o jogador de posição com o vizinho de cima/baixo a cada 1,8 s. Existe só pro
// preview: o deslize da ultrapassagem é a única coisa da torre que não dá pra
// conferir num quadro parado.
function useUltrapassagemDemo() {
  const [passou, setPassou] = useState(false);
  useEffect(() => {
    const t = setInterval(() => setPassou((v) => !v), 1800);
    return () => clearInterval(t);
  }, []);

  const gt3 = OVERLAY_MOCK.classes[0];
  const i = gt3.cars.findIndex((c) => c.player);
  const cars = [...gt3.cars];
  if (passou && i > 0) {
    [cars[i - 1], cars[i]] = [cars[i], cars[i - 1]];
  }
  // `pos` é o lugar na lista — reatribui, senão a torre mostraria dois "12".
  const renumerados = cars.map((c, n) => ({ ...c, pos: n + 1 }));
  return {
    ...OVERLAY_MOCK,
    classes: [{ ...gt3, cars: renumerados }, ...OVERLAY_MOCK.classes.slice(1)],
  };
}

// Raiz da janela de OVERLAY (monitor). Fundo TRANSPARENTE de propósito: por cima
// do iRacing só aparecem os widgets. No preview do navegador coloco um xadrez
// leve atrás pra dar pra ver a transparência.
//
// A torre aqui é o MESMO canvas que vai pro VR (drawTower), então monitor e óculos
// nunca divergem. Dois modos:
//   • preview (browser) → xadrez atrás + dados MOCK, só pra inspecionar o visual.
//   • real (janela Tauri por cima do jogo) → transparente + dados AO VIVO.
export default function OverlayApp({ preview = false }) {
  // Hooks antes do early return (regra do React); no modo real o valor é ignorado.
  const demo = useUltrapassagemDemo();
  if (!preview) return <OverlayLiveView />;

  return (
    <div
      className="min-h-screen w-full"
      style={
        preview
          ? {
              backgroundColor: "#11151a",
              backgroundImage:
                "linear-gradient(45deg, #1b2027 25%, transparent 25%), linear-gradient(-45deg, #1b2027 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #1b2027 75%), linear-gradient(-45deg, transparent 75%, #1b2027 75%)",
              backgroundSize: "24px 24px",
              backgroundPosition: "0 0, 0 12px, 12px -12px, -12px 0px",
            }
          : { background: "transparent" }
      }
    >
      <div className="p-3" style={{ display: "flex", gap: 16, alignItems: "flex-start" }}>
        {/* Torre completa, com o jogador trocando de posição em loop: é aqui que se
            inspeciona o DESLIZE da linha na ultrapassagem. */}
        <TowerCanvasView
          data={demo}
          animate
          style={{
            borderRadius: 10,
            boxShadow: "0 10px 40px rgba(0,0,0,0.55)",
          }}
        />
        {/* Versão MINI ao lado, pra comparar o painel enxuto — com a mesma
            ultrapassagem em loop. A janela da mini é bem mais apertada (topN 2,
            vizinhança 2), então aqui dá pra ver quando a troca vira entrada/saída da
            faixa visível em vez de deslize. */}
        <TowerCanvasView
          data={demo}
          animate
          compact
          style={{
            borderRadius: 10,
            boxShadow: "0 10px 40px rgba(0,0,0,0.55)",
          }}
        />
        {/* Mesma torre em CLASSIFICAÇÃO: o canto direito vira relógio (TIME 3:12/8:00)
            no lugar da volta. É o único jeito de inspecionar esse modo no navegador. */}
        <TowerCanvasView
          data={OVERLAY_MOCK_QUALY}
          style={{
            borderRadius: 10,
            boxShadow: "0 10px 40px rgba(0,0,0,0.55)",
          }}
        />
      </div>
    </div>
  );
}
