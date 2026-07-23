import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";

import useDeferredLoading from "../../hooks/useLoading";
import useCareerStore from "../../stores/useCareerStore";
import { getCategoryColor } from "../../utils/categoryColors";
import { categoryLabel } from "../../utils/formatters";
import { isLegacySeasonPhase } from "../../utils/seasonPhases";
import i18n from "../../i18n/index.js";
import { currentLang } from "../../i18n/format.js";
import {
  ALL_CALENDAR_CATEGORIES,
  CATEGORY_LOGOS,
  buildMonthCells,
  formatIsoDateKey,
  getMonthPhaseLabel,
  getMonthPhaseType,
  getTrackImageSrc,
  parseDisplayDate,
  weatherLabel,
} from "../../utils/calendarShared.js";
import { TRACK_COUNTRIES } from "../../utils/trackCountries.js";
import { getRaceTooltipStyle } from "./CalendarTab";

// Superfície de painel: separa do fundo por elevação (fundo sutil + rim light no
// topo + sombra suave), sem borda — para aliviar o excesso de contornos. O rim é um
// brilho de 1px no topo (não uma borda fechada) que dá a leitura de "levantado".
const SURFACE = "bg-white/[0.03] shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_24px_60px_-24px_rgba(0,0,0,0.65)] backdrop-blur-[10px]";

// Cor da fase por mês (barra na régua do ano + tom sutil das células vazias).
const PHASE_COLOR = {
  mercado: "var(--status-yellow, #d4a017)",
  regular: "var(--accent-primary, #58a6ff)",
  especial: "var(--status-purple, #a371f7)",
  encerramento: "rgba(255,255,255,0.45)",
};

function withFetchedCategory(entries = [], category) {
  return entries.map((entry) => ({
    ...entry,
    categoria: entry.categoria ?? category,
  }));
}

function monthShortLabels(lang = currentLang()) {
  const fmt = new Intl.DateTimeFormat(lang, { month: "short" });
  return Array.from({ length: 12 }, (_, m) => {
    const s = fmt.format(new Date(2000, m, 1)).replace(".", "");
    return s.charAt(0).toUpperCase() + s.slice(1);
  });
}

function weekdayShortLabels(lang = currentLang()) {
  const fmt = new Intl.DateTimeFormat(lang, { weekday: "short" });
  // 2023-01-01 foi um domingo → começa no domingo (igual buildMonthCells).
  return Array.from({ length: 7 }, (_, d) => {
    const s = fmt.format(new Date(2023, 0, 1 + d)).replace(".", "");
    return s.charAt(0).toUpperCase() + s.slice(1);
  });
}

function compareMonth(year, month, current) {
  if (!current) return 0;
  if (year !== current.year) return year < current.year ? -1 : 1;
  if (month === current.month) return 0;
  return month < current.month ? -1 : 1;
}

function sortByCategoryOrder(categories) {
  return [...categories].sort(
    (a, b) => ALL_CALENDAR_CATEGORIES.indexOf(a) - ALL_CALENDAR_CATEGORIES.indexOf(b),
  );
}

