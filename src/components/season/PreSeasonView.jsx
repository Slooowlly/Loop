import { useState, useEffect, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import useCareerStore from "../../stores/useCareerStore";
import { formatSalaryMonthly, formatMoneyCompact, extractNationalityLabel } from "../../utils/formatters";
import TeamLogoMark from "../team/TeamLogoMark";
import FlagIcon from "../ui/FlagIcon";
import GlobalTeamsTab from "../../pages/tabs/GlobalTeamsTab";

// ─── Category Definitions ─────────────────────────────────────────────────────

const CATEGORIES = [
  { id: "all",        label: "Todas",      color: "rgba(255,255,255,0.35)" },
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
  bmw_m2: "BMW Cup",
  production_challenger: "Production",
  gt3: "GT3 Championship",
  gt4: "GT4 Championship",
  lmp2: "LMP2 Prototype Championship",
  endurance: "Endurance Championship",
};

const SUBCAT_LOGOS = {
  mazda: "/utilities/categorias/recortadas/MX5%20CUP.png",
  mazda_amador: "/utilities/categorias/recortadas/MX5%20CUP.png",
  mazda_rookie: "/utilities/categorias/recortadas/MX5%20ROOKIE.png",
  toyota: "/utilities/categorias/recortadas/GR%20CUP.png",
  toyota_amador: "/utilities/categorias/recortadas/GR%20CUP.png",
  toyota_rookie: "/utilities/categorias/recortadas/GR%20ROOKIE.png",
  bmw: "/utilities/categorias/recortadas/M2%20CUP.png",
  bmw_m2: "/utilities/categorias/recortadas/M2%20CUP.png",
  gt4: "/utilities/categorias/recortadas/GT4.png",
  gt3: "/utilities/categorias/recortadas/GT3.png",
  production_challenger: "/utilities/categorias/recortadas/PRODUCTION.png",
  endurance: "/utilities/categorias/recortadas/ENDURANCE.png",
};

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
const PRESTIGE_TIERS   = ["Anônima", "Promissora", "Respeitada", "Renomada", "Lendária"];
const RELIABILITY_TIERS = ["Frágil", "Instável", "Consistente", "Sólida", "Impecável"];
const CAR_TIERS        = ["Defasado", "Modesto", "Competitivo", "Forte", "Ponta"];
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
function tierLabel(value, tiers) {
  return tiers[tierBucket(value)];
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
  if (tenure == null || tenure <= 0) return "Recém-chegado";
  if (tenure === 1) return "1ª temporada na equipe";
  return `${tenure}ª temporada na equipe`;
}

// Rótulo de fama (0–100) na mesma régua de 6 níveis da ficha do piloto
// (fama_level_for_value no backend) — mantém a leitura de estrelato consistente.
function famaTierLabel(fama) {
  const value = Number(fama ?? 0);
  if (value <= 15) return "Anônimo";
  if (value <= 30) return "Discreto";
  if (value <= 50) return "Conhecido";
  if (value <= 70) return "Nome forte";
  if (value <= 87) return "Estrela";
  return "Ídolo";
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
    label: tenureSeasons === 1 ? "New" : `${tenureSeasons} anos`,
    isNewcomer: tenureSeasons === 1,
  };
}

function getTeamMovementBadge(categoriaAnterior, categoriaAtual) {
  const movement = getMovementBadge(categoriaAnterior, categoriaAtual);
  if (!movement) return null;

  if (movement.color === "#3fb950") {
    return { ...movement, label: "Promovido" };
  }

  if (movement.color === "#f85149") {
    return { ...movement, label: "Relegado" };
  }

  return movement;
}

function getTeamMovementOrder(team) {
  const movement = getTeamMovementBadge(team.categoria_anterior, team._categoria || team.classe);
  if (!movement) return 0;
  if (movement.label === "Promovido") return 1;
  if (movement.label === "Relegado") return 2;
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
  return `${driver.last_championship_position}\u00ba/${driver.last_championship_total_drivers}`;
}

function formatWeeklyClosingPosition(position) {
  if (!position) return "--";
  return `${position}\u00ba`;
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

function WeeklyClosingMovement({ event, color, onSelect }) {
  const movementBadge = WEEKLY_MARKET_MOVEMENT_BADGES[event.movement_kind];
  // Vínculo com o jogador (rival / favorito / já-correu) → realce no feed.
  const emphasis = RELATION_EMPHASIS[event.relation];
  const strong = emphasis?.strong;

  return (
    <article
      role="button"
      tabIndex={0}
      onClick={() => onSelect?.(event)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect?.(event);
        }
      }}
      className="cursor-pointer rounded-lg border px-2.5 py-2 transition-colors hover:brightness-125"
      style={
        strong
          ? {
              // Rival/favorito: realce forte — borda viva + glow sutil na cor do vínculo.
              borderColor: emphasis.border,
              background: `linear-gradient(135deg, ${emphasis.bg} 0%, rgba(255,255,255,0.02) 100%)`,
              boxShadow: `0 0 0 1px ${emphasis.border}, 0 0 12px -4px ${emphasis.color}`,
            }
          : {
              borderColor: `${color}26`,
              background: `linear-gradient(135deg, ${color}0f 0%, rgba(255,255,255,0.02) 100%)`,
            }
      }
    >
      <div className="flex min-w-0 items-center gap-2.5">
        {movementBadge && (
          <span
            aria-label={movementBadge.label}
            title={movementBadge.label}
            className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md border text-[13px] font-black leading-none"
            style={{
              color: movementBadge.color,
              background: movementBadge.bg,
              borderColor: movementBadge.border,
            }}
          >
            {movementBadge.symbol}
          </span>
        )}
        {event.championship_position != null && (
          <span
            className="w-8 shrink-0 text-right text-[13px] font-black leading-none"
            style={{ color }}
          >
            {formatWeeklyClosingPosition(event.championship_position)}
          </span>
        )}
        <p className="min-w-0 flex-1 truncate text-[13px] font-extrabold leading-[1.05] text-[color:var(--text-primary)]">
          {event.driver_name}
        </p>
        {emphasis && (
          <span
            title={emphasis.label}
            className={`flex shrink-0 items-center gap-1 rounded-md border px-1.5 py-0.5 leading-none ${
              strong ? "text-[10px] font-black uppercase tracking-[0.08em]" : "text-[10px] font-bold"
            }`}
            style={{ color: emphasis.color, background: emphasis.bg, borderColor: emphasis.border }}
          >
            <span className="text-[11px] leading-none">{emphasis.symbol}</span>
            {strong ? emphasis.label : "Já correu"}
          </span>
        )}
        {event.team_name && (
          <TeamLogoMark
            teamName={event.team_name}
            color={color}
            size="xs"
            testId="weekly-closing-team-logo"
          />
        )}
      </div>
    </article>
  );
}

function MarketCategoryHeader({ categoryKey, detail }) {
  const label = subcatLabel(categoryKey);
  const color = subcatColor(categoryKey);
  const logo = subcatLogo(categoryKey);
  const logoFit = subcatLogoFit(categoryKey);

  return (
    <div
      data-testid={`preseason-category-header-${categoryKey}`}
      className="mb-5 flex flex-col items-center justify-center gap-3 rounded-xl px-4 py-6 text-center"
      style={{
        background: `linear-gradient(135deg, ${color}22 0%, ${color}0a 100%)`,
        borderLeft: `3px solid ${color}`,
        boxShadow: `0 0 18px ${color}18`,
      }}
    >
      {logo ? (
        <div className={`flex w-full items-start justify-center overflow-hidden ${logoFit.frameClassName}`}>
          <img
            data-testid="preseason-category-logo"
            src={logo}
            alt={label}
            className="h-full w-auto max-w-none object-contain"
            style={logoFit.imageStyle}
            draggable={false}
          />
        </div>
      ) : (
        <span
          className="text-[17px] font-bold uppercase tracking-[0.18em]"
          style={{ color }}
        >
          {label}
        </span>
      )}
      <span
        data-testid="preseason-category-count"
        className="shrink-0 rounded-full border px-3 py-1 text-[11px] font-bold uppercase tracking-[0.12em]"
        style={{
          color,
          borderColor: `${color}55`,
          backgroundColor: `${color}14`,
        }}
      >
        {detail}
      </span>
    </div>
  );
}

function TeamDriverRow({ driverName, tenureSeasons, isPrimarySlot = false, accent = "#58a6ff" }) {
  const isOpenSlot = !driverName;

  // Vaga aberta: chip tracejado na cor da categoria (lê como oportunidade, não como
  // "erro"/vazio como o antigo "Sem piloto" vermelho).
  if (isOpenSlot) {
    return (
      <div className="flex items-center py-2">
        <span
          className="flex w-full items-center gap-2 rounded-lg border border-dashed px-3 py-1.5 text-body font-semibold"
          style={{ borderColor: `${accent}66`, color: accent, background: `${accent}12` }}
        >
          <span className="text-[14px] font-bold leading-none opacity-80">+</span>
          Vaga aberta
        </span>
      </div>
    );
  }

  const tenureCounter = formatTenureCounter(tenureSeasons);
  // Pips de tempo de casa: 1 pip por temporada (teto de 5); o rótulo numérico
  // mantém a precisão exata. Estreante (1ª temp.) segue com o badge dedicado.
  const pipCount = Math.min(Math.max(tenureSeasons ?? 0, 0), 5);
  return (
    <div className="flex items-center justify-between gap-3 py-2.5">
      <div className="flex min-w-0 flex-1 items-center">
        <p className={`truncate leading-[1.1] ${isPrimarySlot ? "text-[15px] font-bold text-[color:var(--text-primary)]" : "text-[14px] font-semibold text-[color:var(--text-primary)]"}`}>
            {driverName}
        </p>
      </div>
      {tenureCounter && (
        tenureCounter.isNewcomer ? (
          <span className="shrink-0 rounded-md border border-[#58a6ff55] bg-[#58a6ff1f] px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.08em] text-[#79b8ff]">
            {tenureCounter.label}
          </span>
        ) : (
          <span className="flex shrink-0 items-center gap-2">
            <span className="flex items-center gap-[3px]" aria-hidden="true">
              {Array.from({ length: pipCount }).map((_, i) => (
                <span key={i} className="h-1.5 w-1.5 rounded-full" style={{ background: accent }} />
              ))}
            </span>
            <span className="text-[11px] font-semibold tabular-nums text-[color:var(--text-muted)]">
              {tenureCounter.label}
            </span>
          </span>
        )
      )}
    </div>
  );
}

// ─── FreeAgentCard ────────────────────────────────────────────────────────────

const LICENSE_COLORS = {
  R:  { text: "#9ba3ae", bg: "rgba(155,163,174,0.12)" },
  A:  { text: "#3fb950", bg: "rgba(63,185,80,0.12)"   },
  P:  { text: "#58a6ff", bg: "rgba(88,166,255,0.12)"  },
  SP: { text: "#FF8000", bg: "rgba(255,128,0,0.12)"   },
  E:  { text: "#bc8cff", bg: "rgba(188,140,255,0.12)" },
  SE: { text: "#ffd700", bg: "rgba(255,215,0,0.12)"   },
};

const LICENSE_LABELS = {
  R: "Rookie",
  A: "Amadora",
  P: "Pro",
  SP: "Super Pro",
  E: "Elite",
  SE: "Super Elite",
};

function licenseTooltip(sigla) {
  const label = LICENSE_LABELS[sigla] ?? "Sem licença";
  return `Carteira ${label}`;
}

function FreeAgentCard({ driver, isRookie, onHoverCat }) {
  const destColor = subcatColor(driver.categoria);
  const destLabel = shortDestLabel(driver.categoria);
  const idle = driver.seasons_idle ?? 0;
  const isParado = idle >= 1; // sentou fora ao menos uma temporada
  return (
    <div
      className={`glass-light flex items-center gap-2 rounded-xl px-2.5 py-1.5 transition-opacity ${isParado ? "opacity-55" : ""}`}
      onMouseEnter={() => onHoverCat?.(driver.categoria)}
      onMouseLeave={() => onHoverCat?.(null)}
    >
      {isRookie ? (
        <span className="shrink-0 rounded-md bg-[#bc8cff22] px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.12em] text-[#bc8cff]">
          Novo
        </span>
      ) : (
        driver.previous_team_name ? (
          <TeamLogoMark
            teamName={driver.previous_team_name}
            color={driver.previous_team_color ?? destColor}
            size="xs"
            testId="driver-market-previous-team-logo"
          />
        ) : (
          <span
            className="shrink-0 rounded-md px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.1em]"
            style={{ background: `${destColor}22`, color: destColor }}
          >
            {driver.previous_team_abbr ?? "—"}
          </span>
        )
      )}
      <p className="min-w-0 flex-1 truncate text-body text-[color:var(--text-primary)]">
        {driver.driver_name}
      </p>
      {isParado && (
        <span
          className="shrink-0 rounded-md bg-white/5 px-1.5 py-0.5 text-[9px] font-semibold tabular-nums text-[color:var(--text-muted)]"
          title={`Parado há ${idle} ${idle === 1 ? "temporada" : "temporadas"}`}
        >
          {`parado ${idle}t`}
        </span>
      )}
      {/* Etiqueta de destino provável (categoria onde as propostas chegam) — sempre
          visível no canto, mesmo com separador de marca. Substitui a carteira, escondida. */}
      <span
        className="shrink-0 rounded-md px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.06em]"
        style={{ background: `${destColor}1f`, color: destColor }}
        title={`Destino provável: ${destLabel}`}
      >
        {destLabel}
      </span>
    </div>
  );
}

