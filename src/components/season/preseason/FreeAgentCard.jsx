import { useTranslation } from "react-i18next";
import TeamLogoMark from "../../team/TeamLogoMark";
import { subcatColor, shortDestLabel } from "../preSeasonFormatters.js";

export default function FreeAgentCard({ driver, isRookie, onHoverCat }) {
  const { t } = useTranslation();
  const destColor = subcatColor(driver.categoria);
  const destLabel = shortDestLabel(driver.categoria);
  const idle = driver.seasons_idle ?? 0;
  const isParado = idle >= 1; // sentou fora ao menos uma temporada
  return (
    <div
      className={`glass-light flex items-center gap-2 rounded-xl px-2.5 py-1.5 transition-opacity ${isParado ? "opacity-55" : ""}`}
      onMouseEnter={() => onHoverCat?.(driver.categoria)}
      onMouseLeave={() => onHoverCat?.(null)}
    >
      {isRookie ? (
        <span className="shrink-0 rounded-md bg-[#bc8cff22] px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.12em] text-[#bc8cff]">
          {t("preSeason.market.freeAgent.new")}
        </span>
      ) : (
        driver.previous_team_name ? (
          <TeamLogoMark
            teamName={driver.previous_team_name}
            color={driver.previous_team_color ?? destColor}
            size="xs"
            testId="driver-market-previous-team-logo"
          />
        ) : (
          <span
            className="shrink-0 rounded-md px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.1em]"
            style={{ background: `${destColor}22`, color: destColor }}
          >
            {driver.previous_team_abbr ?? "—"}
          </span>
        )
      )}
      <p className="min-w-0 flex-1 truncate text-body text-[color:var(--text-primary)]">
        {driver.driver_name}
      </p>
      {isParado && (
        <span
          className="shrink-0 rounded-md bg-white/5 px-1.5 py-0.5 text-[9px] font-semibold tabular-nums text-[color:var(--text-muted)]"
          title={t("preSeason.market.freeAgent.idleTooltip", { count: idle })}
        >
          {t("preSeason.market.freeAgent.idleShort", { count: idle })}
        </span>
      )}
      {/* Etiqueta de destino provável (categoria onde as propostas chegam) — sempre
          visível no canto, mesmo com separador de marca. Substitui a carteira, escondida. */}
      <span
        className="shrink-0 rounded-md px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.06em]"
        style={{ background: `${destColor}1f`, color: destColor }}
        title={t("preSeason.market.freeAgent.destinationTooltip", { label: destLabel })}
      >
        {destLabel}
      </span>
    </div>
  );
}
