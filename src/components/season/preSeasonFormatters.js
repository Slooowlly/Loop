// Tabelas de constantes e helpers puros de rótulo, cor e formatação da pré-temporada.
// Extraídos de PreSeasonView.jsx — sem JSX, hook ou estado.

import { ordinal } from "../../i18n/format.js";
import i18n from "../../i18n/index.js";
import { formatMoneyCompact } from "../../utils/formatters";
import { CATEGORY_LOGOS_RECORTADOS } from "../../utils/categoryLogos";

// ─── Category Definitions ─────────────────────────────────────────────────────

const CATEGORIES = [
  // A entrada "sem recorte" não tem `label`: os rótulos das outras são nomes próprios
  // (Mazda, GT3, Endurance), que não mudam de idioma, e este era a única palavra em
  // português da tabela — duplicada, porque os dois cabeçalhos que desenham a régua já
  // pediam `preSeason.filters.all` ao i18n para este id. Sem o campo, quem esquecer o
  // caso especial mostra vazio em vez de mostrar português dentro do inglês.
  { id: "all",        color: "rgba(255,255,255,0.35)" },
  { id: "mazda",      dbIds: ["mazda_rookie", "mazda_amador"],   label: "Mazda",      color: "#F01010" },
  { id: "toyota",     dbIds: ["toyota_rookie", "toyota_amador"], label: "Toyota",     color: "#FF1010" },
  { id: "bmw",        dbIds: ["bmw_m2"],                         label: "BMW",        color: "#F00010" },
  { id: "production_challenger", dbIds: ["production_challenger"], label: "Production", color: "#a855f7" },
  { id: "sep1", isSeparator: true },
  { id: "gt4",       dbIds: ["gt4"],       label: "GT4",       color: "#3080FF" },
  { id: "gt3",       dbIds: ["gt3"],       label: "GT3",       color: "#00FFFF" },
  { id: "endurance", dbIds: ["endurance"], label: "Endurance", color: "#43c948" },
];

const SUBCAT_LABELS = {
  mazda: "Mazda Cup Principal",
  toyota: "Toyota Cup Principal",
  bmw: "BMW Cup Principal",
  mazda_amador: "Mazda Cup",
  mazda_rookie: "Mazda Rookie",
  toyota_amador: "Toyota Cup",
  toyota_rookie: "Toyota Rookie",
  bmw_m2: "BMW",
  production_challenger: "Production",
  gt3: "GT3 Championship",
  gt4: "GT4 Championship",
  lmp2: "LMP2 Prototype Championship",
  endurance: "Endurance Championship",
};

// Brasão da subcategoria: o dicionário é o mesmo de todas as telas
// (`utils/categoryLogos.js`), na variante recortada.
const SUBCAT_LOGOS = CATEGORY_LOGOS_RECORTADOS;

const DEFAULT_LOGO_FIT = {
  frameClassName: "h-40 lg:h-44",
  imageStyle: {},
};

const SUBCAT_LOGO_FITS = {
  toyota: {
    frameClassName: "h-40 lg:h-44",
    imageStyle: { transform: "translateX(0.75%)", transformOrigin: "top center" },
  },
  toyota_amador: {
    frameClassName: "h-40 lg:h-44",
    imageStyle: { transform: "translateX(0.75%)", transformOrigin: "top center" },
  },
  mazda: {
    frameClassName: "h-36 lg:h-40",
    imageStyle: { transform: "translateX(-1.5%) scale(1.38)", transformOrigin: "top center" },
  },
  mazda_amador: {
    frameClassName: "h-36 lg:h-40",
    imageStyle: { transform: "translateX(-1.5%) scale(1.38)", transformOrigin: "top center" },
  },
  mazda_rookie: {
    frameClassName: "h-40 lg:h-44",
    imageStyle: { transform: "translateX(-2%) scale(1.18)", transformOrigin: "top center" },
  },
  toyota_rookie: {
    frameClassName: "h-40 lg:h-44",
    imageStyle: { transform: "translateY(-2%) scale(1.24)", transformOrigin: "top center" },
  },
  bmw: {
    frameClassName: "h-40 lg:h-44",
    imageStyle: { transform: "translateX(-3%) scale(1.1)", transformOrigin: "top center" },
  },
  bmw_m2: {
    frameClassName: "h-40 lg:h-44",
    imageStyle: { transform: "translateX(-3%) scale(1.1)", transformOrigin: "top center" },
  },
  gt4: {
    frameClassName: "h-36 lg:h-40",
    imageStyle: { transform: "translateX(2%) scale(1.52)", transformOrigin: "top center" },
  },
  gt3: {
    frameClassName: "h-36 lg:h-40",
    imageStyle: { transform: "translateX(-1%) scale(1.5)", transformOrigin: "top center" },
  },
  production_challenger: {
    frameClassName: "h-36 lg:h-40",
    imageStyle: {},
  },
  endurance: {
    frameClassName: "h-36 lg:h-40",
    imageStyle: {},
  },
};

