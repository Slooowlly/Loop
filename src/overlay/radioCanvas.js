// Renderizador em CANVAS do card do RÁDIO DA EQUIPE — a versão que vai pro VR (o
// layer OpenXR só entende pixels, não DOM). Espelha o visual do EngineerRadio.css:
// conteúdo CENTRALIZADO, tag com marcador, acento por severidade. DIFERENÇA proposital
// vs desktop: o fundo aqui é OPACO (cobre a linha de texto do iRacing atrás do quad,
// ex. "Press and hold Escape to tow"), enquanto no desktop é translúcido.
//
// Buffer supersampled 2× (1024×256) pra texto nítido; casa com IRACER_ENGINEER_W/H
// no shared_frame.h e com a resolução do `vr_engineer_write_frame`.

export const RADIO_SUPERSAMPLE = 2;
const LOGICAL_W = 512;
const LOGICAL_H = 128;
export const RADIO_VR_W = LOGICAL_W * RADIO_SUPERSAMPLE; // 1024
export const RADIO_VR_H = LOGICAL_H * RADIO_SUPERSAMPLE; // 256

const FONT = "'Space Grotesk Variable', 'Space Grotesk', 'Segoe UI', sans-serif";

function clamp01(v) {
  return Math.max(0, Math.min(1, v));
}

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

// "câmbio quebrou" -> "Câmbio quebrou" (só a 1ª maiúscula, sem ponto). Igual ao desktop.
function capDetail(s) {
  if (!s) return "";
  const t = String(s).trim();
  return t ? t.charAt(0).toUpperCase() + t.slice(1) : "";
}

// Desenha o card. `message = { severity, text, detail, alpha? }` ou null (transparente).
// severity: "light" | "heavy" | "dnf" (rádio) ou "warn" (aviso pessoal do jogador).
export function drawRadioCard(ctx, message) {
  ctx.setTransform(RADIO_SUPERSAMPLE, 0, 0, RADIO_SUPERSAMPLE, 0, 0);
  ctx.clearRect(0, 0, LOGICAL_W, LOGICAL_H);
  if (!message) return; // sem mensagem → frame transparente (o quad não mostra nada)

  const sev = message.severity;
  const isWarn = sev === "warn";
  const isRed = sev === "dnf" || sev === "heavy";
  const accent = isWarn ? "#ffab2e" : isRed ? "#f85149" : "#ff8c1a";
  ctx.globalAlpha = clamp01(message.alpha == null ? 1 : message.alpha);

  // Fundo com FAIXA SUPERIOR opaca que desce pra translúcido: o topo tapa a linha de
  // texto do iRacing atrás (ex. "Press and hold Escape to tow"), o corpo fica leve como
  // no desktop. (No desktop o card inteiro é translúcido — renderizadores independentes.)
  const x = 2;
  const y = 2;
  const w = LOGICAL_W - 4;
  const h = LOGICAL_H - 4;
  const r = 14;
  const base = sev === "dnf" ? "24,7,7" : isWarn ? "26,17,4" : "11,13,16";
  roundRect(ctx, x, y, w, h, r);
  const grad = ctx.createLinearGradient(0, y, 0, y + h);
  grad.addColorStop(0, `rgba(${base},1)`); // topo: opaco (cobre o texto)
  grad.addColorStop(0.5, `rgba(${base},1)`);
  grad.addColorStop(0.78, `rgba(${base},0.82)`); // desce pra translúcido
  grad.addColorStop(1, `rgba(${base},0.82)`);
  ctx.fillStyle = grad;
  ctx.fill();

  // Acento: aviso = moldura âmbar inteira (alerta acionável); rádio = barra à esquerda.
  if (isWarn) {
    ctx.lineWidth = 2;
    ctx.strokeStyle = accent;
    roundRect(ctx, x + 1, y + 1, w - 2, h - 2, r - 1);
    ctx.stroke();
  } else {
    ctx.save();
    roundRect(ctx, x, y, w, h, r);
    ctx.clip();
    ctx.fillStyle = accent;
    ctx.fillRect(x, y, 5, h);
    ctx.restore();
  }

  const cx = LOGICAL_W / 2;
  ctx.textBaseline = "middle";
  ctx.textAlign = "center";

  // ── Tag centralizada + marcador (ponto pro rádio, ⚠ pro aviso) à esquerda dela ──
  const tag = isWarn ? "SEU CARRO · ATENÇÃO" : sev === "dnf" ? "ABANDONO" : "RÁDIO DA EQUIPE";
  ctx.font = `700 11px ${FONT}`;
  const tagY = y + 22;
  const tagW = ctx.measureText(tag).width;
  ctx.fillStyle = isWarn ? "#ffce7a" : "#9aa4ad";
  ctx.fillText(tag, cx, tagY);
  const markX = cx - tagW / 2 - 9;
  if (isWarn) {
    ctx.fillStyle = accent;
    ctx.fillText("⚠", markX, tagY);
  } else {
    ctx.beginPath();
    ctx.arc(markX, tagY, 3.5, 0, Math.PI * 2);
    ctx.fillStyle = accent;
    ctx.fill();
  }

  // ── Texto principal centralizado (até 2 linhas) + detalhe centralizado abaixo ──
  ctx.font = `700 17px ${FONT}`;
  const textLines = wrapText(ctx, message.text, w - 44, 2);
  const detail = capDetail(message.detail);

  const areaTop = tagY + 12;
  const areaBottom = y + h - 10;
  const lineH = 22;
  const detailH = detail ? 18 : 0;
  const blockH = textLines.length * lineH + detailH;
  let ty = areaTop + (areaBottom - areaTop - blockH) / 2 + lineH / 2;

  ctx.fillStyle = isWarn ? "#ffd89a" : "#ffffff";
  for (const line of textLines) {
    ctx.fillText(line, cx, ty);
    ty += lineH;
  }
  if (detail) {
    ctx.font = `500 13px ${FONT}`;
    ctx.fillStyle = isWarn ? "#e9caa1" : "#b6c2cf";
    ctx.fillText(truncate(ctx, `— ${detail}`, w - 44), cx, ty + 1);
  }

  ctx.globalAlpha = 1;
}
