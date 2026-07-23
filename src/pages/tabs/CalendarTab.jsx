import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";

import GlassCard from "../../components/ui/GlassCard";
import useDeferredLoading from "../../hooks/useLoading";
import useCareerStore from "../../stores/useCareerStore";
import { getCategoryColor } from "../../utils/categoryColors";
import { categoryLabel } from "../../utils/formatters";
import { isLegacySeasonPhase } from "../../utils/seasonPhases";
import i18n from "../../i18n/index.js";
import { monthLongLabels, weekdayNarrowLabels } from "../../i18n/format.js";

// Imagens por track_id (ids reais do iRacing)
const TRACK_IMAGES = {
  9: "/utilities/tracks/summitpoint.webp",
  353: "/utilities/tracks/limerock.jpeg",
  586: "/utilities/tracks/lagunaseca.webp",
  166: "/utilities/tracks/okayama.webp",
  180: "/utilities/tracks/oultonpark.jpeg",
  181: "/utilities/tracks/oultonpark.jpeg",
  182: "/utilities/tracks/oultonpark.jpeg",
  324: "/utilities/tracks/Tsukuba.webp",
  449: "/utilities/tracks/motorsport arena.webp",
  451: "/utilities/tracks/rudskogen.jpeg",
  489: "/utilities/tracks/ledenon.webp",
  202: "/utilities/tracks/oranpark.webp",
  440: "/utilities/tracks/winton.jpeg",
  515: "/utilities/tracks/Navarra.webp",
  554: "/utilities/tracks/charlotte.webp",
  465: "/utilities/tracks/virginia.jpeg",
};

const TRACK_IMAGE_FILES = [
  { match: ["charlotte"], file: "charlotte.webp" },
  { match: ["laguna seca"], file: "lagunaseca.webp" },
  { match: ["lime rock"], file: "limerock.jpeg" },
  { match: ["okayama"], file: "okayama.webp" },
  { match: ["oulton"], file: "oultonpark.jpeg" },
  { match: ["snetterton"], file: "snetterton.jpeg" },
  { match: ["summit point", "jefferson"], file: "summitpoint.webp" },
  { match: ["tsukuba"], file: "Tsukuba.webp" },
  { match: ["virginia international raceway", "vir full", "vir patriot"], file: "virginia.jpeg" },
  { match: ["ledenon"], file: "ledenon.webp" },
  { match: ["oschersleben", "motorsport arena"], file: "motorsport arena.webp" },
  { match: ["navarra"], file: "Navarra.webp" },
  { match: ["oran park"], file: "oranpark.webp" },
  { match: ["rudskogen"], file: "rudskogen.jpeg" },
  { match: ["winton"], file: "winton.jpeg" },
];

const CATEGORY_LOGOS = {
  mazda_rookie: "/utilities/categorias/MX5%20ROOKIE.webp",
  toyota_rookie: "/utilities/categorias/GR%20ROOKIE.webp",
  mazda_amador: "/utilities/categorias/MX5%20CUP.webp",
  toyota_amador: "/utilities/categorias/GR%20CUP.webp",
  bmw_m2: "/utilities/categorias/M2%20CUP.webp",
  production_challenger: "/utilities/categorias/PRODUCTION.webp",
  gt4: "/utilities/categorias/GT4.webp",
  gt3: "/utilities/categorias/GT3.webp",
  lmp2: "/utilities/categorias/LMP2.webp",
  endurance: "/utilities/categorias/ENDURANCE.webp",
};

// Constantes visuais
// Nomes de mês e iniciais de dia da semana vêm de Intl (i18n/format.js), por locale.

// Barra de cabeçalho de cada mês (cor por fase). "active" = mês atual (preenchido).
const MONTH_BAR_TINTS = {
  mercado: { idle: "bg-status-yellow/12 text-status-yellow", active: "bg-status-yellow/80 text-[#1a1206]" },
  regular: { idle: "bg-accent-primary/12 text-accent-primary", active: "bg-accent-primary text-[#05080c]" },
  especial: { idle: "bg-status-purple/15 text-status-purple", active: "bg-status-purple/80 text-white" },
  encerramento: { idle: "bg-white/[0.06] text-text-secondary", active: "bg-white/70 text-[#0e0e10]" },
};
const ALL_CALENDAR_CATEGORIES = [
  "mazda_rookie",
  "toyota_rookie",
  "mazda_amador",
  "toyota_amador",
  "bmw_m2",
  "production_challenger",
  "gt4",
  "gt3",
  "endurance",
];
// Fase de cada mes segundo as regras do jogo:
// Jan = Mercado, Fev-Ago = Temporada Regular, Set-Dez = Bloco Especial
function getMonthPhase(monthIndex, isLegacyCalendar = false) {
  // LEGADO 9D: saves pré-v33 ainda exibem a divisão visual do calendário antigo.
  if (isLegacyCalendar) {
    if (monthIndex === 0) {
      return {
        type: "mercado",
        label: i18n.t("calendar.phase.mercado"),
        badgeClass: "bg-status-yellow/15 text-status-yellow",
        cardClass: "border-status-yellow/25",
        emptyText: i18n.t("calendar.phaseEmpty.mercado"),
      };
    }
    if (monthIndex <= 7) {
      return {
        type: "regular",
        label: i18n.t("calendar.phase.regularLegacy"),
        badgeClass: "bg-accent-primary/15 text-accent-primary",
        cardClass: "",
        emptyText: null,
      };
    }
    return {
      type: "especial",
      label: i18n.t("calendar.phase.especial"),
      badgeClass: "bg-status-purple/15 text-status-purple",
      cardClass: "border-status-purple/25",
      emptyText: i18n.t("calendar.phaseEmpty.especial"),
    };
  }

  if (!isLegacyCalendar) {
    if (monthIndex === 0) {
      return {
        type: "mercado",
        label: i18n.t("calendar.phase.pretemporada"),
        badgeClass: "bg-status-yellow/15 text-status-yellow",
        cardClass: "border-status-yellow/25",
        emptyText: i18n.t("calendar.phaseEmpty.pretemporada"),
      };
    }
    if (monthIndex >= 1 && monthIndex <= 10) {
      return {
        type: "regular",
        label: i18n.t("calendar.phase.temporada"),
        badgeClass: "bg-accent-primary/15 text-accent-primary",
        cardClass: "",
        emptyText: null,
      };
    }
    return {
      type: "encerramento",
      label: i18n.t("calendar.phase.encerramento"),
      badgeClass: "bg-white/10 text-text-secondary",
      cardClass: "border-white/10",
      emptyText: i18n.t("calendar.phaseEmpty.encerramento"),
    };
  }

  if (monthIndex === 0) {
    return {
      type: "mercado",
      label: i18n.t("calendar.phase.mercado"),
      badgeClass: "bg-status-yellow/15 text-status-yellow",
      cardClass: "border-status-yellow/25",
      emptyText: i18n.t("calendar.phaseEmpty.mercado"),
    };
  }
  if (monthIndex <= 7) {
    return {
      type: "regular",
      label: i18n.t("calendar.phase.regularLegacy"),
      badgeClass: "bg-accent-primary/15 text-accent-primary",
      cardClass: "",
      emptyText: null,
    };
  }
  return {
    type: "especial",
    label: i18n.t("calendar.phase.especial"),
      badgeClass: "bg-status-purple/15 text-status-purple",
      cardClass: "border-status-purple/25",
      emptyText: i18n.t("calendar.phaseEmpty.especial"),
  };
}