const SUBCAT_COLORS = {
  mazda: "#F01010",
  mazda_rookie: "#FFE000",
  mazda_amador: "#F01010",
  toyota: "#FF1010",
  toyota_rookie: "#FFE000",
  toyota_amador: "#FF1010",
  bmw: "#F00010",
  bmw_m2: "#F00010",
  production_challenger: "#a855f7",
  gt4: "#3080FF",
  gt3: "#00FFFF",
  lmp2: "#F2CC60",
  endurance: "#43c948",
};

// Rótulo curto da CLASSE dentro de categorias multiclasse (Production/Endurance).
const CLASS_LABELS = {
  mazda: "Mazda",
  toyota: "Toyota",
  bmw: "BMW",
  gt3: "GT3",
  gt4: "GT4",
  lmp2: "LMP2",
};

// Ordem usada no grid central quando todas as categorias estao visiveis.
const CLASS_PRIORITY = [
  "endurance",
  "gt3",
  "gt4",
  "production_challenger",
  "bmw_m2", "bmw",
  "toyota_amador", "toyota",
  "mazda_amador", "mazda",
  "toyota_rookie",
  "mazda_rookie",
];

// Ordem das CLASSES (carros) dentro de cada categoria multiclasse.
const MULTICLASS_ORDER = {
  production_challenger: ["bmw", "toyota", "mazda"],
  endurance: ["lmp2", "gt3", "gt4"],
};

// Tom dos divisores de sub-classe DENTRO de uma categoria multiclasse (só o MENU,
// não as equipes): Production em tons de roxo, Endurance em tons de verde — pra o
// divisor puxar a cor da categoria-pai, não a cor original da classe (BMW=vermelho etc.).
const MULTICLASS_SUBCLASS_TONES = {
  production_challenger: { bmw: "#c084fc", toyota: "#a855f7", mazda: "#7c3aed" },
  endurance: { lmp2: "#7ee787", gt3: "#43c948", gt4: "#22a94e" },
};

// Ordem do painel "Mercado de Pilotos": maior categoria primeiro,
// dentro de cada marca Cup > Amador > Rookie
const FREE_AGENT_ORDER = [
  "endurance",
  "gt3",
  "gt4",
  "production_challenger",
  "bmw_m2", "bmw",
  "toyota", "toyota_amador", "toyota_rookie",
  "mazda", "mazda_amador", "mazda_rookie",
];

// Faixas de nível do "Mercado de Pilotos": agrupamos os pilotos livres pelo TIER da
// categoria onde correm hoje (market_tier vindo do backend, 0=Rookie … 6=Endurance),
// não pela carteira nem pelo time anterior. Ordenadas do mais prestigioso pro menos.
// `minTier` casa o primeiro cujo tier do piloto é >= minTier (lista já em ordem desc).
const LEVEL_BANDS = [
  { key: "elite",    label: "Elite",     color: "#43c948", minTier: 5 }, // endurance/lmp2
  { key: "master",   label: "Master",    color: "#00FFFF", minTier: 4 }, // gt3
  { key: "superpro", label: "Super Pro", color: "#3080FF", minTier: 3 }, // gt4
  { key: "pro",      label: "Pro",       color: "#a855f7", minTier: 2 }, // bmw m2 / production
  { key: "amador",   label: "Amador",    color: "#FF3B3B", minTier: 1 }, // amador
  { key: "rookie",   label: "Rookie",    color: "#FFE000", minTier: 0 }, // rookie
];