// ─── Main Component ───────────────────────────────────────────────────────────

export default function PreSeasonView() {
  const careerId             = useCareerStore((s) => s.careerId);
  const preseasonState       = useCareerStore((s) => s.preseasonState);
  const lastMarketWeekResult = useCareerStore((s) => s.lastMarketWeekResult);
  const playerProposals      = useCareerStore((s) => s.playerProposals);
  const respondToProposal    = useCareerStore((s) => s.respondToProposal);
  const transferWindow       = useCareerStore((s) => s.transferWindow);
  const preseasonFreeAgents  = useCareerStore((s) => s.preseasonFreeAgents);
  const isAdvancingWeek      = useCareerStore((s) => s.isAdvancingWeek);
  const advanceMarketWeek    = useCareerStore((s) => s.advanceMarketWeek);
  const finalizePreseason    = useCareerStore((s) => s.finalizePreseason);
  const playerTeam           = useCareerStore((s) => s.playerTeam);

  const [selectedCat, setSelectedCat]           = useState(() => playerCatToFilter(playerTeam?.categoria));
  const [gridData, setGridData]                 = useState([]);
  const [loadingGrid, setLoadingGrid]           = useState(false);
  const [showDisplacedModal, setShowDisplacedModal] = useState(false);
  const [transferDetail, setTransferDetail] = useState(null);
  const [showFreeAgentWarning, setShowFreeAgentWarning] = useState(false);
  const [startError, setStartError] = useState("");
  const [paintToast, setPaintToast] = useState("");
  const [showOffersModal, setShowOffersModal] = useState(false);
  // Categoria filtrada no modal de ofertas (null = mostrar todas).
  const [offersModalCat, setOffersModalCat] = useState(null);
  // Oferta cujo "contrato" (tela detalhada de assinatura) está aberto (null = fechado).
  const [contractOffer, setContractOffer] = useState(null);
  // Animação de assinatura: enquanto true, o nome do piloto é "escrito" na linha
  // antes de a oferta ser efetivada.
  const [isSigning, setIsSigning] = useState(false);
  // Equipe do grid cujo Histórico mundial de equipes (atlas) está aberto — duplo clique no card.
  const [historyTeam, setHistoryTeam] = useState(null);
  // Categoria do piloto livre sob o cursor → acende as equipes dela no grid central.
  const [hoveredFreeAgentCat, setHoveredFreeAgentCat] = useState(null);

  const freeAgentContainerRef = useRef(null);
  const freeAgentSectionRefs  = useRef({});
  const mainGridRef           = useRef(null);
  // Scroll a restaurar após avançar a semana (mantém o usuário onde estava, em vez
  // de jogar os painéis pro topo enquanto o grid recarrega). null = nada a restaurar.
  const preserveScrollRef     = useRef(null);
  // Auto-scroll pra categoria do jogador só na 1ª carga (não brigar com o restore).
  const didInitialScrollRef   = useRef(false);

  // Semana atual e total
  const currentWeek = Math.min(preseasonState?.current_week ?? 1, preseasonState?.total_weeks ?? 1);
  const totalWeeks  = preseasonState?.total_weeks ?? 1;
  const isComplete  = preseasonState?.is_complete ?? false;
  const isMarketOpen = !isComplete;
  const weekProgress = Math.min(100, (currentWeek / totalWeeks) * 100);

  // Ofertas que a Janela de Transferências mandou ao jogador nesta semana.
  const playerOffers = transferWindow?.player_offers ?? [];
  const playerSignedThisWindow = preseasonState?.player_has_team ?? false;

  // Categoria/tier efetivos do jogador (vêm do backend — funciona mesmo como agente
  // livre, quando não há playerTeam). Tier na convenção do backend (= offer.category_tier).
  const playerCategory = transferWindow?.player_category ?? playerTeam?.categoria ?? null;
  const playerTier = transferWindow?.player_tier ?? null;
  const playerBrand = brandOf(playerCategory);
  const playerName = transferWindow?.player_name ?? null;

  // Ofertas agrupadas por categoria (N1/N2 dentro). Ordem: MARCA do jogador primeiro
  // (ex.: Mazda antes de Toyota) e, dentro de cada marca, tier maior primeiro (Cup antes
  // de Rookie). As demais marcas vêm depois, agrupadas, também por tier decrescente.
  const offersByCategory = useMemo(() => {
    const groups = new Map();
    for (const offer of playerOffers) {
      const baseCat = offer.category || "outras";
      // Production/Endurance dividem por CLASSE (carro): chave "categoria:classe".
      const isMulti =
        (baseCat === "production_challenger" || baseCat === "endurance") && offer.class;
      const key = isMulti ? `${baseCat}:${offer.class}` : baseCat;
      if (!groups.has(key)) {
        groups.set(key, {
          cat: key,
          baseCat,
          classe: isMulti ? offer.class : null,
          tier: offer.category_tier ?? CATEGORY_TIER[baseCat] ?? 0,
          label: isMulti
            ? `${shortCatName(baseCat)} · ${CLASS_LABELS[offer.class] ?? offer.class.toUpperCase()}`
            : offer.category_label || subcatLabel(baseCat),
          n1: [],
          n2: [],
        });
      }
      const g = groups.get(key);
      if (offer.role === "N1") g.n1.push(offer);
      else g.n2.push(offer);
    }
    // Bucket de ordenação: 0 = PROMOÇÃO (tier acima do jogador, sempre no topo),
    // 1 = marca do jogador, 2 = demais marcas. Usa a categoria BASE (não a classe).
    // Tier de EXIBIÇÃO usa CATEGORY_TIER (distingue Production=3 de BMW=2, que no
    // backend são ambos tier 2).
    const bucketOf = (g) => {
      if (playerTier != null && g.tier > playerTier) return 0;
      if (playerBrand && brandOf(g.baseCat) === playerBrand) return 1;
      return 2;
    };
    for (const g of groups.values()) g.bucket = bucketOf(g);
    const dispTier = (g) => CATEGORY_TIER[g.baseCat] ?? g.tier;
    return [...groups.values()].sort((a, b) => {
      if (a.bucket !== b.bucket) return a.bucket - b.bucket;
      // Ordena por NÍVEL da categoria: maior no topo, rookies no fundo.
      // (GT3 > GT4 > Production > BMW/Cup > Rookie.)
      const dt = dispTier(b) - dispTier(a);
      if (dt !== 0) return dt;
      // Mesmo nível, mesma categoria multiclasse → ordem MULTICLASS_ORDER das classes.
      if (a.baseCat === b.baseCat && a.classe && b.classe) {
        const order = MULTICLASS_ORDER[a.baseCat] ?? [];
        const ia = order.indexOf(a.classe);
        const ib = order.indexOf(b.classe);
        if (ia !== ib) return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib);
      }
      // Mesmo nível, marcas diferentes (ex.: BMW vs Cups) → desempate por marca
      // (bmw < mazda < toyota), deixando o BMW acima das cups.
      const na = brandOf(a.baseCat) ?? "";
      const nb = brandOf(b.baseCat) ?? "";
      if (na !== nb) return na < nb ? -1 : 1;
      return 0;
    });
  }, [playerOffers, playerBrand, playerTier]);

  const totalOffers = playerOffers.length;

  // Três andares de importância (offersByCategory já vem ordenado por bucket):
  // 0 = promoção (destaque), 1 = marca atual do jogador, 2 = demais marcas.
  const promoOfferGroups = offersByCategory.filter((g) => g.bucket === 0);
  const brandOfferGroups = offersByCategory.filter((g) => g.bucket === 1);
  const otherOfferGroups = offersByCategory.filter((g) => g.bucket === 2);

  // Fileira DENSA (uma linha) para os andares 2 e 3: borda colorida + rótulo +
  // contagem de vagas discreta. Sem o chip numérico grande (era só ruído).
  const renderOfferRowDense = (group) => {
    const n = group.n1.length + group.n2.length;
    const accent = subcatColor(group.cat);
    return (
      <button
        key={group.cat}
        type="button"
        onClick={() => { setOffersModalCat(group.cat); setShowOffersModal(true); }}
        data-testid={`offer-category-row-${group.cat}`}
        className="transition-glass glass-light hover:glass group flex w-full items-center gap-3 rounded-lg py-2 pl-3 pr-2.5 text-left"
        style={{ borderLeft: `3px solid ${accent}` }}
      >
        <span
          className="min-w-0 flex-1 truncate text-[11px] font-black uppercase tracking-[0.12em]"
          style={{ color: accent }}
        >
          {group.label}
        </span>
        <span className="shrink-0 text-[10px] font-semibold uppercase tracking-[0.14em] text-[color:var(--text-muted)]">
          {n} vaga{n > 1 ? "s" : ""}
        </span>
        <span className="shrink-0 text-[color:var(--text-muted)] transition-transform group-hover:translate-x-0.5">›</span>
      </button>
    );
  };

  // Card RICO (ficha da equipe) usado dentro do modal de ofertas.
  // Mostra os dados de scouting; o foco e a relação (vínculo) ficam na tela de contrato.
  const renderOfferCardRich = (offer) => {
    const accent = offer.team_color || "#58a6ff";
    const rookie = isRookieCategory(offer.category);
    const pedigree =
      (offer.team_titles_drivers ?? 0) +
      (offer.team_titles_constructors ?? 0) +
      (offer.team_historic_wins ?? 0);
    const countryLabel = extractNationalityLabel(offer.team_country) || offer.team_country || "";
    const dur = offer.offer_duration ?? 1;
    // No rookie o carro não afeta o resultado → não mostrar (seria enganoso).
    const stats = [
      !rookie && { label: "Carro", value: offer.car_performance_rating, tiers: CAR_TIERS },
      { label: "Confiabilidade", value: offer.team_reliability, tiers: RELIABILITY_TIERS },
      { label: "Prestígio", value: offer.team_reputation, tiers: PRESTIGE_TIERS },
    ].filter(Boolean);
    return (
      <article
        key={offer.seat_id}
        className={[
          "glass animate-scale-in overflow-hidden rounded-2xl",
          offer.active_interest
            ? "ring-2 ring-[#f2c46d]/70 shadow-[0_0_28px_rgba(242,196,109,0.22)]"
            : "",
        ].join(" ")}
        style={{ borderLeft: `3px solid ${accent}` }}
      >
        {offer.active_interest && (
          <div className="flex items-center gap-1.5 bg-[#f2c46d1a] px-4 py-1.5 text-[10px] font-bold uppercase tracking-[0.16em] text-[#f2c46d]">
            <span>◆</span>
            <span>Interesse ativo — te querem pelo seu nome</span>
          </div>
        )}
        {/* Cabeçalho: identidade da equipe */}
        <div
          className="flex items-center gap-3 px-4 py-3.5"
          style={{ background: `linear-gradient(135deg, ${accent}22 0%, rgba(255,255,255,0.02) 100%)` }}
        >
          <TeamLogoMark teamName={offer.team_name} color={accent} size="lg" />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span
                className="rounded-md px-1.5 py-0.5 text-[10px] font-black uppercase tracking-[0.14em]"
                style={{ color: accent, background: `${accent}22` }}
              >
                {offer.role === "N1" ? "Piloto 1" : "Piloto 2"}
              </span>
              <span
                className="text-[10px] font-bold uppercase tracking-[0.16em]"
                style={{ color: accent }}
              >
                {offer.category_label || offer.category}
              </span>
            </div>
            <p className="mt-1 truncate text-title-md font-bold">{offer.team_name}</p>
            <div className="mt-0.5 flex items-center gap-1.5 text-[11px] text-[color:var(--text-muted)]">
              <FlagIcon nacionalidade={offer.team_country} className="h-3.5 w-5" />
              {countryLabel && <span>{countryLabel}</span>}
              {offer.team_founded_year ? <span>· desde {offer.team_founded_year}</span> : null}
            </div>
          </div>
          <div className="text-right">
            <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">Salário</p>
            <p className="num-medium font-bold text-[color:var(--status-green)]">{formatSalaryMonthly(offer.salary)}</p>
          </div>
        </div>

        <div className="space-y-3 px-4 py-3.5">
          {/* Atributos em FAIXA de texto (sem números) */}
          <div className={`grid grid-cols-1 gap-2 ${stats.length === 3 ? "sm:grid-cols-3" : "sm:grid-cols-2"}`}>
            {stats.map(({ label, value, tiers }) => (
              <div key={label} className="glass-light rounded-lg p-2.5">
                <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">{label}</p>
                <p className="mt-0.5 text-body font-bold" style={{ color: tierColor(value) }}>
                  {tierLabel(value, tiers)}
                </p>
                <div className="mt-1.5 flex gap-0.5">
                  {Array.from({ length: PIP_COUNT }).map((_, i) => (
                    <span
                      key={i}
                      className="h-1 flex-1 rounded-full"
                      style={{ background: i < pipsFilled(value) ? tierColor(value) : "#21262d" }}
                    />
                  ))}
                </div>
              </div>
            ))}
          </div>

          {/* Última temporada — em destaque */}
          {(() => {
            const pos = offer.team_last_position;
            const posColor = pos != null ? championshipColor(pos) : "var(--text-muted)";
            return (
              <div
                className="flex items-center justify-between rounded-xl px-3.5 py-2.5"
                style={{ background: `${accent}12`, border: `1px solid ${accent}2e` }}
              >
                <div>
                  <p className="text-[9px] font-bold uppercase tracking-[0.22em] text-[color:var(--text-muted)]">
                    Última temporada
                  </p>
                  <p className="text-[10px] text-[color:var(--text-secondary)]">
                    {pos != null ? "Posição no campeonato" : "Sem histórico ainda"}
                  </p>
                </div>
                <p className="text-[34px] font-black leading-none" style={{ color: posColor }}>
                  {pos != null ? `${pos}º` : "Estreante"}
                </p>
              </div>
            );
          })()}

          {/* Ficha textual */}
          <div className={`grid grid-cols-2 gap-2 ${rookie ? "sm:grid-cols-2" : "sm:grid-cols-3"}`}>
            <div className="glass-light rounded-lg p-2.5">
              <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">Caixa</p>
              <p
                className="mt-0.5 num-medium font-bold"
                style={{ color: offer.team_cash < 0 ? "var(--status-red)" : "var(--status-green)" }}
              >
                {formatCashCompact(offer.team_cash)}
              </p>
            </div>
            {!rookie && (
              <div className="glass-light rounded-lg p-2.5">
                <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">Títulos</p>
                <p className="mt-0.5 num-medium font-bold text-[color:var(--text-primary)]">
                  {(offer.team_titles_drivers ?? 0)}
                  <span className="text-[color:var(--text-muted)]"> · </span>
                  {(offer.team_titles_constructors ?? 0)}
                </p>
                <p className="text-[9px] text-[color:var(--text-muted)]">pilotos · construtores</p>
              </div>
            )}
            <div className="glass-light rounded-lg p-2.5">
              <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">Vitórias · Pódios</p>
              <p className="mt-0.5 num-medium font-bold text-[color:var(--text-primary)]">
                {offer.team_historic_wins ?? 0}
                <span className="text-[color:var(--text-muted)]"> · </span>
                {offer.team_historic_podiums ?? 0}
              </p>
            </div>
          </div>

          {/* Companheiro de equipe (hover → estatísticas) */}
          <div className="glass-light rounded-lg p-2.5">
            <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">Companheiro de equipe</p>
            {offer.teammate_name ? (
              <div className="group relative mt-0.5 cursor-help">
                <div className="flex items-center justify-between gap-2">
                  <div className="flex min-w-0 items-center gap-1.5">
                    <p className="text-body min-w-0 truncate font-semibold text-[color:var(--text-primary)]">
                      {offer.teammate_name}
                    </p>
                    <span className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full border border-white/25 text-[9px] font-bold text-[color:var(--text-muted)] transition-colors group-hover:border-[color:var(--accent-primary)] group-hover:text-[color:var(--accent-primary)]">
                      ?
                    </span>
                  </div>
                  <span className="shrink-0 text-[10px] text-[color:var(--text-muted)]">
                    {formatTeammateTenure(offer.teammate_tenure)}
                  </span>
                </div>
                {/* Tooltip de estatísticas */}
                <div className="pointer-events-none absolute bottom-full left-0 z-30 mb-2 w-64 rounded-xl border border-white/10 bg-[#0d1117] p-3 opacity-0 shadow-[0_8px_24px_rgba(0,0,0,0.5)] transition-opacity duration-150 group-hover:opacity-100">
                  <p className="mb-2 min-w-0 truncate text-[10px] font-bold uppercase tracking-[0.16em] text-[color:var(--text-secondary)]">
                    {offer.teammate_name}
                    {offer.teammate_age != null ? ` · ${offer.teammate_age} anos` : ""}
                  </p>
                  <div className="grid grid-cols-2 gap-x-3 gap-y-1.5">
                    {[
                      ["Corridas", offer.teammate_races ?? 0],
                      ["Vitórias", offer.teammate_wins ?? 0],
                      ["Pódios", offer.teammate_podiums ?? 0],
                      ["Poles", offer.teammate_poles ?? 0],
                      ["Títulos", offer.teammate_titles ?? 0],
                      ["Pontos", Math.round(offer.teammate_career_points ?? 0)],
                    ].map(([label, val]) => (
                      <div key={label} className="flex items-center justify-between">
                        <span className="text-[11px] text-[color:var(--text-muted)]">{label}</span>
                        <span className="num-medium text-[12px] font-bold text-[color:var(--text-primary)]">{val}</span>
                      </div>
                    ))}
                  </div>
                  {(offer.teammate_strengths?.length > 0 || offer.teammate_weaknesses?.length > 0) && (
                    <div className="mt-2 space-y-1 border-t border-white/8 pt-2">
                      {offer.teammate_strengths?.length > 0 && (
                        <div className="flex items-start gap-1.5">
                          <span className="mt-px shrink-0 text-[11px] font-bold text-[color:var(--status-green)]">▲</span>
                          <span className="text-[11px] text-[color:var(--text-secondary)]">
                            {offer.teammate_strengths.join(" · ")}
                          </span>
                        </div>
                      )}
                      {offer.teammate_weaknesses?.length > 0 && (
                        <div className="flex items-start gap-1.5">
                          <span className="mt-px shrink-0 text-[11px] font-bold text-[#f85149]">▼</span>
                          <span className="text-[11px] text-[color:var(--text-secondary)]">
                            {offer.teammate_weaknesses.join(" · ")}
                          </span>
                        </div>
                      )}
                    </div>
                  )}
                  {offer.teammate_fama != null && (
                    <div className="mt-2 flex items-center justify-between border-t border-white/8 pt-2">
                      <span className="text-[11px] text-[color:var(--text-muted)]">Fama</span>
                      <span
                        title={
                          offer.teammate_carisma != null
                            ? `Fama ${offer.teammate_fama}/100 · Carisma ${offer.teammate_carisma}/100`
                            : `Fama ${offer.teammate_fama}/100`
                        }
                        className="num-medium text-[12px] font-bold text-[color:var(--accent-secondary)]"
                      >
                        {famaTierLabel(offer.teammate_fama)}
                      </span>
                    </div>
                  )}
                  <div className="mt-2 flex items-center justify-between border-t border-white/8 pt-2">
                    <span className="text-[11px] text-[color:var(--text-muted)]">Salário</span>
                    <span className="num-medium text-[12px] font-bold text-[color:var(--status-green)]">
                      {offer.teammate_salary != null ? formatSalaryMonthly(offer.teammate_salary) : "—"}
                    </span>
                  </div>
                </div>
              </div>
            ) : (
              <p className="text-body mt-0.5 text-[color:var(--text-muted)]">Vaga livre.</p>
            )}
          </div>

          {/* Duração do contrato ofertado */}
          <div className="flex items-center justify-between rounded-lg bg-black/18 px-3 py-2">
            <span className="text-[9px] font-bold uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
              Contrato ofertado
            </span>
            <span
              className="num-medium text-body font-bold"
              style={{ color: dur >= 2 ? "var(--status-green)" : "var(--text-primary)" }}
            >
              {dur} temporada{dur > 1 ? "s" : ""}
              {dur >= 2 && <span className="ml-1.5 text-[9px] font-semibold">· projeto</span>}
            </span>
          </div>

          <button
            onClick={() => { setIsSigning(false); setContractOffer(offer); }}
            disabled={isAdvancingWeek}
            className="transition-glass glow-blue w-full rounded-lg border border-[#58a6ff66] bg-[#58a6ff33] px-3 py-2.5 text-body font-bold text-[color:var(--accent-primary)] hover:bg-[#58a6ff55] disabled:cursor-not-allowed disabled:opacity-50"
          >
            Ver contrato
          </button>
          {pedigree === 0 && (
            <p className="text-center text-[10px] text-[color:var(--text-muted)]">Equipe sem histórico expressivo ainda.</p>
          )}
        </div>
      </article>
    );
  };

  // Card de time do grid central (reutilizado no fluxo normal e nas sub-classes
  // de Production/Endurance).
  const renderTeamCard = (team, accent) => {
    const rankStyle = getRankStyle(team.temp_posicao);
    const movement = getTeamMovementBadge(team.categoria_anterior, team._categoria || team.classe);
    const teamLogoFallback = movement?.color ?? team.cor_primaria ?? accent;
    // Conexão entre colunas: passar o mouse sobre um piloto livre acende as equipes
    // da mesma categoria (por championship OU por classe) e esmaece as demais.
    const matchesHover = hoveredFreeAgentCat != null
      && (team._categoria === hoveredFreeAgentCat || team.classe === hoveredFreeAgentCat);
    const isDimmed = hoveredFreeAgentCat != null && !matchesHover;
    return (
      <article
        key={team.id}
        onDoubleClick={() => setHistoryTeam(team)}
        title="Duplo clique: histórico mundial de equipes"
        className="glass transition-glass relative cursor-pointer select-none overflow-hidden rounded-xl border p-3 hover:-translate-y-0.5 hover:scale-[1.01]"
        style={{
          borderColor: matchesHover
            ? accent
            : movement
              ? movement.border
              : rankStyle?.border
                ? `${rankStyle.border}88`
                : "rgba(255,255,255,0.11)",
          opacity: isDimmed ? 0.32 : 1,
          boxShadow: matchesHover ? `0 0 0 1px ${accent}, 0 10px 34px -14px ${accent}` : undefined,
          transition: "opacity .16s ease, border-color .16s ease, box-shadow .16s ease, transform .16s ease",
        }}
      >
        {rankStyle && !movement && (
          <div
            className="pointer-events-none absolute right-0 top-0 h-full w-28"
            style={{ background: `radial-gradient(circle at 94% 14%, ${rankStyle.glow} 0%, transparent 68%)` }}
          />
        )}
        {movement && (
          <div
            className="pointer-events-none absolute right-0 top-0 h-full w-32"
            style={{ background: `radial-gradient(circle at 94% 14%, ${movement.bg.replace("0.12", "0.18")} 0%, transparent 68%)` }}
          />
        )}

        <div className="relative mb-3 flex items-start gap-3">
          <div className="flex min-w-0 flex-1 items-center gap-3">
            <TeamLogoMark
              teamName={team.nome}
              color={teamLogoFallback}
              size="md"
              testId="preseason-team-logo"
            />
            <div className="min-w-0 flex-1">
              <p className="truncate text-[19px] font-bold leading-[1.05]">{team.nome}</p>
            </div>
            {movement && (
              <span
                className="ml-auto shrink-0 rounded-md border px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.08em]"
                style={{ color: movement.color, backgroundColor: movement.bg, borderColor: movement.border }}
              >
                {movement.label}
              </span>
            )}
          </div>
        </div>

        <div className="relative divide-y divide-white/8">
          <TeamDriverRow
            driverName={team.piloto_1_nome}
            tenureSeasons={team.piloto_1_tenure_seasons}
            accent={accent}
            isPrimarySlot
          />
          <TeamDriverRow
            driverName={team.piloto_2_nome}
            tenureSeasons={team.piloto_2_tenure_seasons}
            accent={accent}
          />
        </div>
      </article>
    );
  };

  const currentDateLabel = useMemo(
    () => {
      const preseasonDate = preseasonState?.current_display_date;
      if (!preseasonDate) return "-";

      return new Intl.DateTimeFormat("pt-BR", {
        day: "numeric",
        month: "long",
      }).format(new Date(`${preseasonDate}T12:00:00`));
    },
    [preseasonState?.current_display_date],
  );

  // ── Fetch grid ──────────────────────────────────────────────────────────────
  useEffect(() => {
    if (!careerId) return;
    let mounted = true;

    async function fetchGrid() {
      setLoadingGrid(true);
      try {
        const dbIds = new Set();
        if (selectedCat === "all") {
          CATEGORIES.filter((c) => !c.isSeparator && c.id !== "all").forEach((c) =>
            c.dbIds?.forEach((id) => dbIds.add(id)),
          );
        } else {
          const cfg = CATEGORIES.find((c) => c.id === selectedCat);
          if (cfg) cfg.dbIds?.forEach((id) => dbIds.add(id));
        }

        // Busca PARALELA por categoria (era sequencial → grid demorava a refletir as
        // assinaturas após avançar a semana). Tag cada equipe com o dbId usado.
        const perCategory = await Promise.all(
          [...dbIds].map((dbId) =>
            invoke("get_teams_standings", { careerId, category: dbId })
              .then((teams) => teams.map((t) => ({ ...t, _categoria: dbId })))
              .catch(() => []),
          ),
        );
        const all = perCategory.flat();

        // Filtrar por classe quando categoria tem filterClass
        let final = all;
        if (selectedCat !== "all") {
          const cfg = CATEGORIES.find((c) => c.id === selectedCat);
          if (cfg?.filterClass) {
            final = all.filter((t) => {
              if (t.classe === cfg.filterClass) return true;
              if (t._categoria?.startsWith(cfg.filterClass)) return true;
              if (cfg.filterClass === "bmw" && t._categoria === "bmw_m2") return true;
              return false;
            });
          }
        }

        if (mounted) setGridData(final);
      } finally {
        if (mounted) setLoadingGrid(false);
      }
    }

    fetchGrid();
    return () => { mounted = false; };
    // Semana CRUA (não clampada) + resultado da semana → o grid reflete as
    // assinaturas aplicadas a cada avanço, inclusive além do teto de exibição.
  }, [careerId, selectedCat, preseasonState?.current_week, lastMarketWeekResult]);

  // ── Agrupamento e ordenação ─────────────────────────────────────────────────
  const groupedTeams = useMemo(() => {
    const grouped = {};
    gridData.forEach((team) => {
      const key = team._categoria === "endurance" || team._categoria === "production_challenger"
        ? team._categoria
        : team.classe || team._categoria || "outras";
      grouped[key] = grouped[key] ?? [];
      grouped[key].push(team);
    });
    return grouped;
  }, [gridData]);

  const sortedClasses = useMemo(() => {
    return Object.keys(groupedTeams).sort((a, b) => {
      const pa = CLASS_PRIORITY.indexOf(a);
      const pb = CLASS_PRIORITY.indexOf(b);
      if (pa !== -1 && pb !== -1) return pa - pb;
      if (pa !== -1) return -1;
      if (pb !== -1) return 1;
      return a.localeCompare(b);
    });
  }, [groupedTeams]);

  // ── Free agents agrupados por FAIXA DE NÍVEL (onde correm hoje) ─────────────
  // Chave = banda do tier (market_tier), não a categoria/carteira. Dentro da banda,
  // pilotos "frescos" primeiro e os "parados" no fim (marcador de inatividade).
  const freeAgentsByBand = useMemo(() => {
    // Filtro do topo também recorta a coluna: mostra só quem pode pegar vaga na
    // categoria selecionada (interseção com eligible_categories, vindo do backend).
    const filterCfg = selectedCat === "all" ? null : CATEGORIES.find((c) => c.id === selectedCat);
    const filterDbIds = filterCfg?.dbIds ? new Set(filterCfg.dbIds) : null;
    const grouped = {};
    (preseasonFreeAgents ?? []).forEach((d) => {
      const cat = d.categoria || "outras";
      if (!is_regular_market_category(cat)) return;
      if (filterDbIds && !(d.eligible_categories ?? []).some((id) => filterDbIds.has(id))) return;
      const band = bandForTier(d.market_tier);
      (grouped[band.key] = grouped[band.key] ?? []).push(d);
    });
    Object.values(grouped).forEach((list) =>
      list.sort((a, b) => {
        // 1) Agrupa por marca/categoria dentro da banda: Toyota e Mazda têm a mesma cor,
        //    então intercalá-los confunde — cada marca vira uma sequência contígua.
        const pa = FREE_AGENT_ORDER.indexOf(a.categoria);
        const pb = FREE_AGENT_ORDER.indexOf(b.categoria);
        const oa = pa === -1 ? 999 : pa;
        const ob = pb === -1 ? 999 : pb;
        if (oa !== ob) return oa - ob;
        // 2) Dentro da marca: fresco antes do parado.
        const ia = a.seasons_idle ?? 0;
        const ib = b.seasons_idle ?? 0;
        if (ia !== ib) return ia - ib;
        // 3) Por nome.
        return (a.driver_name ?? "").localeCompare(b.driver_name ?? "");
      }),
    );
    return grouped;
  }, [preseasonFreeAgents, selectedCat]);

  // Bandas presentes, do mais prestigioso pro menos (ordem de LEVEL_BANDS).
  const freeAgentBandOrder = useMemo(
    () => LEVEL_BANDS.filter((b) => freeAgentsByBand[b.key]?.length),
    [freeAgentsByBand],
  );

  // Total visível (após o filtro do topo) — alimenta o contador do cabeçalho.
  const visibleFreeAgentCount = useMemo(
    () => freeAgentBandOrder.reduce((n, b) => n + freeAgentsByBand[b.key].length, 0),
    [freeAgentBandOrder, freeAgentsByBand],
  );

  const displacedVeterans = useMemo(
    () => (preseasonFreeAgents ?? []).filter((d) => !d.is_rookie),
    [preseasonFreeAgents],
  );

  const displacedVeteransByCategory = useMemo(() => {
    const grouped = {};

    displacedVeterans.forEach((driver) => {
      const category = driver.categoria || "outras";
      if (!is_regular_market_category(category)) return;
      grouped[category] = grouped[category] ?? [];
      grouped[category].push(driver);
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
      .map(([category, drivers]) => ({
        category,
        color: subcatColor(category),
        label: subcatLabel(category),
        drivers,
      }));
  }, [displacedVeterans]);

  const weeklyClosingGroups = useMemo(
    () => buildWeeklyClosingGroups(lastMarketWeekResult),
    [lastMarketWeekResult],
  );

  // ── Auto-scroll para a BANDA do jogador ao carregar ────────────────────────
  useEffect(() => {
    if (didInitialScrollRef.current) return; // só na 1ª carga; depois preserva o scroll do usuário
    if (!freeAgentBandOrder.length || !playerTeam?.categoria) return;
    const playerBand = bandForTier(MARKET_TIER_BY_CATEGORY[playerTeam.categoria]).key;
    const el = freeAgentSectionRefs.current[playerBand];
    const container = freeAgentContainerRef.current;
    if (el && container) {
      didInitialScrollRef.current = true;
      requestAnimationFrame(() => {
        container.scrollTop = Math.max(0, el.offsetTop - container.offsetTop - 8);
      });
    }
  }, [freeAgentBandOrder.length]); // dispara quando a lista carrega

  // ── Restaura o scroll dos painéis após avançar a semana ────────────────────
  // O grid recarrega de forma assíncrona (mostra "Carregando grid..." e esvazia o
  // conteúdo, o que zera o scrollTop). Guardamos a posição antes de avançar e a
  // devolvemos quando o grid termina de recarregar.
  useEffect(() => {
    if (loadingGrid) return;             // espera o grid voltar a ter conteúdo
    const saved = preserveScrollRef.current;
    if (!saved) return;
    preserveScrollRef.current = null;
    requestAnimationFrame(() => {
      if (mainGridRef.current) mainGridRef.current.scrollTop = saved.main;
      if (freeAgentContainerRef.current) freeAgentContainerRef.current.scrollTop = saved.aside;
    });
  }, [loadingGrid]);

  // ── Ações ───────────────────────────────────────────────────────────────────
  const handleAdvanceWeek = async () => {
    if (isAdvancingWeek) return;
    setStartError("");
    if (isComplete) {
      if (playerProposals.length > 0) return;
      if (displacedVeterans.length > 0) {
        setShowDisplacedModal(true);
        return;
      }
      // Jogador sem equipe: exibe aviso antes de confirmar
      if (!preseasonState?.player_has_team) {
        setShowFreeAgentWarning(true);
        return;
      }
      try {
        await finalizePreseason();
      } catch (e) {
        setStartError(typeof e === "string" ? e : e?.message ?? "Erro ao iniciar a temporada.");
      }
    } else {
      // Guarda o scroll atual dos painéis pra restaurar quando o grid recarregar
      // (senão a tela pula pro topo a cada avanço de semana).
      preserveScrollRef.current = {
        main: mainGridRef.current?.scrollTop ?? 0,
        aside: freeAgentContainerRef.current?.scrollTop ?? 0,
      };
      try { await advanceMarketWeek(); } catch (e) {
        preserveScrollRef.current = null;
        console.error(e);
      }
    }
  };

  const handleConfirmStartSeason = async () => {
    setShowDisplacedModal(false);
    setStartError("");
    try { await finalizePreseason(); } catch (e) {
      setStartError(typeof e === "string" ? e : e?.message ?? "Erro ao iniciar a temporada.");
    }
  };

  const handleConfirmFreeAgentStart = async () => {
    setShowFreeAgentWarning(false);
    setStartError("");
    try {
      await finalizePreseason();
    } catch (e) {
      setStartError(typeof e === "string" ? e : e?.message ?? "Erro ao iniciar a temporada.");
    }
  };

  // Janela de Transferências: aceitar uma oferta fecha a semana do jogador e assina.
  // Ao assinar com a equipe nova, repinta o carro do jogador na cor dela (o ID do
  // iRacing já foi capturado/vinculado) — silencioso, só com um toast discreto.
  const handleAcceptOffer = async (offer) => {
    if (isAdvancingWeek) return;
    setStartError("");
    try {
      await advanceMarketWeek(offer?.seat_id);
    } catch (e) {
      console.error(e);
      return;
    }
    if (!offer?.team_color) return;
    try {
      const res = await invoke("iracing_apply_market_paint", {
        careerId,
        teamColor: offer.team_color,
        category: offer.category ?? offer.category_label ?? "",
      });
      if (res) {
        setPaintToast(`🎨 Cor do carro atualizada para a ${offer.team_name ?? "nova equipe"}.`);
        setTimeout(() => setPaintToast(""), 6000);
      }
    } catch (e) {
      console.error("[paint] falha ao repintar no mercado:", e);
    }
  };

  // Propostas formais ("Proposta recebida"): aceitar assina (respond_to_proposal);
  // recusar dispensa. Ao aceitar, repinta o carro na cor da nova equipe (como nas ofertas).
  const handleRespondProposal = async (proposalId, accept, teamColor, category, teamName) => {
    if (isAdvancingWeek) return;
    setStartError("");
    try {
      await respondToProposal(proposalId, accept);
    } catch (e) {
      console.error(e);
      return;
    }
    if (!accept || !teamColor) return;
    try {
      const res = await invoke("iracing_apply_market_paint", {
        careerId,
        teamColor,
        category: category ?? "",
      });
      if (res) {
        setPaintToast(`🎨 Cor do carro atualizada para a ${teamName ?? "nova equipe"}.`);
        setTimeout(() => setPaintToast(""), 6000);
      }
    } catch (e) {
      console.error("[paint] falha ao repintar apos proposta:", e);
    }
  };

  const renderProposalCard = (p) => (
    <article key={p.proposal_id} className="glass animate-scale-in rounded-xl px-4 py-3.5">
      <div className="flex min-w-0 items-center gap-3">
        <TeamLogoMark
          teamName={p.equipe_nome}
          color={p.equipe_cor_primaria}
          size="md"
          testId="player-proposal-team-logo"
        />
        <div className="min-w-0 flex-1">
          <p
            className="text-body-sm font-bold uppercase tracking-[0.16em]"
            style={{ color: p.equipe_cor_primaria }}
          >
            {p.papel} | {p.categoria_nome}
          </p>
          <p className="mt-1 truncate text-title-md">{p.equipe_nome}</p>
        </div>
        {p.semanas_restantes != null && (
          <span className="shrink-0 rounded-full border border-amber-400/30 bg-amber-400/10 px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.14em] text-amber-300">
            {p.semanas_restantes <= 0 ? "última semana" : `expira em ${p.semanas_restantes} sem`}
          </span>
        )}
      </div>

      <div className="my-3 grid grid-cols-2 gap-2">
        <div className="glass-light rounded-lg p-2.5">
          <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
            Salário
          </p>
          <p className="num-medium mt-0.5 font-bold text-[color:var(--status-green)]">
            {formatSalaryMonthly(p.salario_oferecido)}
          </p>
        </div>
        <div className="glass-light rounded-lg p-2.5">
          <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
            Duração
          </p>
          <p className="num-medium mt-0.5 font-bold text-[color:var(--text-primary)]">
            {p.duracao_anos} ano{p.duracao_anos > 1 ? "s" : ""}
          </p>
        </div>
        {p.companheiro_nome && (
          <div className="glass-light rounded-lg p-2.5">
            <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
              Companheiro
            </p>
            <p className="text-body mt-0.5 font-semibold text-[color:var(--text-primary)] truncate">
              {p.companheiro_nome}
              {p.companheiro_skill != null ? ` (${p.companheiro_skill})` : ""}
            </p>
          </div>
        )}
        <div className={`glass-light rounded-lg p-2.5 ${p.companheiro_nome ? "" : "col-span-2"}`}>
          <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
            Carro
          </p>
          <div className="mt-1.5 flex items-center gap-2">
            <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-[#21262d]">
              <div
                className="h-full rounded-full"
                style={{
                  width: `${p.car_performance_rating ?? 0}%`,
                  backgroundColor: p.equipe_cor_primaria,
                }}
              />
            </div>
            <span className="text-body font-bold">{p.car_performance_rating}</span>
          </div>
        </div>
      </div>

      <div className="flex gap-2">
        <button
          onClick={() =>
            handleRespondProposal(
              p.proposal_id,
              true,
              p.equipe_cor_primaria,
              p.categoria,
              p.equipe_nome,
            )
          }
          disabled={isAdvancingWeek}
          className="transition-glass glow-blue flex-1 rounded-lg border border-[#58a6ff66] bg-[#58a6ff33] px-3 py-2 text-body font-bold text-[color:var(--accent-primary)] hover:bg-[#58a6ff55] disabled:cursor-not-allowed disabled:opacity-50"
        >
          Aceitar
        </button>
        <button
          onClick={() => handleRespondProposal(p.proposal_id, false)}
          disabled={isAdvancingWeek}
          className="rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-body font-semibold text-[color:var(--text-secondary)] hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50"
        >
          Recusar
        </button>
      </div>
    </article>
  );

  // ── Render ──────────────────────────────────────────────────────────────────
  return (
    <div className="app-shell relative h-screen w-full overflow-hidden text-[color:var(--text-primary)]">
      <div className="app-backdrop pointer-events-none absolute inset-0" />

      {/* Toast: cor do carro atualizada ao assinar com a nova equipe */}
      {paintToast && (
        <div className="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-xl border border-[#58a6ff44] bg-[#0d1117] px-4 py-2.5 text-sm font-semibold text-[color:var(--text-primary)] shadow-2xl">
          {paintToast}
        </div>
      )}

      <div className="relative z-10 mx-auto flex h-full max-w-[1680px] flex-col px-3 pb-3 pt-3 sm:px-4 lg:px-5 xl:px-6">

        {/* ══ HEADER ══ */}
        <header className="glass-strong animate-fade-in mb-3 rounded-2xl px-5 py-2 lg:px-6">
          <div className="grid items-start gap-3 lg:grid-cols-[1fr_auto]">

            {/* Título + filtros */}
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <p className="text-body-sm font-bold uppercase tracking-[0.28em] text-[color:var(--accent-primary)]">
                  Pré-temporada
                </p>
                {playerOffers.length > 0 && (
                  <span className="glass-light rounded-full px-2.5 py-1 text-body-sm font-bold tracking-[0.14em] text-[color:var(--accent-primary)]">
                    {playerOffers.length} oferta{playerOffers.length > 1 ? "s" : ""}
                  </span>
                )}
              </div>
              <h1 className="mt-1 text-[20px] font-bold leading-[1.05] tracking-[-0.02em] text-[color:var(--text-primary)] lg:text-[26px]">
                {isComplete ? "Janela Encerrada" : "Mercado de Transferências"}
              </h1>

              {/* Filtros de categoria */}
              <div className="mt-2 max-w-full overflow-x-auto">
                <div className="glass inline-flex w-fit items-center gap-0.5 whitespace-nowrap rounded-full p-1">
                  {CATEGORIES.map((cat, i) => {
                    if (cat.isSeparator) {
                      return <span key={i} className="mx-1 h-4 w-px bg-white/10" />;
                    }
                    const active = selectedCat === cat.id;
                    return (
                      <button
                        key={cat.id}
                        onClick={() => setSelectedCat(cat.id)}
                        className={`transition-glass cursor-pointer rounded-full border px-2.5 py-1 text-body-sm font-semibold ${
                          active
                            ? "border-white/30 bg-white/14 text-[color:var(--accent-primary)]"
                            : "border-transparent bg-white/3 text-[color:var(--text-secondary)] hover:bg-white/8 hover:text-[color:var(--text-primary)]"
                        }`}
                      >
                        <span
                          className="mr-2 inline-block h-1.5 w-1.5 rounded-full"
                          style={{ backgroundColor: cat.color }}
                        />
                        {cat.label}
                      </button>
                    );
                  })}
                </div>
              </div>
            </div>

            {/* Status + semana + botão */}
            <div className="flex items-center gap-3 self-center lg:justify-self-end">
              <span
                className={`shrink-0 rounded-full border px-2.5 py-1 text-body-sm font-bold uppercase tracking-[0.14em] ${
                  isMarketOpen
                    ? "border-[#3fb95066] bg-[#3fb9501a] text-[color:var(--status-green)]"
                    : "border-[#d2992266] bg-[#d2992218] text-[color:var(--status-yellow)]"
                }`}
              >
                {isMarketOpen ? "Mercado aberto" : "Janela fechada"}
              </span>

              <div className="w-[220px] px-1 lg:w-[280px]">
                <div className="mb-1 flex items-center justify-between gap-2">
                  <p className="text-body-sm font-bold uppercase tracking-[0.2em] text-[color:var(--text-secondary)]">
                    Semana{" "}
                    <span className="text-[color:var(--text-primary)]">{currentWeek}</span>
                    /{totalWeeks}
                  </p>
                  <p className="text-body-sm text-[color:var(--text-secondary)]">{currentDateLabel}</p>
                </div>
                <div className="h-[3px] w-full rounded-full bg-[#2a3240]">
                  <div
                    className="h-full rounded-full bg-[color:var(--accent-primary)] transition-all duration-500"
                    style={{ width: `${weekProgress}%` }}
                  />
                </div>
              </div>

              <button
                onClick={handleAdvanceWeek}
                disabled={isAdvancingWeek || (isComplete && playerProposals.length > 0)}
                className={`transition-glass rounded-full border px-6 py-2.5 text-body-lg font-bold uppercase tracking-[0.16em] disabled:cursor-not-allowed disabled:opacity-50 ${
                  isComplete
                    ? "border-[#3fb95099] bg-[#3fb950] text-[#06101f] hover:bg-[#52d16a]"
                    : "glow-blue border-[#58a6ff99] bg-[#58a6ff] text-[#06101f] hover:bg-[#79b8ff]"
                }`}
              >
                {isAdvancingWeek
                  ? "Processando..."
                  : isComplete
                    ? "Iniciar Temporada"
                    : "Avançar Semana"}
              </button>
            </div>
          </div>
          {startError && (
            <p className="mt-2 text-center text-body-sm text-[color:var(--status-red)]">{startError}</p>
          )}
        </header>

        {/* ══ 3 COLUNAS ══ */}
        <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 xl:grid-cols-[20%_62%_18%]">

          {/* ── ESQUERDA: Mercado de Pilotos ── */}
          <aside ref={freeAgentContainerRef} className="glass-strong scroll-area animate-edge-rail-in min-h-0 overflow-y-auto rounded-2xl px-3 py-4 lg:px-4 lg:py-5">
            <div className="mb-4 flex h-6 items-center justify-between">
              <p className="text-body-sm font-bold uppercase tracking-[0.22em] text-[color:var(--accent-primary)]">
                Mercado de Pilotos
              </p>
              {(preseasonFreeAgents ?? []).length > 0 && (
                <span className="text-body-sm text-[color:var(--text-muted)]">
                  {visibleFreeAgentCount} {selectedCat === "all" ? "livres" : "elegíveis"}
                </span>
              )}
            </div>

            {(preseasonFreeAgents ?? []).length === 0 ? (
              <div className="py-10 text-center text-body text-[color:var(--text-muted)]">
                Todos os pilotos têm equipe.
              </div>
            ) : freeAgentBandOrder.length === 0 ? (
              <div className="py-10 text-center text-body text-[color:var(--text-muted)]">
                Nenhum piloto livre elegível para esta categoria.
              </div>
            ) : (
              <div className="space-y-4">
                {freeAgentBandOrder.map((band) => {
                  const drivers = freeAgentsByBand[band.key];
                  return (
                    <section key={band.key} ref={(el) => { freeAgentSectionRefs.current[band.key] = el; }}>
                      <div className="mb-1.5 flex items-center gap-2">
                        <span
                          className="h-2 w-2 shrink-0 rounded-[3px]"
                          style={{ background: band.color, boxShadow: `0 0 8px ${band.color}88` }}
                        />
                        <span
                          className="text-[10px] font-black uppercase tracking-[0.2em]"
                          style={{ color: band.color }}
                        >
                          {band.label}
                        </span>
                        <div
                          className="h-px flex-1"
                          style={{ background: `linear-gradient(to right, ${band.color}55, transparent)` }}
                        />
                        <span className="text-[9px] text-[color:var(--text-muted)]">
                          {drivers.length}
                        </span>
                      </div>
                      <div className="space-y-2.5">
                        {(() => {
                          // Sub-agrupa por marca/categoria (drivers já ordenado por marca).
                          // Separador físico entre marcas quando a banda tem mais de uma
                          // (Amador/Pro/Rookie) — senão Toyota e Mazda, de mesma cor, embolam.
                          const groups = [];
                          drivers.forEach((d) => {
                            const last = groups[groups.length - 1];
                            if (last && last.cat === d.categoria) last.list.push(d);
                            else groups.push({ cat: d.categoria, list: [d] });
                          });
                          const multiBrand = groups.length > 1;
                          return groups.map(({ cat, list }) => (
                            <div key={cat} className="space-y-1">
                              {multiBrand && (
                                <div className="flex items-center gap-2 px-0.5 pt-0.5">
                                  <span
                                    className="text-[8px] font-bold uppercase tracking-[0.18em]"
                                    style={{ color: subcatColor(cat) }}
                                  >
                                    {shortDestLabel(cat)}
                                  </span>
                                  <div
                                    className="h-px flex-1"
                                    style={{ background: `linear-gradient(to right, ${subcatColor(cat)}44, transparent)` }}
                                  />
                                </div>
                              )}
                              {list.map((d) => (
                                <FreeAgentCard
                                  key={d.driver_id}
                                  driver={d}
                                  onHoverCat={setHoveredFreeAgentCat}
                                  isRookie={d.is_rookie}
                                />
                              ))}
                            </div>
                          ));
                        })()}
                      </div>
                    </section>
                  );
                })}
              </div>
            )}
          </aside>

          {/* ── CENTRO: Grid de Equipes ── */}
          <main ref={mainGridRef} className="glass scroll-area animate-fade-in min-h-0 overflow-y-auto rounded-2xl px-5 py-4 lg:px-6 lg:py-5">
            <div className="mb-5 flex h-6 items-center justify-between">
              <p className="text-body-sm font-bold uppercase tracking-[0.2em] text-[color:var(--text-secondary)]">
                Mapeamento das equipes
              </p>
              <p className="text-body text-[color:var(--text-muted)]">Classificação anterior</p>
            </div>

            {loadingGrid ? (
              <div className="py-20 text-center text-body text-[color:var(--text-muted)]">
                Carregando grid...
              </div>
            ) : gridData.length === 0 ? (
              <div className="py-20 text-center text-body text-[color:var(--text-muted)]">
                Nenhuma equipe encontrada.
              </div>
            ) : (
              <div className="space-y-3">
                {sortedClasses.map((teamClass, classIndex) => {
                  const teams = [...groupedTeams[teamClass]].sort((a, b) => {
                    const movementOrderDiff = getTeamMovementOrder(a) - getTeamMovementOrder(b);
                    if (movementOrderDiff !== 0) return movementOrderDiff;

                    const previousPositionDiff = getTeamMappingSortValue(a) - getTeamMappingSortValue(b);
                    if (previousPositionDiff !== 0) return previousPositionDiff;

                    return a.nome.localeCompare(b.nome);
                  });
                  const accent = subcatColor(teamClass);
                  const totalVacancies = teams.reduce((sum, team) => sum + count_team_vacancies(team), 0);
                  const startsRookieBlock =
                    classIndex > 0
                    && isRealCareerDebutCategory(teamClass)
                    && !isRealCareerDebutCategory(sortedClasses[classIndex - 1]);
                  const sectionSpacing = classIndex === 0 ? "" : startsRookieBlock ? "mt-14" : "mt-10";

                  return (
                    <section key={teamClass} className={sectionSpacing}>
                      <MarketCategoryHeader
                        categoryKey={teamClass}
                        detail={`${totalVacancies} ${totalVacancies === 1 ? "vaga" : "vagas"}`}
                      />

                      {MULTICLASS_ORDER[teamClass] ? (
                        <div className="space-y-5">
                          {(() => {
                            const order = MULTICLASS_ORDER[teamClass];
                            const byClass = new Map();
                            for (const team of teams) {
                              const cls = team.classe || "outras";
                              if (!byClass.has(cls)) byClass.set(cls, []);
                              byClass.get(cls).push(team);
                            }
                            const orderedClasses = [...byClass.keys()].sort((a, b) => {
                              const ia = order.indexOf(a);
                              const ib = order.indexOf(b);
                              if (ia !== -1 && ib !== -1) return ia - ib;
                              if (ia !== -1) return -1;
                              if (ib !== -1) return 1;
                              return a.localeCompare(b);
                            });
                            return orderedClasses.map((cls) => {
                              const clsColor = subcatColor(cls);
                              // Cor do MENU (divisor): puxa o tom da categoria-pai (roxo
                              // no Production, verde no Endurance); cai na cor da classe
                              // fora das multiclasses. As equipes seguem com a cor delas.
                              const dividerColor = MULTICLASS_SUBCLASS_TONES[teamClass]?.[cls] ?? clsColor;
                              const clsTeams = byClass.get(cls);
                              return (
                                <div key={cls}>
                                  {/* Divisor GRANDE centralizado por classe (carro). */}
                                  <div className="mb-4 mt-1 flex items-center gap-4">
                                    <div
                                      className="h-px flex-1"
                                      style={{ background: `linear-gradient(to right, transparent, ${dividerColor}88)` }}
                                    />
                                    <div className="flex flex-col items-center">
                                      <span
                                        className="text-[24px] font-black uppercase leading-none tracking-[0.22em]"
                                        style={{ color: dividerColor, textShadow: `0 0 22px ${dividerColor}55` }}
                                      >
                                        {CLASS_LABELS[cls] ?? cls.toUpperCase()}
                                      </span>
                                      <span className="mt-1.5 text-[9px] font-semibold uppercase tracking-[0.16em] text-[color:var(--text-muted)]">
                                        {clsTeams.length} {clsTeams.length === 1 ? "equipe" : "equipes"}
                                      </span>
                                    </div>
                                    <div
                                      className="h-px flex-1"
                                      style={{ background: `linear-gradient(to left, transparent, ${dividerColor}88)` }}
                                    />
                                  </div>
                                  {/* accent do card = tom da categoria (roxo/verde): pinta as
                                      pills "Vaga aberta" e o glow de hover; o logo do time
                                      segue com a cor_primaria própria. */}
                                  <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
                                    {clsTeams.map((team) => renderTeamCard(team, dividerColor))}
                                  </div>
                                </div>
                              );
                            });
                          })()}
                        </div>
                      ) : (
                        <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
                          {teams.map((team) => renderTeamCard(team, accent))}
                        </div>
                      )}
                    </section>
                  );
                })}
              </div>
            )}
          </main>

          {/* ── DIREITA: Decisões Pendentes ── */}
          {/* min-h-0 + overflow-y-auto (igual às outras 2 colunas): fica preso à altura
              da linha do grid e rola até o fim. NÃO usar self-start + max-h fixo, senão
              o painel cresce com o conteúdo e o fundo some sob o overflow-hidden do shell. */}
          <aside className="glass scroll-area animate-drawer-in min-h-0 overflow-y-auto rounded-2xl px-4 py-4 lg:px-5 lg:py-5">
            {/* Propostas formais: equipes que cortejam o jogador por mérito (com prazo). */}
            {playerProposals.length > 0 && (
              <div className="mb-5">
                <div className="mb-4 flex h-6 items-center gap-2">
                  <span className="relative inline-flex h-2.5 w-2.5">
                    <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-amber-400/80" />
                    <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-amber-400" />
                  </span>
                  <p className="text-body-sm font-bold uppercase tracking-[0.22em] text-amber-300">
                    Propostas recebidas
                  </p>
                </div>
                <div className="space-y-3">
                  <p className="text-body-sm text-[color:var(--text-secondary)]">
                    Equipes que querem VOCÊ. Aceitar assina o contrato; recusar dispensa. Cada uma expira em algumas semanas.
                  </p>
                  {playerProposals.map(renderProposalCard)}
                </div>
              </div>
            )}

            <div className="mb-4 flex h-6 items-center gap-2">
              <span className="relative inline-flex h-2.5 w-2.5">
                {playerOffers.length > 0 && (
                  <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-[#58a6ff]/80" />
                )}
                <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-[color:var(--accent-primary)]" />
              </span>
              <p className="text-body-sm font-bold uppercase tracking-[0.22em] text-[color:var(--accent-primary)]">
                Suas ofertas
              </p>
            </div>

            {playerOffers.length === 0 ? (
              <div className="glass-light rounded-xl border-dashed p-6 text-center text-body text-[color:var(--text-secondary)]">
                {playerSignedThisWindow
                  ? "Você já tem contrato. Avance as semanas até a janela fechar."
                  : isComplete
                    ? "Janela de transferências fechada."
                    : "Nenhuma oferta nova esta semana. Avance a semana para esperar — um time ainda pode aparecer."}
              </div>
            ) : (
              <div className="space-y-4">
                <p className="text-body-sm text-[color:var(--text-secondary)]">
                  {totalOffers} vaga{totalOffers > 1 ? "s" : ""} te querendo esta semana. Toque numa categoria para ver as fichas das equipes.
                </p>

                {/* ANDAR 1 — Promoção: destaque. Card grande, verde, no topo. */}
                {promoOfferGroups.length > 0 && (
                  <div className="space-y-2">
                    {promoOfferGroups.map((group) => {
                      const n = group.n1.length + group.n2.length;
                      return (
                        <button
                          key={group.cat}
                          type="button"
                          onClick={() => { setOffersModalCat(group.cat); setShowOffersModal(true); }}
                          data-testid={`offer-category-row-${group.cat}`}
                          className="transition-glass glow-green group flex w-full items-center gap-3 rounded-xl border border-[color:var(--status-green)]/45 bg-[color:var(--status-green)]/10 px-4 py-3.5 text-left hover:bg-[color:var(--status-green)]/16"
                        >
                          <span className="text-[20px] leading-none">⭐</span>
                          <span className="min-w-0 flex-1">
                            <span className="block text-[9px] font-black uppercase tracking-[0.22em] text-[color:var(--status-green)]">
                              Promoção
                            </span>
                            <span className="mt-0.5 block truncate text-title-md font-black">
                              {group.label}
                            </span>
                            <span className="block text-[10px] uppercase tracking-[0.16em] text-[color:var(--text-muted)]">
                              {n} vaga{n > 1 ? "s" : ""} · subir de categoria
                            </span>
                          </span>
                          <span className="text-title-md text-[color:var(--status-green)] transition-transform group-hover:translate-x-0.5">›</span>
                        </button>
                      );
                    })}
                  </div>
                )}

                {/* ANDAR 2 — Sua marca: agrupada, fileira densa. */}
                {brandOfferGroups.length > 0 && (
                  <div className="space-y-1.5">
                    <p className="px-0.5 text-[9px] font-black uppercase tracking-[0.22em] text-[color:var(--text-muted)]">
                      Continuar na {CLASS_LABELS[playerBrand] ?? playerBrand}
                    </p>
                    {brandOfferGroups.map(renderOfferRowDense)}
                  </div>
                )}

                {/* ANDAR 3 — Outras oportunidades: fileira densa, sem chip. */}
                {otherOfferGroups.length > 0 && (
                  <div className="space-y-1.5">
                    <p className="px-0.5 text-[9px] font-black uppercase tracking-[0.22em] text-[color:var(--text-muted)]">
                      Outras oportunidades
                    </p>
                    {otherOfferGroups.map(renderOfferRowDense)}
                  </div>
                )}

                <button
                  type="button"
                  onClick={() => { setOffersModalCat(null); setShowOffersModal(true); }}
                  className="transition-glass glow-blue mt-1 w-full rounded-xl border border-[#58a6ff66] bg-[#58a6ff22] px-3 py-2.5 text-body font-bold text-[color:var(--accent-primary)] hover:bg-[#58a6ff44]"
                >
                  Ver ofertas ({totalOffers})
                </button>
              </div>
            )}

            <div
              data-testid="weekly-closing-market"
              className="mt-4 rounded-xl border border-white/8 bg-black/18 px-4 py-4"
            >
              <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[color:var(--text-muted)]">
                Fechamento da semana
              </p>
              {weeklyClosingGroups.length ? (
                <div className="mt-3 space-y-3">
                  {weeklyClosingGroups.map((group) => (
                    <section key={group.category} className="space-y-2">
                      <div
                        className="flex items-center justify-center rounded-lg border px-3 py-2"
                        style={{
                          borderColor: `${group.color}30`,
                          background: `linear-gradient(135deg, ${group.color}16 0%, rgba(255,255,255,0.025) 100%)`,
                        }}
                      >
                        <p
                          className="text-center text-[11px] font-black uppercase tracking-[0.16em]"
                          style={{ color: group.color }}
                        >
                          {group.label}
                        </p>
                      </div>
                      <div className="space-y-2">
                        {group.events.map((event, index) => (
                          <WeeklyClosingMovement
                            key={`${event.event_type}-${event.driver_id ?? event.driver_name}-${index}`}
                            event={event}
                            color={group.color}
                            onSelect={setTransferDetail}
                          />
                        ))}
                      </div>
                    </section>
                  ))}
                </div>
              ) : (
                <p className="mt-2 text-body text-[color:var(--text-secondary)]">
                  As movimentações do mercado vão aparecer aqui após avançar a semana.
                </p>
              )}
            </div>
          </aside>

        </div>
      </div>

      {/* ══ MODAL: Suas ofertas (fichas das equipes) ══ */}
      {showOffersModal && totalOffers > 0 && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm"
          onClick={(e) => { if (e.target === e.currentTarget) setShowOffersModal(false); }}
        >
          {(() => {
          const modalGroups = offersModalCat
            ? offersByCategory.filter((g) => g.cat === offersModalCat)
            : offersByCategory;
          const modalCount = modalGroups.reduce((sum, g) => sum + g.n1.length + g.n2.length, 0);
          const modalCatLabel = offersModalCat ? modalGroups[0]?.label : null;
          return (
          <div className="glass-strong animate-fade-in flex max-h-[90vh] w-full max-w-5xl flex-col rounded-2xl">
            <div className="flex items-start justify-between gap-4 border-b border-white/8 px-6 py-5">
              <div>
                <div className="text-body-sm font-bold uppercase tracking-[0.22em] text-[color:var(--accent-primary)]">
                  {modalCatLabel ? `Ofertas · ${modalCatLabel}` : "Suas ofertas"}
                </div>
                <h2 className="mt-1 text-[18px] font-bold leading-tight text-[color:var(--text-primary)]">
                  {modalCount} vaga{modalCount > 1 ? "s" : ""} te querendo
                </h2>
                <p className="mt-1 text-body-sm text-[color:var(--text-secondary)]">
                  Aceitar fecha sua semana e assina o contrato. Avançar sem aceitar = esperar por algo melhor (risco: a vaga pode sumir).
                </p>
                {offersModalCat && offersByCategory.length > 1 && (
                  <button
                    type="button"
                    onClick={() => setOffersModalCat(null)}
                    className="mt-2 text-body-sm font-semibold text-[color:var(--accent-primary)] hover:underline"
                  >
                    Ver todas as ofertas ({totalOffers})
                  </button>
                )}
              </div>
              <button
                type="button"
                onClick={() => setShowOffersModal(false)}
                className="transition-glass glass-light shrink-0 rounded-lg px-3 py-2 text-body font-bold text-[color:var(--text-secondary)] hover:text-[color:var(--text-primary)]"
                aria-label="Fechar"
              >
                ✕
              </button>
            </div>

            <div className="scroll-area space-y-5 overflow-y-auto px-6 py-5">
              {modalGroups.map((group) => {
                const n = group.n1.length + group.n2.length;
                const isPromotion = playerTier != null && group.tier > playerTier;
                const accent = subcatColor(group.cat);
                return (
                  <section key={group.cat} className="space-y-3">
                    <div className="flex items-center gap-3">
                      <span
                        className="text-body font-black uppercase tracking-[0.16em]"
                        style={{ color: accent }}
                      >
                        {group.label}
                      </span>
                      {isPromotion && (
                        <span className="rounded-md bg-[rgba(63,185,80,0.14)] px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.14em] text-[color:var(--status-green)]">
                          ↑ Promoção
                        </span>
                      )}
                      <div className="h-px flex-1" style={{ background: `linear-gradient(to right, ${accent}55, transparent)` }} />
                      <span className="text-body-sm text-[color:var(--text-muted)]">
                        {n} vaga{n > 1 ? "s" : ""}
                      </span>
                    </div>
                    {[["Piloto Nº 1", group.n1], ["Piloto Nº 2", group.n2]].map(([roleLabel, list]) =>
                      list.length === 0 ? null : (
                        <div key={roleLabel} className="space-y-3">
                          <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
                            {roleLabel}
                          </p>
                          <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
                            {list.map(renderOfferCardRich)}
                          </div>
                        </div>
                      ),
                    )}
                  </section>
                );
              })}
            </div>
          </div>
          );
          })()}
        </div>
      )}

      {/* ══ MODAL: Contrato — documento A4 de assinatura ══ */}
      {contractOffer && (() => {
        const offer = contractOffer;
        const accent = offer.team_color || "#58a6ff";
        const countryLabel = extractNationalityLabel(offer.team_country) || offer.team_country || "";
        const bondLevel = offer.bond_level ?? 1;
        const bondColor = BOND_LEVEL_COLORS[Math.min(bondLevel, BOND_LEVEL_COLORS.length) - 1];
        const hasHistory = bondLevel >= 2;
        const dur = offer.offer_duration ?? 1;
        const isProject = dur >= 2;
        const docRef = String(offer.seat_id ?? "").replace(/[^a-zA-Z0-9]/g, "").slice(-6).toUpperCase() || "000000";
        const signName = playerName || "Piloto";
        // Paleta do documento (folha escura, tinta clara — combina com o resto do app).
        const paper = "#0e1319";
        const ink = "var(--text-primary)";
        const inkSoft = "var(--text-secondary)";
        const inkMute = "var(--text-muted)";
        const hair = "rgba(255,255,255,0.08)";
        const money = "var(--status-green)";
        return (
          <div
            className="fixed inset-0 z-[60] flex items-center justify-center bg-black/75 p-4 backdrop-blur-sm"
            onClick={(e) => { if (isSigning) return; if (e.target === e.currentTarget) setContractOffer(null); }}
          >
            <div
              className="animate-scale-in relative flex max-h-[94vh] w-full max-w-[600px] flex-col overflow-hidden rounded-[14px] shadow-[0_40px_120px_-24px_rgba(0,0,0,0.85)] ring-1 ring-white/10"
              style={{ background: paper }}
            >
              {/* Faixa de cor da equipe no topo da folha */}
              <div className="h-1.5 w-full shrink-0" style={{ background: accent }} />

              {/* Botão fechar (canto) */}
              <button
                type="button"
                onClick={() => setContractOffer(null)}
                disabled={isSigning}
                className="absolute right-3 top-4 z-10 rounded-lg bg-white/5 px-3 py-2 text-body font-bold transition-colors hover:bg-white/10 disabled:opacity-40"
                style={{ color: inkSoft }}
                aria-label="Fechar"
              >
                ✕
              </button>

              {/* Folha rolável, com moldura interna estilo documento */}
              <div className="scroll-area relative flex-1 overflow-y-auto">
                {/* Marca d'água: logo gigante da equipe ao fundo */}
                <div className="pointer-events-none absolute inset-0 flex items-center justify-center opacity-[0.04]">
                  <div className="scale-[3.2]">
                    <TeamLogoMark teamName={offer.team_name} color="#ffffff" size="hero" />
                  </div>
                </div>

                {/* Moldura interna (margem do documento) */}
                <div className="pointer-events-none absolute inset-4 rounded-md border" style={{ borderColor: hair }} />

                <div className="relative px-8 py-8 sm:px-10">
                  {/* ── Timbre: logo + identidade da equipe ── */}
                  <header className="flex flex-col items-center text-center">
                    <TeamLogoMark teamName={offer.team_name} color={accent} size="lg" />
                    <h2 className="mt-3 text-[22px] font-black leading-tight" style={{ color: ink }}>
                      {offer.team_name}
                    </h2>
                    <div className="mt-1 flex items-center justify-center gap-1.5 text-[11px]" style={{ color: inkMute }}>
                      <FlagIcon nacionalidade={offer.team_country} className="h-3.5 w-5" />
                      {countryLabel && <span>{countryLabel}</span>}
                      {offer.team_founded_year ? <span>· desde {offer.team_founded_year}</span> : null}
                    </div>
                  </header>

                  {/* ── Título do documento ── */}
                  <div className="mt-6 flex items-center gap-3">
                    <div className="h-px flex-1" style={{ background: `linear-gradient(to right, transparent, ${accent}88)` }} />
                    <div className="text-center">
                      <p className="text-[15px] font-black uppercase tracking-[0.3em]" style={{ color: ink }}>
                        Contrato de Piloto
                      </p>
                      <p className="mt-1 text-[9px] font-semibold uppercase tracking-[0.24em]" style={{ color: accent }}>
                        {offer.category_label || offer.category} · Ref. {docRef}
                      </p>
                    </div>
                    <div className="h-px flex-1" style={{ background: `linear-gradient(to left, transparent, ${accent}88)` }} />
                  </div>

                  {/* ── Preâmbulo ── */}
                  <p className="mt-5 text-[12px] leading-relaxed" style={{ color: inkSoft }}>
                    <span className="font-bold" style={{ color: ink }}>{offer.team_name}</span>
                    {" "}(a Equipe) e <span className="font-bold" style={{ color: ink }}>{signName}</span>{" "}
                    (o Piloto) firmam o presente contrato de pilotagem, nos termos das cláusulas a seguir:
                  </p>

                  {/* ── Cláusulas ── */}
                  <div className="mt-5 space-y-0">
                    {/* I — Função */}
                    <div className="py-3.5" style={{ borderTop: `1px solid ${hair}` }}>
                      <div className="flex items-baseline justify-between gap-3">
                        <p className="text-[10px] font-black uppercase tracking-[0.2em]" style={{ color: inkMute }}>
                          Cláusula I · Função
                        </p>
                        <p className="text-right text-body font-bold" style={{ color: accent }}>
                          {offer.role === "N1" ? "Piloto Titular (N1)" : "Segundo Piloto (N2)"}
                        </p>
                      </div>
                    </div>

                    {/* II — Remuneração */}
                    <div className="py-3.5" style={{ borderTop: `1px solid ${hair}` }}>
                      <div className="flex items-baseline justify-between gap-3">
                        <p className="text-[10px] font-black uppercase tracking-[0.2em]" style={{ color: inkMute }}>
                          Cláusula II · Remuneração
                        </p>
                        <p className="num-medium text-title-md font-black" style={{ color: money }}>
                          {formatSalaryMonthly(offer.salary)}
                        </p>
                      </div>
                    </div>

                    {/* III — Vigência */}
                    <div className="py-3.5" style={{ borderTop: `1px solid ${hair}` }}>
                      <div className="flex items-baseline justify-between gap-3">
                        <p className="text-[10px] font-black uppercase tracking-[0.2em]" style={{ color: inkMute }}>
                          Cláusula III · Vigência
                        </p>
                        <p className="num-medium text-body font-bold" style={{ color: isProject ? money : ink }}>
                          {dur} temporada{dur > 1 ? "s" : ""}
                        </p>
                      </div>
                    </div>

                    {/* IV — Projeto esportivo (foco do time) */}
                    <div className="py-3.5" style={{ borderTop: `1px solid ${hair}` }}>
                      <div className="flex items-baseline justify-between gap-3">
                        <p className="text-[10px] font-black uppercase tracking-[0.2em]" style={{ color: inkMute }}>
                          Cláusula IV · Projeto Esportivo
                        </p>
                        <p className="text-right text-body font-bold" style={{ color: accent }}>
                          {offer.team_focus || "Meio de grid"}
                        </p>
                      </div>
                    </div>

                    {/* V — Relação com a equipe (vínculo) */}
                    <div className="py-3.5" style={{ borderTop: `1px solid ${hair}`, borderBottom: `1px solid ${hair}` }}>
                      <div className="flex items-center justify-between gap-3">
                        <p className="text-[10px] font-black uppercase tracking-[0.2em]" style={{ color: inkMute }}>
                          Cláusula V · Relação com a Equipe
                        </p>
                        <div className="flex items-center gap-2">
                          <div className="flex gap-0.5">
                            {Array.from({ length: 6 }).map((_, i) => (
                              <span
                                key={i}
                                className="h-2 w-4 rounded-full"
                                style={{ background: i < bondLevel ? bondColor : "#21262d" }}
                              />
                            ))}
                          </div>
                          <span className="text-body-sm font-bold" style={{ color: hasHistory ? bondColor : inkMute }}>
                            {offer.bond_label || "Recém-chegado"}
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* ── Área de assinatura ── */}
                  <div className="mt-8 grid grid-cols-2 gap-8">
                    {/* Piloto (você) — assinado com animação manuscrita ao aceitar */}
                    <div>
                      <div
                        className="flex h-14 items-end justify-center overflow-hidden border-b-2 border-dashed"
                        style={{ borderColor: `${accent}88` }}
                      >
                        {isSigning ? (
                          <span
                            className="animate-signature truncate pb-0.5 text-[24px] leading-none"
                            style={{ fontFamily: "'Segoe Script','Brush Script MT','Comic Sans MS',cursive", color: accent }}
                          >
                            {signName}
                          </span>
                        ) : (
                          <span className="pb-1.5 text-[11px] italic" style={{ color: inkMute }}>
                            assine ao aceitar ↓
                          </span>
                        )}
                      </div>
                      <p className="mt-2 text-center text-[9px] uppercase tracking-[0.22em]" style={{ color: inkMute }}>
                        Piloto
                      </p>
                      <p className="text-center text-[12px] font-bold" style={{ color: ink }}>{signName}</p>
                    </div>
                    {/* Equipe (já assinado) */}
                    <div>
                      <div className="flex h-14 items-end justify-center border-b-2" style={{ borderColor: "rgba(255,255,255,0.25)" }}>
                        <span
                          className="truncate pb-0.5 text-[24px] leading-none"
                          style={{ fontFamily: "'Segoe Script','Brush Script MT','Comic Sans MS',cursive", color: accent }}
                        >
                          {offer.team_name}
                        </span>
                      </div>
                      <p className="mt-2 text-center text-[9px] uppercase tracking-[0.22em]" style={{ color: inkMute }}>
                        Equipe
                      </p>
                      <p className="text-center text-[12px] font-bold" style={{ color: ink }}>{offer.team_name}</p>
                    </div>
                  </div>
                </div>
              </div>

              {/* ── Rodapé de ações ── */}
              <div className="flex shrink-0 gap-3 border-t border-white/10 bg-black/30 px-6 py-4">
                <button
                  type="button"
                  onClick={() => setContractOffer(null)}
                  disabled={isSigning}
                  className="transition-glass glass-light rounded-lg px-4 py-2.5 text-body font-bold text-[color:var(--text-secondary)] hover:text-[color:var(--text-primary)] disabled:cursor-not-allowed disabled:opacity-40"
                >
                  Voltar
                </button>
                <button
                  type="button"
                  onClick={() => {
                    if (isSigning) return;
                    // Escreve a assinatura (~1.25s) e só então efetiva a oferta.
                    setIsSigning(true);
                    setTimeout(() => {
                      setContractOffer(null);
                      setShowOffersModal(false);
                      handleAcceptOffer(offer);
                    }, 1550);
                  }}
                  disabled={isAdvancingWeek || isSigning}
                  className="transition-glass glow-blue flex flex-1 items-center justify-center gap-2 rounded-lg border border-[#58a6ff66] bg-[#58a6ff33] px-4 py-2.5 text-body font-bold text-[color:var(--accent-primary)] hover:bg-[#58a6ff55] disabled:cursor-not-allowed disabled:opacity-60"
                >
                  <span className="text-[15px] leading-none">✒️</span>
                  {isSigning ? "Assinando…" : "Assinar contrato"}
                </button>
              </div>
            </div>
          </div>
        );
      })()}

      {/* ══ OVERLAY: Histórico mundial de equipes (atlas, duplo clique no card) ══ */}
      {historyTeam ? (
        <div
          className="fixed inset-0 z-[80] overflow-y-auto bg-black/80 backdrop-blur-md"
          onClick={() => setHistoryTeam(null)}
        >
          <div
            className="mx-auto max-w-7xl px-4 py-6 sm:px-6"
            onClick={(e) => e.stopPropagation()}
          >
            <GlobalTeamsTab
              selectedTeamId={historyTeam.id}
              selectedTeamCategory={historyTeam._categoria ?? historyTeam.categoria ?? null}
              selectedTeamClassName={historyTeam.classe ?? null}
              initialZoomYears={10}
              pinnedTeamId={historyTeam.id}
              drawerPlacement="center"
              onBack={() => setHistoryTeam(null)}
            />
          </div>
        </div>
      ) : null}

      {/* ══ MODAL: Pilotos sem vaga ══ */}
      {showDisplacedModal && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
          onClick={(e) => { if (e.target === e.currentTarget) setShowDisplacedModal(false); }}
        >
          <div className="glass-strong animate-fade-in mx-4 w-full max-w-4xl rounded-2xl p-6 md:p-7">
            <div className="mb-1 text-body-sm font-bold uppercase tracking-[0.22em] text-[#f85149]">
              Fim da pré-temporada
            </div>
            <h2 className="mb-1 text-[18px] font-bold leading-tight text-[color:var(--text-primary)]">
              Pilotos sem vaga
            </h2>
            <p className="mb-5 text-body text-[color:var(--text-secondary)]">
              {displacedVeterans.length === 1
                ? "Este piloto encerrou a pré-temporada sem equipe."
                : `Estes ${displacedVeterans.length} pilotos encerraram a pré-temporada sem equipe.`}
            </p>

            <div className="mb-6 max-h-[70vh] space-y-4 overflow-y-auto pr-1">
              {displacedVeteransByCategory.map((group) => (
                <section key={group.category} className="space-y-2.5">
                  <div
                    className="flex items-center gap-3 rounded-xl px-3 py-2"
                    style={{
                      background: `linear-gradient(135deg, ${group.color}22 0%, rgba(255,255,255,0.03) 100%)`,
                      borderLeft: `3px solid ${group.color}`,
                    }}
                  >
                    <span
                      className="text-[13px] font-bold uppercase tracking-[0.16em]"
                      style={{ color: group.color }}
                    >
                      {group.label}
                    </span>
                    <div
                      className="h-px flex-1"
                      style={{ background: `linear-gradient(to right, ${group.color}55, transparent)` }}
                    />
                    <span className="text-body-sm text-[color:var(--text-muted)]">
                      {group.drivers.length}
                    </span>
                  </div>

                  <div className="grid grid-cols-1 gap-2 xl:grid-cols-2">
                    {group.drivers.map((d) => {
                      const lic = LICENSE_COLORS[d.license_sigla] ?? LICENSE_COLORS.R;
                      const licenseTitle = licenseTooltip(d.license_sigla);
                      const lastChampionshipResult = formatLastChampionshipResult(d);

                      return (
                        <div
                          key={d.driver_id}
                          className="flex items-center gap-3 rounded-xl px-3.5 py-3 shadow-[0_10px_24px_rgba(0,0,0,0.18)]"
                          style={{
                            background: "rgba(8, 13, 24, 0.76)",
                            border: "1px solid rgba(255, 255, 255, 0.12)",
                            boxShadow:
                              "inset 0 1px 0 rgba(255,255,255,0.05), 0 10px 24px rgba(0,0,0,0.18)",
                          }}
                        >
                          <div className="min-w-0 flex-1">
                            <div className="flex flex-wrap items-center gap-2">
                              <p className="text-[17px] font-bold leading-tight text-[color:var(--text-primary)]">
                                {d.driver_name}
                              </p>
                            </div>
                            <div className="mt-1.5 space-y-0.5 text-body-sm text-[color:var(--text-muted)]">
                              {d.previous_team_name && d.seasons_at_last_team > 0 && (
                                <div className="min-w-0">
                                  <div className="text-[10px] font-bold uppercase tracking-[0.14em] text-[color:var(--text-muted)]">
                                    Ex-equipe
                                  </div>
                                  <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-1">
                                    <TeamLogoMark
                                      teamName={d.previous_team_name}
                                      color={d.previous_team_color}
                                      size="xs"
                                      testId="displaced-driver-previous-team-logo"
                                    />
                                    <span
                                      className="block truncate text-[14px] font-semibold"
                                      style={{ color: d.previous_team_color ?? "var(--text-secondary)" }}
                                    >
                                      {d.previous_team_name}
                                    </span>
                                    {lastChampionshipResult && (
                                      <span className="text-[13px] font-bold text-[color:var(--text-secondary)]">
                                        {`• ${lastChampionshipResult}`}
                                      </span>
                                    )}
                                  </div>
                                  <span className="text-[12px]">{`${d.seasons_at_last_team} ${d.seasons_at_last_team === 1 ? "temporada" : "temporadas"}`}</span>
                                </div>
                              )}
                            </div>
                          </div>
                          <span
                            aria-label={licenseTitle}
                            className="shrink-0 rounded-lg px-2 py-1.5 text-[11px] text-[10px] font-black uppercase tracking-[0.12em] min-w-[3.25rem] min-w-[2.4rem] text-center shadow-[inset_0_1px_0_rgba(255,255,255,0.18)]"
                            style={{ background: lic.bg, color: lic.text }}
                            title={licenseTitle}
                          >
                            {d.license_sigla}
                          </span>
                        </div>
                      );
                    })}
                  </div>
                </section>
              ))}
            </div>

            <div className="flex gap-3">
              <button
                onClick={() => setShowDisplacedModal(false)}
                className="transition-glass flex-1 rounded-xl border border-white/15 bg-white/5 px-4 py-2.5 text-body font-semibold text-[color:var(--text-secondary)] hover:bg-white/10"
              >
                Voltar
              </button>
              <button
                onClick={handleConfirmStartSeason}
                className="transition-glass glow-blue flex-1 rounded-xl border border-[#3fb95099] bg-[#3fb950] px-4 py-2.5 text-body font-bold text-[#06101f] hover:bg-[#52d16a]"
              >
                Iniciar Temporada
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ══ MODAL: Detalhe da transferência ══ */}
      {transferDetail && (() => {
        const ev = transferDetail;
        const badge = WEEKLY_MARKET_MOVEMENT_BADGES[ev.movement_kind];
        const emphasis = RELATION_EMPHASIS[ev.relation];
        const isDebut = !ev.from_team;
        const fromTeam = ev.from_team;
        const toTeam = ev.to_team || ev.team_name;
        const isRenewal = ev.movement_kind === "renewal" || (fromTeam && fromTeam === toTeam);
        const accent = badge?.color ?? subcatColor(ev.categoria);
        const tenure = ev.seasons_at_previous;
        const fromCatLabel = ev.from_categoria ? subcatLabel(ev.from_categoria) : null;
        const toCatLabel = ev.categoria ? subcatLabel(ev.categoria) : null;
        const sameCat = fromCatLabel && fromCatLabel === toCatLabel;

        return (
          <div
            className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
            onClick={(e) => { if (e.target === e.currentTarget) setTransferDetail(null); }}
          >
            <div className="glass-strong animate-fade-in relative mx-4 w-full max-w-lg rounded-2xl p-6 md:p-7">
              <button
                onClick={() => setTransferDetail(null)}
                aria-label="Fechar"
                className="absolute right-4 top-4 flex h-8 w-8 items-center justify-center rounded-lg border border-white/12 bg-white/5 text-[color:var(--text-muted)] transition-glass hover:bg-white/10 hover:text-[color:var(--text-primary)]"
              >
                ✕
              </button>

              <div className="mb-1 flex flex-wrap items-center gap-1.5">
                {badge && (
                  <div
                    className="inline-flex items-center gap-1.5 rounded-md border px-2 py-0.5 text-[11px] font-black uppercase tracking-[0.16em]"
                    style={{ color: badge.color, background: badge.bg, borderColor: badge.border }}
                  >
                    <span className="text-[13px] leading-none">{badge.symbol}</span>
                    {badge.label}
                  </div>
                )}
                {emphasis && (
                  <div
                    className="inline-flex items-center gap-1.5 rounded-md border px-2 py-0.5 text-[11px] font-black uppercase tracking-[0.16em]"
                    style={{ color: emphasis.color, background: emphasis.bg, borderColor: emphasis.border }}
                  >
                    <span className="text-[13px] leading-none">{emphasis.symbol}</span>
                    {emphasis.label}
                  </div>
                )}
              </div>
              <h2 className="mb-5 text-[20px] font-bold leading-tight text-[color:var(--text-primary)]">
                {ev.driver_name}
              </h2>

              {/* Renovação: permaneceu na mesma equipe (sem De → Para) */}
              {isRenewal ? (
                <div className="mb-5 flex flex-col items-center gap-2 rounded-xl border border-white/10 bg-black/25 px-4 py-5 text-center">
                  <div className="text-[10px] font-bold uppercase tracking-[0.16em] text-[color:var(--text-muted)]">
                    Permaneceu
                  </div>
                  <TeamLogoMark teamName={toTeam} color={accent} size="md" />
                  <span className="block truncate text-[15px] font-bold text-[color:var(--text-primary)]">
                    {toTeam}
                  </span>
                </div>
              ) : (
              /* De → Para (equipes) */
              <div className="mb-5 flex items-center justify-between gap-3 rounded-xl border border-white/10 bg-black/25 px-4 py-4">
                <div className="flex min-w-0 flex-1 flex-col items-center gap-2 text-center">
                  <div className="text-[10px] font-bold uppercase tracking-[0.16em] text-[color:var(--text-muted)]">
                    De
                  </div>
                  {isDebut ? (
                    <span className="text-[14px] font-semibold text-[color:var(--text-secondary)]">
                      Estreia na carreira
                    </span>
                  ) : (
                    <>
                      <TeamLogoMark teamName={fromTeam} color={accent} size="md" />
                      <span className="block truncate text-[14px] font-bold text-[color:var(--text-primary)]">
                        {fromTeam}
                      </span>
                    </>
                  )}
                </div>

                <span className="shrink-0 text-[22px] font-black" style={{ color: accent }}>
                  →
                </span>

                <div className="flex min-w-0 flex-1 flex-col items-center gap-2 text-center">
                  <div className="text-[10px] font-bold uppercase tracking-[0.16em] text-[color:var(--text-muted)]">
                    Para
                  </div>
                  {toTeam ? (
                    <>
                      <TeamLogoMark teamName={toTeam} color={accent} size="md" />
                      <span className="block truncate text-[14px] font-bold text-[color:var(--text-primary)]">
                        {toTeam}
                      </span>
                    </>
                  ) : (
                    <span className="text-[14px] font-semibold text-[color:var(--text-secondary)]">
                      Sem equipe
                    </span>
                  )}
                </div>
              </div>
              )}

              {/* Categoria */}
              {toCatLabel && (
                <div className="mb-3 flex items-center justify-center gap-2 text-[13px]">
                  {fromCatLabel && !sameCat ? (
                    <>
                      <span className="font-semibold text-[color:var(--text-secondary)]">{fromCatLabel}</span>
                      <span className="font-black" style={{ color: accent }}>→</span>
                      <span className="font-bold text-[color:var(--text-primary)]">{toCatLabel}</span>
                    </>
                  ) : (
                    <span className="font-bold text-[color:var(--text-primary)]">{toCatLabel}</span>
                  )}
                </div>
              )}

              {/* Tempo de casa */}
              <p className="text-center text-body-sm text-[color:var(--text-muted)]">
                {isDebut
                  ? "Estreia na carreira"
                  : tenure != null && tenure > 0
                    ? `${tenure} ${tenure === 1 ? "temporada" : "temporadas"} ${isRenewal ? "na equipe" : "na equipe anterior"}`
                    : isRenewal
                      ? "Renovou contrato"
                      : "Equipe anterior"}
              </p>
            </div>
          </div>
        );
      })()}

      {/* ── Modal: Iniciar temporada sem equipe ── */}
      {showFreeAgentWarning && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
          onClick={(e) => { if (e.target === e.currentTarget) setShowFreeAgentWarning(false); }}
        >
          <div className="glass-strong animate-fade-in mx-4 w-full max-w-md rounded-2xl p-6 md:p-7">
            <div className="mb-1 text-body-sm font-bold uppercase tracking-[0.22em] text-[#f85149]">
              Atenção
            </div>
            <h2 className="mb-3 text-[18px] font-bold leading-tight text-[color:var(--text-primary)]">
              Você está sem equipe
            </h2>
            <p className="mb-2 text-body text-[color:var(--text-secondary)]">
              A pré-temporada encerrou sem que você fechasse um contrato. Se confirmar, iniciará
              a temporada como <span className="font-semibold text-[color:var(--text-primary)]">agente livre</span> — sem correr nenhuma etapa.
            </p>
            <p className="mb-6 text-body text-[color:var(--text-secondary)]">
              Ao final da temporada, você poderá tentar o mercado novamente. Após uma temporada
              inteira sem equipe, uma proposta de reserva será garantida na pré-temporada seguinte.
            </p>
            <div className="flex gap-3">
              <button
                onClick={() => setShowFreeAgentWarning(false)}
                className="transition-glass flex-1 rounded-xl border border-white/15 bg-white/5 px-4 py-2.5 text-body font-semibold text-[color:var(--text-secondary)] hover:bg-white/10"
              >
                Voltar
              </button>
              <button
                onClick={handleConfirmFreeAgentStart}
                className="transition-glass flex-1 rounded-xl border border-[#f8514999] bg-[#f85149]/20 px-4 py-2.5 text-body font-bold text-[#f85149] hover:bg-[#f85149]/30"
              >
                Confirmar mesmo assim
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
