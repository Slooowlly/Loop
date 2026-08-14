import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown } from "lucide-react";

import { categoryLabel } from "../../utils/formatters";
import i18n from "../../i18n/index.js";
import { monthLongLabels } from "../../i18n/format.js";
import {
  buildMonthGrid,
  formatIsoDateKey,
  getMonthPhaseType,
  parseDisplayDate,
} from "../../utils/calendarShared.js";
import CalendarTicketTooltip from "../../components/calendar/CalendarTicketTooltip.jsx";
import DayCellV2 from "../../components/calendar/DayCellV2.jsx";
import EventRow from "../../components/calendar/EventRow.jsx";
import MiniMonth from "../../components/calendar/MiniMonth.jsx";
import StatTile from "../../components/calendar/StatTile.jsx";
import useCalendarData from "../../components/calendar/useCalendarData.js";
import {
  SURFACE,
  formatTrackTime,
  monthShortLabels,
  weekdayShortLabels,
} from "../../components/calendar/calendarViewHelpers.js";

function CalendarTabRedesign({ activeTab, raceArrivalFeedbackActive = false }) {
  const { t } = useTranslation();
  const {
    careerId,
    playerTeam,
    nextRace,
    season,
    temporalSummary,
    loading,
    showLoadingUI,
    error,
    isLegacyCalendar,
    displayedCalendar,
    seasonYear,
    racesByDate,
    otherCategoryRacesByDate,
    currentDateParts,
    nextRaceEntry,
    upcoming,
    stats,
  } = useCalendarData(activeTab);

  const [selectedMonth, setSelectedMonth] = useState(null);
  const [showAllEvents, setShowAllEvents] = useState(false);
  const [tooltip, setTooltip] = useState(null);
  const [focusedDateKey, setFocusedDateKey] = useState(null);
  const rootRef = useRef(null);
  const lastCurrentMonthRef = useRef(null);

  // O destaque do dia é um piscar, não um estado: some sozinho para não virar uma
  // segunda "seleção" competindo com o dia atual.
  useEffect(() => {
    if (!focusedDateKey) return undefined;
    const timer = setTimeout(() => setFocusedDateKey(null), 2600);
    return () => clearTimeout(timer);
  }, [focusedDateKey]);

  // Reseta o mês selecionado ao trocar de carreira (deixa o auto-pick reescolher).
  useEffect(() => {
    setSelectedMonth(null);
  }, [careerId]);

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
    // A chamada é opcional, e não só o `rootRef.current`: o jsdom não implementa
    // `scrollIntoView`, então no teste o `?.` do objeto passava e o método estourava
    // `TypeError` fora do fluxo do React — erro não capturado que derrubava a suíte
    // inteira com todos os casos verdes. Mesma forma do `GlobalDriversTab`.
    rootRef.current?.scrollIntoView?.({ behavior: "smooth", block: "start" });
  }

  // Abre um mês em foco (grande) e sobe a tela até o topo.
  function openMonth(target) {
    setSelectedMonth(target);
    rootRef.current?.scrollIntoView?.({ behavior: "smooth", block: "start" });
  }

  // Clique numa etapa da lista: leva a grade até o mês dela e pisca o dia.
  function focusRace(race) {
    const parsed = parseDisplayDate(race?.display_date);
    if (!parsed) return;
    openMonth(parsed.month);
    // Chave montada com `seasonYear` — é o ano com que a grade é desenhada, então é
    // o que casa com o data-testid das células.
    setFocusedDateKey(formatIsoDateKey(seasonYear, parsed.month, parsed.day));
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
                    highlighted={dateKey != null && dateKey === focusedDateKey}
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
            {/* "Ver todos" expande a lista — antes chamava `goToday`, o mesmo handler do
                botão "Hoje", e o clique só trocava o mês da grade sem tocar na lista. */}
            {upcomingList.length > 5 && !showAllEvents && (
              <button
                type="button"
                onClick={() => setShowAllEvents(true)}
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
                    onSelect={focusRace}
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
                  <ChevronDown
                    size={14}
                    strokeWidth={2.4}
                    aria-hidden="true"
                    className={`transition-transform ${showAllEvents ? "rotate-180" : ""}`}
                  />
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
            <span className="h-px flex-1 bg-white/[0.08]" />
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

export default CalendarTabRedesign;
