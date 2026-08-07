import { useTranslation } from "react-i18next";
import TeamLogoMark from "../../../team/TeamLogoMark";
import Tooltip from "../../../ui/Tooltip";
import DriverMiniCard from "../../../driver/DriverMiniCard";
import {
  subcatColor,
  shortDestLabel,
  formatSafeLastChampionshipResult,
} from "../../preSeasonFormatters.js";

// Ficha do piloto livre no v2.
//
// O v1 mostrava nome + categoria de destino, e nada mais. Licença, resultado do
// último campeonato e temporadas de carreira já vinham no `FreeAgentPreview` e
// eram descartados — sem eles o jogador não distingue o campeão que ficou sem
// vaga do piloto que terminou em último.
export default function FreeAgentCardV2({ driver, isRookie, onHoverCat }) {
  const { t } = useTranslation();
  const destColor = subcatColor(driver.categoria);
  const destLabel = shortDestLabel(driver.categoria);
  const idle = driver.seasons_idle ?? 0;
  const isParado = idle >= 1;
  const championship = formatSafeLastChampionshipResult(driver);
  const seasons = driver.total_career_seasons ?? 0;

  // Campeão ou vice sem equipe é a oportunidade da janela: acende a ficha.
  const isStandout = !isRookie && (driver.last_championship_position ?? 99) <= 2 && !isParado;

  // Uma linha por piloto, como no v1.
  //
  // A ficha de duas linhas custava metade da coluna: onde cabiam treze pilotos
  // passaram a caber oito, e a lista virou rolagem. O que a segunda linha trazia de
  // essencial — a colocação, que é o que ordena a fila — cabe na primeira. O tempo
  // de carreira, que é contexto e não critério, foi para o tooltip.
  const recordLabel = isRookie
    ? t("preSeason.v2.market.debut")
    : championship ?? t("preSeason.v2.market.noRecord");
  const recordTooltip = isRookie
    ? t("preSeason.v2.market.debut")
    : t("preSeason.v2.market.record", { result: championship ?? "—", count: seasons });

  return (
    <div
      data-testid="preseason-free-agent-v2"
      onMouseEnter={() => onHoverCat?.(driver.categoria)}
      onMouseLeave={() => onHoverCat?.(null)}
      className={`flex items-center gap-2 rounded-xl border px-2.5 py-1.5 transition-opacity ${
        isParado ? "opacity-55" : ""
      }`}
      style={{
        borderColor: isStandout ? "rgba(63,185,80,0.34)" : "rgba(255,255,255,0.07)",
        background: isStandout
          ? "linear-gradient(135deg, rgba(63,185,80,0.09), rgba(10,15,28,0.2))"
          : "rgba(10,15,28,0.18)",
      }}
    >
      {isRookie ? (
        <span className="shrink-0 rounded-md bg-[#bc8cff22] px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.12em] text-[#bc8cff]">
          {t("preSeason.market.freeAgent.new")}
        </span>
      ) : driver.previous_team_name ? (
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
      )}

      <DriverMiniCard driverId={driver.driver_id}>
        <p className="min-w-0 flex-1 truncate text-body text-[color:var(--text-primary)]">
          {driver.driver_name}
        </p>
      </DriverMiniCard>

      {!isRookie && (
        <Tooltip texto={recordTooltip}>
          <span className="shrink-0 text-[10px] font-bold tabular-nums text-[color:var(--text-muted)]">
            {recordLabel}
          </span>
        </Tooltip>
      )}

      {isParado && (
        <Tooltip texto={t("preSeason.market.freeAgent.idleTooltip", { count: idle })}>
          <span className="shrink-0 rounded bg-white/5 px-1 py-px text-[9px] font-semibold tabular-nums text-[color:var(--text-muted)]">
            {t("preSeason.market.freeAgent.idleShort", { count: idle })}
          </span>
        </Tooltip>
      )}

      <Tooltip texto={t("preSeason.market.freeAgent.destinationTooltip", { label: destLabel })}>
        <span
          className="shrink-0 rounded px-1.5 py-px text-[9px] font-bold uppercase tracking-[0.06em]"
          style={{ background: `${destColor}1f`, color: destColor }}
        >
          {destLabel}
        </span>
      </Tooltip>
    </div>
  );
}