function CalendarTabRedesignV2({ activeTab, raceArrivalFeedbackActive = false }) {
  const { t } = useTranslation();
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const nextRace = useCareerStore((state) => state.nextRace);
  const season = useCareerStore((state) => state.season);
  const acceptedSpecialOffer = useCareerStore((state) => state.acceptedSpecialOffer);
  const calendarDisplayDate = useCareerStore((state) => state.calendarDisplayDate);
  const temporalSummary = useCareerStore((state) => state.temporalSummary);

  const [calendar, setCalendar] = useState([]);
  const [specialCalendar, setSpecialCalendar] = useState([]);
  const [otherCalendars, setOtherCalendars] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [selectedMonth, setSelectedMonth] = useState(null);
  const [showAllEvents, setShowAllEvents] = useState(false);
  const [tooltip, setTooltip] = useState(null);

  const showLoadingUI = useDeferredLoading(loading);
  const isLegacyCalendar = isLegacySeasonPhase(season?.fase);

  // Reseta o mês selecionado ao trocar de carreira (deixa o auto-pick reescolher).
  useEffect(() => {
    setSelectedMonth(null);
  }, [careerId]);

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
          invoke("get_calendar_for_category", { careerId, category: playerTeam.categoria })
            .then((entries) => withFetchedCategory(entries, playerTeam.categoria)),
          specialCategory
            ? invoke("get_calendar_for_category", { careerId, category: specialCategory })
              .then((entries) => withFetchedCategory(entries, specialCategory))
            : Promise.resolve([]),
        ]);

        if (!mounted) return;
        setCalendar(regularEntries);
        setSpecialCalendar(specialEntries);
        setLoading(false);

        Promise.all(
          otherCategories.map((category) => (
            invoke("get_calendar_for_category", { careerId, category })
              .then((entries) => withFetchedCategory(entries, category))
          )),
        )
          .then((otherEntries) => {
            if (mounted) setOtherCalendars(otherEntries.flat());
          })
          .catch(() => {
            if (mounted) setOtherCalendars([]);
          });
      } catch (err) {
        if (mounted) setError(typeof err === "string" ? err : i18n.t("calendar.loadError"));
      } finally {
        if (mounted) setLoading(false);
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
      map[formatIsoDateKey(parsed.year, parsed.month, parsed.day)] = {
        ...race,
        _isSpecialRace: race.season_phase === "BlocoEspecial",
      };
    }
    return map;
  }, [displayedCalendar]);

  const otherCategoryRacesByDate = useMemo(() => {
    const map = {};
    for (const race of otherCalendars) {
      const parsed = parseDisplayDate(race.display_date);
      if (!parsed) continue;
      const key = formatIsoDateKey(parsed.year, parsed.month, parsed.day);
      (map[key] ??= []).push(race);
    }
    return map;
  }, [otherCalendars]);

  const currentDateParts = useMemo(() => {
    if (activeTab !== "calendar") return null;
    return parseDisplayDate(calendarDisplayDate ?? temporalSummary?.current_display_date ?? null);
  }, [activeTab, calendarDisplayDate, temporalSummary]);

  const nextRaceEntry = useMemo(
    () => displayedCalendar.find((race) => race.id === nextRace?.id) ?? null,
    [displayedCalendar, nextRace?.id],
  );

  // Escolhe o mês inicial: mês atual → mês da próxima corrida → mês da 1ª etapa → 0.
  useEffect(() => {
    if (selectedMonth != null) return;
    const nextMonth = parseDisplayDate(nextRaceEntry?.display_date)?.month;
    const firstMonth = parseDisplayDate(displayedCalendar[0]?.display_date)?.month;
    const pick = currentDateParts?.month ?? nextMonth ?? firstMonth ?? 0;
    setSelectedMonth(pick);
  }, [selectedMonth, currentDateParts, nextRaceEntry, displayedCalendar]);

  const month = selectedMonth ?? currentDateParts?.month ?? 0;

  // Categorias que correm em cada mês (bolinhas da régua do ano).
  const monthCategories = useMemo(() => {
    const sets = Array.from({ length: 12 }, () => new Set());
    const add = (race) => {
      const parsed = parseDisplayDate(race.display_date);
      if (parsed) sets[parsed.month].add(race.categoria);
    };
    displayedCalendar.forEach(add);
    otherCalendars.forEach(add);
    return sets.map((s) => sortByCategoryOrder([...s]));
  }, [displayedCalendar, otherCalendars]);

  const legendCategories = useMemo(() => {
    const all = new Set(monthCategories.flat());
    if (playerTeam?.categoria) all.add(playerTeam.categoria);
    return sortByCategoryOrder([...all]);
  }, [monthCategories, playerTeam?.categoria]);

  const upcoming = useMemo(() => {
    return [...displayedCalendar]
      .filter((race) => race.display_date && race.status !== "Concluida")
      .sort((a, b) => a.display_date.localeCompare(b.display_date));
  }, [displayedCalendar]);

  const stats = useMemo(() => {
    const done = displayedCalendar.filter((race) => race.status === "Concluida").length;
    const tracks = new Set(displayedCalendar.map((race) => race.track_name).filter(Boolean)).size;
    const countries = new Set(
      displayedCalendar.map((race) => TRACK_COUNTRIES[race.track_name]).filter(Boolean),
    ).size;
    return { rounds: displayedCalendar.length, done, tracks, countries };
  }, [displayedCalendar]);

  const monthShort = monthShortLabels();
  const weekdays = weekdayShortLabels();
  // Sempre 6 linhas (42 células), como calendários padrão: assim a célula tem o
  // MESMO tamanho em todo mês (fev tem 4 semanas, mai tem 6) e ainda preenche a altura.
  const rawCells = buildMonthCells(seasonYear, month);
  const cells = rawCells.length < 42
    ? [...rawCells, ...Array(42 - rawCells.length).fill(null)]
    : rawCells;
  const gridRows = 6;
  const hasCurrent = Boolean(currentDateParts);

  function goToday() {
    const nextMonth = parseDisplayDate(nextRaceEntry?.display_date)?.month;
    setSelectedMonth(currentDateParts?.month ?? nextMonth ?? 0);
  }

  return (
    <div className="grid grid-cols-1 gap-4 xl:h-[calc(100vh-132px)] xl:min-h-[600px] xl:grid-cols-[minmax(0,1fr)_340px]">
      {/* ── Coluna principal ── */}
      <div className={`${SURFACE} flex min-h-0 flex-col overflow-hidden rounded-[28px] xl:h-full`}>
        {/* Header */}
        <div className="flex shrink-0 flex-col gap-2 px-6 py-4 sm:flex-row sm:items-end sm:justify-between">
          <div>
            <p className="kcal text-[11px] uppercase tracking-[0.24em] text-accent-primary">
              {t("calendar.title")}
            </p>
            <h2 className="kcal mt-2 text-3xl font-semibold uppercase italic tracking-tight text-text-primary">
              {categoryLabel(playerTeam?.categoria)}
            </h2>
          </div>
          <p className="text-sm text-text-secondary">
            {t("calendar.stepsDone", { completed: stats.done, total: stats.rounds })}
            {season?.ano ? ` · ${season.ano}` : ""}
          </p>
        </div>

        {/* Régua do ano */}
        <div className="flex shrink-0 items-center gap-2 px-5 py-2.5">
          <button
            type="button"
            aria-label={t("calendar.v2.prevMonth")}
            onClick={() => setSelectedMonth((m) => Math.max(0, (m ?? month) - 1))}
            className="grid h-[34px] w-[34px] shrink-0 place-items-center rounded-xl bg-white/[0.06] text-text-secondary transition-glass hover:bg-white/[0.11] hover:text-text-primary"
          >
            ‹
          </button>
          <button
            type="button"
            aria-label={t("calendar.v2.nextMonth")}
            onClick={() => setSelectedMonth((m) => Math.min(11, (m ?? month) + 1))}
            className="grid h-[34px] w-[34px] shrink-0 place-items-center rounded-xl bg-white/[0.06] text-text-secondary transition-glass hover:bg-white/[0.11] hover:text-text-primary"
          >
            ›
          </button>
          <button
            type="button"
            onClick={goToday}
            disabled={!hasCurrent}
            className="h-[34px] shrink-0 rounded-xl bg-white/[0.06] px-3.5 text-xs font-semibold text-text-primary transition-glass hover:bg-white/[0.11] disabled:opacity-40"
          >
            {t("calendar.v2.today")}
          </button>

          <div className="ml-1.5 grid flex-1 grid-cols-12 gap-1">
            {monthShort.map((label, index) => {
              const isSelected = index === month;
              const isCurrentMonth = hasCurrent && currentDateParts.month === index && currentDateParts.year === seasonYear;
              const dots = monthCategories[index].slice(0, 4);
              return (
                <button
                  key={index}
                  type="button"
                  onClick={() => setSelectedMonth(index)}
                  title={getMonthPhaseLabel(index, isLegacyCalendar)}
                  className={[
                    "relative rounded-xl px-0.5 pb-2 pt-1.5 text-center transition-glass",
                    isSelected
                      ? "bg-accent-primary/[0.20]"
                      : isCurrentMonth
                        ? "bg-accent-primary/[0.10]"
                        : "hover:bg-white/[0.06]",
                  ].join(" ")}
                >
                  {isCurrentMonth && (
                    <span
                      aria-hidden="true"
                      className="absolute right-1.5 top-1.5 h-1.5 w-1.5 rounded-full bg-accent-hover shadow-[0_0_6px_rgba(88,166,255,0.9)]"
                    />
                  )}
                  <span className={[
                    "kcal block text-[12px] italic tracking-[0.08em]",
                    isSelected ? "text-white" : isCurrentMonth ? "text-accent-hover" : "text-text-muted",
                  ].join(" ")}>
                    {label}
                  </span>
                  <span className="mt-1 flex min-h-[5px] items-center justify-center gap-[2px]">
                    {dots.map((cat) => (
                      <span
                        key={cat}
                        className="h-1 w-1 rounded-full"
                        style={{ backgroundColor: getCategoryColor(cat) }}
                      />
                    ))}
                  </span>
                  <span
                    className="absolute inset-x-2 bottom-[3px] h-[2px] rounded-full"
                    style={{ backgroundColor: PHASE_COLOR[getMonthPhaseType(index, isLegacyCalendar)] }}
                  />
                </button>
              );
            })}
          </div>
        </div>

        {/* Legenda: categorias + fases */}
        <div className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-1.5 px-6 pb-2.5">
          {legendCategories.map((cat) => (
            <span key={cat} className="flex items-center gap-1.5 text-[11px] text-text-secondary">
              <span className="h-2 w-2 rounded-full" style={{ backgroundColor: getCategoryColor(cat) }} />
              {categoryLabel(cat)}
            </span>
          ))}
          {legendCategories.length > 0 && <span className="h-3.5 w-px bg-white/12" />}
          <span className="flex items-center gap-1.5 text-[11px] text-text-secondary">
            <span className="h-2.5 w-2.5 rounded-sm bg-accent-primary/60" />{t("calendar.legend.temporada")}
          </span>
          <span className="flex items-center gap-1.5 text-[11px] text-text-secondary">
            <span className="h-2.5 w-2.5 rounded-sm bg-status-yellow" />{t("calendar.legend.pretemporada")}
          </span>
          <span className="flex items-center gap-1.5 text-[11px] text-text-secondary">
            <span className="h-2.5 w-2.5 rounded-sm bg-white/50" />{t("calendar.legend.encerramento")}
          </span>
        </div>

        {/* Grade do mês */}
        {loading ? (
          showLoadingUI ? (
            <p className="flex-1 px-6 pb-8 text-sm text-text-secondary">{t("calendar.loading")}</p>
          ) : <div className="flex-1" />
        ) : error ? (
          <div className="mx-6 mb-6 rounded-2xl border border-status-red/30 bg-status-red/10 px-4 py-3 text-sm text-status-red">
            {error}
          </div>
        ) : (
          <div className="flex min-h-0 flex-1 flex-col px-6 pb-5">
            <div className="mb-1.5 grid shrink-0 grid-cols-7 gap-1.5">
              {weekdays.map((label, index) => (
                <div key={index} className="text-center text-[10px] font-medium uppercase tracking-[0.1em] text-text-muted/60">
                  {label}
                </div>
              ))}
            </div>
            <div
              className="grid min-h-0 flex-1 grid-cols-7 gap-1.5"
              style={{ gridTemplateRows: `repeat(${gridRows}, minmax(0, 1fr))` }}
            >
              {cells.map((day, index) => (
                <DayCellV2
                  key={index}
                  day={day}
                  year={seasonYear}
                  month={month}
                  race={day != null ? racesByDate[formatIsoDateKey(seasonYear, month, day)] ?? null : null}
                  otherRaces={day != null ? otherCategoryRacesByDate[formatIsoDateKey(seasonYear, month, day)] ?? [] : []}
                  phaseType={getMonthPhaseType(month, isLegacyCalendar)}
                  nextRaceId={nextRace?.id}
                  currentDateParts={currentDateParts}
                  seasonYear={seasonYear}
                  raceArrivalFeedbackActive={raceArrivalFeedbackActive}
                  onHover={setTooltip}
                />
              ))}
            </div>
            {!loading && stats.rounds > 0 && cells.every((d) => d == null || !racesByDate[formatIsoDateKey(seasonYear, month, d)]) && (
              <p className="mt-3 text-[11px] text-text-muted/60">
                {t("calendar.v2.empty")} · {getMonthPhaseLabel(month, isLegacyCalendar)}
              </p>
            )}
          </div>
        )}
      </div>

      {/* ── Sidebar ── */}
      <div className="flex flex-col gap-4">
        <div className={`${SURFACE} overflow-hidden rounded-[28px]`}>
          <div className="flex items-center justify-between gap-2 px-5 pb-4 pt-5">
            <span className="kcal whitespace-nowrap text-xs uppercase italic tracking-[0.14em] text-text-secondary">
              {t("calendar.v2.upcoming")}
            </span>
            {nextRaceEntry && (
              <button
                type="button"
                onClick={goToday}
                className="shrink-0 whitespace-nowrap text-[10px] font-bold uppercase tracking-[0.08em] text-accent-primary transition-glass hover:text-accent-hover"
              >
                {t("calendar.v2.seeAll")}
              </button>
            )}
          </div>
          {upcoming.length === 0 ? (
            <p className="px-5 pb-5 text-[12px] text-text-muted">{t("calendar.v2.noUpcoming")}</p>
          ) : (
            <>
              <div className="flex flex-col gap-2.5 px-4 pb-4">
                {(showAllEvents ? upcoming : upcoming.slice(0, 5)).map((race) => (
                  <EventRow
                    key={race.id}
                    race={race}
                    monthShort={monthShort}
                    isNext={race.id === nextRace?.id}
                    t={t}
                  />
                ))}
              </div>
              {upcoming.length > 5 && (
                <button
                  type="button"
                  onClick={() => setShowAllEvents((v) => !v)}
                  className="mx-4 mb-4 flex items-center justify-center gap-1.5 rounded-2xl bg-white/[0.05] py-2.5 text-[11px] font-bold uppercase tracking-[0.1em] text-accent-primary transition-glass hover:bg-white/[0.09]"
                >
                  {showAllEvents ? t("calendar.v2.seeLess") : t("calendar.v2.seeMore")}
                  <span className={`transition-transform ${showAllEvents ? "rotate-180" : ""}`}>⌄</span>
                </button>
              )}
            </>
          )}
        </div>

        <div className={`${SURFACE} overflow-hidden rounded-[28px]`}>
          <div className="px-5 pb-3.5 pt-5">
            <span className="kcal text-xs uppercase italic tracking-[0.14em] text-text-secondary">
              {t("calendar.v2.summary")}
            </span>
          </div>
          <div className="grid grid-cols-2 gap-2.5 px-4 pb-5 sm:grid-cols-4 xl:grid-cols-2">
            <StatTile icon="calendar" tone="blue" value={stats.rounds} label={t("calendar.v2.statEvents")} />
            <StatTile icon="trophy" tone="amber" value={stats.done} label={t("calendar.v2.statDone")} />
            <StatTile icon="pin" tone="coral" value={stats.tracks} label={t("calendar.v2.statTracks")} />
            <StatTile icon="globe" tone="teal" value={stats.countries} label={t("calendar.v2.statCountries")} />
          </div>
        </div>
      </div>

      {tooltip && (
        <CalendarTicketTooltip
          race={tooltip.race}
          otherRaces={tooltip.otherRaces}
          cellRect={tooltip.rect}
        />
      )}
    </div>
  );
}

