import { useState, useEffect, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import useCareerStore from "../../stores/useCareerStore";
import { formatSalary } from "../../utils/formatters";
import TeamLogoMark from "../team/TeamLogoMark";

// ─── Category Definitions ─────────────────────────────────────────────────────

const CATEGORIES = [
  { id: "all",        label: "Todas",      color: "rgba(255,255,255,0.35)" },
  { id: "mazda",      dbIds: ["mazda_rookie", "mazda_amador"],   label: "Mazda",      color: "#F01010" },
  { id: "toyota",     dbIds: ["toyota_rookie", "toyota_amador"], label: "Toyota",     color: "#FF1010" },
  { id: "bmw",        dbIds: ["bmw_m2"],                         label: "BMW M2",     color: "#F00010" },
  { id: "sep1", isSeparator: true },
  { id: "gt4",       dbIds: ["gt4"],       label: "GT4",       color: "#3080FF" },
  { id: "gt3",       dbIds: ["gt3"],       label: "GT3",       color: "#00FFFF" },
  { id: "lmp2",      dbIds: ["lmp2"],      label: "LMP2",      color: "#F2CC60" },
];

const SUBCAT_LABELS = {
  mazda: "Mazda Cup Principal",
  toyota: "Toyota Cup Principal",
  bmw: "BMW M2 Cup Principal",
  mazda_amador: "Mazda Cup",
  mazda_rookie: "Mazda Rookie",
  toyota_amador: "Toyota Cup",
  toyota_rookie: "Toyota Rookie",
  bmw_m2: "BMW M2 Cup",
  production_challenger: "Production Challenger",
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
  lmp2: "/utilities/categorias/recortadas/LMP2.png",
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
  lmp2: {
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
  production_challenger: "#3fb950",
  gt4: "#3080FF",
  gt3: "#00FFFF",
  lmp2: "#F2CC60",
  endurance: "#3671C6",
};

// Ordem usada no grid central quando todas as categorias estao visiveis.
const CLASS_PRIORITY = [
  "lmp2",
  "gt3",
  "gt4",
  "bmw_m2", "bmw",
  "toyota_amador", "toyota",
  "mazda_amador", "mazda",
  "toyota_rookie",
  "mazda_rookie",
  "production_challenger",
  "endurance",
];

// Ordem do painel "Mercado de Pilotos": maior categoria primeiro,
// dentro de cada marca Cup > Amador > Rookie
const FREE_AGENT_ORDER = [
  "lmp2",
  "gt3",
  "gt4",
  "bmw_m2", "bmw",
  "toyota", "toyota_amador", "toyota_rookie",
  "mazda", "mazda_amador", "mazda_rookie",
];

const REGULAR_MARKET_CATEGORY_IDS = new Set([
  "mazda_rookie",
  "mazda_amador",
  "toyota_rookie",
  "toyota_amador",
  "bmw_m2",
  "gt4",
  "gt3",
  "lmp2",
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

function subcatLabel(key) {
  return SUBCAT_LABELS[key] ?? key;
}

function subcatColor(key) {
  return SUBCAT_COLORS[key] ?? "#58a6ff";
}

function subcatLogo(key) {
  return SUBCAT_LOGOS[key] ?? null;
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
  if (categoria === "gt4") return "gt4";
  if (categoria === "gt3") return "gt3";
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

function WeeklyClosingMovement({ event, color }) {
  const movementBadge = WEEKLY_MARKET_MOVEMENT_BADGES[event.movement_kind];

  return (
    <article
      className="rounded-lg border px-2.5 py-2"
      style={{
        borderColor: `${color}26`,
        background: `linear-gradient(135deg, ${color}0f 0%, rgba(255,255,255,0.02) 100%)`,
      }}
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
        <span
          className="w-8 shrink-0 text-right text-[13px] font-black leading-none"
          style={{ color }}
        >
          {formatWeeklyClosingPosition(event.championship_position)}
        </span>
        <p className="min-w-0 flex-1 truncate text-[13px] font-extrabold leading-[1.05] text-[color:var(--text-primary)]">
          {event.driver_name}
        </p>
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

function TeamDriverRow({ driverName, tenureSeasons, isPrimarySlot = false }) {
  const isOpenSlot = !driverName;
  const tenureCounter = !isOpenSlot ? formatTenureCounter(tenureSeasons) : null;
  return (
    <div className="flex items-center justify-between gap-3 py-2.5">
      <div className="flex min-w-0 flex-1 items-center">
        <p className={`truncate leading-[1.1] ${isOpenSlot ? "text-body font-semibold text-[#f85149]" : isPrimarySlot ? "text-[15px] font-bold text-[color:var(--text-primary)]" : "text-[14px] font-semibold text-[color:var(--text-primary)]"}`}>
            {driverName ?? "Sem piloto"}
        </p>
      </div>
      {tenureCounter && (
        tenureCounter.isNewcomer ? (
          <span className="shrink-0 rounded-md border border-[#58a6ff55] bg-[#58a6ff1f] px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.08em] text-[#79b8ff]">
            {tenureCounter.label}
          </span>
        ) : (
          <span className="shrink-0 text-[11px] font-semibold text-[color:var(--text-muted)]">
            {tenureCounter.label}
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

function FreeAgentCard({ driver, color, isRookie }) {
  const lic = LICENSE_COLORS[driver.license_sigla] ?? LICENSE_COLORS.R;
  const licenseTitle = licenseTooltip(driver.license_sigla);
  return (
    <div className="glass-light flex items-center gap-2 rounded-xl px-2.5 py-2">
      {isRookie ? (
        <span className="shrink-0 rounded-md bg-[#bc8cff22] px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.12em] text-[#bc8cff]">
          Novo
        </span>
      ) : (
        driver.previous_team_name ? (
          <TeamLogoMark
            teamName={driver.previous_team_name}
            color={driver.previous_team_color ?? color}
            size="xs"
            testId="driver-market-previous-team-logo"
          />
        ) : (
          <span
            className="shrink-0 rounded-md px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.1em]"
            style={{ background: `${color}22`, color }}
          >
            {driver.previous_team_abbr ?? "—"}
          </span>
        )
      )}
      <p className="min-w-0 flex-1 truncate text-body text-[color:var(--text-primary)]">
        {driver.driver_name}
      </p>
      <span
        aria-label={licenseTitle}
        className="shrink-0 rounded-md px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.1em]"
        style={{ background: lic.bg, color: lic.text }}
        title={licenseTitle}
      >
        {driver.license_sigla}
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
  const preseasonFreeAgents  = useCareerStore((s) => s.preseasonFreeAgents);
  const isAdvancingWeek      = useCareerStore((s) => s.isAdvancingWeek);
  const isRespondingProposal = useCareerStore((s) => s.isRespondingProposal);
  const advanceMarketWeek    = useCareerStore((s) => s.advanceMarketWeek);
  const respondToProposal    = useCareerStore((s) => s.respondToProposal);
  const finalizePreseason    = useCareerStore((s) => s.finalizePreseason);
  const playerTeam           = useCareerStore((s) => s.playerTeam);

  const [selectedCat, setSelectedCat]           = useState(() => playerCatToFilter(playerTeam?.categoria));
  const [gridData, setGridData]                 = useState([]);
  const [loadingGrid, setLoadingGrid]           = useState(false);
  const [showDisplacedModal, setShowDisplacedModal] = useState(false);
  const [showFreeAgentWarning, setShowFreeAgentWarning] = useState(false);
  const [startError, setStartError] = useState("");

  const freeAgentContainerRef = useRef(null);
  const freeAgentSectionRefs  = useRef({});

  // Semana atual e total
  const currentWeek = Math.min(preseasonState?.current_week ?? 1, preseasonState?.total_weeks ?? 1);
  const totalWeeks  = preseasonState?.total_weeks ?? 1;
  const isComplete  = preseasonState?.is_complete ?? false;
  const isMarketOpen = !isComplete;
  const weekProgress = Math.min(100, (currentWeek / totalWeeks) * 100);

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

        const all = [];
        for (const dbId of dbIds) {
          try {
            const teams = await invoke("get_teams_standings", { careerId, category: dbId });
            // Tag cada equipe com o dbId usado — TeamStanding não tem campo categoria
            teams.forEach((t) => all.push({ ...t, _categoria: dbId }));
          } catch {
            /* categoria pode não existir ainda */
          }
        }

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
  }, [careerId, selectedCat, currentWeek]);

  // ── Agrupamento e ordenação ─────────────────────────────────────────────────
  const groupedTeams = useMemo(() => {
    const grouped = {};
    gridData.forEach((team) => {
      const key = team.classe || team._categoria || "outras";
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

  // ── Free agents agrupados por categoria ────────────────────────────────────
  const freeAgentsByCategory = useMemo(() => {
    const grouped = {};
    (preseasonFreeAgents ?? []).forEach((d) => {
      const cat = d.categoria || "outras";
      if (!is_regular_market_category(cat)) return;
      grouped[cat] = grouped[cat] ?? { veterans: [], rookies: [] };
      if (d.is_rookie) grouped[cat].rookies.push(d);
      else grouped[cat].veterans.push(d);
    });
    return grouped;
  }, [preseasonFreeAgents]);

  // Endurance → GT3 → GT4 → ... → Toyota Cup → Rookie → Mazda Cup → Rookie → outras
  const freeAgentCategoryOrder = useMemo(() => {
    return Object.keys(freeAgentsByCategory).sort((a, b) => {
      const pa = FREE_AGENT_ORDER.indexOf(a);
      const pb = FREE_AGENT_ORDER.indexOf(b);
      if (pa !== -1 && pb !== -1) return pa - pb;
      if (pa !== -1) return -1;
      if (pb !== -1) return 1;
      return a.localeCompare(b);
    });
  }, [freeAgentsByCategory]);

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

  // ── Auto-scroll para categoria do jogador ao carregar ──────────────────────
  useEffect(() => {
    if (!freeAgentCategoryOrder.length || !playerTeam?.categoria) return;
    const playerCat = playerTeam.categoria;
    const el = freeAgentSectionRefs.current[playerCat];
    const container = freeAgentContainerRef.current;
    if (el && container) {
      requestAnimationFrame(() => {
        container.scrollTop = Math.max(0, el.offsetTop - container.offsetTop - 8);
      });
    }
  }, [freeAgentCategoryOrder.length]); // dispara quando a lista carrega

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
      try { await advanceMarketWeek(); } catch (e) { console.error(e); }
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

  const handleProposal = async (proposalId, accept) => {
    try { await respondToProposal(proposalId, accept); } catch (e) { console.error(e); }
  };

  // ── Render ──────────────────────────────────────────────────────────────────
  return (
    <div className="app-shell relative h-screen w-full overflow-hidden text-[color:var(--text-primary)]">
      <div className="app-backdrop pointer-events-none absolute inset-0" />

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
                {playerProposals.length > 0 && (
                  <span className="glass-light rounded-full px-2.5 py-1 text-body-sm font-bold tracking-[0.14em] text-[color:var(--accent-primary)]">
                    {playerProposals.length} proposta{playerProposals.length > 1 ? "s" : ""}
                  </span>
                )}
              </div>
              <h1 className="mt-1 text-[20px] font-bold leading-[1.05] tracking-[-0.02em] text-[color:var(--text-primary)] lg:text-[26px]">
                Mercado de Transferências
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
                {isMarketOpen ? "Mercado aberto" : "Última semana"}
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
                  {(preseasonFreeAgents ?? []).length} livres
                </span>
              )}
            </div>

            {(preseasonFreeAgents ?? []).length === 0 ? (
              <div className="py-10 text-center text-body text-[color:var(--text-muted)]">
                Todos os pilotos têm equipe.
              </div>
            ) : (
              <div className="space-y-5">
                {freeAgentCategoryOrder.map((cat) => {
                  const { veterans, rookies } = freeAgentsByCategory[cat];
                  const color = SUBCAT_COLORS[cat] ?? "#58a6ff";
                  return (
                    <section key={cat} ref={(el) => { freeAgentSectionRefs.current[cat] = el; }}>
                      <div className="mb-2 flex items-center gap-2">
                        <span
                          className="text-[9px] font-bold uppercase tracking-[0.2em]"
                          style={{ color }}
                        >
                          {subcatLabel(cat)}
                        </span>
                        <div
                          className="h-px flex-1"
                          style={{ background: `linear-gradient(to right, ${color}55, transparent)` }}
                        />
                        <span className="text-[9px] text-[color:var(--text-muted)]">
                          {veterans.length + rookies.length}
                        </span>
                      </div>
                      <div className="space-y-1.5">
                        {veterans.map((d) => (
                          <FreeAgentCard key={d.driver_id} driver={d} color={color} />
                        ))}
                        {rookies.map((d) => (
                          <FreeAgentCard key={d.driver_id} driver={d} color={color} isRookie />
                        ))}
                      </div>
                    </section>
                  );
                })}
              </div>
            )}
          </aside>

          {/* ── CENTRO: Grid de Equipes ── */}
          <main className="glass scroll-area animate-fade-in min-h-0 overflow-y-auto rounded-2xl px-5 py-4 lg:px-6 lg:py-5">
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

                      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
                        {teams.map((team) => {
                          const rankStyle = getRankStyle(team.temp_posicao);
                          const movement = getTeamMovementBadge(team.categoria_anterior, team._categoria || team.classe);
                          const teamLogoFallback = movement?.color ?? team.cor_primaria ?? accent;

                          return (
                            <article
                              key={team.id}
                              className="glass transition-glass relative overflow-hidden rounded-xl border p-3 hover:-translate-y-0.5 hover:scale-[1.01]"
                              style={{
                                borderColor: movement
                                  ? movement.border
                                  : rankStyle?.border
                                    ? `${rankStyle.border}88`
                                    : "rgba(255,255,255,0.11)",
                              }}
                            >
                              {rankStyle && !movement && (
                                <div
                                  className="pointer-events-none absolute right-0 top-0 h-full w-28"
                                  style={{
                                    background: `radial-gradient(circle at 94% 14%, ${rankStyle.glow} 0%, transparent 68%)`,
                                  }}
                                />
                              )}
                              {movement && (
                                <div
                                  className="pointer-events-none absolute right-0 top-0 h-full w-32"
                                  style={{
                                    background: `radial-gradient(circle at 94% 14%, ${movement.bg.replace("0.12", "0.18")} 0%, transparent 68%)`,
                                  }}
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
                                  isPrimarySlot
                                />
                                <TeamDriverRow
                                  driverName={team.piloto_2_nome}
                                  tenureSeasons={team.piloto_2_tenure_seasons}
                                />
                              </div>
                            </article>
                          );
                        })}
                      </div>
                    </section>
                  );
                })}
              </div>
            )}
          </main>

          {/* ── DIREITA: Decisões Pendentes ── */}
          <aside className="glass scroll-area animate-drawer-in self-start overflow-y-auto rounded-2xl px-4 py-4 lg:px-5 lg:py-5 xl:max-h-[calc(100vh-96px)]">
            <div className="mb-4 flex h-6 items-center gap-2">
              <span className="relative inline-flex h-2.5 w-2.5">
                {playerProposals.length > 0 && (
                  <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-[#58a6ff]/80" />
                )}
                <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-[color:var(--accent-primary)]" />
              </span>
              <p className="text-body-sm font-bold uppercase tracking-[0.22em] text-[color:var(--accent-primary)]">
                Decisões pendentes
              </p>
            </div>

            {playerProposals.length === 0 ? (
              <div className="glass-light rounded-xl border-dashed p-6 text-center text-body text-[color:var(--text-secondary)]">
                Nenhuma proposta pendente.
              </div>
            ) : (
              <div className="space-y-3">
                {playerProposals.map((prop) => (
                  <article key={prop.proposal_id} className="glass animate-scale-in rounded-xl px-4 py-3.5">
                    <div className="flex min-w-0 items-center gap-3">
                      <TeamLogoMark
                        teamName={prop.equipe_nome}
                        color={prop.equipe_cor_primaria}
                        size="md"
                        testId="preseason-proposal-team-logo"
                      />
                      <div className="min-w-0 flex-1">
                        <p
                          className="text-body-sm font-bold uppercase tracking-[0.16em]"
                          style={{ color: prop.equipe_cor_primaria }}
                        >
                          {prop.papel} | {prop.categoria_nome || prop.categoria}
                        </p>
                        <p className="mt-1 truncate text-title-md">{prop.equipe_nome}</p>
                      </div>
                    </div>

                    <div className="my-3 grid grid-cols-2 gap-2">
                      <div className="glass-light rounded-lg p-2.5">
                        <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
                          Salário
                        </p>
                        <p className="num-medium mt-0.5 font-bold text-[color:var(--status-green)]">
                          {formatSalary(prop.salario_oferecido)}
                        </p>
                      </div>
                      <div className="glass-light rounded-lg p-2.5">
                        <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
                          Duração
                        </p>
                        <p className="num-medium mt-0.5 font-bold text-[color:var(--text-primary)]">
                          {prop.duracao_anos} ano{prop.duracao_anos > 1 ? "s" : ""}
                        </p>
                      </div>
                      {prop.companheiro_nome && (
                        <div className="glass-light rounded-lg p-2.5">
                          <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
                            Companheiro
                          </p>
                          <p className="text-body mt-0.5 font-semibold text-[color:var(--text-primary)] truncate">
                            {prop.companheiro_nome}
                          </p>
                        </div>
                      )}
                      <div className={`glass-light rounded-lg p-2.5 ${prop.companheiro_nome ? "" : "col-span-2"}`}>
                        <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
                          Carro
                        </p>
                        <div className="mt-1.5 flex items-center gap-2">
                          <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-[#21262d]">
                            <div
                              className="h-full rounded-full"
                              style={{
                                width: `${prop.car_performance_rating ?? 0}%`,
                                backgroundColor: prop.equipe_cor_primaria,
                              }}
                            />
                          </div>
                          <span className="text-body font-bold">{prop.car_performance_rating}</span>
                        </div>
                      </div>
                    </div>

                    <div className="flex gap-2">
                      <button
                        onClick={() => handleProposal(prop.proposal_id, true)}
                        disabled={isRespondingProposal}
                        className="transition-glass glow-blue flex-1 rounded-lg border border-[#58a6ff66] bg-[#58a6ff33] px-3 py-2 text-body font-bold text-[color:var(--accent-primary)] hover:bg-[#58a6ff55] disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        Aceitar
                      </button>
                      <button
                        onClick={() => handleProposal(prop.proposal_id, false)}
                        disabled={isRespondingProposal}
                        className="transition-glass flex-1 rounded-lg border border-white/15 bg-white/5 px-3 py-2 text-body font-bold text-[color:var(--text-secondary)] hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        Recusar
                      </button>
                    </div>
                  </article>
                ))}
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
