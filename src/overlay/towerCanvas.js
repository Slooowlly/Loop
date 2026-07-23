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
import { tireCompoundDryWet } from "./tireCompounds";

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
  return m.logo ? `/utilities/categorias/recortadas/${encodeURIComponent(m.logo)}.webp` : null;
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
const SEP_H = 18;
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
// Tamanho VISÍVEL (do conteúdo, sem a margem transparente) que todos os ícones da coluna de
// paradas compartilham — normaliza pneu/combustível/triângulo pra ficarem do mesmo tamanho.
const ICON_TARGET = 16;
// Combustível/peça são menores que o pneu → sobra folga ao lado. Quando um deles é vizinho de
// um pneu, encosta MAIS (passo menor) do que pneu↔pneu, pra ficar "grudado".
const SMALL_ICON_GLUE = 3;
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
const TIRE_DRY_SRC = `${TX}Pneu%20Seco.webp`;
const TIRE_WET_SRC = `${TX}Pneu%20Molhado.webp`;
const FUEL_SRC = `${TX}Fuel.webp`;

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
  const [tireDry, tireWet, fuel, categoryLogo] = await Promise.all([
    loadImage(TIRE_DRY_SRC),
    loadImage(TIRE_WET_SRC),
    loadImage(FUEL_SRC),
    catSrc ? loadImage(catSrc) : Promise.resolve(null),
  ]);
  const categoryLogoTrim = categoryLogo ? trimTransparent(categoryLogo, catSrc) : null;
  return { logos, tireDry, tireWet, fuel, categoryLogo, categoryLogoTrim };
}

