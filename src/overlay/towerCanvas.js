// Renderizador ÚNICO da torre de tempos — serve o overlay de monitor E o de VR.
//
// Canvas (e não React/DOM) porque o VR precisa dos pixels crus. É a mesma função
// nos dois lugares, então o que muda aqui aparece nos dois.
//
// O VISUAL vem de um TEMA (towerThemes.js): mesma estrutura, 3 peles. O fundo do
// canvas é TRANSPARENTE — só o painel (0..PANEL_W) é pintado; a faixa da direita
// fica livre pros PINS flutuarem sobre o jogo (molde RaceLab).

import { getTeamLogoSrc } from "../components/team/TeamLogoMark";
import { buildTowerSections, isTeammate, playerTeam } from "./towerRows";
import { DEFAULT_THEME } from "./towerThemes";

// Categoria do EVENTO -> nome exibido + arquivo do logo (public/utilities/categorias/recortadas).
const CATEGORY_META = {
  gt3: { name: "GT3", logo: "GT3" },
  gt4: { name: "GT4", logo: "GT4" },
  lmp2: { name: "LMP2", logo: "LMP2" },
  endurance: { name: "ENDURANCE", logo: "ENDURANCE" },
  production_challenger: { name: "PRODUCTION", logo: "PRODUCTION" },
  production: { name: "PRODUCTION", logo: "PRODUCTION" },
  bmw_m2: { name: "BMW M2", logo: "M2 CUP" },
  toyota_amador: { name: "GR CUP", logo: "GR CUP" },
  mazda_amador: { name: "MX5 CUP", logo: "MX5 CUP" },
  toyota_rookie: { name: "GR ROOKIE", logo: "GR ROOKIE" },
  mazda_rookie: { name: "MX5 ROOKIE", logo: "MX5 ROOKIE" },
};

function categoryMeta(id) {
  return CATEGORY_META[id] || { name: String(id || "").toUpperCase(), logo: null };
}

function categoryLogoSrc(id) {
  const m = categoryMeta(id);
  return m.logo ? `/utilities/categorias/recortadas/${encodeURIComponent(m.logo)}.png` : null;
}

// O layout é todo pensado em unidades LÓGICAS (512×1024). Pra deixar o texto
// nítido no VR (e no monitor) a gente renderiza num buffer SUPERSAMPLED: o canvas
// tem VR_W×VR_H pixels reais e `drawTower` aplica um scale de SUPERSAMPLE, então
// nenhuma métrica/fonte abaixo precisa mudar — só cai mais pixel na mesma área.
// VR_W/VR_H (o buffer que vai pra memória compartilhada) DEVE casar com
// IRACER_OVERLAY_W/H (shared_frame.h) e W/H (vr_overlay.rs).
export const SUPERSAMPLE = 2;
const LOGICAL_W = 512;
const LOGICAL_H = 1024;
export const VR_W = LOGICAL_W * SUPERSAMPLE;
export const VR_H = LOGICAL_H * SUPERSAMPLE;

const FONT = "'Space Grotesk', 'Segoe UI', sans-serif";

// ─── Métricas (compartilhadas por todos os temas) ─────────────────────────────
export const PANEL_W = 452;
const PAD = 10;
export const SESSION_H = 58; // 2 linhas de info + os rótulos "pits"/"best" no fim
const CLASS_H = 26;
const ROW_H = 30;
const SEP_H = 12;
const CLASS_GAP = 5;

const POS_RIGHT = 28;
const POS_CHIP_W = 28; // largura do chip de posição (tema "block")
const LOGO_X = 34;
const LOGO_W = 30;
const LOGO_H = 20;
const NAME_X = 70;
const NAME_RIGHT = 196;
const DELTA_RIGHT = 232;
const STOPS_CENTER = 256;
const STOPS_LEFT = 236;
const TIRE_SIZE = 20;
const TIRE_STEP = 13;
const TIRE_MAX = 6;
const TIRE_SPAN = 54;
const FASTEST_RIGHT = 356;
const POINTS_RIGHT = 414;
const POINTS_GAIN_X = 418;
const PIN_X = PANEL_W;

export function formatTowerPosition(position) {
  return position > 0 ? String(position) : "–";
}