function bandForTier(tier) {
  const t = typeof tier === "number" ? tier : 0;
  return LEVEL_BANDS.find((b) => t >= b.minTier) ?? LEVEL_BANDS[LEVEL_BANDS.length - 1];
}

// Espelha os tiers do backend (constants/categories.rs) — usado só para achar a banda
// do JOGADOR (que não vem com market_tier, ao contrário dos agentes livres).
const MARKET_TIER_BY_CATEGORY = {
  mazda_rookie: 0, toyota_rookie: 0,
  mazda_amador: 1, toyota_amador: 1,
  bmw_m2: 2, production_challenger: 2,
  gt4: 3, gt3: 4, lmp2: 5, endurance: 6,
};

const REGULAR_MARKET_CATEGORY_IDS = new Set([
  "mazda_rookie",
  "mazda_amador",
  "toyota_rookie",
  "toyota_amador",
  "bmw_m2",
  "production_challenger",
  "gt4",
  "gt3",
  "endurance",
]);

const WEEKLY_CLOSING_EVENT_TYPES = new Set([
  "ContractExpired",
  "PlayerProposalReceived",
  "TransferCompleted",
  "RookieSigned",
]);

const CATEGORY_TIER = {
  mazda_rookie: 1,
  toyota_rookie: 1,
  mazda_amador: 2,
  toyota_amador: 2,
  bmw_m2: 2,
  production_challenger: 3,
  gt4: 4,
  gt3: 5,
  lmp2: 6,
  endurance: 7,
};

const WEEKLY_MARKET_MOVEMENT_BADGES = {
  rookie: {
    label: "Estreia",
    symbol: "\u2605",
    color: "#58a6ff",
    bg: "rgba(88,166,255,0.15)",
    border: "rgba(88,166,255,0.42)",
  },
  lateral: {
    label: "Troca lateral",
    symbol: "\u2192",
    color: "#d0d7de",
    bg: "rgba(208,215,222,0.11)",
    border: "rgba(208,215,222,0.32)",
  },
  renewal: {
    label: "Renova\u00e7\u00e3o",
    symbol: "\u21bb",
    color: "#7ee787",
    bg: "rgba(126,231,135,0.12)",
    border: "rgba(126,231,135,0.34)",
  },
  signing: {
    label: "Contratação",
    symbol: "+",
    color: "#79c0ff",
    bg: "rgba(121,192,255,0.13)",
    border: "rgba(121,192,255,0.36)",
  },
  proposal: {
    label: "Proposta recebida",
    symbol: "!",
    color: "#f2cc60",
    bg: "rgba(242,204,96,0.13)",
    border: "rgba(242,204,96,0.36)",
  },
  departure: {
    label: "Saiu da equipe",
    symbol: "\u00d7",
    color: "#f2cc60",
    bg: "rgba(242,204,96,0.13)",
    border: "rgba(242,204,96,0.36)",
  },
  // Aposentadoria: sai do grid e n\u00e3o volta, ent\u00e3o tem selo pr\u00f3prio em vez de virar
  // "saiu da equipe" \u2014 quem se aposenta n\u00e3o est\u00e1 no mercado da semana seguinte.
  retirement: {
    label: "Aposentadoria",
    symbol: "\u2691",
    color: "#a9b1ba",
    bg: "rgba(169,177,186,0.11)",
    border: "rgba(169,177,186,0.32)",
  },
  promotion: {
    label: "Promoção",
    symbol: "\u2191",
    color: "#3fb950",
    bg: "rgba(63,185,80,0.13)",
    border: "rgba(63,185,80,0.36)",
  },
  relegation: {
    label: "Rebaixamento",
    symbol: "\u2193",
    color: "#f85149",
    bg: "rgba(248,81,73,0.13)",
    border: "rgba(248,81,73,0.36)",
  },
};