function DayCellV2({
  day,
  month,
  race,
  otherRaces,
  phaseType,
  nextRaceId,
  currentDateParts,
  seasonYear,
  raceArrivalFeedbackActive,
  onHover,
}) {
  if (day == null) return <div className="h-full min-h-[62px]" />;

  const monthOrder = compareMonth(seasonYear, month, currentDateParts);
  const isCurrentMonth = monthOrder === 0 && currentDateParts != null;
  const isCurrentDay = isCurrentMonth && currentDateParts.day === day;
  const isPast = monthOrder < 0 || (isCurrentMonth && day < currentDateParts.day);
  const isFuture = monthOrder > 0 || (isCurrentMonth && day > currentDateParts.day);

  const otherDots = otherRaces.slice(0, 3);
  const hiddenCount = Math.max(otherRaces.length - otherDots.length, 0);

  // Hover → ingresso (a corrida do dia + eventuais corridas de outras categorias).
  const canHover = Boolean(race) || otherRaces.length > 0;
  const hoverHandlers = canHover
    ? {
      onMouseEnter: (event) => onHover?.({
        race: race ?? null,
        otherRaces,
        rect: event.currentTarget.getBoundingClientRect(),
      }),
      onMouseLeave: () => onHover?.(null),
    }
    : {};

  const currentRail = isCurrentDay ? (
    <span
      aria-hidden="true"
      className="pointer-events-none absolute inset-y-0 left-0 z-20 w-[3px] rounded-r-full bg-gradient-to-b from-accent-hover to-accent-primary"
    />
  ) : null;

  const dotsNode = otherRaces.length > 0 ? (
    <div className="pointer-events-none absolute bottom-1.5 right-1.5 z-20 flex items-center gap-[3px]">
      {otherDots.map((r, i) => (
        <span
          key={r.id ?? i}
          className="h-[5px] w-[5px] rounded-full ring-1 ring-black/45"
          style={{ backgroundColor: getCategoryColor(r.categoria, "#9ca3af") }}
        />
      ))}
      {hiddenCount > 0 && <span className="text-[8px] font-semibold leading-none text-white/60">+{hiddenCount}</span>}
    </div>
  ) : null;

  if (!race) {
    const phaseTint =
      phaseType === "mercado"
        ? "bg-status-yellow/[0.06]"
        : phaseType === "especial"
          ? "bg-status-purple/[0.06]"
          : phaseType === "encerramento"
            ? "bg-white/[0.025]"
            : "bg-white/[0.05]";
    return (
      <div
        {...hoverHandlers}
        data-testid={`calendar-day-${formatIsoDateKey(seasonYear, month, day)}`}
        className={[
          "relative flex h-full min-h-[62px] items-start justify-start overflow-hidden rounded-xl p-1.5 transition-colors",
          isCurrentDay ? "bg-accent-primary/[0.14]" : phaseTint,
          !isCurrentDay && canHover ? "hover:bg-white/[0.07]" : "",
          isPast ? "opacity-50" : "",
          canHover ? "cursor-pointer" : "",
        ].join(" ")}
      >
        {currentRail}
        <span className={[
          "text-[12px] font-semibold tabular-nums",
          isCurrentDay ? "text-text-primary" : isFuture ? "text-text-muted/60" : "text-text-secondary",
        ].join(" ")}>
          {day}
        </span>
        {dotsNode}
      </div>
    );
  }

  const image = getTrackImageSrc(race);
  const isNext = nextRaceId === race.id;
  const isConcluida = race.status === "Concluida";
  const isSpecial = race._isSpecialRace;
  const flash = raceArrivalFeedbackActive && isCurrentDay;

  return (
    <div
      {...hoverHandlers}
      data-testid={`calendar-day-${formatIsoDateKey(seasonYear, month, day)}`}
      className={[
        "group relative h-full min-h-[62px] cursor-pointer overflow-hidden rounded-xl transition-transform duration-300 hover:-translate-y-0.5",
        isNext
          ? "shadow-[inset_0_0_0_2px_var(--accent-primary,#58a6ff),0_0_22px_-6px_rgba(88,166,255,0.7)]"
          : isCurrentDay
            ? "shadow-[inset_0_0_0_1.5px_rgba(88,166,255,0.85)]"
            : isSpecial
              ? "shadow-[inset_0_0_0_1px_rgba(163,113,247,0.55)]"
              : "",
      ].join(" ")}
    >
      {image ? (
        <img
          src={image}
          alt={race.track_name}
          draggable={false}
          className={[
            "absolute inset-0 h-full w-full object-cover transition-transform duration-300 group-hover:scale-110",
            isPast || isFuture ? "opacity-70 saturate-[0.8] brightness-[0.82]" : "saturate-[1.08] brightness-[1.06]",
            isConcluida ? "saturate-50" : "",
          ].join(" ")}
        />
      ) : (
        <div className="absolute inset-0 bg-gradient-to-br from-slate-600 to-slate-900" />
      )}
      <div className={[
        "absolute inset-0",
        isPast || isFuture ? "bg-black/55" : "bg-gradient-to-b from-black/5 via-black/25 to-black/68",
      ].join(" ")} />
      {flash && (
        <div
          data-testid="calendar-race-arrival-flash"
          className="calendar-race-arrival-flash pointer-events-none absolute inset-[2px] z-20 border border-accent-hover/70 bg-accent-primary/10"
        />
      )}
      {currentRail}

      <span className={[
        "absolute right-1.5 top-1.5 z-10 rounded px-1.5 py-[1px] text-[9px] font-bold tabular-nums",
        isNext ? "bg-accent-primary text-[#04121f]" : "bg-black/45 text-white/80",
      ].join(" ")}>
        {isSpecial ? "E" : "C"}{race.rodada}
      </span>
      <span className="kcal absolute inset-x-1.5 bottom-1.5 z-10 truncate text-[12.5px] font-bold uppercase italic leading-tight text-white drop-shadow-[0_1px_4px_rgba(0,0,0,0.85)]">
        {race.track_name}
      </span>
      {dotsNode}
    </div>
  );
}

