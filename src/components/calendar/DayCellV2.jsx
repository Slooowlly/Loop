import { getCategoryColor } from "../../utils/categoryColors";
import { categoryLabel } from "../../utils/formatters";
import i18n from "../../i18n/index.js";
import {
  ALL_CALENDAR_CATEGORIES,
  CATEGORY_LOGOS,
  formatIsoDateKey,
} from "../../utils/calendarShared.js";
import { compareMonth } from "./calendarViewHelpers.js";

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

export default DayCellV2;