// Ênfase do feed por VÍNCULO do piloto com o jogador (vem de event.relation, marcado
// no backend). O feed mostra TODOS os eventos; estes só ganham realce. `strong` =
// destaque forte (raros, significativos: rival/favorito); sem strong = realce leve
// (já-correu, comum). Prioridade já resolvida no backend (favorite > rival > raced).
const RELATION_EMPHASIS = {
  favorite: {
    label: "Favorito",
    symbol: "★",
    color: "#fbbf24",
    bg: "rgba(251,191,36,0.14)",
    border: "rgba(251,191,36,0.45)",
    strong: true,
  },
  rival: {
    label: "Rival",
    symbol: "⚔",
    color: "#f87171",
    bg: "rgba(248,113,113,0.14)",
    border: "rgba(248,113,113,0.45)",
    strong: true,
  },
  raced: {
    label: "Já correu com você",
    symbol: "•",
    color: "#94a3b8",
    bg: "rgba(148,163,184,0.12)",
    border: "rgba(148,163,184,0.3)",
    strong: false,
  },
};

// ─── Helpers ──────────────────────────────────────────────────────────────────

function getMovementBadge(categoriaAnterior, categoriaAtual) {
  if (!categoriaAnterior || categoriaAnterior === categoriaAtual) return null;

  const from = CATEGORY_TIER[categoriaAnterior] ?? 0;
  const to   = CATEGORY_TIER[categoriaAtual]    ?? 0;

  if (to > from) return { label: "Promovida", arrow: "↑", color: "#3fb950", bg: "rgba(63,185,80,0.12)", border: "rgba(63,185,80,0.35)" };
  if (to < from) return { label: "Rebaixada",  arrow: "↓", color: "#f85149", bg: "rgba(248,81,73,0.12)", border: "rgba(248,81,73,0.35)" };
  return null;
}

function getRankStyle(pos) {
  if (pos === 1) return { border: "#ffd700", glow: "rgba(255, 215, 0, 0.24)" };
  if (pos === 2) return { border: "#c0c0c0", glow: "rgba(192, 192, 192, 0.2)" };
  if (pos === 3) return { border: "#cd7f32", glow: "rgba(205, 127, 50, 0.2)" };
  return null;
}

// Nome curto de uma categoria multiclasse (pra compor "Production · Mazda").
function shortCatName(cat) {
  if (cat === "production_challenger") return "Production";
  if (cat === "endurance") return "Endurance";
  return SUBCAT_LABELS[cat] ?? cat;
}

// Chaves podem ser compostas "categoria:classe" (Production/Endurance) nas ofertas.
function subcatLabel(key) {
  if (typeof key === "string" && key.includes(":")) {
    const [cat, cls] = key.split(":");
    return `${shortCatName(cat)} · ${CLASS_LABELS[cls] ?? cls.toUpperCase()}`;
  }
  return SUBCAT_LABELS[key] ?? key;
}

function subcatColor(key) {
  const base = typeof key === "string" && key.includes(":") ? key.split(":")[0] : key;
  return SUBCAT_COLORS[base] ?? "#58a6ff";
}

// Rótulo CURTO da categoria-destino (etiqueta à direita do piloto livre): tira o
// "Championship"/"Prototype" do label longo. Ex.: gt3 → "GT3", endurance → "Endurance".
function shortDestLabel(key) {
  return (SUBCAT_LABELS[key] ?? key)
    .replace(" Championship", "")
    .replace(" Prototype", "")
    .replace(" Principal", "");
}

function subcatLogo(key) {
  if (typeof key === "string" && key.includes(":")) return null;
  return SUBCAT_LOGOS[key] ?? null;
}

