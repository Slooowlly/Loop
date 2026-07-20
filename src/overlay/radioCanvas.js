// Renderizador em CANVAS do card do RÁDIO DA EQUIPE — a versão que vai pro VR (o
// layer OpenXR só entende pixels, não DOM). Espelha o visual do EngineerRadio.css.
//
// Buffer supersampled 2× (1024×256) pra texto nítido; casa com IRACER_ENGINEER_W/H
// no shared_frame.h e com a resolução do `vr_engineer_write_frame`.

export const RADIO_SUPERSAMPLE = 2;
const LOGICAL_W = 512;
const LOGICAL_H = 128;
export const RADIO_VR_W = LOGICAL_W * RADIO_SUPERSAMPLE; // 1024
export const RADIO_VR_H = LOGICAL_H * RADIO_SUPERSAMPLE; // 256

const FONT = "'Space Grotesk Variable', 'Space Grotesk', 'Segoe UI', sans-serif";

function roundRect(ctx, x, y, w, h, r) {
  ctx.beginPath();
  ctx.roundRect(x, y, w, h, r);
}

function truncate(ctx, text, maxW) {
  if (ctx.measureText(text).width <= maxW) return text;
  let t = text;
  while (t.length > 1 && ctx.measureText(t + "…").width > maxW) t = t.slice(0, -1);
  return t + "…";
}

function wrapText(ctx, text, maxW, maxLines) {
  const words = String(text).split(" ");
  const lines = [];
  let cur = "";
  for (const w of words) {
    const test = cur ? `${cur} ${w}` : w;
    if (ctx.measureText(test).width > maxW && cur) {
      lines.push(cur);
      cur = w;
    } else {
      cur = test;
    }
  }
  if (cur) lines.push(cur);
  if (lines.length <= maxLines) return lines;
  // Estourou: mantém as primeiras e junta+trunca o resto na última linha.
  const kept = lines.slice(0, maxLines - 1);
  kept.push(truncate(ctx, lines.slice(maxLines - 1).join(" "), maxW));
  return kept;
}

// Desenha o card. `message = { severity, text, detail, alpha? }` ou null (transparente).
export function drawRadioCard(ctx, message) {
  ctx.setTransform(RADIO_SUPERSAMPLE, 0, 0, RADIO_SUPERSAMPLE, 0, 0);
  ctx.clearRect(0, 0, LOGICAL_W, LOGICAL_H);
  if (!message) return; // sem mensagem → frame transparente (o quad não mostra nada)

  const sev =
    message.severity === "dnf" ? "dnf" : message.severity === "heavy" ? "heavy" : "light";
  const accent = sev === "light" ? "#ff8c1a" : "#f85149";
  const alpha = message.alpha == null ? 1 : Math.max(0, Math.min(1, message.alpha));
  ctx.globalAlpha = alpha;

  const x = 6;
  const y = 14;
  const w = 500;
  const h = 100;
  const r = 12;

  // Fundo do card.
  roundRect(ctx, x, y, w, h, r);
  ctx.fillStyle = sev === "dnf" ? "rgba(24,7,7,0.90)" : "rgba(11,13,16,0.88)";
  ctx.fill();

  // Acento na esquerda (recortado no cantinho arredondado).
  ctx.save();
  roundRect(ctx, x, y, w, h, r);
  ctx.clip();
  ctx.fillStyle = accent;
  ctx.fillRect(x, y, 5, h);
  ctx.restore();

  ctx.textBaseline = "middle";
  ctx.textAlign = "left";

  // Tag + ponto do rádio.
  ctx.fillStyle = accent;
  ctx.beginPath();
  ctx.arc(x + 24, y + 20, 3.5, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = "#9aa4ad";
  ctx.font = `700 11px ${FONT}`;
  ctx.fillText(sev === "dnf" ? "ABANDONO" : "RÁDIO DA EQUIPE", x + 34, y + 20);

  // Texto principal (até 2 linhas).
  ctx.fillStyle = "#ffffff";
  ctx.font = `700 17px ${FONT}`;
  const lines = wrapText(ctx, message.text, w - 36, 2);
  let ty = y + 46;
  for (const line of lines) {
    ctx.fillText(line, x + 18, ty);
    ty += 21;
  }

  // Detalhe (1 linha).
  if (message.detail) {
    ctx.fillStyle = "#c9d1d9";
    ctx.font = `500 12px ${FONT}`;
    ctx.fillText(truncate(ctx, message.detail, w - 36), x + 18, y + h - 15);
  }

  ctx.globalAlpha = 1;
}
