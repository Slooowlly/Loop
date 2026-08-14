import { useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { getCategoryColor } from "../../utils/categoryColors";
import { categoryLabel } from "../../utils/formatters";
import i18n from "../../i18n/index.js";
import {
  CATEGORY_LOGOS,
  getRaceTooltipStyle,
  weatherLabel,
} from "../../utils/calendarShared.js";

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
    <div className="min-w-[76px] rounded-full border border-white/[0.08] bg-white/[0.052] px-2 py-1.5 text-center">
      <b className="block text-[8px] font-bold uppercase leading-none tracking-[0.12em] text-text-muted">
        {label}
      </b>
      <span className="mt-1 block text-[10px] font-extrabold leading-none text-white">
        {value}
      </span>
    </div>
  );
}

export default CalendarTicketTooltip;
