import { getCategoryColor } from "../../utils/categoryColors";
import {
  ALL_CALENDAR_CATEGORIES,
  buildMonthGrid,
  formatIsoDateKey,
  getMonthPhaseType,
} from "../../utils/calendarShared.js";
import { PHASE_COLOR } from "./calendarViewHelpers.js";

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

export default MiniMonth;
