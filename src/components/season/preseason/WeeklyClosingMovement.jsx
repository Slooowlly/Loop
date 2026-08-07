import { useTranslation } from "react-i18next";
import TeamLogoMark from "../../team/TeamLogoMark";
import Tooltip from "../../ui/Tooltip";
import {
  WEEKLY_MARKET_MOVEMENT_BADGES,
  RELATION_EMPHASIS,
  formatWeeklyClosingPosition,
} from "../preSeasonFormatters.js";

export default function WeeklyClosingMovement({ event, color, onSelect }) {
  const { t } = useTranslation();
  const movementBadge = WEEKLY_MARKET_MOVEMENT_BADGES[event.movement_kind];
  const movementLabel = movementBadge ? t(`preSeason.movementBadge.${event.movement_kind}`) : "";
  // Vínculo com o jogador (rival / favorito / já-correu) → realce no feed.
  const emphasis = RELATION_EMPHASIS[event.relation];
  const emphasisLabel = emphasis ? t(`preSeason.relation.${event.relation}`) : "";
  const strong = emphasis?.strong;

  return (
    <article
      role="button"
      tabIndex={0}
      onClick={() => onSelect?.(event)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect?.(event);
        }
      }}
      className="cursor-pointer rounded-lg border px-2.5 py-2 transition-colors hover:brightness-125"
      style={
        strong
          ? {
              // Rival/favorito: realce forte — borda viva + glow sutil na cor do vínculo.
              borderColor: emphasis.border,
              background: `linear-gradient(135deg, ${emphasis.bg} 0%, rgba(255,255,255,0.02) 100%)`,
              boxShadow: `0 0 0 1px ${emphasis.border}, 0 0 12px -4px ${emphasis.color}`,
            }
          : {
              borderColor: `${color}26`,
              background: `linear-gradient(135deg, ${color}0f 0%, rgba(255,255,255,0.02) 100%)`,
            }
      }
    >
      <div className="flex min-w-0 items-center gap-2.5">
        {movementBadge && (
          <Tooltip texto={movementLabel}>
            <span
              aria-label={movementLabel}
              className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md border text-[13px] font-black leading-none"
              style={{
                color: movementBadge.color,
                background: movementBadge.bg,
                borderColor: movementBadge.border,
              }}
            >
              {movementBadge.symbol}
            </span>
          </Tooltip>
        )}
        {event.championship_position != null && (
          <span
            className="w-8 shrink-0 text-right text-[13px] font-black leading-none"
            style={{ color }}
          >
            {formatWeeklyClosingPosition(event.championship_position)}
          </span>
        )}
        <p className="min-w-0 flex-1 truncate text-[13px] font-extrabold leading-[1.05] text-[color:var(--text-primary)]">
          {event.driver_name}
        </p>
        {emphasis &&
          (strong ? (
            // Rival/favorito: raros e significativos → mantêm o rótulo escrito.
            // E por isso não levam balão: ele repetiria em miúdo a palavra que
            // já está escrita ao lado do símbolo.
            <span
              className="flex shrink-0 items-center gap-1 rounded-md border px-1.5 py-0.5 text-[10px] font-black uppercase leading-none tracking-[0.08em]"
              style={{ color: emphasis.color, background: emphasis.bg, borderColor: emphasis.border }}
            >
              <span className="text-[11px] leading-none">{emphasis.symbol}</span>
              {emphasisLabel}
            </span>
          ) : (
            // "Já correu": comum → só um marcador pequeno com tooltip, pra não
            // roubar largura do nome do piloto.
            <Tooltip texto={emphasisLabel}>
              <span
                aria-label={emphasisLabel}
                className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full border text-[11px] font-black leading-none"
                style={{ color: emphasis.color, background: emphasis.bg, borderColor: emphasis.border }}
              >
                {emphasis.symbol}
              </span>
            </Tooltip>
          ))}
        {event.team_name && (
          <TeamLogoMark
            teamName={event.team_name}
            color={color}
            size="xs"
            testId="weekly-closing-team-logo"
          />
        )}
      </div>
    </article>
  );
}
