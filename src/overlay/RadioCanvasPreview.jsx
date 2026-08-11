import { useEffect, useRef } from "react";
import { RADIO_VR_W, RADIO_VR_H, drawRadioCard } from "./radioCanvas";

// Preview no NAVEGADOR do canvas do rádio VR (`#radio-canvas`) — só pra inspecionar o
// layout (centralização, fundo opaco cobrindo texto atrás, card de aviso âmbar). Cada
// canvas é desenhado sobre um "texto do iRacing" fake pra conferir a opacidade.
//
// i18n-ignore-file: bancada de inspeção, alcançada só pelo hash `#radio-canvas`. O jogador
// nunca chega aqui, e traduzir o rótulo de uma régua de layout seria trabalho puro.

const SAMPLES = [
  { severity: "light", text: "Matthias DeSmet apresenta um problema no motor", detail: "superaquecimento" },
  { severity: "heavy", text: "Adrian Alvarez apresenta problemas graves nos freios", detail: "disco trincado" },
  { severity: "dnf", text: "Enrico Russo foi retirado da corrida devido a problemas no câmbio", detail: "" },
  { severity: "warn", text: "Atenção no seu motor — ouço algo estranho no rádio", detail: "pode falhar a qualquer momento" },
];

function Card({ sample }) {
  const ref = useRef(null);
  useEffect(() => {
    const ctx = ref.current.getContext("2d");
    drawRadioCard(ctx, sample);
  }, [sample]);
  return (
    <div
      style={{
        position: "relative",
        width: RADIO_VR_W / 2,
        height: RADIO_VR_H / 2,
        marginBottom: 18,
      }}
    >
      {/* "texto do iRacing" fake atrás — o card opaco deve tapá-lo */}
      <div
        style={{
          position: "absolute",
          inset: 0,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "#ffd400",
          font: "700 15px system-ui, sans-serif",
          textShadow: "0 0 4px #000",
        }}
      >
        Press and hold Escape to tow and get out or Reset to tow.
      </div>
      <canvas
        ref={ref}
        width={RADIO_VR_W}
        height={RADIO_VR_H}
        style={{ position: "absolute", inset: 0, width: "100%", height: "100%" }}
      />
    </div>
  );
}

export default function RadioCanvasPreview() {
  return (
    <div
      style={{
        minHeight: "100vh",
        background: "#0a2a12",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        padding: 24,
        gap: 4,
      }}
    >
      <h3 style={{ color: "#e6edf3", font: "600 14px system-ui", marginBottom: 12 }}>
        Radio VR canvas — layout centralizado, fundo opaco, card de aviso âmbar
      </h3>
      {SAMPLES.map((s) => (
        <Card key={s.severity} sample={s} />
      ))}
    </div>
  );
}