// ─── Pins externos (iguais em todos os temas) ─────────────────────────────────
// Cada pino é um descritor `{ type, ... }`, desenhado da esquerda pra direita. O "P"
// (no box) vem ANTES do triângulo de alerta — quando o carro entra pra reparar, o P
// aparece primeiro e o triângulo fica ao lado. O tempo de pit NÃO é pino: é desenhado
// em cima da coluna de pneus (ver `drawPitTimeBadge`), pois é a métrica da parada.
export function pinsFor(car) {
  const pins = [];
  if (car.pit) pins.push({ type: "pit" }); // "P" primeiro
  if (car.alert === "heavy") pins.push({ type: "alertHeavy" });
  else if (car.alert === "light") pins.push({ type: "alertLight" });
  if (car.fol) pins.push({ type: "fastest" });
  if (car.flag === "black") pins.push({ type: "black" }); // DNF (!dq) — IA vem direto pra cá
  if (car.flag === "checkered") pins.push({ type: "checkered" });
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

// Triângulo de ALERTA de peça quebrada (piscando). GRANDE — preenche quase todo o
// quadrado, sobrando só uma moldura escura fina, pra ler como um triângulo de aviso
// (⚠) e NÃO como o meatball. Laranja claro = penalidade leve (`!black` curto),
// vermelho claro = grave (`!black` longo). Some quando o carro sai do box já
// reparado (o backend zera `car.alert`). DNF NÃO usa isto — usa a bandeira preta.
function drawAlertTriangle(ctx, left, cy, heavy) {
  const color = heavy ? "#ff5347" : "#ffab2e";
  const top = cy - PIN_SIZE / 2;
  const c = left + PIN_SIZE / 2;
  // Moldura escura fina só pra dar contorno sobre o jogo (não é um "quadrado" cheio).
  ctx.fillStyle = "rgba(11,13,16,0.85)";
  ctx.fillRect(left, top, PIN_SIZE, PIN_SIZE);
  const blink = 0.5 + 0.5 * Math.sin(Date.now() / 300);
  ctx.save();
  ctx.globalAlpha = 0.72 + 0.28 * blink; // brilhante; pulsa de leve, sem apagar
  ctx.beginPath();
  ctx.moveTo(c, top + 1); // ápice em cima, quase encostando na borda
  ctx.lineTo(left + 1, top + PIN_SIZE - 1.5);
  ctx.lineTo(left + PIN_SIZE - 1, top + PIN_SIZE - 1.5);
  ctx.closePath();
  ctx.fillStyle = color;
  ctx.fill();
  // "!" escuro no miolo do triângulo
  ctx.fillStyle = "#0b0d10";
  ctx.fillRect(c - 0.9, cy - 1.5, 1.8, 3.6);
  ctx.fillRect(c - 0.9, cy + 3.4, 1.8, 1.8);
  ctx.restore();
}

// Badge de TEMPO DE PIT desenhado EM CIMA da coluna de pneus (é a métrica da
// parada). Cronômetro + segundos parados na caixa. Compacto e TRANSLÚCIDO de
// propósito, pra ainda dar pra ver a cor das rodas por baixo. Aparece por ~3 voltas
// após a parada (o backend decide a janela e só então manda `pitSecs`). Centrado em `cx`.
function drawPitTimeBadge(ctx, cx, cy, secs) {
  const label = `${secs}s`;
  const H = 13;
  ctx.font = `800 8px ${FONT}`;
  const tw = ctx.measureText(label).width;
  const w = 12 + tw + 6; // cronômetro + texto + folga (menor)
  const x = cx - w / 2;
  const top = cy - H / 2;
  // Só o FUNDO é translúcido — pra ainda ver a cor dos pneus por baixo.
  roundRect(ctx, x, top, w, H, 3);
  ctx.fillStyle = "rgba(11,13,16,0.45)";
  ctx.fill();
  ctx.strokeStyle = "rgba(255,255,255,0.3)";
  ctx.lineWidth = 1;
  ctx.stroke();
  // Cronômetro + texto a 100% (nítidos sobre o fundo translúcido).
  drawStopwatch(ctx, x + 7, cy);
  ctx.fillStyle = "#ffffff";
  ctx.textAlign = "left";
  ctx.textBaseline = "middle";
  ctx.fillText(label, x + 12, cy + 0.5);
}

function drawPin(ctx, pin, left, cy, theme) {
  const type = pin.type;
  const c = left + PIN_SIZE / 2;
  if (type === "fastest") {
    pinSquare(ctx, left, cy, theme.purple);
    drawStopwatch(ctx, c, cy);
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
  } else if (type === "alertLight" || type === "alertHeavy") {
    drawAlertTriangle(ctx, left, cy, type === "alertHeavy");
  }
  return PIN_SIZE;
}

function drawPins(ctx, car, cy, theme) {
  let left = PIN_X;
  for (const pin of pinsFor(car)) {
    left += drawPin(ctx, pin, left, cy, theme) + PIN_GAP;
  }
}

// Composto de LARGADA escolhido pelo carro, do `CarIdxTireCompound` (a mesma info que o
// RaceLab mostra), resolvido pelo mapa índice→composto (`tireCompounds.js`). A coluna de
// paradas só tem ícone seco/chuva, então achatamos os slicks (macio/médio/duro) em "dry"
// aqui; o nome fino aparece no painel de compostos de largada. -1/desconhecido → cai no que
// já se sabia (car.tire) ou seco. É isso que deixa a torre revelar o pneu da IA ANTES da
// largada, e não só o padrão seco pra todos.
function startCompound(car) {
  const dryWet = tireCompoundDryWet(undefined, car.tireCompound);
  if (dryWet) return dryWet;
  return car.tire === "wet" ? "wet" : "dry";
}

function tireStints(car) {
  if (Array.isArray(car.tireHistory) && car.tireHistory.length) return car.tireHistory;
  const n = (car.stops ?? 0) + 1;
  return Array(n).fill(startCompound(car));
}

// Sequência de ícones da coluna de paradas: 1º = pneu de largada, depois um por PARADA
// ("dry"/"wet" = troca de pneu; "fuel" = só abasteceu; "part" = reparo de peça). Vem do
// backend (`pitIcons`); sem ele (pré-corrida) cai nos compostos por stint.
function stopIcons(car) {
  if (Array.isArray(car.pitIcons) && car.pitIcons.length) return car.pitIcons;
  return tireStints(car);
}

// Desenha um asset com o CONTEÚDO (sem a margem transparente do PNG) normalizado a `target`
// px, centrado em (cx,cy). Sem isso cada PNG sai com tamanho visual diferente (o Fuel.png tem
// ~40% de vazio → saía minúsculo; o pneu quase preenche → maior). Cacheado por `key`.
function drawIconFit(ctx, img, key, cx, cy, target) {
  const t = trimTransparent(img, key);
  const scale = target / Math.max(t.w, t.h);
  const w = t.w * scale;
  const h = t.h * scale;
  ctx.drawImage(img, t.x, t.y, t.w, t.h, cx - w / 2, cy - h / 2, w, h);
}

// Triângulo de "parou pra arrumar peça" (no lugar do pneu). Laranja, fixo (histórico da
// parada, não pisca como o alerta ao vivo). Sólido "pesa" mais que o anel do pneu, então usa
// um alvo um tico menor. Posição = 60% acima / 40% abaixo de `cy`: entre o baricentro (subia
// demais) e o centro geométrico (afundava) → alinhado com os pneus.
function drawPartIcon(ctx, cx, cy) {
  const s = ICON_TARGET - 1;
  const apexY = cy - s * 0.6;
  const baseY = cy + s * 0.4;
  ctx.fillStyle = "#ffab2e";
  ctx.beginPath();
  ctx.moveTo(cx, apexY);
  ctx.lineTo(cx - s / 2, baseY);
  ctx.lineTo(cx + s / 2, baseY);
  ctx.closePath();
  ctx.fill();
  ctx.strokeStyle = "rgba(0,0,0,0.5)";
  ctx.lineWidth = 1;
  ctx.stroke();
  ctx.fillStyle = "#0b0d10"; // "!" no meio da massa
  ctx.fillRect(cx - 1, cy - 1, 2, 3.6);
  ctx.fillRect(cx - 1, cy + 4, 2, 1.8);
}

function drawStopStack(ctx, car, cy, assets) {
  const icons = stopIcons(car);
  const count = Math.min(icons.length, TIRE_MAX);
  const step = count > 1 ? Math.min(TIRE_STEP, (TIRE_SPAN - TIRE_SIZE) / (count - 1)) : 0;
  const small = (k) => k === "fuel" || k === "part";
  let x = STOPS_LEFT;
  for (let i = 0; i < count; i++) {
    if (i > 0) {
      // Cola mais quando o ícone atual OU o anterior é menor (combustível/peça).
      const glue = small(icons[i]) || small(icons[i - 1]) ? SMALL_ICON_GLUE : 0;
      x += step - glue;
    }
    const kind = icons[i];
    const cx = x + TIRE_SIZE / 2;
    if (kind === "fuel") {
      // Fuel.png é mais "cheio" que o anel do pneu → 2px menor pra bater o peso visual.
      if (assets.fuel) drawIconFit(ctx, assets.fuel, FUEL_SRC, cx, cy, ICON_TARGET - 2);
    } else if (kind === "part") {
      drawPartIcon(ctx, cx, cy);
    } else {
      const img = kind === "wet" ? assets.tireWet : assets.tireDry;
      const key = kind === "wet" ? TIRE_WET_SRC : TIRE_DRY_SRC;
      if (img) drawIconFit(ctx, img, key, cx, cy, ICON_TARGET);
    }
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

  // FLASH de quebra: a linha PISCA (lento) por ~5 s quando o piloto acaba de quebrar (em
  // sincronia com o rádio). A COR segue a severidade: âmbar = leve, vermelho = grave, preto
  // = DNF (bandeira preta). Preenchimento com piso alto pra aparecer sobre cores escuras.
  if (car.flash) {
    const blink = 0.5 + 0.5 * Math.sin(Date.now() / 430); // pulso lento (~2,7 s por ciclo)
    let fillRGB;
    let borderRGB;
    let fillA;
    if (car.flag === "black") {
      // DNF → preto: blackout QUASE OPACO (o texto fica por cima), senão some sobre linha escura.
      fillRGB = "0,0,0";
      borderRGB = "255,255,255"; // borda branca emoldura o preto
      fillA = 0.7 + 0.25 * blink; // 0.70 → 0.95
    } else if (car.alert === "heavy") {
      fillRGB = "248,81,73"; // grave → vermelho
      borderRGB = "255,99,90";
      fillA = 0.32 + 0.3 * blink;
    } else {
      fillRGB = "255,193,48"; // leve → âmbar
      borderRGB = "255,214,64";
      fillA = 0.32 + 0.3 * blink;
    }
    ctx.fillStyle = `rgba(${fillRGB},${fillA.toFixed(3)})`;
    ctx.fillRect(0, y, PANEL_W, ROW_H);
    ctx.strokeStyle = `rgba(${borderRGB},${(0.55 + 0.45 * blink).toFixed(3)})`;
    ctx.lineWidth = 2;
    ctx.strokeRect(1, y + 1, PANEL_W - 2, ROW_H - 2);
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

  // Coluna de paradas: pneu (seco/molhado) / combustível / reparo de peça por parada,
  // com o tempo de pit por cima, se o carro parou há ≤3 voltas.
  drawStopStack(ctx, car, cy, assets);
  if (car.pitSecs != null) drawPitTimeBadge(ctx, STOPS_CENTER, cy, car.pitSecs);

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
  // Divisor GROSSO entre o pódio e a vizinhança do jogador: uma faixa escurecida
  // de fundo (dá peso ao corte) + linha central sólida e forte, com um fio sutil
  // acima/abaixo pra realçar o degrau. Continua "vazando" pros lados (gradiente) pra
  // não virar uma barra dura.
  const band = ctx.createLinearGradient(0, 0, PANEL_W, 0);
  band.addColorStop(0, "rgba(0,0,0,0.34)");
  band.addColorStop(0.5, "rgba(0,0,0,0.42)");
  band.addColorStop(1, "rgba(0,0,0,0.34)");
  ctx.fillStyle = band;
  ctx.fillRect(0, y + 1, PANEL_W, SEP_H - 2);

  // Fios finos de contorno da faixa (topo e base).
  ctx.strokeStyle = "rgba(255,255,255,0.06)";
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(0, y + 1.5);
  ctx.lineTo(PANEL_W, y + 1.5);
  ctx.moveTo(0, y + SEP_H - 1.5);
  ctx.lineTo(PANEL_W, y + SEP_H - 1.5);
  ctx.stroke();

  // Linha central: forte, sólida, esvaindo pras bordas.
  const line = ctx.createLinearGradient(0, 0, PANEL_W, 0);
  line.addColorStop(0, "rgba(255,255,255,0.05)");
  line.addColorStop(0.5, "rgba(255,255,255,0.42)");
  line.addColorStop(1, "rgba(255,255,255,0.05)");
  ctx.strokeStyle = line;
  ctx.lineWidth = 2.5;
  ctx.beginPath();
  ctx.moveTo(PAD, cy);
  ctx.lineTo(PANEL_W - PAD, cy);
  ctx.stroke();
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