function EventRow({ race, monthShort, isNext, t }) {
  const parsed = parseDisplayDate(race.display_date);
  const image = getTrackImageSrc(race);
  const color = getCategoryColor(race.categoria);

  return (
    <div className="flex items-center gap-3 rounded-2xl bg-white/[0.05] px-3 py-2.5 transition-glass hover:bg-white/[0.08]">
      <div className="w-[44px] shrink-0 rounded-xl bg-black/20 py-2 text-center">
        <div className="kcal text-[18px] font-bold italic leading-none tabular-nums text-text-primary">
          {parsed ? String(parsed.day).padStart(2, "0") : "--"}
        </div>
        <div className="mt-1 text-[8px] font-bold uppercase tracking-[0.12em] text-text-muted">
          {parsed ? monthShort[parsed.month] : ""}
        </div>
      </div>
      <div className="h-10 w-[52px] shrink-0 overflow-hidden rounded-lg bg-black/40">
        {image ? (
          <img src={image} alt={race.track_name} className="h-full w-full object-cover" draggable={false} />
        ) : (
          <div className="grid h-full w-full place-items-center" style={{ backgroundColor: `${color}26` }}>
            <span className="h-2 w-2 rounded-full" style={{ backgroundColor: color }} />
          </div>
        )}
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-[13px] font-semibold text-text-primary">{race.track_name}</div>
        <div className="truncate text-[11px] text-text-muted">
          {categoryLabel(race.categoria)} · {t("calendar.v2.raceNumber", { n: race.rodada })}
        </div>
      </div>
      <div className="shrink-0 text-right">
        {race.horario && (
          <div className="text-[12px] font-semibold tabular-nums text-text-secondary">{race.horario}</div>
        )}
        <div className={[
          "mt-0.5 text-[8px] font-extrabold uppercase tracking-[0.08em]",
          isNext ? "text-accent-hover" : "text-status-red",
        ].join(" ")}>
          {isNext ? t("calendar.v2.next") : t("calendar.v2.race")}
        </div>
      </div>
    </div>
  );
}