function hexToRgba(hex, a) {
  if (typeof hex !== "string" || !/^#([0-9a-f]{6})$/i.test(hex)) {
    return `rgba(125,133,144,${a})`;
  }
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r},${g},${b},${a})`;
}

// Texto legível sobre um fundo colorido (claro -> texto escuro; escuro -> branco).
function readableOn(hex) {
  if (typeof hex !== "string" || !/^#([0-9a-f]{6})$/i.test(hex)) return "#ffffff";
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  const lum = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
  return lum > 0.55 ? "#0b0d10" : "#ffffff";
}

function truncate(ctx, text, maxW) {
  if (ctx.measureText(text).width <= maxW) return text;
  let t = text;
  while (t.length > 1 && ctx.measureText(t + "…").width > maxW) t = t.slice(0, -1);
  return t + "…";
}

function roundRect(ctx, x, y, w, h, r) {
  ctx.beginPath();
  const hasRadius = Array.isArray(r) ? r.some((v) => v > 0) : r > 0;
  if (hasRadius) ctx.roundRect(x, y, w, h, r);
  else ctx.rect(x, y, w, h);
}

// Sombra de LEGIBILIDADE pro texto/ícones sobre o jogo (cockpit escuro OU pista
// clara). Liga ANTES de desenhar o conteúdo de uma linha e desliga DEPOIS — nunca
// sobre os grandes retângulos de fundo (senão fica um borrão embaixo de cada faixa).
function fgShadow(ctx, on) {
  ctx.shadowColor = on ? "rgba(0,0,0,0.9)" : "transparent";
  ctx.shadowBlur = on ? 3 : 0;
  ctx.shadowOffsetX = 0;
  ctx.shadowOffsetY = on ? 1 : 0;
}

// ─── Assets (logos + pneus) ───────────────────────────────────────────────────
const TX = "/utilities/textures/";
const TIRE_DRY_SRC = `${TX}Pneu%20Seco.png`;
const TIRE_WET_SRC = `${TX}Pneu%20Molhado.png`;

function loadImage(src) {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => resolve(null);
    img.src = src;
  });
}

// Caixa útil de uma imagem — aparando margens/lixo pra todo selo preencher a
// caixa do cabeçalho igual, sem depender do recorte do arquivo.
//
// Aparo por COBERTURA, não por "qualquer pixel". Alguns PNGs (ex.: GT3) têm o selo
// no topo e, lá embaixo, uma barra/véu SEMITRANSPARENTE (alpha ~180) que um aparo
// ingênuo (>8) conta como conteúdo — daí a caixa cobria a imagem toda e sobrava
// vazio embaixo. Aqui uma linha/coluna só conta se tiver uma fração mínima de
// pixels REALMENTE opacos (alpha > ALPHA_SOLID). Margens transparentes e véus
// fracos ficam de fora; selos limpos não mudam. Cacheado por src (o loop é caro).
const _trimCache = new Map();
const ALPHA_SOLID = 200; // pixel "de verdade" (acima do véu/anti-aliasing)
const COVER_FRAC = 0.05; // fração do eixo cruzado pra linha/coluna contar
function trimTransparent(img, key) {
  if (key && _trimCache.has(key)) return _trimCache.get(key);
  const w = img.naturalWidth || img.width;
  const h = img.naturalHeight || img.height;
  let box = { x: 0, y: 0, w, h };
  try {
    const c = document.createElement("canvas");
    c.width = w;
    c.height = h;
    const cx = c.getContext("2d", { willReadFrequently: true });
    cx.drawImage(img, 0, 0);
    const data = cx.getImageData(0, 0, w, h).data;
    const rowCount = new Array(h).fill(0);
    const colCount = new Array(w).fill(0);
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        if (data[(y * w + x) * 4 + 3] > ALPHA_SOLID) {
          rowCount[y]++;
          colCount[x]++;
        }
      }
    }
    const rowMin = Math.max(2, COVER_FRAC * w);
    const colMin = Math.max(2, COVER_FRAC * h);
    let minX = w;
    let minY = h;
    let maxX = -1;
    let maxY = -1;
    for (let y = 0; y < h; y++) {
      if (rowCount[y] >= rowMin) {
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
      }
    }
    for (let x = 0; x < w; x++) {
      if (colCount[x] >= colMin) {
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
      }
    }
    // Achou conteúdo opaco → usa a caixa. Senão (logo inteiro fraco) mantém tudo.
    if (maxX >= minX && maxY >= minY) {
      box = { x: minX, y: minY, w: maxX - minX + 1, h: maxY - minY + 1 };
    }
  } catch {
    /* getImageData falhou (imagem "suja"/CORS) — usa a imagem inteira */
  }
  if (key) _trimCache.set(key, box);
  return box;
}

export async function preloadAssets(data) {
  const teams = new Set();
  data.classes.forEach((c) => c.cars.forEach((car) => teams.add(car.team)));

  const logos = new Map();
  await Promise.all(
    [...teams].map(async (team) => {
      const src = getTeamLogoSrc(team);
      if (!src) return;
      const img = await loadImage(src);
      if (img) logos.set(team, img);
    }),
  );

  const catSrc = categoryLogoSrc(data.session?.category);
  const [tireDry, tireWet, categoryLogo] = await Promise.all([
    loadImage(TIRE_DRY_SRC),
    loadImage(TIRE_WET_SRC),
    catSrc ? loadImage(catSrc) : Promise.resolve(null),
  ]);
  const categoryLogoTrim = categoryLogo ? trimTransparent(categoryLogo, catSrc) : null;
  return { logos, tireDry, tireWet, categoryLogo, categoryLogoTrim };
}

// ─── Pins externos (iguais em todos os temas) ─────────────────────────────────
function pinsFor(car) {
  const pins = [];
  if (car.fol) pins.push("fastest");
  if (car.flag === "meatball") pins.push("meatball");
  if (car.flag === "black") pins.push("black");
  if (car.flag === "checkered") pins.push("checkered");
  if (car.pit) pins.push("pit");
  return pins;
}

const PIN_SIZE = 16;
const PIN_GAP = 4;

function pinSquare(ctx, left, cy, fill) {
  const top = cy - PIN_SIZE / 2;
  ctx.fillStyle = fill;
  ctx.fillRect(left, top, PIN_SIZE, PIN_SIZE);
  ctx.strokeStyle = "rgba(255,255,255,0.35)";
  ctx.lineWidth = 1;
  ctx.strokeRect(left + 0.5, top + 0.5, PIN_SIZE - 1, PIN_SIZE - 1);
}

function drawStopwatch(ctx, cx, cy) {
  ctx.strokeStyle = "#ffffff";
  ctx.lineWidth = 1.2;
  ctx.beginPath();
  ctx.arc(cx, cy + 0.5, 4, 0, Math.PI * 2);
  ctx.stroke();
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(cx - 1.2, cy - 6.5, 2.4, 2);
  ctx.beginPath();
  ctx.moveTo(cx, cy + 0.5);
  ctx.lineTo(cx, cy - 2.2);
  ctx.moveTo(cx, cy + 0.5);
  ctx.lineTo(cx + 2.4, cy + 1.4);
  ctx.stroke();
}

function drawPin(ctx, type, left, cy, theme) {
  const c = left + PIN_SIZE / 2;
  if (type === "fastest") {
    pinSquare(ctx, left, cy, theme.purple);
    drawStopwatch(ctx, c, cy);
  } else if (type === "meatball") {
    pinSquare(ctx, left, cy, "#0b0d10");
    ctx.fillStyle = "#ff8c1a";
    ctx.beginPath();
    ctx.arc(c, cy, 4.2, 0, Math.PI * 2);
    ctx.fill();
  } else if (type === "black") {
    pinSquare(ctx, left, cy, "#0b0d10");
  } else if (type === "checkered") {
    const s = PIN_SIZE / 4;
    const top = cy - PIN_SIZE / 2;
    for (let r = 0; r < 4; r++) {
      for (let col = 0; col < 4; col++) {
        ctx.fillStyle = (r + col) % 2 === 0 ? "#ffffff" : "#0b0d10";
        ctx.fillRect(left + col * s, top + r * s, s, s);
      }
    }
    ctx.strokeStyle = "rgba(255,255,255,0.35)";
    ctx.lineWidth = 1;
    ctx.strokeRect(left + 0.5, top + 0.5, PIN_SIZE - 1, PIN_SIZE - 1);
  } else if (type === "pit") {
    pinSquare(ctx, left, cy, "#ffffff");
    ctx.fillStyle = "#0b0d10";
    ctx.font = `800 11px ${FONT}`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText("P", c, cy + 0.5);
  }
  return PIN_SIZE;
}

function drawPins(ctx, car, cy, theme) {
  let left = PIN_X;
  for (const type of pinsFor(car)) {
    left += drawPin(ctx, type, left, cy, theme) + PIN_GAP;
  }
}

function tireStints(car) {
  if (Array.isArray(car.tireHistory) && car.tireHistory.length) return car.tireHistory;
  const n = (car.stops ?? 0) + 1;
  const compound = car.tire === "wet" ? "wet" : "dry";
  return Array(n).fill(compound);
}

function drawTireStack(ctx, car, cy, assets) {
  const stints = tireStints(car);
  const count = Math.min(stints.length, TIRE_MAX);
  const step = count > 1 ? Math.min(TIRE_STEP, (TIRE_SPAN - TIRE_SIZE) / (count - 1)) : 0;
  for (let i = 0; i < count; i++) {
    const img = stints[i] === "wet" ? assets.tireWet : assets.tireDry;
    if (img) ctx.drawImage(img, STOPS_LEFT + i * step, cy - TIRE_SIZE / 2, TIRE_SIZE, TIRE_SIZE);
  }
}

// ─── Linha ───────────────────────────────────────────────────────────────────
function drawRow(ctx, car, y, assets, team, theme) {
  const isPlayer = Boolean(car.player);
  const mate = isTeammate(car, team);
  const cy = y + ROW_H / 2;

  // Fundo/realce da linha — depende do tema. O realce de você/companheiro NUNCA
  // é uma cor de linha diferente; é sempre a cor da PRÓPRIA equipe (+ nome).
  if (theme.rowStyle === "block") {
    ctx.fillStyle = hexToRgba(car.color, theme.rowAlpha * 0.6);
    ctx.fillRect(0, y, PANEL_W, ROW_H);
  } else {
    const grad = ctx.createLinearGradient(0, y, PANEL_W, y);
    grad.addColorStop(0, hexToRgba(car.color, theme.rowAlpha));
    grad.addColorStop(0.35, hexToRgba(car.color, theme.rowAlpha * 0.35));
    grad.addColorStop(0.7, "rgba(0,0,0,0)");
    ctx.fillStyle = grad;
    ctx.fillRect(0, y, PANEL_W, ROW_H);
    if (theme.sheen) {
      ctx.fillStyle = "rgba(255,255,255,0.05)";
      ctx.fillRect(0, y, PANEL_W, 1);
    }
  }

  fgShadow(ctx, true); // conteúdo da linha (texto/logo/pneus/pins) com sombra
  ctx.textBaseline = "middle";

  // Posição: chip sólido (block) ou número + acento lateral (gradient/glow).
  if (theme.rowStyle === "block") {
    roundRect(ctx, 3, y + 3, POS_CHIP_W, ROW_H - 6, 3);
    ctx.fillStyle = car.color;
    ctx.fill();
    ctx.fillStyle = readableOn(car.color);
    ctx.font = `800 14px ${FONT}`;
    ctx.textAlign = "center";
    ctx.fillText(formatTowerPosition(car.pos), 3 + POS_CHIP_W / 2, cy);
  } else {
    if (theme.accentWidth > 0) {
      ctx.fillStyle = car.color;
      ctx.fillRect(0, y, theme.accentWidth, ROW_H);
    }
    ctx.fillStyle = theme.posColor;
    ctx.font = `700 14px ${FONT}`;
    ctx.textAlign = "right";
    ctx.fillText(formatTowerPosition(car.pos), POS_RIGHT, cy);
  }

  const img = assets.logos.get(car.team);
  if (img) ctx.drawImage(img, LOGO_X, y + (ROW_H - LOGO_H) / 2, LOGO_W, LOGO_H);

  // Marcador de rivalidade (💥 nemesis / 🔥 rival) à esquerda do nome, empurrando-o.
  ctx.textAlign = "left";
  let nameX = NAME_X;
  const rivalGlyph =
    car.rivalRole === "nemesis" ? "\u{1F4A5}" : car.rivalRole === "rival" ? "\u{1F525}" : null;
  if (rivalGlyph) {
    ctx.font = `12px ${FONT}`;
    ctx.fillText(rivalGlyph, nameX, cy);
    nameX += ctx.measureText(rivalGlyph).width + 3;
  }

  // Nome: verde = você, azul = companheiro, senão o texto do tema.
  const nameColor = isPlayer ? theme.playerColor : mate ? theme.teammateColor : theme.text;
  const nameWeight = isPlayer || mate || theme.rowStyle === "block" ? 700 : 600;
  ctx.fillStyle = nameColor;
  ctx.font = `${nameWeight} 14px ${FONT}`;
  ctx.fillText(truncate(ctx, car.name, NAME_RIGHT - nameX), nameX, cy);

  // Delta.
  ctx.textAlign = "right";
  ctx.font = `600 12px ${FONT}`;
  if (!car.delta) {
    ctx.fillStyle = theme.textMuted;
    ctx.fillText("— 0", DELTA_RIGHT, cy);
  } else {
    const up = car.delta > 0;
    ctx.fillStyle = up ? theme.up : theme.down;
    ctx.fillText(`${up ? "▲" : "▼"} ${Math.abs(car.delta)}`, DELTA_RIGHT, cy);
  }

  // Pilha de pneus.
  drawTireStack(ctx, car, cy, assets);

  // Melhor volta.
  ctx.fillStyle = car.fol ? theme.purple : theme.text;
  ctx.font = `600 13px ${FONT}`;
  ctx.textAlign = "right";
  ctx.fillText(car.fastest, FASTEST_RIGHT, cy);

  // Pontos + ganho.
  ctx.fillStyle = theme.text;
  ctx.font = `700 13px ${FONT}`;
  ctx.textAlign = "right";
  ctx.fillText(String(car.points ?? 0), POINTS_RIGHT, cy);
  if (car.gain > 0) {
    ctx.fillStyle = theme.gainGreen;
    ctx.font = `700 10px ${FONT}`;
    ctx.textAlign = "left";
    ctx.fillText(`+${car.gain}`, POINTS_GAIN_X, cy);
  }

  drawPins(ctx, car, cy, theme);
  fgShadow(ctx, false);
}

function drawSeparator(ctx, y) {
  const cy = y + SEP_H / 2;
  ctx.strokeStyle = "rgba(255,255,255,0.16)";
  ctx.lineWidth = 1;
  ctx.setLineDash([3, 4]);
  ctx.beginPath();
  ctx.moveTo(PAD, cy);
  ctx.lineTo(PANEL_W - PAD, cy);
  ctx.stroke();
  ctx.setLineDash([]);
}

// Cabeçalho de uma classe — 3 estilos.
// Carrinho miúdo (silhueta lateral) pro selo de contagem da classe.
function drawCarIcon(ctx, cx, cy, color) {
  ctx.fillStyle = color;
  ctx.beginPath();
  ctx.roundRect(cx - 7, cy - 1, 14, 4, 1.5); // chassi
  ctx.fill();
  ctx.beginPath(); // cabine
  ctx.moveTo(cx - 4, cy - 1);
  ctx.lineTo(cx - 2, cy - 4.5);
  ctx.lineTo(cx + 2.5, cy - 4.5);
  ctx.lineTo(cx + 4, cy - 1);
  ctx.closePath();
  ctx.fill();
  ctx.beginPath(); // rodas
  ctx.arc(cx - 4, cy + 3, 2, 0, Math.PI * 2);
  ctx.arc(cx + 4.5, cy + 3, 2, 0, Math.PI * 2);
  ctx.fill();
}

function drawClassHeader(ctx, cls, y, theme) {
  const midY = y + CLASS_H / 2;

  if (theme.classStyle === "band") {
    // Banda robusta: wash da cor da classe + acento + rótulo + selo com carrinho.
    const g = ctx.createLinearGradient(0, y, PANEL_W, y);
    g.addColorStop(0, hexToRgba(cls.color, 0.5));
    g.addColorStop(0.45, hexToRgba(cls.color, 0.16));
    g.addColorStop(1, "rgba(0,0,0,0.28)");
    ctx.fillStyle = g;
    ctx.fillRect(0, y, PANEL_W, CLASS_H);
    ctx.fillStyle = cls.color;
    ctx.fillRect(0, y, 4, CLASS_H);

    ctx.font = `italic 800 15px ${FONT}`;
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    ctx.fillStyle = "#ffffff";
    ctx.fillText(cls.label, PAD, midY);

    // Selo: pill na cor da classe com contagem + carrinho.
    const countStr = String(cls.cars.length);
    ctx.font = `800 12px ${FONT}`;
    const cw = ctx.measureText(countStr).width;
    const pillW = cw + 34;
    const pillX = PANEL_W - PAD - pillW;
    roundRect(ctx, pillX, midY - 9, pillW, 18, 4);
    ctx.fillStyle = cls.color;
    ctx.fill();
    const ink = readableOn(cls.color);
    ctx.fillStyle = ink;
    ctx.textAlign = "left";
    ctx.fillText(countStr, pillX + 9, midY);
    drawCarIcon(ctx, pillX + 9 + cw + 12, midY, ink);
    return; // a banda desenha o próprio selo de contagem
  }

  if (theme.classStyle === "tab") {
    ctx.fillStyle = theme.classBg;
    ctx.fillRect(0, y, PANEL_W, CLASS_H);
    ctx.font = `800 11px ${FONT}`;
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    const tabW = ctx.measureText(cls.label).width + 12;
    roundRect(ctx, 0, midY - 7, tabW, 14, [0, 3, 3, 0]);
    ctx.fillStyle = cls.color;
    ctx.fill();
    ctx.fillStyle = "#0b0d10";
    ctx.fillText(cls.label, 6, midY);
  } else if (theme.classStyle === "underline") {
    ctx.fillStyle = theme.classBg;
    ctx.fillRect(0, y, PANEL_W, CLASS_H);
    ctx.font = `800 12px ${FONT}`;
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    ctx.fillStyle = cls.color;
    ctx.fillText(cls.label, PAD, midY);
    ctx.fillStyle = cls.color;
    ctx.fillRect(0, y + CLASS_H - 2, PANEL_W, 2);
  } else {
    // label: sem faixa; rótulo grande + barrinha curta embaixo.
    ctx.font = `800 13px ${FONT}`;
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    ctx.fillStyle = cls.color;
    ctx.fillText(cls.label, PAD, midY - 1);
    const w = ctx.measureText(cls.label).width;
    ctx.fillRect(PAD, midY + 8, w, 2);
  }

  ctx.fillStyle = cls.color;
  ctx.font = `700 11px ${FONT}`;
  ctx.textAlign = "right";
  ctx.textBaseline = "middle";
  ctx.fillText(String(cls.cars.length), PANEL_W - PAD, midY);
}

// Desenha uma imagem preservando o aspecto, centrada numa caixa. `src` (opcional)
// = recorte útil da imagem (via trimTransparent), pra ignorar margens vazias.
function drawImageFit(ctx, img, bx, by, bw, bh, src) {
  const sw = src ? src.w : img.naturalWidth || img.width;
  const sh = src ? src.h : img.naturalHeight || img.height;
  const scale = Math.min(bw / sw, bh / sh);
  const w = sw * scale;
  const h = sh * scale;
  const dx = bx + (bw - w) / 2;
  const dy = by + (bh - h) / 2;
  if (src) ctx.drawImage(img, src.x, src.y, src.w, src.h, dx, dy, w, h);
  else ctx.drawImage(img, dx, dy, w, h);
}

// Ícones de clima desenhados no canvas (não há PNG). Centrados em (cx,cy).
function drawWeatherIcon(ctx, cx, cy, condition) {
  const cloud = (fill) => {
    ctx.fillStyle = fill;
    ctx.beginPath();
    ctx.arc(cx - 4, cy + 1, 4, 0, Math.PI * 2);
    ctx.arc(cx + 1, cy - 2, 5, 0, Math.PI * 2);
    ctx.arc(cx + 5, cy + 1, 4, 0, Math.PI * 2);
    ctx.rect(cx - 8, cy + 1, 16, 4);
    ctx.fill();
  };

  if (condition === "clear" || condition === "sun") {
    ctx.strokeStyle = "#ffd400";
    ctx.lineWidth = 1.4;
    for (let a = 0; a < 8; a++) {
      const ang = (a * Math.PI) / 4;
      ctx.beginPath();
      ctx.moveTo(cx + Math.cos(ang) * 6.5, cy + Math.sin(ang) * 6.5);
      ctx.lineTo(cx + Math.cos(ang) * 9, cy + Math.sin(ang) * 9);
      ctx.stroke();
    }
    ctx.fillStyle = "#ffd400";
    ctx.beginPath();
    ctx.arc(cx, cy, 4.5, 0, Math.PI * 2);
    ctx.fill();
  } else if (condition === "rain" || condition === "storm") {
    cloud("#9aa4ad");
    const drop = condition === "storm" ? "#ffd400" : "#58a6ff";
    ctx.strokeStyle = drop;
    ctx.lineWidth = 1.6;
    for (let i = -1; i <= 1; i++) {
      ctx.beginPath();
      ctx.moveTo(cx + i * 4, cy + 6);
      ctx.lineTo(cx + i * 4 - 1.5, cy + 10);
      ctx.stroke();
    }
  } else {
    // clouds (nublado)
    cloud("#c9d1d9");
    ctx.fillStyle = "rgba(255,212,0,0.9)";
    ctx.beginPath();
    ctx.arc(cx + 6, cy - 5, 3, 0, Math.PI * 2); // solzinho espiando
    ctx.fill();
    cloud("#aeb7bf");
  }
}

// Cabeçalho da sessão — o "topo" da torre. Estado da bandeira dá a cor.
const FLAG_COLORS = {
  green: "#3fb950",
  yellow: "#e3b341",
  red: "#f85149",
  checkered: "#f0f6fc",
  white: "#f0f6fc",
};
const FLAG_WORDS = {
  green: "GREEN",
  yellow: "YELLOW",
  red: "RED",
  checkered: "CHECKERED",
  white: "LAST LAP",
};
const SESSION_WORDS = { R: "RACE", Q: "QUALIFYING", P: "PRACTICE" };

function drawSessionHeader(ctx, session, theme, assets) {
  const H = SESSION_H;
  const flagKey = session.flag && FLAG_COLORS[session.flag] ? session.flag : "green";
  const flag = FLAG_COLORS[flagKey];
  // Cor de ESTADO da sessão: roxo na classificação, senão a cor da bandeira
  // (verde / amarelo / quadriculada). É ela que tinge o acento do header.
  const isQualy = session.type === "Q";
  const stateColor = isQualy ? theme.purple : flag;
  const cat = categoryMeta(session.category);
  const cy1 = 18; // linha de cima (categoria / clima)
  const cy2 = 35; // linha de baixo (sessão·bandeira / voltas)

  // Fundo: degradê vertical + cantos superiores do tema.
  const g = ctx.createLinearGradient(0, 0, 0, H);
  g.addColorStop(0, theme.sessionTop || theme.sessionBg);
  g.addColorStop(1, theme.sessionBg);
  roundRect(ctx, 0, 0, PANEL_W, H, [theme.blockRadius, theme.blockRadius, 0, 0]);
  ctx.fillStyle = g;
  ctx.fill();

  fgShadow(ctx, true); // textos do cabeçalho (categoria/sessão/clima/voltas)
  ctx.textBaseline = "middle";

  // ── ESQUERDA: logo da categoria + nome + sessão/bandeira ──
  const logoBoxW = 60;
  if (assets.categoryLogo) {
    drawImageFit(ctx, assets.categoryLogo, PAD, 5, logoBoxW, 40, assets.categoryLogoTrim);
  }
  const nameX = PAD + logoBoxW + 12;

  ctx.textAlign = "left";
  ctx.fillStyle = theme.text;
  ctx.font = `800 17px ${FONT}`;
  ctx.fillText(cat.name, nameX, cy1);

  const sess = SESSION_WORDS[session.type] || "RACE";
  ctx.font = `700 9px ${FONT}`;
  // Palavra da sessão em ROXO na classificação (destaque), senão apagada.
  ctx.fillStyle = isQualy ? stateColor : theme.textMuted;
  ctx.fillText(sess, nameX, cy2);
  const sw = ctx.measureText(sess).width;
  ctx.fillStyle = flag; // ponto + palavra na cor real da bandeira
  ctx.beginPath();
  ctx.arc(nameX + sw + 10, cy2, 3, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillText(FLAG_WORDS[flagKey], nameX + sw + 17, cy2);

  // ── DIREITA: clima (cima) + voltas (baixo) ──
  const rx = PANEL_W - PAD;
  const w = session.weather || {};

  // Clima: [ícone] AR° (só o ar).
  ctx.textAlign = "right";
  ctx.font = `800 14px ${FONT}`;
  ctx.fillStyle = theme.text;
  const airStr = w.airTemp != null ? `${w.airTemp}°` : "--";
  ctx.fillText(airStr, rx, cy1);
  const airW = ctx.measureText(airStr).width;

  drawWeatherIcon(ctx, rx - airW - 14, cy1, w.condition);

  // Voltas: LAPS 36/40
  ctx.textAlign = "right";
  ctx.font = `700 12px ${FONT}`;
  ctx.fillStyle = theme.textMuted;
  const totalStr = `/${session.totalLaps}`;
  ctx.fillText(totalStr, rx, cy2);
  const totalW = ctx.measureText(totalStr).width;

  ctx.font = `800 18px ${FONT}`;
  ctx.fillStyle = theme.text;
  const lapStr = String(session.lap);
  ctx.fillText(lapStr, rx - totalW - 2, cy2);
  const lapW = ctx.measureText(lapStr).width;

  ctx.font = `700 8px ${FONT}`;
  ctx.fillStyle = theme.textMuted;
  ctx.fillText("LAPS", rx - totalW - lapW - 8, cy2);

  // Dois rótulos que ajudam: "pits" sobre a coluna de pneus e "best" sobre a
  // melhor volta — no chrome do header, acima da linha de acento.
  ctx.font = `700 10px ${FONT}`;
  ctx.fillStyle = theme.textMuted;
  ctx.textAlign = "center";
  ctx.fillText("PITS", STOPS_CENTER, H - 10);
  ctx.fillText("BEST", 325, H - 10);

  fgShadow(ctx, false);

  // Acento inferior na cor do ESTADO (verde/amarelo/quadriculada/roxo-quali),
  // esvaindo pra direita — é o sinal de "a bandeira mudou".
  const line = ctx.createLinearGradient(0, 0, PANEL_W, 0);
  line.addColorStop(0, stateColor);
  line.addColorStop(0.65, hexToRgba(stateColor, 0));
  ctx.fillStyle = line;
  ctx.fillRect(0, H - 2, PANEL_W, 2);
}

function sectionsHeight(sections) {
  let h = SESSION_H; // o colhead agora vive DENTRO do header
  sections.forEach((s, i) => {
    if (i > 0) h += CLASS_GAP;
    h += CLASS_H + s.rows.reduce((a, r) => a + (r.kind === "separator" ? SEP_H : ROW_H), 0);
  });
  return h;
}

// Altura (px lógicos = CSS) que a torre ocupa pros dados atuais — usada pra dizer
// ao vigia de cursor onde é "em cima da torre" (área de hover).
export function towerContentHeight(data) {
  if (!data) return 0;
  return sectionsHeight(buildTowerSections(data));
}

// ─── Torre ───────────────────────────────────────────────────────────────────
export function drawTower(ctx, data, assets, theme = DEFAULT_THEME) {
  // Desenha em coordenadas LÓGICAS; o supersample vira pixels de verdade.
  ctx.setTransform(SUPERSAMPLE, 0, 0, SUPERSAMPLE, 0, 0);
  ctx.clearRect(0, 0, LOGICAL_W, LOGICAL_H);

  const team = playerTeam(data);
  const sections = buildTowerSections(data);

  // Classe única (MX5 Cup, GR Rookie, etc.): o iRacing não manda nome de classe,
  // então o `label` vem vazio e a banda fica só com o contador, sem "GT3". Cai pro
  // nome da categoria do evento (o mesmo que aparece no topo). Copia a `cls` pra
  // não mexer no dado ao vivo.
  const fallbackLabel = categoryMeta(data.session?.category).name;
  sections.forEach((s) => {
    if (!s.cls.label && fallbackLabel) s.cls = { ...s.cls, label: fallbackLabel };
  });

  const total = sectionsHeight(sections);

  // Faixa da sessão (topo do painel).
  drawSessionHeader(ctx, data.session, theme, assets);

  // A TABELA começa logo abaixo do header. O cabeçalho de colunas fica ANEXADO
  // ao topo do 1º bloco (mesmo fundo), então não flutua solto no meio.
  let y = SESSION_H;

  sections.forEach(({ cls, rows }, i) => {
    if (i > 0) y += CLASS_GAP;

    const isLast = i === sections.length - 1;
    const bodyH = rows.reduce((a, r) => a + (r.kind === "separator" ? SEP_H : ROW_H), 0);
    const blockH = CLASS_H + bodyH;

    // Fundo do bloco (classe + corpo), cantos do tema.
    const br = theme.blockRadius;
    roundRect(ctx, 0, y, PANEL_W, blockH, [0, 0, isLast ? br : 0, isLast ? br : 0]);
    ctx.fillStyle = theme.panelBg;
    ctx.fill();

    drawClassHeader(ctx, cls, y, theme);
    y += CLASS_H;

    rows.forEach((row) => {
      if (row.kind === "separator") {
        drawSeparator(ctx, y);
        y += SEP_H;
      } else {
        drawRow(ctx, row.car, y, assets, team, theme);
        y += ROW_H;
      }
    });
  });

  return total;
}