// Helpers
function parseDisplayDate(dateStr) {
  if (!dateStr) return null;
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(dateStr);
  if (!match) return null;
  return { year: Number(match[1]), month: Number(match[2]) - 1, day: Number(match[3]) };
}

function formatIsoDateKey(year, month, day) {
  return `${year}-${String(month + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}

function parseIsoDateUtc(dateStr) {
  const parsed = parseDisplayDate(dateStr);
  if (!parsed) return null;
  return new Date(Date.UTC(parsed.year, parsed.month, parsed.day));
}

function formatDateKeyFromUtc(date) {
  return formatIsoDateKey(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate());
}

function nthWeekdayOfMonthUtc(year, month, weekday, nth) {
  const firstDay = new Date(Date.UTC(year, month, 1));
  const offset = (7 + weekday - firstDay.getUTCDay()) % 7;
  return new Date(Date.UTC(year, month, 1 + offset + (Math.max(1, nth) - 1) * 7));
}

function getFallbackFirstSpecialRaceDate(year) {
  return nthWeekdayOfMonthUtc(year, 8, 0, 2);
}

function buildConvocationWindowDateKeys(firstSpecialRaceDate, totalDays, seasonYear) {
  const anchorDate = parseIsoDateUtc(firstSpecialRaceDate) ?? getFallbackFirstSpecialRaceDate(seasonYear);
  if (!anchorDate) {
    return new Set();
  }

  const safeTotalDays = Math.max(1, totalDays ?? 7);
  const keys = new Set();

  for (let offset = safeTotalDays; offset >= 1; offset -= 1) {
    const day = new Date(anchorDate.getTime());
    day.setUTCDate(day.getUTCDate() - offset);
    keys.add(formatDateKeyFromUtc(day));
  }

  return keys;
}

function getConvocationStartDateKey(firstSpecialRaceDate, totalDays, seasonYear) {
  const anchorDate = parseIsoDateUtc(firstSpecialRaceDate) ?? getFallbackFirstSpecialRaceDate(seasonYear);
  if (!anchorDate) {
    return null;
  }

  const startDate = new Date(anchorDate.getTime());
  startDate.setUTCDate(startDate.getUTCDate() - Math.max(1, totalDays ?? 7));
  return formatDateKeyFromUtc(startDate);
}

function withFetchedCategory(entries = [], category) {
  return entries.map((entry) => ({
    ...entry,
    categoria: entry.categoria ?? category,
  }));
}

function buildMonthCells(year, month) {
  const firstWeekday = new Date(year, month, 1).getDay();
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const cells = [];
  for (let i = 0; i < firstWeekday; i += 1) cells.push(null);
  for (let day = 1; day <= daysInMonth; day += 1) cells.push(day);
  while (cells.length % 7 !== 0) cells.push(null);
  return cells;
}

function compareCalendarMonth(year, month, currentDateParts) {
  if (!currentDateParts) return 0;
  if (year !== currentDateParts.year) {
    return year < currentDateParts.year ? -1 : 1;
  }
  if (month === currentDateParts.month) return 0;
  return month < currentDateParts.month ? -1 : 1;
}

function weatherLabel(value) {
  if (value === "HeavyRain") return i18n.t("weather.heavyRain");
  if (value === "Wet") return i18n.t("weather.wet");
  if (value === "Damp") return i18n.t("weather.damp");
  return i18n.t("weather.dry");
}

function getTrackAssetPath(file) {
  if (!file) return null;
  if (file.startsWith("/utilities/tracks/")) {
    return `/utilities/tracks/${encodeURIComponent(file.slice("/utilities/tracks/".length))}`;
  }
  return `/utilities/tracks/${encodeURIComponent(file)}`;
}

function normalizeTrackName(trackName) {
  return (trackName ?? "")
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();
}

function getTrackImageSrc(race) {
  const normalizedName = normalizeTrackName(race?.track_name);
  const entry = TRACK_IMAGE_FILES.find(({ match }) =>
    match.some((candidate) => normalizedName.includes(candidate)),
  );

  if (entry) {
    return getTrackAssetPath(entry.file);
  }

  return getTrackAssetPath(TRACK_IMAGES[race?.track_id]);
}

export function getRaceTooltipStyle(cellRect, viewport = {}, tooltipSize = {}, options = {}) {
  const tooltipWidth = tooltipSize.width ?? 208;
  const tooltipHeight = tooltipSize.height ?? 176;
  const margin = 12;
  const gap = 8;
  const verticalOffset = options.verticalOffset ?? 0;
  const viewportWidth = viewport.width
    ?? document.documentElement?.clientWidth
    ?? window.innerWidth;
  const viewportHeight = viewport.height
    ?? document.documentElement?.clientHeight
    ?? window.innerHeight;

  const minLeft = margin;
  const maxLeft = Math.max(margin, viewportWidth - tooltipWidth - margin);
  const centeredLeft = cellRect.left + (cellRect.width / 2) - (tooltipWidth / 2);
  const spaceLeft = cellRect.left - margin;
  const spaceRight = viewportWidth - (cellRect.left + cellRect.width) - margin;

  let left = centeredLeft;
  if (spaceRight < tooltipWidth && spaceLeft >= tooltipWidth) {
    left = cellRect.left + cellRect.width - tooltipWidth;
  } else if (spaceLeft < tooltipWidth && spaceRight >= tooltipWidth) {
    left = cellRect.left;
  } else if (centeredLeft + tooltipWidth > viewportWidth - margin) {
    left = cellRect.left + cellRect.width - tooltipWidth;
  } else if (centeredLeft < margin) {
    left = cellRect.left;
  }
  left = Math.min(Math.max(left, minLeft), maxLeft);

  const hasRoomAbove = cellRect.top >= tooltipHeight + gap + margin;
  const belowTop = cellRect.top + cellRect.height + gap - verticalOffset;
  const aboveTop = cellRect.top - tooltipHeight - gap - verticalOffset;
  const maxTop = Math.max(margin, viewportHeight - tooltipHeight - margin);
  const preferredTop = hasRoomAbove ? aboveTop : belowTop;
  const top = Math.min(Math.max(preferredTop, margin), maxTop);

  return {
    position: "fixed",
    left,
    top,
    transform: "translate(0, 0)",
    zIndex: 9999,
    pointerEvents: "none",
  };
}

function CalendarTab({ activeTab, raceArrivalFeedbackActive = false }) {
  const { t } = useTranslation();
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const nextRace = useCareerStore((state) => state.nextRace);
  const season = useCareerStore((state) => state.season);
  const specialWindowState = useCareerStore((state) => state.specialWindowState);
  const acceptedSpecialOffer = useCareerStore((state) => state.acceptedSpecialOffer);
  const isCalendarAdvancing = useCareerStore((state) => state.isCalendarAdvancing);
  const calendarDisplayDate = useCareerStore((state) => state.calendarDisplayDate);
  const temporalSummary = useCareerStore((state) => state.temporalSummary);

  const [calendar, setCalendar] = useState([]);
  const [specialCalendar, setSpecialCalendar] = useState([]);
  const [otherCalendars, setOtherCalendars] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [tooltip, setTooltip] = useState(null);
  // Só exibe "Carregando calendário..." se demorar de verdade — evita o flash na
  // troca de aba, já que o fetch local resolve em poucos ms.
  const showLoadingUI = useDeferredLoading(loading);
  const isLegacyCalendar = isLegacySeasonPhase(season?.fase);

  useEffect(() => {
    let mounted = true;

    async function fetchCalendar() {
      if (!careerId || !playerTeam?.categoria) {
        setCalendar([]);
        setSpecialCalendar([]);
        setOtherCalendars([]);
        setLoading(false);
        return;
      }

      setLoading(true);
      setError("");
      setOtherCalendars([]);

      try {
        const specialCategory = isLegacyCalendar ? acceptedSpecialOffer?.special_category ?? null : null;
        const visibleCategories = new Set([playerTeam.categoria, specialCategory].filter(Boolean));
        const otherCategories = ALL_CALENDAR_CATEGORIES.filter((category) => !visibleCategories.has(category));
        const [regularEntries, specialEntries] = await Promise.all([
          invoke("get_calendar_for_category", {
            careerId,
            category: playerTeam.categoria,
          }).then((entries) => withFetchedCategory(entries, playerTeam.categoria)),
          specialCategory
            ? invoke("get_calendar_for_category", {
              careerId,
              category: specialCategory,
            }).then((entries) => withFetchedCategory(entries, specialCategory))
            : Promise.resolve([]),
        ]);

        if (!mounted) return;
        setCalendar(regularEntries);
        setSpecialCalendar(specialEntries);
        setLoading(false);

        Promise.all(
          otherCategories.map((category) => (
            invoke("get_calendar_for_category", {
              careerId,
              category,
            }).then((entries) => withFetchedCategory(entries, category))
          )),
        )
          .then((otherEntries) => {
            if (mounted) {
              setOtherCalendars(otherEntries.flat());
            }
          })
          .catch(() => {
            if (mounted) {
              setOtherCalendars([]);
            }
          });
      } catch (err) {
        if (mounted) {
          setError(typeof err === "string" ? err : i18n.t("calendar.loadError"));
        }
      } finally {
        if (mounted) {
          setLoading(false);
        }
      }
    }

    fetchCalendar();
    return () => {
      mounted = false;
    };
  }, [
    acceptedSpecialOffer?.special_category,
    careerId,
    isLegacyCalendar,
    playerTeam?.categoria,
    season?.rodada_atual,
  ]);

  const displayedCalendar = useMemo(
    () => [...calendar, ...specialCalendar],
    [calendar, specialCalendar],
  );
  const otherCategoryRacesByDate = useMemo(() => {
    const map = {};

    for (const race of otherCalendars) {
      const parsed = parseDisplayDate(race.display_date);
      if (!parsed) continue;

      const dateKey = formatIsoDateKey(parsed.year, parsed.month, parsed.day);
      if (!map[dateKey]) {
        map[dateKey] = [];
      }
      map[dateKey].push(race);
    }

    return map;
  }, [otherCalendars]);

  const seasonYear = useMemo(() => {
    if (season?.ano) return season.ano;
    for (const race of displayedCalendar) {
      const parsed = parseDisplayDate(race.display_date);
      if (parsed) return parsed.year;
    }
    return new Date().getFullYear();
  }, [displayedCalendar, season]);

  const racesByDate = useMemo(() => {
    const map = {};
    for (const race of displayedCalendar) {
      const parsed = parseDisplayDate(race.display_date);
      if (!parsed) continue;
      const key = formatIsoDateKey(parsed.year, parsed.month, parsed.day);
      map[key] = {
        ...race,
        _day: parsed.day,
        _isSpecialRace: race.season_phase === "BlocoEspecial",
      };
    }
    return map;
  }, [displayedCalendar]);

  const firstSpecialRaceDate = useMemo(() => {
    const dates = specialCalendar
      .map((race) => race.display_date)
      .filter(Boolean)
      .sort();
    return dates[0] ?? null;
  }, [specialCalendar]);

  const convocationWindowDateKeys = useMemo(
    () => (
      isLegacyCalendar
        ? buildConvocationWindowDateKeys(
          firstSpecialRaceDate,
          specialWindowState?.total_days ?? 7,
          seasonYear,
        )
        : new Set()
    ),
    [firstSpecialRaceDate, isLegacyCalendar, seasonYear, specialWindowState?.total_days],
  );
  const convocationStartDateKey = useMemo(
    () => (
      isLegacyCalendar
        ? getConvocationStartDateKey(
          firstSpecialRaceDate,
          specialWindowState?.total_days ?? 7,
          seasonYear,
        )
        : null
    ),
    [firstSpecialRaceDate, isLegacyCalendar, seasonYear, specialWindowState?.total_days],
  );

  const completed = displayedCalendar.filter((race) => race.status === "Concluida").length;
  const currentDateParts = useMemo(() => {
    if (activeTab !== "calendar") {
      return null;
    }
    return parseDisplayDate(calendarDisplayDate ?? temporalSummary?.current_display_date ?? null);
  }, [activeTab, calendarDisplayDate, temporalSummary]);

  return (
    <GlassCard hover={false} className="rounded-[28px]">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <p className="kcal text-[11px] uppercase tracking-[0.22em] text-accent-primary">{t("calendar.title")}</p>
          <h2 className="kcal mt-2 text-3xl font-semibold text-text-primary">
            {categoryLabel(playerTeam?.categoria)}
          </h2>
        </div>
        <p className="text-sm text-text-secondary">
          {t("calendar.stepsDone", { completed, total: displayedCalendar.length })}
        </p>
      </div>

      <div data-testid="calendar-legend" className="mt-4 flex flex-wrap items-center gap-x-5 gap-y-2">
        {isLegacyCalendar ? (
          <>
            <LegendItem color="bg-status-yellow" label={t("calendar.legend.mercado")} />
            <LegendItem color="bg-orange-400" label={t("calendar.legend.convocacao")} />
            <LegendItem color="bg-status-purple" label={t("calendar.legend.especial")} />
          </>
        ) : (
          <>
            <LegendItem color="bg-status-yellow" label={t("calendar.legend.pretemporada")} />
            <LegendItem color="bg-accent-primary" label={t("calendar.legend.temporada")} />
            <LegendItem color="bg-white/60" label={t("calendar.legend.encerramento")} />
          </>
        )}
      </div>

      {loading ? (
        showLoadingUI ? (
          <p className="mt-8 text-sm text-text-secondary">{t("calendar.loading")}</p>
        ) : null
      ) : error ? (
        <div className="mt-6 rounded-2xl border border-status-red/30 bg-status-red/10 px-4 py-3 text-sm text-status-red">
          {error}
        </div>
      ) : (
        <div className="relative mt-6 space-y-4">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            {Array.from({ length: 8 }, (_, month) => (
              <MonthCard
                key={month}
                year={seasonYear}
                month={month}
                racesByDate={racesByDate}
                otherCategoryRacesByDate={otherCategoryRacesByDate}
                nextRaceId={nextRace?.id}
                currentDateParts={currentDateParts}
                convocationDateKeys={convocationWindowDateKeys}
                convocationStartDateKey={convocationStartDateKey}
                isLegacyCalendar={isLegacyCalendar}
                raceArrivalFeedbackActive={raceArrivalFeedbackActive}
                showAnimatedProgress={isCalendarAdvancing}
                onCellHover={setTooltip}
              />
            ))}
          </div>

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {Array.from({ length: 4 }, (_, offset) => (
              <MonthCard
                key={8 + offset}
                year={seasonYear}
                month={8 + offset}
                racesByDate={racesByDate}
                otherCategoryRacesByDate={otherCategoryRacesByDate}
                nextRaceId={nextRace?.id}
                currentDateParts={currentDateParts}
                convocationDateKeys={convocationWindowDateKeys}
                convocationStartDateKey={convocationStartDateKey}
                isLegacyCalendar={isLegacyCalendar}
                raceArrivalFeedbackActive={raceArrivalFeedbackActive}
                showAnimatedProgress={isCalendarAdvancing}
                onCellHover={setTooltip}
              />
            ))}
          </div>

          {tooltip && <CalendarTooltip race={tooltip.race} otherRaces={tooltip.otherRaces} cellRect={tooltip.rect} />}
        </div>
      )}
    </GlassCard>
  );
}

function LegendItem({ color, label }) {
  return (
    <div className="flex items-center gap-1.5">
      <div className={`h-2 w-2 rounded-full ${color}`} />
      <span className="text-[11px] text-text-muted">{label}</span>
    </div>
  );
}

function CurrentDayRail() {
  return (
    <span
      aria-hidden="true"
      className="pointer-events-none absolute inset-y-0 left-0 z-20 w-[3px] rounded-r-full bg-gradient-to-b from-accent-hover via-accent-primary to-accent-primary/65 shadow-[0_0_12px_rgba(88,166,255,0.5)]"
    />
  );
}

function MonthCard({
  year,
  month,
  racesByDate,
  otherCategoryRacesByDate,
  nextRaceId,
  currentDateParts,
  convocationDateKeys,
  convocationStartDateKey,
  isLegacyCalendar,
  raceArrivalFeedbackActive,
  showAnimatedProgress,
  onCellHover,
}) {
  const phase = getMonthPhase(month, isLegacyCalendar);
  const tint = MONTH_BAR_TINTS[phase.type] ?? MONTH_BAR_TINTS.regular;
  const cells = buildMonthCells(year, month);
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const isCurrentMonth = currentDateParts?.year === year && currentDateParts?.month === month;
  const monthTimelineOrder = compareCalendarMonth(year, month, currentDateParts);
  const isFutureMonth = monthTimelineOrder > 0;
  const isReachedMonth = monthTimelineOrder <= 0;
  const isAnimatedMonth = isCurrentMonth && showAnimatedProgress;
  const animatedProgress = isCurrentMonth
    ? Math.max(6, Math.round((currentDateParts.day / daysInMonth) * 100))
    : 0;
  const monthIso = `${year}-${String(month + 1).padStart(2, "0")}`;

  const racesThisMonth = cells
    .filter(Boolean)
    .reduce((acc, day) => {
      const key = formatIsoDateKey(year, month, day);
      if (racesByDate[key]) acc[day] = racesByDate[key];
      return acc;
    }, {});

  const hasRaces = Object.keys(racesThisMonth).length > 0;

  // Card de vidro por mês (profundidade + separação). O mês ATUAL ganha borda/glow
  // de destaque; os demais um vidro sutil que levanta no hover.
  const cardBase = "relative overflow-hidden rounded-2xl p-3 transition-all duration-300";
  const cardSkin = isCurrentMonth
    ? "border border-accent-primary/45 bg-[linear-gradient(180deg,rgba(88,166,255,0.10),rgba(88,166,255,0.02))] shadow-[0_0_0_1px_rgba(88,166,255,0.22),0_0_30px_-6px_rgba(88,166,255,0.40),inset_0_1px_0_rgba(255,255,255,0.06)]"
    : "border border-white/[0.06] bg-gradient-to-b from-white/[0.035] to-white/[0.008] shadow-[inset_0_1px_0_rgba(255,255,255,0.05),0_12px_30px_-14px_rgba(0,0,0,0.55)] hover:-translate-y-[1px] hover:border-white/[0.12] hover:bg-white/[0.05] hover:shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_16px_36px_-14px_rgba(0,0,0,0.6)]";

  return (
    <div
      className={`${cardBase} ${cardSkin}`}
      data-testid={`calendar-month-${monthIso}`}
      data-active-month-window={isCurrentMonth ? "true" : "false"}
      data-animated-month={isAnimatedMonth ? "true" : "false"}
    >
      {isAnimatedMonth && (
        <div className="absolute left-3 right-3 top-0 h-[3px] rounded-b-full bg-white/8">
          <div
            data-testid={`calendar-progress-${monthIso}`}
            data-animated-month="true"
            className="h-full rounded-b-full bg-gradient-to-r from-accent-primary/35 via-accent-primary to-accent-hover/70 shadow-[0_0_18px_rgba(88,166,255,0.35)] transition-[width] duration-200"
            style={{ width: `${animatedProgress}%` }}
          />
        </div>
      )}

      <div
        className={`kcal mb-2 rounded-lg px-3 py-1.5 text-center text-[15px] font-bold uppercase tracking-[0.16em] ${
          isCurrentMonth ? tint.active : tint.idle
        }`}
      >
        {monthLongLabels()[month]}
      </div>

      <div className="grid grid-cols-7 gap-[2px]">
        {weekdayNarrowLabels().map((weekday, index) => (
          <div
            key={index}
            className="pb-1 text-center text-[9px] font-medium text-text-muted/50"
          >
            {weekday}
          </div>
        ))}

        {cells.map((day, index) => {
          const race = day != null ? (racesThisMonth[day] ?? null) : null;
          const isNext = race != null && nextRaceId === race.id;
          const dateKey = day != null ? formatIsoDateKey(year, month, day) : null;
          const otherCategoryRaces = dateKey != null ? (otherCategoryRacesByDate?.[dateKey] ?? []) : [];
          const isConvocationDay = dateKey != null && convocationDateKeys?.has(dateKey);
          const isPreSpecialDay = Boolean(
            dateKey != null &&
            phase.type === "especial" &&
            convocationStartDateKey != null &&
            dateKey < convocationStartDateKey,
          );

          return (
            <DayCell
              key={index}
              day={day}
              race={race}
              isNext={isNext}
              phase={phase}
              year={year}
              month={month}
              isConvocationDay={Boolean(isConvocationDay)}
              isPreSpecialDay={isPreSpecialDay}
              isSpecialRace={Boolean(race?._isSpecialRace)}
              otherCategoryRaces={otherCategoryRaces}
              isCurrentMonth={isCurrentMonth}
              isFutureMonth={isFutureMonth}
              isReachedMonth={isReachedMonth}
              currentDayOfMonth={isCurrentMonth ? currentDateParts?.day ?? null : null}
              raceArrivalFeedbackActive={raceArrivalFeedbackActive}
              isAnimatedCurrentDay={Boolean(
                day != null &&
                isCurrentMonth &&
                currentDateParts?.day === day,
              )}
              onHover={onCellHover}
            />
          );
        })}
      </div>

      {!hasRaces && phase.emptyText && (
        <p className="mt-3 text-[10px] leading-relaxed text-text-muted/60">
          {phase.emptyText}
        </p>
      )}
    </div>
  );
}

function DayCell({
  day,
  race,
  isNext,
  phase,
  year,
  month,
  isConvocationDay,
  isPreSpecialDay,
  isSpecialRace,
  otherCategoryRaces,
  isCurrentMonth,
  isFutureMonth,
  isReachedMonth,
  currentDayOfMonth,
  raceArrivalFeedbackActive,
  isAnimatedCurrentDay,
  onHover,
}) {
  if (day == null) return <div className="aspect-square" />;

  const otherCategoryCount = otherCategoryRaces?.length ?? 0;
  const dateKey = formatIsoDateKey(year, month, day);
  const isReachedCurrentMonthDay = isCurrentMonth && currentDayOfMonth != null && day <= currentDayOfMonth;
  const isFutureCurrentMonthDay = isCurrentMonth && currentDayOfMonth != null && day > currentDayOfMonth;
  const visualMonthState = isFutureMonth ? "future-month" : "active-month";
  const currentMonthProgress = isAnimatedCurrentDay
    ? "current"
    : isReachedCurrentMonthDay
      ? "reached"
      : isFutureCurrentMonthDay
        ? "future"
        : "outside";

  if (!race) {
    const dayBg =
      phase.type === "mercado"
        ? "bg-status-yellow/[0.04] border-status-yellow/10"
        : phase.type === "especial"
          ? "bg-status-purple/[0.04] border-status-purple/10"
          : "bg-white/[0.04] border-white/[0.07]";
    const preSpecialBg = isPreSpecialDay
      ? "bg-white/[0.04] border-white/[0.07]"
      : dayBg;
    const convocationBg = isConvocationDay
      ? "border-orange-300/55 bg-gradient-to-br from-orange-400/25 via-amber-400/18 to-orange-600/22 text-orange-50 shadow-[0_0_16px_rgba(251,146,60,0.18)]"
      : preSpecialBg;
    const visibleBg = isAnimatedCurrentDay
      ? "border-accent-primary/80 bg-accent-primary/[0.06] text-text-primary"
      : convocationBg;
    const dayNumberTone = isAnimatedCurrentDay
      ? ""
      : isFutureCurrentMonthDay || isFutureMonth
        ? "text-text-muted/50"
        : "text-text-secondary";

    return (
      <div
        data-testid={`calendar-day-${formatIsoDateKey(year, month, day)}`}
        data-animated-current-day={isAnimatedCurrentDay ? "true" : "false"}
        data-current-calendar-day={isAnimatedCurrentDay ? "true" : "false"}
        data-animated-visual={isAnimatedCurrentDay ? "true" : "false"}
        data-convocation-day={isConvocationDay ? "true" : "false"}
        data-pre-special-day={isPreSpecialDay ? "true" : "false"}
        data-visual-month-state={visualMonthState}
        data-current-month-progress={currentMonthProgress}
        data-other-category-count={String(otherCategoryCount)}
        className={`relative flex aspect-square items-center justify-center border text-[10px] transition-all duration-300 ${otherCategoryCount > 0 ? "cursor-pointer" : ""} ${dayNumberTone} ${visibleBg}`}
        onMouseEnter={otherCategoryCount > 0 ? (event) => {
          const rect = event.currentTarget.getBoundingClientRect();
          onHover({ race: null, otherRaces: otherCategoryRaces, rect });
        } : undefined}
        onMouseLeave={otherCategoryCount > 0 ? () => onHover(null) : undefined}
      >
        {isAnimatedCurrentDay && <CurrentDayRail />}
        {otherCategoryCount > 0 && <OtherCategoryDots dateKey={dateKey} races={otherCategoryRaces} />}
        {day}
      </div>
    );
  }

  const image = getTrackImageSrc(race);
  const isConcluida = race.status === "Concluida";
  const shouldFlashRaceArrival = Boolean(raceArrivalFeedbackActive && isAnimatedCurrentDay);

  const baseOverlayClass = isNext
    ? "bg-accent-primary/28 ring-1 ring-inset ring-accent-primary/85 shadow-[inset_0_0_18px_rgba(88,166,255,0.22)]"
    : isConcluida
      ? "bg-black/65"
      : "bg-black/28";
  const overlayClass = isFutureCurrentMonthDay || isFutureMonth ? "bg-black/52" : baseOverlayClass;
  const animatedOverlayClass = isAnimatedCurrentDay
    ? "bg-black/20 ring-1 ring-inset ring-accent-primary/80"
    : overlayClass;
  const specialRaceFrameClass = isSpecialRace && !isAnimatedCurrentDay
    ? "ring-1 ring-inset ring-status-purple/65 shadow-[0_0_16px_rgba(168,85,247,0.18)]"
    : "";
  const raceImageTone = isFutureCurrentMonthDay || isFutureMonth
    ? "opacity-65 saturate-[0.8] brightness-[0.82]"
    : isConcluida
      ? "saturate-50"
      : "";

  return (
    <div
      data-testid={`calendar-day-${formatIsoDateKey(year, month, day)}`}
      data-animated-current-day={isAnimatedCurrentDay ? "true" : "false"}
      data-current-calendar-day={isAnimatedCurrentDay ? "true" : "false"}
      data-animated-visual={isAnimatedCurrentDay ? "true" : "false"}
      data-convocation-day={isConvocationDay ? "true" : "false"}
      data-pre-special-day={isPreSpecialDay ? "true" : "false"}
      data-special-race-day={isSpecialRace ? "true" : "false"}
      data-visual-month-state={visualMonthState}
      data-current-month-progress={currentMonthProgress}
      data-race-arrival-feedback={shouldFlashRaceArrival ? "true" : "false"}
      data-other-category-count={String(otherCategoryCount)}
      className={[
        "group relative aspect-square cursor-pointer overflow-hidden transition-transform duration-300",
        specialRaceFrameClass,
      ].join(" ")}
      onMouseEnter={(event) => {
        const rect = event.currentTarget.getBoundingClientRect();
        onHover({ race, otherRaces: otherCategoryRaces, rect });
      }}
      onMouseLeave={() => onHover(null)}
    >
      {image ? (
        <img
          src={image}
          alt={race.track_name}
          className={[
            "absolute inset-0 h-full w-full object-cover transition-transform duration-300 group-hover:scale-110",
            raceImageTone,
          ].join(" ")}
          draggable={false}
        />
      ) : (
        <div className="absolute inset-0 bg-gradient-to-br from-slate-600 to-slate-900" />
      )}

      <div className={`absolute inset-0 transition-all duration-300 ${animatedOverlayClass}`} />
      {shouldFlashRaceArrival && (
        <div
          data-testid="calendar-race-arrival-flash"
          className="calendar-race-arrival-flash pointer-events-none absolute inset-[2px] z-20 border border-accent-hover/70 bg-accent-primary/10"
        />
      )}

      <div className="absolute inset-0">
        {isAnimatedCurrentDay && <CurrentDayRail />}
        {isSpecialRace && !isAnimatedCurrentDay && (
          <span className="absolute bottom-[3px] left-[3px] rounded bg-status-purple/80 px-1 py-[1px] text-[7px] font-bold uppercase tracking-[0.08em] text-white">
            Esp
          </span>
        )}
      </div>
      {otherCategoryCount > 0 && <OtherCategoryDots dateKey={dateKey} races={otherCategoryRaces} />}
    </div>
  );
}

function OtherCategoryDots({ dateKey, races = [] }) {
  const visibleRaces = races.slice(0, 3);
  const hiddenRaceCount = Math.max(races.length - visibleRaces.length, 0);

  return (
    <div
      data-testid={`calendar-other-categories-${dateKey}`}
      className="pointer-events-none absolute bottom-[3px] right-[3px] flex items-center gap-[3px] opacity-90"
    >
      {visibleRaces.map((race, index) => (
        <span
          key={race.id ?? `${dateKey}-${race.categoria}-${index}`}
          data-testid="calendar-other-category-dot"
          data-category={race.categoria}
          className="h-[5px] w-[5px] rounded-full ring-1 ring-black/45 shadow-[0_0_4px_rgba(0,0,0,0.55)]"
          style={{ backgroundColor: getCategoryColor(race.categoria, "#9ca3af") }}
        />
      ))}
      {hiddenRaceCount > 0 && (
        <span className="text-[7px] font-semibold leading-none text-white/55">
          +{hiddenRaceCount}
        </span>
      )}
    </div>
  );
}

function CalendarTooltip({ race, otherRaces = [], cellRect }) {
  const tooltipRef = useRef(null);
  const [tooltipSize, setTooltipSize] = useState({ width: 224, height: 176 });

  useLayoutEffect(() => {
    if (!tooltipRef.current) return;

    const nextSize = {
      width: tooltipRef.current.offsetWidth,
      height: tooltipRef.current.offsetHeight,
    };

    setTooltipSize((current) => (
      current.width === nextSize.width && current.height === nextSize.height
        ? current
        : nextSize
    ));
  }, [otherRaces, race?.id]);

  const style = getRaceTooltipStyle(
    cellRect,
    {},
    tooltipSize,
    { verticalOffset: race ? 0 : 20 },
  );

  const tooltipElement = (
    <div data-testid="calendar-tooltip" style={style}>
      <div
        ref={tooltipRef}
        data-testid="calendar-tooltip-surface"
        className="w-[42rem] max-w-[calc(100vw-24px)] space-y-2"
      >
        {race ? (
          <OtherCategoryRaceTicket
            race={race}
            testId="calendar-tooltip-race-ticket"
            logoTestId="calendar-tooltip-category-logo"
          />
        ) : null}

        {otherRaces.length > 0 ? (
          <div className="space-y-2">
            {otherRaces.map((otherRace) => (
              <OtherCategoryRaceTicket
                key={otherRace.id}
                race={otherRace}
                compact={Boolean(race)}
              />
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );

  return createPortal(tooltipElement, document.body);
}

function OtherCategoryRaceTicket({
  race,
  compact = false,
  testId = "calendar-tooltip-other-race-ticket",
  logoTestId = "calendar-tooltip-other-category-logo",
}) {
  const categoryLogo = CATEGORY_LOGOS[race.categoria] ?? null;
  const categoryColor = getCategoryColor(race.categoria, "#E73F47");
  const isConcluida = race.status === "Concluida";
  const weatherValue = isConcluida ? weatherLabel(race.clima) : i18n.t("calendar.tbd");

  return (
    <div
      data-testid={testId}
      className={[
        "relative grid items-center overflow-hidden rounded-[28px] border border-white/15 bg-[rgba(13,19,32,0.94)] shadow-[0_24px_70px_rgba(0,0,0,0.42)]",
        compact
          ? "grid-cols-[90px_minmax(0,1fr)_28px] gap-2 px-2 py-2 pr-8"
          : "grid-cols-[200px_minmax(0,1fr)_52px] gap-4 px-4 py-4 pr-[72px]",
      ].join(" ")}
      style={{
        background: `radial-gradient(circle at 13% 50%, ${categoryColor}4d, transparent 34%), linear-gradient(90deg, ${categoryColor}2e, rgba(255,255,255,0.025) 34%, transparent 72%), rgba(13,19,32,0.94)`,
      }}
    >
      {categoryLogo ? (
        <img
          data-testid={logoTestId}
          src={categoryLogo}
          alt={categoryLabel(race.categoria)}
          className={[
            "object-contain drop-shadow-[0_4px_14px_rgba(0,0,0,0.6)]",
            compact ? "h-16 w-[90px]" : "h-32 w-[200px]",
          ].join(" ")}
          style={{ filter: `drop-shadow(0 14px 34px ${categoryColor}73) drop-shadow(0 4px 14px rgba(0,0,0,0.6))` }}
          draggable={false}
        />
      ) : null}

      <div className="min-w-0">
        <div className="flex items-center justify-between gap-3 text-[10px] font-extrabold text-text-muted">
          <span
            className="kcal shrink-0 rounded-full border px-2.5 py-1 text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]"
            style={{ borderColor: `${categoryColor}75`, backgroundColor: `${categoryColor}2e` }}
          >
            Etapa {race.rodada}
          </span>
          <span className="truncate">{categoryLabel(race.categoria)}</span>
        </div>

        <p
          className={[
            "kcal mt-2 truncate font-black leading-none tracking-[-0.055em] text-white",
            compact ? "text-lg" : "text-[34px]",
          ].join(" ")}
        >
          {race.track_name}
        </p>

        <div className="mt-3 flex flex-wrap gap-1.5">
          <TicketDetail label={i18n.t("calendar.detail.duration")} value={`${race.duracao_corrida_min} min`} testId="calendar-tooltip-ticket-detail-duration" />
          <TicketDetail label={i18n.t("calendar.detail.weather")} value={weatherValue} />
          <TicketDetail label={i18n.t("calendar.detail.status")} value={isConcluida ? i18n.t("calendar.detail.done") : i18n.t("calendar.detail.pending")} />
        </div>
      </div>

      <div
        data-testid="calendar-tooltip-ticket-barcode"
        className="absolute bottom-0 right-0 top-0 grid w-12 place-items-center border-l border-dashed border-white/20 opacity-85"
        aria-hidden="true"
      >
        <div
          className="h-full w-[28px] opacity-45"
          style={{
            background: "repeating-linear-gradient(0deg, #fff 0 2px, transparent 2px 5px, #fff 5px 7px, transparent 7px 10px, #fff 10px 15px, transparent 15px 18px, #fff 18px 20px, transparent 20px 25px, #fff 25px 31px, transparent 31px 34px, #fff 34px 36px, transparent 36px 42px)",
          }}
        />
      </div>
    </div>
  );
}

function TicketDetail({ label, value, testId }) {
  return (
    <div
      data-testid={testId}
      className="min-w-[76px] rounded-full border border-white/8 bg-white/[0.052] px-2 py-1.5 text-center"
    >
      <b className="block text-[8px] font-bold uppercase leading-none tracking-[0.12em] text-text-muted">
        {label}
      </b>
      <span className="mt-1 block text-[10px] font-extrabold leading-none text-white">
        {value}
      </span>
    </div>
  );
}

function RaceTooltip({ race, cellRect }) {
  const image = getTrackImageSrc(race);
  const isConcluida = race.status === "Concluida";
  const weatherValue = isConcluida ? weatherLabel(race.clima) : i18n.t("calendar.tbd");
  const tooltipRef = useRef(null);
  const [tooltipSize, setTooltipSize] = useState({ width: 208, height: 176 });

  useLayoutEffect(() => {
    if (!tooltipRef.current) return;

    const nextSize = {
      width: tooltipRef.current.offsetWidth,
      height: tooltipRef.current.offsetHeight,
    };

    setTooltipSize((current) => (
      current.width === nextSize.width && current.height === nextSize.height
        ? current
        : nextSize
    ));
  }, [race.id]);

  const style = getRaceTooltipStyle(cellRect, {}, tooltipSize);

  return (
    <div style={style}>
      <div
        ref={tooltipRef}
        className="w-52 overflow-hidden rounded-xl border border-white/20 shadow-2xl"
        style={{ background: "rgba(12,18,32,0.97)", backdropFilter: "blur(20px)" }}
      >
        {image ? (
          <div className="relative h-28 w-full overflow-hidden">
            <img src={image} alt={race.track_name} className="h-full w-full object-cover" draggable={false} />
            <div className="absolute inset-0 bg-gradient-to-t from-black/75 via-black/20 to-transparent" />
            <div className="absolute bottom-2 left-3 right-3">
              <p className="text-[11px] font-bold leading-tight text-white drop-shadow">
                {race.track_name}
              </p>
            </div>
          </div>
        ) : (
          <div className="flex h-16 items-end bg-gradient-to-br from-slate-700 to-slate-900 p-3">
            <p className="text-[11px] font-bold text-white">{race.track_name}</p>
          </div>
        )}

        <div className="space-y-[5px] p-3">
          <DetailRow label={i18n.t("calendar.detail.round")} value={`R${race.rodada}`} accent />
          <DetailRow label={i18n.t("calendar.detail.duration")} value={`${race.duracao_corrida_min} min`} />
          <DetailRow label={i18n.t("calendar.detail.weather")} value={weatherValue} />
          <DetailRow
            label={i18n.t("calendar.detail.status")}
            value={isConcluida ? i18n.t("calendar.detail.done") : i18n.t("calendar.detail.pending")}
            valueClass={isConcluida ? "text-status-green" : "text-text-secondary"}
          />
        </div>
      </div>
    </div>
  );
}

function DetailRow({ label, value, accent = false, valueClass = "" }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-[10px] text-text-muted">{label}</span>
      <span
        className={[
          "text-[10px] font-semibold",
          accent ? "text-accent-primary" : "text-text-primary",
          valueClass,
        ].join(" ")}
      >
        {value}
      </span>
    </div>
  );
}

export default CalendarTab;
