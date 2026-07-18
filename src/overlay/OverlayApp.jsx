import TowerCanvasView from "./TowerCanvasView";
import OverlayLiveView from "./OverlayLiveView";

// Raiz da janela de OVERLAY (monitor). Fundo TRANSPARENTE de propósito: por cima
// do iRacing só aparecem os widgets. No preview do navegador coloco um xadrez
// leve atrás pra dar pra ver a transparência.
//
// A torre aqui é o MESMO canvas que vai pro VR (drawTower), então monitor e óculos
// nunca divergem. Dois modos:
//   • preview (browser) → xadrez atrás + dados MOCK, só pra inspecionar o visual.
//   • real (janela Tauri por cima do jogo) → transparente + dados AO VIVO.
export default function OverlayApp({ preview = false }) {
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
      <div className="p-3">
        <TowerCanvasView
          style={{
            borderRadius: 10,
            boxShadow: "0 10px 40px rgba(0,0,0,0.55)",
          }}
        />
      </div>
    </div>
  );
}