const STAT_TONE = {
  blue: "#58a6ff",
  amber: "#e3b341",
  coral: "#f0785a",
  teal: "#3fc2a8",
};

const STAT_ICON_PATHS = {
  calendar: (
    <>
      <rect x="3.5" y="4.5" width="17" height="16" rx="2.5" />
      <path d="M3.5 9.5h17M8 3v3M16 3v3" />
    </>
  ),
  trophy: (
    <>
      <path d="M8 21h8M12 17v4" />
      <path d="M7 4h10v5a5 5 0 0 1-10 0V4Z" />
      <path d="M7 5H4v2a3 3 0 0 0 3 3M17 5h3v2a3 3 0 0 1-3 3" />
    </>
  ),
  pin: (
    <>
      <path d="M12 21s6-5.4 6-10a6 6 0 1 0-12 0c0 4.6 6 10 6 10Z" />
      <circle cx="12" cy="11" r="2.3" />
    </>
  ),
  globe: (
    <>
      <circle cx="12" cy="12" r="8.2" />
      <path d="M3.8 12h16.4M12 3.8c2.2 2.4 3.4 5.2 3.4 8.2s-1.2 5.8-3.4 8.2c-2.2-2.4-3.4-5.2-3.4-8.2S9.8 6.2 12 3.8Z" />
    </>
  ),
};

