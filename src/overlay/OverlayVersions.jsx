import TowerCanvasView from "./TowerCanvasView";
import { THEME_LIST } from "./towerThemes";

// Comparador das 3 versões lado a lado, sobre um fundo que imita gameplay (pra
// julgar transparência e os pins "por fora"). Abra em #overlay-versions.
export default function OverlayVersions() {
  return (
    <div
      style={{
        minHeight: "100vh",
        padding: 20,
        display: "flex",
        gap: 24,
        alignItems: "flex-start",
        justifyContent: "center",
        flexWrap: "wrap",
        // "cenário": degradê de asfalto + manchas claras pra ver a transparência
        background:
          "radial-gradient(circle at 20% 15%, #3a4657 0%, transparent 40%), radial-gradient(circle at 80% 70%, #46525f 0%, transparent 45%), linear-gradient(160deg, #10141a 0%, #1c2530 55%, #0c1016 100%)",
      }}
    >
      {THEME_LIST.map((theme) => (
        <div key={theme.key} style={{ textAlign: "center" }}>
          <div
            style={{
              color: "#e6edf3",
              font: "600 13px 'Space Grotesk', sans-serif",
              marginBottom: 10,
              letterSpacing: "0.08em",
              textTransform: "uppercase",
            }}
          >
            {theme.label}
          </div>
          <TowerCanvasView
            theme={theme}
            style={{ width: 384, height: 768, filter: "drop-shadow(0 12px 40px rgba(0,0,0,0.5))" }}
          />
        </div>
      ))}
    </div>
  );
}