// ─── Faixas de texto (Set A) para os atributos da equipe nas ofertas ──────────
// Rótulos vêm do i18n (preSeason.offers.card.stats.* / .tierScales.*); aqui fica a paleta.
const TIER_COLORS      = ["#f85149", "#f0a45a", "#e3c15a", "#7ee787", "#3fb950"];

// Cores do selo de vínculo piloto-equipe (6 níveis: Recém-chegado → Casa).
const BOND_LEVEL_COLORS = ["#8b949e", "#58a6ff", "#56d4dd", "#7ee787", "#e3c15a", "#f0a45a"];

const PIP_COUNT = 6;
function tierBucket(value) {
  const v = Math.max(0, Math.min(100, value ?? 0));
  return Math.min(4, Math.floor(v / 20));
}
// Quantos pips acender (0..6) proporcional ao valor 0-100.
function pipsFilled(value) {
  const v = Math.max(0, Math.min(100, value ?? 0));
  return Math.min(PIP_COUNT, Math.ceil((v / 100) * PIP_COUNT));
}
// Cor da posição no campeonato (pódio dourado/prata/bronze).
function championshipColor(pos) {
  if (pos === 1) return "#ffd700";
  if (pos === 2) return "#c0c0c0";
  if (pos === 3) return "#cd7f32";
  return "var(--text-primary)";
}
function tierColor(value) {
  return TIER_COLORS[tierBucket(value)];
}

function isRookieCategory(cat) {
  return typeof cat === "string" && cat.endsWith("_rookie");
}

// Marca da categoria — usada pra agrupar/ordenar as ofertas pela marca do jogador.
function brandOf(cat) {
  if (!cat) return null;
  if (cat.startsWith("mazda")) return "mazda";
  if (cat.startsWith("toyota")) return "toyota";
  if (cat.startsWith("bmw")) return "bmw";
  return cat;
}

// Tempo do companheiro na equipe (temporadas consecutivas).
function formatTeammateTenure(tenure) {
  if (tenure == null || tenure <= 0) return i18n.t("preSeason.offers.teammate.newcomer");
  return i18n.t("preSeason.offers.teammate.tenure", { pos: ordinal(tenure) });
}

// Rótulo de fama (0–100) na mesma régua de 6 níveis da ficha do piloto
// (fama_level_for_value no backend) — mantém a leitura de estrelato consistente.
function famaTierLabel(fama) {
  const value = Number(fama ?? 0);
  if (value <= 15) return i18n.t("preSeason.offers.fama.anonymous");
  if (value <= 30) return i18n.t("preSeason.offers.fama.discreet");
  if (value <= 50) return i18n.t("preSeason.offers.fama.known");
  if (value <= 70) return i18n.t("preSeason.offers.fama.strong");
  if (value <= 87) return i18n.t("preSeason.offers.fama.star");
  return i18n.t("preSeason.offers.fama.idol");
}

// Caixa real compacto da equipe: $450 mil / $1,2 mi.
function formatCashCompact(value) {
  if (value == null) return "—";
  return formatMoneyCompact(value);
}

function subcatLogoFit(key) {
  return SUBCAT_LOGO_FITS[key] ?? DEFAULT_LOGO_FIT;
}

function is_regular_market_category(category) {
  return REGULAR_MARKET_CATEGORY_IDS.has(category);
}

// ─── Mapeamento categoria do jogador → filtro inicial ─────────────────────────

function playerCatToFilter(categoria) {
  if (!categoria) return "all";
  if (categoria === "mazda_rookie" || categoria === "mazda_amador") return "mazda";
  if (categoria === "toyota_rookie" || categoria === "toyota_amador") return "toyota";
  if (categoria === "bmw_m2") return "bmw";
  if (categoria === "production_challenger") return "production_challenger";
  if (categoria === "gt4") return "gt4";
  if (categoria === "gt3") return "gt3";
  if (categoria === "endurance") return "endurance";
  return "all";
}

// ─── GridSlot ─────────────────────────────────────────────────────────────────