function StatTile({ icon, tone = "blue", value, label }) {
  const color = STAT_TONE[tone] ?? STAT_TONE.blue;
  return (
    <div className="flex items-center gap-2.5 rounded-2xl bg-white/[0.05] p-3.5">
      <span
        className="grid h-[34px] w-[34px] shrink-0 place-items-center rounded-xl"
        style={{ backgroundColor: `${color}1f`, color }}
      >
        <svg
          viewBox="0 0 24 24"
          width="17"
          height="17"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.7"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          {STAT_ICON_PATHS[icon]}
        </svg>
      </span>
      <div>
        <div className="kcal text-xl font-bold italic leading-none tabular-nums text-text-primary">{value}</div>
        <div className="mt-0.5 text-[10px] uppercase tracking-[0.06em] text-text-muted">{label}</div>
      </div>
    </div>
  );
}

// ── Ingresso (hover) ─────────────────────────────────────────────────────────
// Portado do calendário legado: hover num dia abre um "ingresso" com o evento.
function CalendarTicketTooltip({ race, otherRaces = [], cellRect }) {
  const tooltipRef = useRef(null);
  const [tooltipSize, setTooltipSize] = useState({ width: 224, height: 176 });

  useLayoutEffect(() => {
    if (!tooltipRef.current) return;
    const next = {
      width: tooltipRef.current.offsetWidth,
      height: tooltipRef.current.offsetHeight,
    };
    setTooltipSize((current) => (
      current.width === next.width && current.height === next.height ? current : next
    ));
  }, [otherRaces, race?.id]);

  const style = getRaceTooltipStyle(cellRect, {}, tooltipSize, { verticalOffset: race ? 0 : 20 });

  return createPortal(
    <div data-testid="calendar-tooltip" style={style}>
      <div ref={tooltipRef} className="w-[38rem] max-w-[calc(100vw-24px)] space-y-2">
        {race ? <RaceTicket race={race} /> : null}
        {otherRaces.length > 0 ? (
          <div className="space-y-2">
            {otherRaces.map((otherRace) => (
              <RaceTicket key={otherRace.id} race={otherRace} compact={Boolean(race)} />
            ))}
          </div>
        ) : null}
      </div>
    </div>,
    document.body,
  );
}

