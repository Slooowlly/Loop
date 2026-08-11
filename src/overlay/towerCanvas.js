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
import { rowKey } from "./towerAnimation";
import { DEFAULT_THEME } from "./towerThemes";
import { tireCompoundDryWet } from "./tireCompounds";
import { categoryLogoFile, categoryLogoSrc } from "../utils/categoryLogos";

// Categoria do EVENTO -> nome exibido na faixa da torre. O ARQUIVO do brasão não mora aqui:
// vem do dicionário único de `utils/categoryLogos.js`, o mesmo que o atlas, a pré-temporada,
// a convocação e o calendário usam. Só o nome é próprio da torre, porque ela mostra o rótulo
// curto que cabe na largura do painel ("GR CUP", e não "Toyota Cup Principal").
const CATEGORY_NAME = {
  gt3: "GT3",
  gt4: "GT4",
  lmp2: "LMP2",
  endurance: "ENDURANCE",
  production_challenger: "PRODUCTION",
  production: "PRODUCTION",
  bmw_m2: "BMW M2",
  toyota_amador: "GR CUP",
  mazda_amador: "MX5 CUP",
  toyota_rookie: "GR ROOKIE",
  mazda_rookie: "MX5 ROOKIE",
};

function categoryMeta(id) {
  return {
    name: CATEGORY_NAME[id] || String(id || "").toUpperCase(),
    logo: categoryLogoFile(id),
  };
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
// Painel mais estreito desde que o nome virou "Primeiro S." (shortDriverName): a coluna
// do nome caiu de 126 -> 104px e TODAS as colunas à direita dela vieram 22px pra esquerda.
export const PANEL_W = 430;
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
const NAME_RIGHT = 174;
const DELTA_RIGHT = 210;
const STOPS_CENTER = 234;
const STOPS_LEFT = 214;
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
const FASTEST_RIGHT = 334;
const POINTS_RIGHT = 392;
const POINTS_GAIN_X = 396;
const PIN_X = PANEL_W;
// Folga entre o fim do nome e o emoji de rivalidade (que vem DEPOIS do nome).
const RIVAL_GAP = 4;
// Quanto a coluna do nome cede quando a coluna do meio mostra INTERVALO DE TEMPO em vez
// das setas de posição. Tudo medido nas fontes reais: o intervalo ocupa ~39 px em
// qualquer faixa (é o que a precisão decrescente de `formatQualyGap` garante) contra
// 25 px do "▲ 10", e o nome já encurtado chega a 75,7 px ("Matthew K." no peso do tema)
// mais 20,5 px quando ele tem marcador de rivalidade. Ceder a diferença cheia truncaria
// justamente esse nome; com 6 px o nome ainda cabe e sobram 3 px até o intervalo.
const GAP_COL_EXTRA = 6;

// ─── Modo MINI ────────────────────────────────────────────────────────────────
// A torre tem duas larguras: a COMPLETA (todas as colunas) e a MINI — um painel
// estreito só com pos · logo · nome · delta, pra um HUD discreto. A altura das linhas
// não muda; o que encolhe é a LARGURA e o nº de carros (`MINI_SECTION_OPTS`). O VR
// nunca usa mini — é coisa só do monitor.
export const MINI_PANEL_W = 208;

// Menos carros na mini: top 2 de cada classe + 3 na frente/3 atrás do jogador.
// A vizinhança era de 2 e ficava confusa em corrida: com tão pouco contexto em volta,
// uma ultrapassagem mexia em quase metade das linhas visíveis e não dava pra ler o
// que tinha acontecido. Custa +2 linhas de altura (60 px) — a mini continua estreita,
// que é o que define o modo; o que ela ganha é fôlego vertical.
export const MINI_SECTION_OPTS = { topN: 2, neighbors: 3, maxRows: 6 };

// Métricas de layout por modo. `drawTower` monta uma destas e passa pras funções de
// desenho, que leem SEMPRE do layout (nunca das consts de largura direto).
function layoutFor(compact) {
  if (!compact) {
    return {
      compact: false,
      panelW: PANEL_W,
      posRight: POS_RIGHT,
      logo: true,
      logoX: LOGO_X,
      nameX: NAME_X,
      nameRight: NAME_RIGHT,
      deltaRight: DELTA_RIGHT,
      stops: true,
      best: true,
      points: true,
    };
  }
  const panelW = MINI_PANEL_W;
  return {
    compact: true,
    panelW,
    posRight: 24,
    logo: true,
    logoX: 30,
    nameX: 66, // depois da posição + logo
    nameRight: panelW - 46,
    deltaRight: panelW - 8,
    stops: false,
    best: false,
    points: false,
  };
}

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

// Clareia um hex misturando com branco (amt 0..1). Fallback cinza claro.
function lighten(hex, amt) {
  if (typeof hex !== "string" || !/^#([0-9a-f]{6})$/i.test(hex)) return "#c9d1d9";
  const mix = (c) => Math.round(c + (255 - c) * amt);
  const r = mix(parseInt(hex.slice(1, 3), 16));
  const g = mix(parseInt(hex.slice(3, 5), 16));
  const b = mix(parseInt(hex.slice(5, 7), 16));
  return `rgb(${r},${g},${b})`;
}

// Cor do nome dos pilotos do SEU time = a cor DA EQUIPE (car.color, ex.: vermelho). Só
// clareia se o time for bem escuro, pra não sumir no painel escuro (vermelho segue vermelho).
function teamNameColor(hex) {
  if (typeof hex !== "string" || !/^#([0-9a-f]{6})$/i.test(hex)) return "#f0f6fc";
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  const lum = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
  return lum < 0.18 ? lighten(hex, 0.45) : hex;
}

// Nome do piloto ENCURTADO pra torre: primeiro nome + inicial do segundo, e nada do
// terceiro em diante:
//   "Matthew Koontz"      -> "Matthew K."
//   "Marco I D'Acunto"    -> "Marco I."     (o 3º nome cai fora)
//   "Pawel T. Okreglicki" -> "Pawel T."
//   "Ayrton"              -> "Ayrton"       (nome único fica inteiro)
// Cabe na coluna sem "…", inclusive quando o emoji de rival entra depois do nome.
export function shortDriverName(name) {
  const parts = String(name ?? "")
    .trim()
    .split(/\s+/)
    .filter(Boolean);
  if (parts.length === 0) return "";
  if (parts.length === 1) return parts[0];
  return `${parts[0]} ${parts[1][0].toUpperCase()}.`;
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

// CACHE por src, e obrigatório: `preloadAssets` é chamado a CADA tick de dados
// (2x/s durante a corrida). Sem cache, cada tick criava um `new Image()` por equipe
// do grid — ~30-60 bitmaps decodificados por meio segundo, retidos até o GC dar
// conta. Numa corrida longa isso estoura a memória do WebView2 ("Esta página está
// com problemas · Out of Memory"). Os arquivos são um conjunto FIXO: uma decodifi-
// cação por src basta pra vida do processo. Guarda a Promise (não a imagem) pra
// duas chamadas concorrentes do mesmo src compartilharem o mesmo carregamento.
const _imgCache = new Map();
function loadImage(src) {
  const hit = _imgCache.get(src);
  if (hit) return hit;
  const p = new Promise((resolve) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => {
      _imgCache.delete(src); // falhou: não fixa o erro pra sempre
      resolve(null);
    };
    img.src = src;
  });
  _imgCache.set(src, p);
  return p;
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

// Memo do RESULTADO, além do cache de imagens: o pacote só muda quando o conjunto de
// equipes no grid (ou a categoria) muda — o que acontece uma vez por sessão, não 2x/s.
let _assetsMemo = { key: null, promise: null };

export async function preloadAssets(data) {
  const teams = new Set();
  data.classes.forEach((c) => c.cars.forEach((car) => teams.add(car.team)));

  const memoKey = `${data.session?.category ?? ""}|${[...teams].sort().join("")}`;
  if (_assetsMemo.key === memoKey) return _assetsMemo.promise;

  const promise = buildAssets(data, teams);
  _assetsMemo = { key: memoKey, promise };
  return promise;
}

async function buildAssets(data, teams) {
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

function drawPins(ctx, car, cy, theme, panelW = PIN_X) {
  let left = panelW;
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
function drawRow(ctx, car, y, assets, team, theme, L, col = {}) {
  const isPlayer = Boolean(car.player);
  const mate = isTeammate(car, team);
  const ownTeam = isPlayer || mate; // seus dois carros: nome na cor da equipe
  const cy = y + ROW_H / 2;
  const panelW = L.panelW;

  // Fundo/realce da linha — depende do tema. O realce de você/companheiro NUNCA
  // é uma cor de linha diferente; é sempre a cor da PRÓPRIA equipe (+ nome).
  if (theme.rowStyle === "block") {
    ctx.fillStyle = hexToRgba(car.color, theme.rowAlpha * 0.6);
    ctx.fillRect(0, y, panelW, ROW_H);
  } else {
    const grad = ctx.createLinearGradient(0, y, panelW, y);
    grad.addColorStop(0, hexToRgba(car.color, theme.rowAlpha));
    grad.addColorStop(0.35, hexToRgba(car.color, theme.rowAlpha * 0.35));
    grad.addColorStop(0.7, "rgba(0,0,0,0)");
    ctx.fillStyle = grad;
    ctx.fillRect(0, y, panelW, ROW_H);
    if (theme.sheen) {
      ctx.fillStyle = "rgba(255,255,255,0.05)";
      ctx.fillRect(0, y, panelW, 1);
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
    ctx.fillRect(0, y, panelW, ROW_H);
    ctx.strokeStyle = `rgba(${borderRGB},${(0.55 + 0.45 * blink).toFixed(3)})`;
    ctx.lineWidth = 2;
    ctx.strokeRect(1, y + 1, panelW - 2, ROW_H - 2);
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
    ctx.fillText(formatTowerPosition(car.pos), L.posRight, cy);
  }

  // Logo da equipe ao lado do nome (posição vem do layout).
  if (L.logo) {
    const img = assets.logos.get(car.team);
    if (img) ctx.drawImage(img, L.logoX, y + (ROW_H - LOGO_H) / 2, LOGO_W, LOGO_H);
  }

  // Marcador de rivalidade (💥 nemesis / 🔥 rival): vem DEPOIS do nome.
  // SÓ na torre GRANDE (completa) — a mini é enxuta demais pra emoji.
  ctx.textAlign = "left";
  const nameX = L.nameX;
  const rivalGlyph =
    car.rivalRole === "nemesis" ? "\u{1F4A5}" : car.rivalRole === "rival" ? "\u{1F525}" : null;
  const showRival = Boolean(rivalGlyph) && !L.compact;

  // Reserva a largura do emoji ANTES de cortar o nome, senão ele estouraria a coluna.
  let rivalW = 0;
  if (showRival) {
    ctx.font = `12px ${FONT}`;
    rivalW = ctx.measureText(rivalGlyph).width + RIVAL_GAP;
  }

  // Nome do SEU time (jogador + companheiro) na COR DA EQUIPE (car.color, ex.: vermelho);
  // os demais no texto do tema. Times bem escuros são clareados só o necessário pra ler.
  const nameColor = ownTeam ? teamNameColor(car.color) : theme.text;
  const nameWeight = theme.rowStyle === "block" ? 700 : 600;
  ctx.fillStyle = nameColor;
  ctx.font = `${nameWeight} 14px ${FONT}`;
  // O intervalo de tempo é mais largo que as setas de posição (43 px de "+12.345" contra
  // 25 de "▲ 10"), então na classificação o nome cede a diferença. Sem isso o gap entra
  // por cima da última letra do nome mais comprido.
  const nameLimit = L.nameRight - nameX - rivalW - (col.gap ? GAP_COL_EXTRA : 0);
  const nameText = truncate(ctx, shortDriverName(car.name), nameLimit);
  ctx.fillText(nameText, nameX, cy);

  // Emoji logo APÓS o nome (mede o nome ainda na fonte dele, depois troca pra do emoji).
  if (showRival) {
    const nameW = ctx.measureText(nameText).width;
    ctx.font = `12px ${FONT}`;
    ctx.fillText(rivalGlyph, nameX + nameW + RIVAL_GAP, cy);
  }

  // Coluna do meio. Em CORRIDA são as posições ganhas/perdidas desde a largada (▲/▼).
  // Na CLASSIFICATÓRIA isso não existe — não houve largada, e a coluna ficava com um
  // "— 0" morto na grade inteira. Ali ela mostra o INTERVALO para o melhor tempo da
  // classe, que é a leitura da sessão: quanto falta para a pole.
  ctx.textAlign = "right";
  ctx.font = `600 12px ${FONT}`;
  if (col.gap) {
    const gapStr = formatQualyGap(car.bestMs, col.refMs);
    ctx.fillStyle = gapStr.startsWith("+") ? theme.text : theme.textMuted;
    ctx.fillText(gapStr, L.deltaRight, cy);
  } else if (!car.delta) {
    ctx.fillStyle = theme.textMuted;
    ctx.fillText("— 0", L.deltaRight, cy);
  } else {
    const up = car.delta > 0;
    ctx.fillStyle = up ? theme.up : theme.down;
    ctx.fillText(`${up ? "▲" : "▼"} ${Math.abs(car.delta)}`, L.deltaRight, cy);
  }

  // Coluna de paradas: pneu (seco/molhado) / combustível / reparo de peça por parada,
  // com o tempo de pit por cima, se o carro parou há ≤3 voltas.
  if (L.stops) {
    drawStopStack(ctx, car, cy, assets);
    if (car.pitSecs != null) drawPitTimeBadge(ctx, STOPS_CENTER, cy, car.pitSecs);
  }

  // Melhor volta.
  if (L.best) {
    ctx.fillStyle = car.fol ? theme.purple : theme.text;
    ctx.font = `600 13px ${FONT}`;
    ctx.textAlign = "right";
    ctx.fillText(car.fastest, FASTEST_RIGHT, cy);
  }

  // Pontos + ganho.
  if (L.points) {
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
  }

  drawPins(ctx, car, cy, theme, panelW);
  fgShadow(ctx, false);
}

function drawSeparator(ctx, y, panelW = PANEL_W) {
  const cy = y + SEP_H / 2;
  // Divisor GROSSO entre o pódio e a vizinhança do jogador: uma faixa escurecida
  // de fundo (dá peso ao corte) + linha central sólida e forte, com um fio sutil
  // acima/abaixo pra realçar o degrau. Continua "vazando" pros lados (gradiente) pra
  // não virar uma barra dura.
  const band = ctx.createLinearGradient(0, 0, panelW, 0);
  band.addColorStop(0, "rgba(0,0,0,0.34)");
  band.addColorStop(0.5, "rgba(0,0,0,0.42)");
  band.addColorStop(1, "rgba(0,0,0,0.34)");
  ctx.fillStyle = band;
  ctx.fillRect(0, y + 1, panelW, SEP_H - 2);

  // Fios finos de contorno da faixa (topo e base).
  ctx.strokeStyle = "rgba(255,255,255,0.06)";
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(0, y + 1.5);
  ctx.lineTo(panelW, y + 1.5);
  ctx.moveTo(0, y + SEP_H - 1.5);
  ctx.lineTo(panelW, y + SEP_H - 1.5);
  ctx.stroke();

  // Linha central: forte, sólida, esvaindo pras bordas.
  const line = ctx.createLinearGradient(0, 0, panelW, 0);
  line.addColorStop(0, "rgba(255,255,255,0.05)");
  line.addColorStop(0.5, "rgba(255,255,255,0.42)");
  line.addColorStop(1, "rgba(255,255,255,0.05)");
  ctx.strokeStyle = line;
  ctx.lineWidth = 2.5;
  ctx.beginPath();
  ctx.moveTo(PAD, cy);
  ctx.lineTo(panelW - PAD, cy);
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

function drawClassHeader(ctx, cls, y, theme, panelW = PANEL_W) {
  const midY = y + CLASS_H / 2;

  if (theme.classStyle === "band") {
    // Banda robusta: wash da cor da classe + acento + rótulo + selo com carrinho.
    const g = ctx.createLinearGradient(0, y, panelW, y);
    g.addColorStop(0, hexToRgba(cls.color, 0.5));
    g.addColorStop(0.45, hexToRgba(cls.color, 0.16));
    g.addColorStop(1, "rgba(0,0,0,0.28)");
    ctx.fillStyle = g;
    ctx.fillRect(0, y, panelW, CLASS_H);
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
    const pillX = panelW - PAD - pillW;
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
    ctx.fillRect(0, y, panelW, CLASS_H);
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
    ctx.fillRect(0, y, panelW, CLASS_H);
    ctx.font = `800 12px ${FONT}`;
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    ctx.fillStyle = cls.color;
    ctx.fillText(cls.label, PAD, midY);
    ctx.fillStyle = cls.color;
    ctx.fillRect(0, y + CLASS_H - 2, panelW, 2);
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
  ctx.fillText(String(cls.cars.length), panelW - PAD, midY);
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

// Relógio da sessão: segundos -> "M:SS", ou "H:MM:SS" quando passa da hora (enduro).
// Trunca em vez de arredondar, pra o contador não pular pro minuto seguinte antes da
// hora — 7:59.9 ainda é 7:59.
export function formatSessionClock(secs) {
  if (!Number.isFinite(secs) || secs < 0) return "--:--";
  const total = Math.floor(secs);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

// Contador do canto direito do cabeçalho, escolhido pelo TIPO da sessão.
//
// Na CORRIDA a régua é a volta (LAPS 36/40). Na CLASSIFICATÓRIA e no TREINO a régua é o
// relógio, sempre — em prova de 8 minutos o que decide é quanto ainda sobra, e a volta em
// que o líder está não quer dizer nada. Antes a escolha era feita pela DISPONIBILIDADE do
// dado (`durationS > 0`), então bastava o iRacing não declarar a duração para a quali cair
// na apresentação de voltas da corrida. Agora o tipo manda, e é o dado de tempo que cede:
// relógio completo se houver, senão o tempo restante, senão o placeholder.
export function sessionCounter(session = {}) {
  if (session.type === "R") {
    return {
      label: "LAPS",
      big: String(session.lap ?? 0),
      tail: session.totalLaps > 0 ? `/${session.totalLaps}` : "", // total desconhecido: sem "/0"
    };
  }
  if (session.elapsedS != null && session.durationS > 0) {
    return {
      label: "TIME",
      big: formatSessionClock(session.elapsedS),
      tail: `/${formatSessionClock(session.durationS)}`,
    };
  }
  if (session.remainingS != null && session.remainingS >= 0) {
    return { label: "LEFT", big: formatSessionClock(session.remainingS), tail: "" };
  }
  if (session.durationS > 0) {
    return { label: "TIME", big: "--:--", tail: `/${formatSessionClock(session.durationS)}` };
  }
  return { label: "TIME", big: "--:--", tail: "" };
}

// Intervalo para o melhor tempo da classe — o que a coluna do meio mostra FORA da corrida.
// `bestMs`/`refMs` em milissegundos, 0 = sem volta marcada.
// A precisão cai conforme o intervalo cresce, e não por gosto: a coluna tem largura fixa
// e o milésimo só decide alguma coisa quando a disputa é de décimos. Assim o texto fica
// sempre com ~6 caracteres, seja "+0.243" ou "+1:03.0".
export function formatQualyGap(bestMs, refMs) {
  if (!(bestMs > 0)) return "--"; // ainda não marcou
  if (!(refMs > 0) || bestMs <= refMs) return "—"; // é a referência da classe
  const diff = bestMs - refMs;
  if (diff < 10_000) return `+${(diff / 1000).toFixed(3)}`;
  if (diff < 60_000) return `+${(diff / 1000).toFixed(2)}`;
  const m = Math.floor(diff / 60_000);
  const s = (diff % 60_000) / 1000;
  return `+${m}:${s.toFixed(1).padStart(4, "0")}`;
}

function drawSessionHeader(ctx, session, theme, assets, L = layoutFor(false)) {
  const H = SESSION_H;
  const panelW = L.panelW;
  const compact = L.compact;
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
  roundRect(ctx, 0, 0, panelW, H, [theme.blockRadius, theme.blockRadius, 0, 0]);
  ctx.fillStyle = g;
  ctx.fill();

  fgShadow(ctx, true); // textos do cabeçalho (categoria/sessão/clima/voltas)
  ctx.textBaseline = "middle";

  // ── ESQUERDA: logo da categoria (só na completa) + nome + sessão/bandeira ──
  // Mini: sem logo (falta largura), nome encosta na margem e fonte menor.
  const logoBoxW = compact ? 0 : 60;
  if (!compact && assets.categoryLogo) {
    drawImageFit(ctx, assets.categoryLogo, PAD, 5, logoBoxW, 40, assets.categoryLogoTrim);
  }
  const nameX = compact ? PAD : PAD + logoBoxW + 12;

  ctx.textAlign = "left";
  ctx.fillStyle = theme.text;
  ctx.font = `800 ${compact ? 15 : 17}px ${FONT}`;
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

  // ── DIREITA: clima (cima, só na completa) + voltas (baixo) ──
  const rx = panelW - PAD;
  const w = session.weather || {};

  // Clima: [ícone] AR° (só o ar). Cortado na mini pra caber — o arco de chuva embaixo
  // da torre já cobre a previsão quando importa.
  if (!compact) {
    ctx.textAlign = "right";
    ctx.font = `800 14px ${FONT}`;
    ctx.fillStyle = theme.text;
    const airStr = w.airTemp != null ? `${w.airTemp}°` : "--";
    ctx.fillText(airStr, rx, cy1);
    const airW = ctx.measureText(airStr).width;
    drawWeatherIcon(ctx, rx - airW - 14, cy1, w.condition);
  }

  // Contador do canto direito: voltas na corrida, relógio na classificação (ver
  // `sessionCounter`).
  const { label, big: bigStr, tail: tailStr } = sessionCounter(session);

  ctx.textAlign = "right";
  ctx.font = `700 12px ${FONT}`;
  ctx.fillStyle = theme.textMuted;
  ctx.fillText(tailStr, rx, cy2);
  const tailW = ctx.measureText(tailStr).width;

  ctx.font = `800 18px ${FONT}`;
  ctx.fillStyle = theme.text;
  ctx.fillText(bigStr, rx - tailW - 2, cy2);
  const bigW = ctx.measureText(bigStr).width;

  ctx.font = `700 8px ${FONT}`;
  ctx.fillStyle = theme.textMuted;
  ctx.fillText(label, rx - tailW - bigW - 8, cy2);

  // Dois rótulos que ajudam: "pits" sobre a coluna de pneus e "best" sobre a
  // melhor volta — no chrome do header, acima da linha de acento. Só na completa
  // (a mini não tem essas colunas).
  if (!compact) {
    ctx.font = `700 10px ${FONT}`;
    ctx.fillStyle = theme.textMuted;
    ctx.textAlign = "center";
    ctx.fillText("PITS", STOPS_CENTER, H - 10);
    ctx.fillText("BEST", 303, H - 10); // acompanha FASTEST_RIGHT (334)
    // Fora da corrida a coluna do meio vira intervalo de tempo, e aí ela ganha rótulo:
    // na corrida as setas de posição se explicam sozinhas, "+0.243" não.
    if (session.type !== "R") {
      ctx.textAlign = "right";
      ctx.fillText("GAP", DELTA_RIGHT, H - 10);
    }
  }

  fgShadow(ctx, false);

  // Acento inferior na cor do ESTADO (verde/amarelo/quadriculada/roxo-quali),
  // esvaindo pra direita — é o sinal de "a bandeira mudou".
  const line = ctx.createLinearGradient(0, 0, panelW, 0);
  line.addColorStop(0, stateColor);
  line.addColorStop(0.65, hexToRgba(stateColor, 0));
  ctx.fillStyle = line;
  ctx.fillRect(0, H - 2, panelW, 2);
}

// Percorre as seções UMA vez e devolve tudo que o desenho precisa: os blocos de
// classe (fundo + cabeçalho) e as linhas já com o y final. Existir separado do
// desenho é o que permite a animação — dá pra comparar o layout de agora com o de
// antes sem tocar num `ctx` —, e mantém uma única conta de y (antes o cálculo de
// altura e o laço de desenho repetiam a mesma soma, livres pra divergir).
/** Melhor tempo (ms) de uma classe. 0 = ninguém marcou volta ainda. */
function classBestMs(cls) {
  return (cls?.cars ?? []).reduce(
    (best, c) => (c.bestMs > 0 && (best === 0 || c.bestMs < best) ? c.bestMs : best),
    0,
  );
}

export function towerLayout(sections) {
  const blocks = [];
  const items = [];
  let y = SESSION_H; // o colhead vive DENTRO do header
  sections.forEach(({ cls, rows }, i) => {
    if (i > 0) y += CLASS_GAP;
    // Referência de tempo da classe (a "pole"): sai da classe INTEIRA, não das linhas
    // visíveis — com a janela do jogador aberta o líder pode nem estar na tela, e o
    // intervalo continua tendo de ser contado a partir dele.
    const refMs = classBestMs(cls);
    const bodyH = rows.reduce((a, r) => a + (r.kind === "separator" ? SEP_H : ROW_H), 0);
    blocks.push({ cls, y, blockH: CLASS_H + bodyH, isLast: i === sections.length - 1 });
    y += CLASS_H;
    // Limites do CORPO da classe: uma linha em trânsito é recortada neles, pra não
    // pintar por cima do cabeçalho da classe nem vazar pro bloco vizinho.
    const bodyTop = y;
    const bodyBottom = y + bodyH;
    rows.forEach((row) => {
      if (row.kind === "separator") {
        items.push({ kind: "separator", y, bodyTop, bodyBottom });
        y += SEP_H;
      } else {
        items.push({
          kind: "car",
          car: row.car,
          key: rowKey(row.car),
          y,
          bodyTop,
          bodyBottom,
          refMs,
        });
        y += ROW_H;
      }
    });
  });
  return { blocks, items, total: y };
}

function sectionsHeight(sections) {
  return towerLayout(sections).total;
}

// Altura (px lógicos = CSS) que a torre ocupa pros dados atuais — usada pra dizer
// ao vigia de cursor onde é "em cima da torre" (área de hover).
export function towerContentHeight(data, sectionOpts) {
  if (!data) return 0;
  return sectionsHeight(buildTowerSections(data, sectionOpts));
}

// ─── Torre ───────────────────────────────────────────────────────────────────
// `opts`: { compact?: bool, sections?: opts de buildTowerSections }. O monitor passa
// { compact:true, sections: MINI_SECTION_OPTS } no modo mini; VR e preview usam o padrão.
export function drawTower(ctx, data, assets, theme = DEFAULT_THEME, opts = {}) {
  const L = layoutFor(Boolean(opts.compact));
  const panelW = L.panelW;

  // Desenha em coordenadas LÓGICAS; o supersample vira pixels de verdade.
  ctx.setTransform(SUPERSAMPLE, 0, 0, SUPERSAMPLE, 0, 0);
  ctx.clearRect(0, 0, LOGICAL_W, LOGICAL_H);

  const team = playerTeam(data);
  const sections = buildTowerSections(data, opts.sections);

  // Classe única (MX5 Cup, GR Rookie, etc.): o iRacing não manda nome de classe,
  // então o `label` vem vazio e a banda fica só com o contador, sem "GT3". Cai pro
  // nome da categoria do evento (o mesmo que aparece no topo). Copia a `cls` pra
  // não mexer no dado ao vivo.
  const fallbackLabel = categoryMeta(data.session?.category).name;
  sections.forEach((s) => {
    if (!s.cls.label && fallbackLabel) s.cls = { ...s.cls, label: fallbackLabel };
  });

  const { blocks, items, total } = towerLayout(sections);

  // Animação de posição (opcional). Sem `opts.anim` a torre desenha como sempre —
  // é o caminho do VR e dos testes, que não têm relógio.
  const anim = opts.anim ?? null;
  const now = opts.now ?? 0;
  if (anim) {
    anim.sync(
      new Map(items.filter((it) => it.kind === "car").map((it) => [it.key, it.y])),
      now,
    );
  }

  // Faixa da sessão (topo do painel).
  drawSessionHeader(ctx, data.session, theme, assets, L);

  // A TABELA começa logo abaixo do header. O cabeçalho de colunas fica ANEXADO
  // ao topo do 1º bloco (mesmo fundo), então não flutua solto no meio.
  blocks.forEach(({ cls, y, blockH, isLast }) => {
    // Fundo do bloco (classe + corpo), cantos do tema.
    const br = theme.blockRadius;
    roundRect(ctx, 0, y, panelW, blockH, [0, 0, isLast ? br : 0, isLast ? br : 0]);
    ctx.fillStyle = theme.panelBg;
    ctx.fill();
    drawClassHeader(ctx, cls, y, theme, panelW);
  });

  // Linhas paradas primeiro; as EM TRÂNSITO por último, pra quem está passando ficar
  // por cima de quem é passado (a faixa do jogador é opaca — desenhada antes, ela
  // levaria o texto do ultrapassado por cima no meio do cruzamento).
  // Fora da CORRIDA a coluna do meio troca de assunto: intervalo de tempo no lugar das
  // posições ganhas/perdidas (ver `drawRow`). Sem tipo de sessão, o padrão é corrida.
  const porTempo = Boolean(data.session?.type) && data.session.type !== "R";

  const moving = [];
  items.forEach((item) => {
    if (item.kind === "separator") {
      drawSeparator(ctx, item.y, panelW);
      return;
    }
    const st = anim ? anim.state(item.key, now) : null;
    if (st && (st.alpha < 1 || Math.abs(st.y - item.y) > 0.01)) {
      moving.push({ item, st });
      return;
    }
    drawRow(ctx, item.car, item.y, assets, team, theme, L, {
      gap: porTempo,
      refMs: item.refMs,
    });
  });

  moving.forEach(({ item, st }) => {
    ctx.save();
    // Recorte só na VERTICAL (largura cheia): os pins de alerta/penalidade moram fora
    // do painel, à direita, e um clip em `panelW` os decapitaria durante o deslize.
    ctx.beginPath();
    ctx.rect(0, item.bodyTop, LOGICAL_W, item.bodyBottom - item.bodyTop);
    ctx.clip();
    ctx.globalAlpha = st.alpha;
    drawRow(ctx, item.car, st.y, assets, team, theme, L, { gap: porTempo, refMs: item.refMs });
    ctx.restore();
  });

  return total;
}