function formatTenureBadge(tenureSeasons) {
  if (!tenureSeasons || tenureSeasons <= 0) return null;
  if (tenureSeasons === 1) return { label: "Novo", color: "#58a6ff", bg: "rgba(88,166,255,0.12)" };
  return {
    label: `${tenureSeasons}ª temp.`,
    color: "#f2cc60",
    bg: "rgba(242,204,96,0.12)",
  };
}

function formatTenureCounter(tenureSeasons) {
  if (!tenureSeasons || tenureSeasons <= 0) return null;
  return {
    label: tenureSeasons === 1
      ? i18n.t("preSeason.roster.tenureNew")
      : i18n.t("preSeason.roster.tenureYears", { count: tenureSeasons }),
    isNewcomer: tenureSeasons === 1,
  };
}

function getTeamMovementBadge(categoriaAnterior, categoriaAtual) {
  const movement = getMovementBadge(categoriaAnterior, categoriaAtual);
  if (!movement) return null;

  if (movement.color === "#3fb950") {
    return { ...movement, kind: "promoted" };
  }

  if (movement.color === "#f85149") {
    return { ...movement, kind: "relegated" };
  }

  return movement;
}

function getTeamMovementOrder(team) {
  const movement = getTeamMovementBadge(team.categoria_anterior, team._categoria || team.classe);
  if (!movement) return 0;
  if (movement.kind === "promoted") return 1;
  if (movement.kind === "relegated") return 2;
  return 0;
}

function getTeamMappingSortValue(team) {
  return team.temp_posicao && team.temp_posicao > 0 ? team.temp_posicao : 999;
}

function count_team_vacancies(team) {
  let total = 0;
  if (!team.piloto_1_nome) total += 1;
  if (!team.piloto_2_nome) total += 1;
  return total;
}

function formatSafeLastChampionshipResult(driver) {
  if (!driver?.last_championship_position || !driver?.last_championship_total_drivers) {
    return null;
  }
  return `${driver.last_championship_position}º/${driver.last_championship_total_drivers}`;
}

function formatSafeWeeklyClosingPosition(position) {
  if (!position) return "--";
  return `${position}º`;
}

function formatLastChampionshipResult(driver) {
  if (!driver?.last_championship_position || !driver?.last_championship_total_drivers) {
    return null;
  }
  return `${ordinal(driver.last_championship_position)}/${driver.last_championship_total_drivers}`;
}

function formatWeeklyClosingPosition(position) {
  if (!position) return "--";
  return ordinal(position);
}

function isRealCareerDebutCategory(category) {
  return category === "mazda_rookie" || category === "toyota_rookie";
}

function inferWeeklyMovementKind(event) {
  if (event.movement_kind && WEEKLY_MARKET_MOVEMENT_BADGES[event.movement_kind]) {
    return event.movement_kind;
  }

  if (event.event_type === "RookieSigned") {
    return isRealCareerDebutCategory(event.categoria) ? "rookie" : "signing";
  }
  if (event.event_type === "PlayerProposalReceived") return "proposal";
  if (event.event_type === "ContractExpired") return "departure";
  if (event.event_type !== "TransferCompleted") return null;

  const destTeam = event.to_team || event.team_name;
  if (event.from_team && destTeam && event.from_team === destTeam) return "renewal";

  const from = CATEGORY_TIER[event.from_categoria] ?? 0;
  const to = CATEGORY_TIER[event.categoria] ?? 0;
  if (!from || !to) return "signing";
  if (from === to) return "lateral";
  return to > from ? "promotion" : "relegation";
}

