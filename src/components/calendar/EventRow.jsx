import { getCategoryColor } from "../../utils/categoryColors";
import { categoryLabel } from "../../utils/formatters";
import { getTrackImageSrc, parseDisplayDate } from "../../utils/calendarShared.js";

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

export default EventRow;
