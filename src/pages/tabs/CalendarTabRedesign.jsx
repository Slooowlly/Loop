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
import { currentLang, monthLongLabels } from "../../i18n/format.js";
import {
  ALL_CALENDAR_CATEGORIES,
  CATEGORY_LOGOS,
  buildMonthGrid,
  formatIsoDateKey,
  getMonthPhaseType,
  getRaceTooltipStyle,
  getTrackImageSrc,
  parseDisplayDate,
  weatherLabel,
} from "../../utils/calendarShared.js";
import { TRACK_COUNTRIES } from "../../utils/trackCountries.js";

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

function formatTrackTime(min) {
  if (!min) return "0min";
  const h = Math.floor(min / 60);
  const m = min % 60;
  if (h === 0) return `${m}min`;
  return m === 0 ? `${h}h` : `${h}h${String(m).padStart(2, "0")}`;
}

function compareMonth(year, month, current) {
  if (!current) return 0;
  if (year !== current.year) return year < current.year ? -1 : 1;
  if (month === current.month) return 0;
  return month < current.month ? -1 : 1;
}

function CalendarTabRedesign({ activeTab, raceArrivalFeedbackActive = false }) {
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
  const rootRef = useRef(null);
  const lastCurrentMonthRef = useRef(null);

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

  // Mês inicial (atual → próxima corrida → 1ª etapa → 0) E acompanhamento: quando o
  // tempo avança e cruza a virada de mês, o foco segue o novo mês atual.
  useEffect(() => {
    const cm = currentDateParts?.month ?? null;
    if (selectedMonth == null) {
      const nextMonth = parseDisplayDate(nextRaceEntry?.display_date)?.month;
      const firstMonth = parseDisplayDate(displayedCalendar[0]?.display_date)?.month;
      setSelectedMonth(cm ?? nextMonth ?? firstMonth ?? 0);
      lastCurrentMonthRef.current = cm;
      return;
    }
    if (cm != null && lastCurrentMonthRef.current != null && cm !== lastCurrentMonthRef.current) {
      setSelectedMonth(cm);
    }
    lastCurrentMonthRef.current = cm;
  }, [selectedMonth, currentDateParts, nextRaceEntry, displayedCalendar]);

  const month = selectedMonth ?? currentDateParts?.month ?? 0;

  const upcoming = useMemo(() => {
    return [...displayedCalendar]
      .filter((race) => race.display_date && race.status !== "Concluida")
      .sort((a, b) => a.display_date.localeCompare(b.display_date));
  }, [displayedCalendar]);

  const stats = useMemo(() => {
    const total = displayedCalendar.length;
    const done = displayedCalendar.filter((race) => race.status === "Concluida").length;
    const countries = new Set(
      displayedCalendar.map((race) => TRACK_COUNTRIES[race.track_name]).filter(Boolean),
    ).size;
    const durationMin = displayedCalendar.reduce((sum, race) => sum + (race.duracao_corrida_min || 0), 0);
    const wet = displayedCalendar.filter((race) => race.clima === "Wet" || race.clima === "HeavyRain").length;
    const specials = displayedCalendar.filter((race) => race.season_phase === "BlocoEspecial").length;
    return { total, done, countries, durationMin, wet, specials };
  }, [displayedCalendar]);

  const monthShort = monthShortLabels();
  const monthLong = monthLongLabels();
  const weekdays = weekdayShortLabels();
  // Grade fixa de 6 linhas (42 células) com dias vizinhos esmaecidos: célula do mesmo
  // tamanho em todo mês, preenche a altura, e sem buracos.
  const cells = buildMonthGrid(seasonYear, month);
  const gridRows = 6;
  const hasCurrent = Boolean(currentDateParts);
  const seasonProgress = stats.total > 0 ? Math.round((stats.done / stats.total) * 100) : 0;
  const daysUntilNext = temporalSummary?.days_until_next_event ?? null;
  const upcomingList = upcoming;

  function goToday() {
    const nextMonth = parseDisplayDate(nextRaceEntry?.display_date)?.month;
    setSelectedMonth(currentDateParts?.month ?? nextMonth ?? 0);
    rootRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  // Abre um mês em foco (grande) e sobe a tela até o topo.
  function openMonth(target) {
    setSelectedMonth(target);
    rootRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  // Meses seguintes ao mês em foco — exibidos em miniatura abaixo (estilo antigo).
  const followingMonths = Array.from({ length: Math.max(0, 11 - month) }, (_, i) => month + 1 + i);

  return (
    <div ref={rootRef} className="flex scroll-mt-4 flex-col gap-6">
    <div className="grid grid-cols-1 gap-4 xl:h-[calc(100vh-132px)] xl:min-h-[600px] xl:grid-cols-[minmax(0,1fr)_340px]">
      {/* ── Coluna principal ── */}
      <div className={`${SURFACE} flex min-h-0 flex-col overflow-hidden rounded-[28px] xl:h-full`}>
        {/* Header */}
        <div className="shrink-0 px-6 pb-3 pt-4">
          <div className="flex flex-col gap-2 sm:flex-row sm:items-end sm:justify-between">
            <div>
              <p className="kcal text-[11px] uppercase tracking-[0.24em] text-accent-primary">
                {t("calendar.title")}
                {season?.ano ? ` · ${season.ano}` : ""}
              </p>
              <h2 className="kcal mt-2 bg-gradient-to-b from-white to-[#aebccd] bg-clip-text text-3xl font-semibold uppercase italic tracking-tight text-transparent">
                {categoryLabel(playerTeam?.categoria)}
              </h2>
            </div>
            <p className="text-sm text-text-secondary">
              {t("calendar.stepsDone", { completed: stats.done, total: stats.total })}
            </p>
          </div>
          {/* Barra de progresso da temporada */}
          <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-white/[0.06]">
            <div
              className="h-full rounded-full bg-gradient-to-r from-accent-primary/70 to-accent-hover transition-[width] duration-500"
              style={{ width: `${Math.max(seasonProgress, stats.done > 0 ? 4 : 0)}%` }}
            />
          </div>
        </div>

        {/* Navegação do mês grande (setas + nome do mês) */}
        <div className="flex shrink-0 items-center justify-between gap-2 px-6 py-2">
          <div className="flex items-center gap-2">
            <button
              type="button"
              aria-label={t("calendar.v2.prevMonth")}
              onClick={() => setSelectedMonth((m) => Math.max(0, (m ?? month) - 1))}
              disabled={month <= 0}
              className="grid h-[34px] w-[34px] shrink-0 place-items-center rounded-xl bg-white/[0.06] text-text-secondary transition-glass hover:bg-white/[0.11] hover:text-text-primary disabled:opacity-30"
            >
              ‹
            </button>
            <h3 className="kcal min-w-[190px] text-center text-xl font-bold uppercase italic tracking-wide text-text-primary">
              {monthLong[month]} <span className="text-text-muted">{seasonYear}</span>
            </h3>
            <button
              type="button"
              aria-label={t("calendar.v2.nextMonth")}
              onClick={() => setSelectedMonth((m) => Math.min(11, (m ?? month) + 1))}
              disabled={month >= 11}
              className="grid h-[34px] w-[34px] shrink-0 place-items-center rounded-xl bg-white/[0.06] text-text-secondary transition-glass hover:bg-white/[0.11] hover:text-text-primary disabled:opacity-30"
            >
              ›
            </button>
          </div>
          <button
            type="button"
            onClick={goToday}
            disabled={!hasCurrent}
            className="h-[34px] shrink-0 rounded-xl bg-white/[0.06] px-3.5 text-xs font-semibold text-text-primary transition-glass hover:bg-white/[0.11] disabled:opacity-40"
          >
            {t("calendar.v2.today")}
          </button>
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
          <div key={month} className="tab-pane-fade relative flex min-h-0 flex-1 flex-col px-6 pb-5">
            <div className="mb-1.5 grid shrink-0 grid-cols-7 gap-1.5">
              {weekdays.map((label, index) => (
                <div key={index} className="text-center text-[10px] font-semibold uppercase tracking-[0.16em] text-text-muted/70">
                  {label}
                </div>
              ))}
            </div>
            <div
              className="grid min-h-0 flex-1 grid-cols-7 gap-1.5"
              style={{ gridTemplateRows: `repeat(${gridRows}, minmax(0, 1fr))` }}
            >
              {cells.map((cell, index) => {
                const dateKey = cell.outside ? null : formatIsoDateKey(seasonYear, month, cell.day);
                return (
                  <DayCellV2
                    key={index}
                    day={cell.day}
                    outside={cell.outside}
                    year={seasonYear}
                    month={month}
                    race={dateKey ? racesByDate[dateKey] ?? null : null}
                    otherRaces={dateKey ? otherCategoryRacesByDate[dateKey] ?? [] : []}
                    phaseType={getMonthPhaseType(month, isLegacyCalendar)}
                    nextRaceId={nextRace?.id}
                    currentDateParts={currentDateParts}
                    seasonYear={seasonYear}
                    raceArrivalFeedbackActive={raceArrivalFeedbackActive}
                    onHover={setTooltip}
                  />
                );
              })}
            </div>
          </div>
        )}
      </div>

      {/* ── Sidebar ── */}
      <div className="flex flex-col gap-4 xl:self-start">
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
          {upcomingList.length === 0 ? (
            <p className="px-5 pb-5 text-[12px] text-text-muted">{t("calendar.v2.noUpcoming")}</p>
          ) : (
            <>
              <div className="flex flex-col gap-2.5 px-4 pb-4">
                {(showAllEvents ? upcomingList : upcomingList.slice(0, 5)).map((race) => (
                  <EventRow
                    key={race.id}
                    race={race}
                    monthShort={monthShort}
                    isNext={race.id === nextRace?.id}
                    t={t}
                  />
                ))}
              </div>
              {upcomingList.length > 5 && (
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
          <div className="grid grid-cols-2 gap-2.5 px-4 pb-5 sm:grid-cols-3 xl:grid-cols-2">
            <StatTile icon="flag" tone="blue" value={`${stats.done}/${stats.total}`} label={t("calendar.v2.statRounds")} />
            <StatTile
              icon="hourglass"
              tone="amber"
              value={daysUntilNext == null ? "—" : daysUntilNext <= 0 ? i18n.t("calendar.v2.today") : `${daysUntilNext}d`}
              label={t("calendar.v2.statNextDays")}
            />
            <StatTile icon="clock" tone="teal" value={formatTrackTime(stats.durationMin)} label={t("calendar.v2.statTrackTime")} />
            <StatTile icon="globe" tone="coral" value={stats.countries} label={t("calendar.v2.statCountries")} />
            <StatTile icon="rain" tone="cyan" value={stats.wet} label={t("calendar.v2.statWet")} />
            <StatTile icon="star" tone="purple" value={stats.specials} label={t("calendar.v2.statSpecial")} />
          </div>
        </div>
      </div>
    </div>

      {/* Próximos meses em miniatura (estilo antigo), abaixo do mês em foco */}
      {followingMonths.length > 0 && (
        <div className={`${SURFACE} rounded-[28px] px-5 py-5`}>
          <div className="mb-4 flex items-center gap-3">
            <span className="kcal text-xs uppercase italic tracking-[0.14em] text-text-secondary">
              {t("calendar.v2.nextMonths")}
            </span>
            <span className="h-px flex-1 bg-white/8" />
          </div>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            {followingMonths.map((mIdx) => (
              <MiniMonth
                key={mIdx}
                year={seasonYear}
                month={mIdx}
                racesByDate={racesByDate}
                otherCategoryRacesByDate={otherCategoryRacesByDate}
                nextRaceId={nextRace?.id}
                currentDateParts={currentDateParts}
                isLegacyCalendar={isLegacyCalendar}
                weekdays={weekdays}
                monthLong={monthLong}
                onOpen={openMonth}
                onHover={setTooltip}
              />
            ))}
          </div>
        </div>
      )}

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
  outside = false,
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

  // Dia do mês vizinho: bem apagado, sem conteúdo (fantasma). O contraste com o mês
  // em foco (aceso) é o que separa visualmente — sem borda nem canto arredondado.
  if (outside) {
    return (
      <div className="relative h-full min-h-[62px] overflow-hidden bg-white/[0.006] p-1.5">
        <span className="text-[13px] font-semibold tabular-nums text-white/[0.07]">{day}</span>
      </div>
    );
  }

  const monthOrder = compareMonth(seasonYear, month, currentDateParts);
  const isCurrentMonth = monthOrder === 0 && currentDateParts != null;
  const isCurrentDay = isCurrentMonth && currentDateParts.day === day;
  // Só dias REALMENTE passados (deste mês ou anteriores) ganham esmaecimento leve.
  // Meses futuros em foco NÃO são apagados — o mês que você está vendo fica aceso.
  const isPast = monthOrder < 0 || (isCurrentMonth && day < currentDateParts.day);

  // Corridas do dia: o jogador tem prioridade; senão, a de maior prioridade de
  // categoria vira a "principal". +N indica outras categorias no mesmo dia.
  const dayRaces = [...(race ? [race] : []), ...otherRaces];
  const primary = race
    ?? [...otherRaces].sort(
      (a, b) => ALL_CALENDAR_CATEGORIES.indexOf(a.categoria) - ALL_CALENDAR_CATEGORIES.indexOf(b.categoria),
    )[0]
    ?? null;
  const extra = Math.max(0, dayRaces.length - 1);

  const canHover = dayRaces.length > 0;
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

  // Dias passados: escurece o fundo + hachura diagonal ("traçejado") por cima.
  const pastHatch = isPast ? (
    <div
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 z-[12]"
      style={{
        backgroundColor: "rgba(0,0,0,0.42)",
        backgroundImage: "repeating-linear-gradient(45deg, rgba(0,0,0,0.4) 0, rgba(0,0,0,0.4) 4px, transparent 4px, transparent 9px)",
      }}
    />
  ) : null;

  const currentRail = isCurrentDay ? (
    <span
      aria-hidden="true"
      className="pointer-events-none absolute inset-y-0 left-0 z-20 w-[3px] bg-gradient-to-b from-accent-hover to-accent-primary"
    />
  ) : null;

  const todayPill = isCurrentDay ? (
    <span className="absolute right-1.5 top-1.5 z-[11] rounded bg-accent-hover px-1 py-[1px] text-[7.5px] font-extrabold uppercase leading-none tracking-[0.06em] text-[#04121f]">
      {i18n.t("calendar.v2.today")}
    </span>
  ) : null;

  // ── Dia sem corrida: célula "acesa" do mês em foco ──
  if (!primary) {
    const phaseGrad =
      phaseType === "mercado"
        ? "bg-gradient-to-b from-status-yellow/[0.15] to-status-yellow/[0.04]"
        : phaseType === "especial"
          ? "bg-gradient-to-b from-status-purple/[0.15] to-status-purple/[0.04]"
          : phaseType === "encerramento"
            ? "bg-gradient-to-b from-white/[0.06] to-white/[0.02]"
            : "bg-gradient-to-b from-white/[0.13] to-white/[0.045]";
    return (
      <div
        data-testid={`calendar-day-${formatIsoDateKey(seasonYear, month, day)}`}
        className={[
          "relative flex h-full min-h-[62px] items-start justify-start overflow-hidden p-1.5 shadow-[inset_0_1px_0_rgba(255,255,255,0.07)]",
          isCurrentDay ? "bg-accent-primary/[0.18]" : phaseGrad,
        ].join(" ")}
      >
        {currentRail}
        {todayPill}
        <span className={[
          "text-[13px] font-semibold tabular-nums",
          isCurrentDay ? "text-text-primary" : isPast ? "text-text-muted/50" : "text-text-secondary",
        ].join(" ")}>
          {day}
        </span>
        {pastHatch}
      </div>
    );
  }

  // ── Dia com corrida: quadrado inteiro na cor da categoria + logo do campeonato ──
  const color = getCategoryColor(primary.categoria);
  const logo = CATEGORY_LOGOS[primary.categoria] ?? null;
  const isCompleted = primary.status === "Concluida";
  // Glow respirante: só nas SUAS corridas que ainda NÃO foram disputadas.
  const glow = race != null && !isCompleted;
  const isSpecial = Boolean(primary._isSpecialRace);
  const flash = raceArrivalFeedbackActive && isCurrentDay;

  return (
    <div
      {...hoverHandlers}
      data-testid={`calendar-day-${formatIsoDateKey(seasonYear, month, day)}`}
      style={glow ? { "--cal-glow": color } : undefined}
      className={[
        "group relative flex h-full min-h-[62px] cursor-pointer items-center justify-center overflow-hidden transition-transform duration-300 hover:-translate-y-0.5",
        glow
          ? "calendar-user-glow" // glow respirante na cor da categoria
          : isCurrentDay
            ? "shadow-[inset_0_0_0_1.5px_rgba(88,166,255,0.9)]"
            : isSpecial
              ? "shadow-[inset_0_0_0_1px_rgba(163,113,247,0.6)]"
              : "",
      ].join(" ")}
    >
      <div className="absolute inset-0" style={{ backgroundColor: color }} />
      <div className="absolute inset-0 bg-gradient-to-b from-white/[0.12] via-transparent to-black/[0.42]" />
      {logo ? (
        <img
          src={logo}
          alt={categoryLabel(primary.categoria)}
          draggable={false}
          className="relative z-10 max-h-[66%] w-[84%] object-contain drop-shadow-[0_2px_5px_rgba(0,0,0,0.55)] transition-transform duration-300 group-hover:scale-105"
        />
      ) : (
        <span className="kcal relative z-10 px-1 text-center text-[11px] font-bold uppercase italic leading-tight text-white drop-shadow-[0_1px_3px_rgba(0,0,0,0.7)]">
          {categoryLabel(primary.categoria)}
        </span>
      )}
      <span className="absolute left-1.5 top-1 z-[11] text-[13px] font-bold tabular-nums text-white drop-shadow-[0_1px_2px_rgba(0,0,0,0.8)]">
        {day}
      </span>
      {extra > 0 && (
        <span className="absolute right-1 top-1 z-[11] rounded bg-black/55 px-1 text-[8px] font-bold leading-tight text-white">
          +{extra}
        </span>
      )}
      {isCurrentDay && (
        <span className="absolute bottom-1 left-1.5 z-[11] rounded bg-accent-hover px-1 py-[1px] text-[7.5px] font-extrabold uppercase leading-none tracking-[0.06em] text-[#04121f]">
          {i18n.t("calendar.v2.today")}
        </span>
      )}
      {flash && (
        <div
          data-testid="calendar-race-arrival-flash"
          className="calendar-race-arrival-flash pointer-events-none absolute inset-[2px] z-20 border border-accent-hover/70 bg-accent-primary/10"
        />
      )}
      {currentRail}
      {pastHatch}
    </div>
  );
}

function MiniMonth({
  year,
  month,
  racesByDate,
  otherCategoryRacesByDate,
  nextRaceId,
  currentDateParts,
  isLegacyCalendar,
  weekdays,
  monthLong,
  onOpen,
  onHover,
}) {
  const cells = buildMonthGrid(year, month);
  const phaseColor = PHASE_COLOR[getMonthPhaseType(month, isLegacyCalendar)];
  const isCurrentMonth = currentDateParts?.month === month && currentDateParts?.year === year;

  return (
    <div className="group rounded-2xl bg-white/[0.03] p-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.05)] transition hover:bg-white/[0.055]">
      <button
        type="button"
        onClick={() => onOpen(month)}
        className="mb-2 flex w-full items-center justify-between"
      >
        <span className="kcal text-sm font-bold uppercase italic tracking-wide text-text-primary group-hover:text-accent-hover">
          {monthLong[month]}
        </span>
        {isCurrentMonth
          ? <span className="h-1.5 w-1.5 rounded-full bg-accent-hover shadow-[0_0_6px_rgba(88,166,255,0.9)]" />
          : <span className="h-[3px] w-7 rounded-full opacity-80" style={{ backgroundColor: phaseColor }} />}
      </button>

      <div className="grid grid-cols-7 gap-[3px]">
        {weekdays.map((w, i) => (
          <div key={i} className="pb-0.5 text-center text-[7px] font-semibold uppercase text-text-muted/45">
            {w.slice(0, 1)}
          </div>
        ))}
        {cells.map((cell, i) => {
          if (cell.outside) return <div key={i} className="aspect-square" />;
          const key = formatIsoDateKey(year, month, cell.day);
          const race = racesByDate[key] ?? null;
          const others = otherCategoryRacesByDate[key] ?? [];
          const primary = race
            ?? [...others].sort(
              (a, b) => ALL_CALENDAR_CATEGORIES.indexOf(a.categoria) - ALL_CALENDAR_CATEGORIES.indexOf(b.categoria),
            )[0]
            ?? null;
          const isNext = race != null && nextRaceId === race.id;
          // Glow nas SUAS corridas ainda não disputadas (todas, não só a próxima).
          const glow = race != null && race.status !== "Concluida";
          const canHover = Boolean(primary);
          const hover = canHover
            ? {
              onMouseEnter: (e) => onHover?.({ race: race ?? null, otherRaces: others, rect: e.currentTarget.getBoundingClientRect() }),
              onMouseLeave: () => onHover?.(null),
            }
            : {};

          // Dia com corrida (jogador OU outra categoria): quadrado inteiro na cor.
          if (primary) {
            const cor = getCategoryColor(primary.categoria);
            return (
              <div
                key={i}
                {...hover}
                className={[
                  "relative flex aspect-square cursor-pointer items-center justify-center rounded-[4px]",
                  isNext ? "ring-1 ring-white/70" : "",
                ].join(" ")}
                style={glow ? { backgroundColor: cor, "--cal-glow": cor } : { backgroundColor: cor }}
              >
                {glow && <span aria-hidden="true" className="calendar-glow-layer" />}
                <span className="relative text-[7.5px] font-bold text-black/75">{cell.day}</span>
              </div>
            );
          }
          return (
            <div
              key={i}
              className="flex aspect-square items-center justify-center rounded-[4px] bg-white/[0.03] text-[7.5px] text-text-muted/55"
            >
              {cell.day}
            </div>
          );
        })}
      </div>
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
  cyan: "#4bc4e0",
  purple: "#a371f7",
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
  flag: (
    <>
      <path d="M5.5 21V4" />
      <path d="M5.5 4.5h11l-1.6 3.4 1.6 3.4h-11" />
    </>
  ),
  clock: (
    <>
      <circle cx="12" cy="12" r="8.2" />
      <path d="M12 7.5V12l3 2" />
    </>
  ),
  hourglass: (
    <>
      <path d="M6.5 3h11M6.5 21h11" />
      <path d="M7 3c0 4 4 5.4 4 9s-4 5-4 9M17 3c0 4-4 5.4-4 9s4 5 4 9" />
    </>
  ),
  rain: (
    <>
      <path d="M7 15.5a4.5 4.5 0 0 1 .5-8.98A5 5 0 0 1 17 7.5a3.5 3.5 0 0 1 .5 8" />
      <path d="M8 18l-1 2M12 18l-1 2M16 18l-1 2" />
    </>
  ),
  star: (
    <>
      <path d="M12 3.5l2.6 5.3 5.9.86-4.25 4.14 1 5.88L12 17l-5.25 2.76 1-5.88L3.5 9.66l5.9-.86Z" />
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

export default CalendarTabRedesign;