function buildWeeklyClosingGroups(weekResult) {
  const grouped = {};

  (weekResult?.events ?? []).forEach((event) => {
    if (!WEEKLY_CLOSING_EVENT_TYPES.has(event.event_type)) return;
    if (!event.driver_name) return;
    const movementKind = inferWeeklyMovementKind(event);
    if (!movementKind) return;
    const category = event.categoria || "outras";
    if (!is_regular_market_category(category)) return;
    grouped[category] = grouped[category] ?? [];
    grouped[category].push({ ...event, movement_kind: movementKind });
  });

  return Object.entries(grouped)
    .sort(([a], [b]) => {
      const pa = FREE_AGENT_ORDER.indexOf(a);
      const pb = FREE_AGENT_ORDER.indexOf(b);
      if (pa !== -1 && pb !== -1) return pa - pb;
      if (pa !== -1) return -1;
      if (pb !== -1) return 1;
      return a.localeCompare(b);
    })
    .map(([category, events]) => ({
      category,
      color: subcatColor(category),
      label: subcatLabel(category),
      events: [...events].sort((a, b) => {
        const posA = a.championship_position ?? 999;
        const posB = b.championship_position ?? 999;
        if (posA !== posB) return posA - posB;
        return (a.driver_name ?? "").localeCompare(b.driver_name ?? "");
      }),
    }));
}

// ─── Licenças ────────────────────────────────────────────────────────────────

const LICENSE_COLORS = {
  R:  { text: "#9ba3ae", bg: "rgba(155,163,174,0.12)" },
  A:  { text: "#3fb950", bg: "rgba(63,185,80,0.12)"   },
  P:  { text: "#58a6ff", bg: "rgba(88,166,255,0.12)"  },
  SP: { text: "#FF8000", bg: "rgba(255,128,0,0.12)"   },
  E:  { text: "#bc8cff", bg: "rgba(188,140,255,0.12)" },
  SE: { text: "#ffd700", bg: "rgba(255,215,0,0.12)"   },
};

const LICENSE_LABEL_KEYS = {
  R: "rookie",
  A: "amateur",
  P: "pro",
  SP: "superPro",
  E: "elite",
  SE: "superElite",
};

function licenseTooltip(sigla) {
  const key = LICENSE_LABEL_KEYS[sigla];
  const label = key ? i18n.t(`preSeason.license.labels.${key}`) : i18n.t("preSeason.license.none");
  return i18n.t("preSeason.license.tooltip", { label });
}

export {
  CATEGORIES,
  SUBCAT_LABELS,
  SUBCAT_LOGOS,
  DEFAULT_LOGO_FIT,
  SUBCAT_LOGO_FITS,
  SUBCAT_COLORS,
  CLASS_LABELS,
  CLASS_PRIORITY,
  MULTICLASS_ORDER,
  MULTICLASS_SUBCLASS_TONES,
  FREE_AGENT_ORDER,
  LEVEL_BANDS,
  bandForTier,
  MARKET_TIER_BY_CATEGORY,
  REGULAR_MARKET_CATEGORY_IDS,
  WEEKLY_CLOSING_EVENT_TYPES,
  CATEGORY_TIER,
  WEEKLY_MARKET_MOVEMENT_BADGES,
  RELATION_EMPHASIS,
  getMovementBadge,
  getRankStyle,
  shortCatName,
  subcatLabel,
  subcatColor,
  shortDestLabel,
  subcatLogo,
  TIER_COLORS,
  BOND_LEVEL_COLORS,
  PIP_COUNT,
  tierBucket,
  pipsFilled,
  championshipColor,
  tierColor,
  isRookieCategory,
  brandOf,
  formatTeammateTenure,
  famaTierLabel,
  formatCashCompact,
  subcatLogoFit,
  is_regular_market_category,
  playerCatToFilter,
  formatTenureBadge,
  formatTenureCounter,
  getTeamMovementBadge,
  getTeamMovementOrder,
  getTeamMappingSortValue,
  count_team_vacancies,
  formatSafeLastChampionshipResult,
  formatSafeWeeklyClosingPosition,
  formatLastChampionshipResult,
  formatWeeklyClosingPosition,
  isRealCareerDebutCategory,
  inferWeeklyMovementKind,
  buildWeeklyClosingGroups,
  LICENSE_COLORS,
  LICENSE_LABEL_KEYS,
  licenseTooltip,
};
