import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { createPortal } from "react-dom";

import GlassCard from "../../components/ui/GlassCard";
import useCareerStore from "../../stores/useCareerStore";
import { getCategoryColor } from "../../utils/categoryColors";
import { categoryLabel } from "../../utils/formatters";

// Imagens por track_id
const TRACK_IMAGES = {
  8: "/utilities/tracks/summitpoint.png",
  9: "/utilities/tracks/summitpoint.png",
  14: "/utilities/tracks/limerock.jpeg",
  47: "/utilities/tracks/lagunaseca.png",
  166: "/utilities/tracks/okayama.png",
  261: "/utilities/tracks/oultonpark.jpeg",
  300: null,
  301: null,
  325: "/utilities/tracks/Tsukuba.png",
  341: "/utilities/tracks/oultonpark.jpeg",
  449: "/utilities/tracks/motorsport arena.png",
  451: "/utilities/tracks/rudskogen.jpeg",
  489: "/utilities/tracks/ledenon.png",
  202: "/utilities/tracks/oranpark.png",
  440: "/utilities/tracks/winton.jpeg",
  515: "/utilities/tracks/Navarra.png",
  554: "/utilities/tracks/charlotte.png",
  45: null,
  51: null,
  52: null,
  53: null,
  58: "/utilities/tracks/virginia.jpeg",
  67: null,
};

const TRACK_IMAGE_FILES = [
  { match: ["charlotte"], file: "charlotte.png" },
  { match: ["laguna seca"], file: "lagunaseca.png" },
  { match: ["lime rock"], file: "limerock.jpeg" },
  { match: ["okayama"], file: "okayama.png" },
  { match: ["oulton"], file: "oultonpark.jpeg" },
  { match: ["snetterton"], file: "snetterton.jpeg" },
  { match: ["summit point", "jefferson"], file: "summitpoint.png" },
  { match: ["tsukuba"], file: "Tsukuba.png" },
  { match: ["virginia international raceway", "vir full", "vir patriot"], file: "virginia.jpeg" },
  { match: ["ledenon"], file: "ledenon.png" },
  { match: ["oschersleben", "motorsport arena"], file: "motorsport arena.png" },
  { match: ["navarra"], file: "Navarra.png" },
  { match: ["oran park"], file: "oranpark.png" },
  { match: ["rudskogen"], file: "rudskogen.jpeg" },
  { match: ["winton"], file: "winton.jpeg" },
];

const CATEGORY_LOGOS = {
  mazda_rookie: "/utilities/categorias/MX5%20ROOKIE.png",
  toyota_rookie: "/utilities/categorias/GR%20ROOKIE.png",
  mazda_amador: "/utilities/categorias/MX5%20CUP.png",
  toyota_amador: "/utilities/categorias/GR%20CUP.png",
  bmw_m2: "/utilities/categorias/M2%20CUP.png",
  production_challenger: "/utilities/categorias/PRODUCTION.png",
  gt4: "/utilities/categorias/GT4.png",
  gt3: "/utilities/categorias/GT3.png",
  endurance: "/utilities/categorias/ENDURANCE.png",
};

// Constantes visuais
const MONTH_NAMES = [
  "Janeiro", "Fevereiro", "Março", "Abril", "Maio", "Junho",
  "Julho", "Agosto", "Setembro", "Outubro", "Novembro", "Dezembro",
];