function RaceTicket({ race, compact = false }) {
  const categoryLogo = CATEGORY_LOGOS[race.categoria] ?? null;
  const categoryColor = getCategoryColor(race.categoria, "#E73F47");
  const isConcluida = race.status === "Concluida";
  const weatherValue = isConcluida ? weatherLabel(race.clima) : i18n.t("calendar.tbd");

  return (
    <div
      className={[
        "relative grid items-center overflow-hidden rounded-[28px] border border-white/15 bg-[rgba(13,19,32,0.94)] shadow-[0_24px_70px_rgba(0,0,0,0.42)]",
        compact
          ? "grid-cols-[90px_minmax(0,1fr)_28px] gap-2 px-2 py-2 pr-8"
          : "grid-cols-[190px_minmax(0,1fr)_48px] gap-4 px-4 py-4 pr-[64px]",
      ].join(" ")}
      style={{
        background: `radial-gradient(circle at 13% 50%, ${categoryColor}4d, transparent 34%), linear-gradient(90deg, ${categoryColor}2e, rgba(255,255,255,0.025) 34%, transparent 72%), rgba(13,19,32,0.94)`,
      }}
    >
      {categoryLogo ? (
        <img
          src={categoryLogo}
          alt={categoryLabel(race.categoria)}
          className={["object-contain", compact ? "h-16 w-[90px]" : "h-28 w-[190px]"].join(" ")}
          style={{ filter: `drop-shadow(0 14px 34px ${categoryColor}73) drop-shadow(0 4px 14px rgba(0,0,0,0.6))` }}
          draggable={false}
        />
      ) : <span />}

      <div className="min-w-0">
        <div className="flex items-center justify-between gap-3 text-[10px] font-extrabold text-text-muted">
          <span
            className="kcal shrink-0 rounded-full border px-2.5 py-1 text-white"
            style={{ borderColor: `${categoryColor}75`, backgroundColor: `${categoryColor}2e` }}
          >
            {i18n.t("calendar.v2.raceNumber", { n: race.rodada })}
          </span>
          <span className="truncate">{categoryLabel(race.categoria)}</span>
        </div>

        <p
          className={[
            "kcal mt-2 truncate font-black uppercase italic leading-none tracking-[-0.03em] text-white",
            compact ? "text-lg" : "text-[30px]",
          ].join(" ")}
        >
          {race.track_name}
        </p>

        <div className="mt-3 flex flex-wrap gap-1.5">
          <TicketDetail label={i18n.t("calendar.detail.duration")} value={`${race.duracao_corrida_min} min`} />
          <TicketDetail label={i18n.t("calendar.detail.weather")} value={weatherValue} />
          <TicketDetail
            label={i18n.t("calendar.detail.status")}
            value={isConcluida ? i18n.t("calendar.detail.done") : i18n.t("calendar.detail.pending")}
          />
        </div>
      </div>

      <div
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

function TicketDetail({ label, value }) {
  return (
    <div className="min-w-[76px] rounded-full border border-white/8 bg-white/[0.052] px-2 py-1.5 text-center">
      <b className="block text-[8px] font-bold uppercase leading-none tracking-[0.12em] text-text-muted">
        {label}
      </b>
      <span className="mt-1 block text-[10px] font-extrabold leading-none text-white">
        {value}
      </span>
    </div>
  );
}

export default CalendarTabRedesignV2;