const WEEKDAY_LABELS = ["D", "S", "T", "Q", "Q", "S", "S"];
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
function getMonthPhase(monthIndex) {
  if (monthIndex === 0) {
    return {
      type: "mercado",
      label: "Mercado",
      badgeClass: "bg-status-yellow/15 text-status-yellow",
      cardClass: "border-status-yellow/25",
      emptyText: "Período de transferências e contratos para a temporada.",
    };
  }
  if (monthIndex <= 7) {
    return {
      type: "regular",
      label: "Temporada Regular",
      badgeClass: "bg-accent-primary/15 text-accent-primary",
      cardClass: "",
      emptyText: null,
    };
  }
  return {
    type: "especial",
    label: "Bloco Especial",
      badgeClass: "bg-status-purple/15 text-status-purple",
      cardClass: "border-status-purple/25",
      emptyText: "Etapas do bloco especial e janela de convocação.",
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
  if (value === "HeavyRain") return "Chuva forte";
  if (value === "Wet") return "Chuva";
  if (value === "Damp") return "Úmido";
  return "Seco";
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
        const specialCategory = acceptedSpecialOffer?.special_category ?? null;
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
          setError(typeof err === "string" ? err : "Não foi possível carregar o calendário.");
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
  }, [acceptedSpecialOffer?.special_category, careerId, playerTeam?.categoria, season?.rodada_atual]);

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
    () => buildConvocationWindowDateKeys(
      firstSpecialRaceDate,
      specialWindowState?.total_days ?? 7,
      seasonYear,
    ),
    [firstSpecialRaceDate, seasonYear, specialWindowState?.total_days],
  );
  const convocationStartDateKey = useMemo(
    () => getConvocationStartDateKey(
      firstSpecialRaceDate,
      specialWindowState?.total_days ?? 7,
      seasonYear,
    ),
    [firstSpecialRaceDate, seasonYear, specialWindowState?.total_days],
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
          <p className="text-[11px] uppercase tracking-[0.22em] text-accent-primary">Calendário</p>
          <h2 className="mt-2 text-3xl font-semibold text-text-primary">
            {categoryLabel(playerTeam?.categoria)}
          </h2>
        </div>
        <p className="text-sm text-text-secondary">
          {completed}/{displayedCalendar.length} etapas concluídas
        </p>
      </div>

      <div data-testid="calendar-legend" className="mt-4 flex flex-wrap items-center gap-x-5 gap-y-2">
        <LegendItem color="bg-status-yellow" label="Mercado" />
        <LegendItem color="bg-orange-400" label="Convocação" />
        <LegendItem color="bg-status-purple" label="Bloco Especial" />
      </div>

      {loading ? (
        <p className="mt-8 text-sm text-text-secondary">Carregando calendário da temporada...</p>
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
  raceArrivalFeedbackActive,
  showAnimatedProgress,
  onCellHover,
}) {
  const phase = getMonthPhase(month);
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

  return (
    <div
      className={[
        "relative overflow-hidden rounded-2xl border bg-white/[0.03] p-4 backdrop-blur-sm transition-all duration-300",
        phase.cardClass || "border-white/10",
        isCurrentMonth
          ? "border-accent-primary/30 bg-[linear-gradient(180deg,rgba(88,166,255,0.075),rgba(255,255,255,0.03)_38%,rgba(255,255,255,0.02)_100%)] shadow-[0_18px_44px_rgba(88,166,255,0.11)]"
          : "",
      ].join(" ")}
      data-testid={`calendar-month-${monthIso}`}
      data-active-month-window={isCurrentMonth ? "true" : "false"}
      data-animated-month={isAnimatedMonth ? "true" : "false"}
    >
      {isAnimatedMonth && (
        <div className="absolute left-4 right-4 top-0 h-[3px] rounded-b-full bg-white/8">
          <div
            data-testid={`calendar-progress-${monthIso}`}
            data-animated-month="true"
            className="h-full rounded-b-full bg-gradient-to-r from-accent-primary/35 via-accent-primary to-accent-hover/70 shadow-[0_0_18px_rgba(88,166,255,0.35)] transition-[width] duration-200"
            style={{ width: `${animatedProgress}%` }}
          />
        </div>
      )}

      <div className="mb-2 flex items-center justify-between gap-2">
        <span className={`text-[13px] font-semibold ${isCurrentMonth ? "text-accent-hover" : "text-text-primary"}`}>
          {MONTH_NAMES[month]}
        </span>
        <span
          className={`rounded-full px-2 py-0.5 text-[9px] font-semibold uppercase tracking-wider ${phase.badgeClass}`}
        >
          {phase.label}
        </span>
      </div>

      <div className="grid grid-cols-7 gap-[3px]">
        {WEEKDAY_LABELS.map((weekday, index) => (
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
      ? "border-accent-primary/75 bg-[linear-gradient(180deg,rgba(88,166,255,0.15),rgba(88,166,255,0.05))] text-text-primary shadow-[0_0_0_1px_rgba(88,166,255,0.14),0_12px_24px_rgba(0,0,0,0.18)]"
      : convocationBg;
    const dayNumberTone = isAnimatedCurrentDay
      ? ""
      : isFutureCurrentMonthDay || isFutureMonth
        ? "text-text-muted/50"
        : "text-text-primary";

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
        className={`relative flex aspect-square items-center justify-center rounded border text-[10px] transition-all duration-300 ${otherCategoryCount > 0 ? "cursor-pointer" : ""} ${dayNumberTone} ${visibleBg}`}
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
    ? "bg-[linear-gradient(180deg,rgba(88,166,255,0.16),rgba(88,166,255,0.045))] ring-1 ring-inset ring-accent-primary/70 shadow-[0_0_20px_rgba(88,166,255,0.18)]"
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
        "group relative aspect-square cursor-pointer overflow-hidden rounded-lg transition-transform duration-300",
        specialRaceFrameClass,
        isAnimatedCurrentDay ? "shadow-[0_0_0_1px_rgba(88,166,255,0.14),0_12px_24px_rgba(0,0,0,0.18)]" : "",
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
          className="calendar-race-arrival-flash pointer-events-none absolute inset-[2px] z-20 rounded-md border border-accent-hover/70 bg-accent-primary/10"
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
  const weatherValue = isConcluida ? weatherLabel(race.clima) : "A definir";

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
            className="shrink-0 rounded-full border px-2.5 py-1 text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]"
            style={{ borderColor: `${categoryColor}75`, backgroundColor: `${categoryColor}2e` }}
          >
            Etapa {race.rodada}
          </span>
          <span className="truncate">{categoryLabel(race.categoria)}</span>
        </div>

        <p
          className={[
            "mt-2 truncate font-black leading-none tracking-[-0.055em] text-white",
            compact ? "text-lg" : "text-[34px]",
          ].join(" ")}
        >
          {race.track_name}
        </p>

        <div className="mt-3 flex flex-wrap gap-1.5">
          <TicketDetail label="Duração" value={`${race.duracao_corrida_min} min`} testId="calendar-tooltip-ticket-detail-duration" />
          <TicketDetail label="Clima" value={weatherValue} />
          <TicketDetail label="Status" value={isConcluida ? "Concluída" : "Pendente"} />
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
  const weatherValue = isConcluida ? weatherLabel(race.clima) : "A definir";
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
          <DetailRow label="Rodada" value={`R${race.rodada}`} accent />
          <DetailRow label="Duração" value={`${race.duracao_corrida_min} min`} />
          <DetailRow label="Clima" value={weatherValue} />
          <DetailRow
            label="Status"
            value={isConcluida ? "Concluída" : "Pendente"}
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
