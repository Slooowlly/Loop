import { Fragment } from "react";
import { useTranslation } from "react-i18next";

import i18n from "../../i18n/index.js";
import RivalMarker from "../driver/RivalMarker";
import TeamLogoMarkShared from "../team/TeamLogoMark";
import FlagIcon from "../ui/FlagIcon";
import ResultBadge from "./ResultBadge";
import { SpecialClassHeader } from "./SpecialStandingNotices";
import { getReadableTeamColor, podiumClass } from "./standingsFormatting";

const SEVERE_INJURY_TYPES = new Set(["Grave", "Critica"]);

function injuryStatusMarker(injuryType) {
  if (!injuryType) {
    return null;
  }

  if (SEVERE_INJURY_TYPES.has(injuryType)) {
    return { emoji: "🚑", title: i18n.t("standings.badge.injurySevere") };
  }

  return { emoji: "🩸", title: i18n.t("standings.badge.injuryActive") };
}

export function DriverStatusMarkers({ driver }) {
  const injuryMarker = injuryStatusMarker(driver.lesao_ativa_tipo);

  return (
    <>
      <RivalMarker driverId={driver.id} />
      {driver.is_estreante_da_vida ? (
        <span className="shrink-0 text-xs" title={i18n.t("standings.badge.lifeRookie")} aria-label={i18n.t("standings.badge.lifeRookie")}>
          {"\u{1F331}"}
        </span>
      ) : null}
      {driver.is_estreante && !driver.is_estreante_da_vida ? (
        <span className="shrink-0 text-xs" title={i18n.t("standings.badge.rookie")} aria-label={i18n.t("standings.badge.rookie")}>
          {"\u2B50"}
        </span>
      ) : null}
      {injuryMarker ? (
        <span className="shrink-0 text-xs" title={injuryMarker.title} aria-label={injuryMarker.title}>
          {injuryMarker.emoji}
        </span>
      ) : null}
      {driver.is_aposentado ? (
        <span
          className="shrink-0 rounded bg-white/10 px-1 text-[10px] font-semibold uppercase tracking-wide text-white/55"
          title={i18n.t("standings.badge.retiredTitle")}
          aria-label={i18n.t("standings.badge.retired")}
        >
          {i18n.t("standings.badge.retired")}
        </span>
      ) : null}
    </>
  );
}

function PositionDelta({ delta }) {
  if (!delta) {
    return <span className="text-[10px] font-semibold text-text-muted">•</span>;
  }

  if (delta > 0) {
    return (
      <span className="inline-flex items-center justify-center gap-0.5 text-[10px] font-semibold leading-none text-status-green">
        ▲{delta}
      </span>
    );
  }

  return (
    <span className="inline-flex items-center justify-center gap-0.5 text-[10px] font-semibold leading-none text-status-red">
      ▼{Math.abs(delta)}
    </span>
  );
}

// A tabela de pilotos: uma coluna por rodada (duplo clique numa rodada concluída
// reabre a corrida), realce da dupla da equipe em foco e as faixas de classe quando a
// categoria é multiclasse.
function DriverStandingsTable({
  sections,
  specialClassGroups,
  totalRodadas,
  completedRounds,
  currentRound,
  positionDeltaMap,
  previousChampionId,
  selectedTeamId,
  selectedTeamColor,
  onDriverHover,
  onDriverClick,
  onDriverDoubleClick,
  onDriverActivate,
  onReviewRace,
}) {
  const { t } = useTranslation();

  return (
    <div className="mt-6 overflow-x-auto">
      <table className="w-full table-fixed" style={{ minWidth: 536 + totalRodadas * 68 }}>
        <colgroup>
          <col style={{ width: "62px" }} />
          <col style={{ width: "34px" }} />
          <col style={{ width: "42px" }} />
          <col style={{ width: "214px" }} />
          <col style={{ width: "118px" }} />
          {Array.from({ length: totalRodadas }, (_, index) => (
            <col key={`rodada-col-${index + 1}`} style={{ width: "68px" }} />
          ))}
          <col style={{ width: "66px" }} />
        </colgroup>
        <thead>
          <tr className="border-b border-white/10 text-left text-[11px] uppercase tracking-[0.18em] text-text-muted">
            <th className="py-3 pr-0.5">{t("standings.col.pos")}</th>
            <th className="py-3 pr-0.5 text-center" />
            <th className="py-3 pr-1 text-center">Id</th>
            <th className="py-3 pr-1">{t("standings.col.driver")}</th>
            <th className="py-3 pr-1">{t("standings.col.team")}</th>
            {Array.from({ length: totalRodadas }, (_, index) => {
              const rodada = index + 1;
              const isCompleted = rodada <= completedRounds;
              return (
                <th
                  key={rodada}
                  className={[
                    "px-1.5 py-3 text-center",
                    rodada > completedRounds ? "opacity-30" : "",
                    rodada === currentRound ? "text-accent-primary" : "",
                    isCompleted ? "cursor-pointer hover:text-accent-primary transition-colors" : "",
                  ].join(" ")}
                  title={isCompleted ? t("standings.badge.reviewRace") : undefined}
                  onDoubleClick={isCompleted ? () => onReviewRace?.(rodada) : undefined}
                >
                  R{rodada}
                </th>
              );
            })}
            <th className="py-3 pl-3 text-right">Pts</th>
          </tr>
        </thead>
        <tbody>
          {sections.map((section) => (
            <Fragment key={`drivers-${section.id}`}>
              {specialClassGroups ? (
                <tr>
                  <td colSpan={totalRodadas + 6} className="px-0 pt-4 pb-2">
                    <SpecialClassHeader section={section} sticky />
                  </td>
                </tr>
              ) : null}
              {section.items.map((driver, index) => {
                const isInSelectedTeam = selectedTeamId != null && driver.equipe_id === selectedTeamId;
                const teamColor = selectedTeamColor;
                const displayPosition = specialClassGroups ? index + 1 : driver.posicao_campeonato;

                return (
                  <tr
                    key={driver.id}
                    role="button"
                    tabIndex={0}
                    onMouseEnter={() => onDriverHover(driver.id)}
                    onMouseLeave={() => onDriverHover(null)}
                    onClick={() => onDriverClick(driver.id)}
                    onDoubleClick={() => onDriverDoubleClick(driver.id)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        onDriverActivate(driver.id);
                      }
                    }}
                    className={[
                      "cursor-pointer border-b border-white/5 transition-glass",
                      !isInSelectedTeam && driver.is_jogador
                        ? "bg-accent-primary/8 hover:bg-accent-primary/15"
                        : !isInSelectedTeam
                          ? "hover:bg-white/5 focus-visible:bg-white/5"
                          : "",
                    ].join(" ")}
                    style={
                      isInSelectedTeam && teamColor
                        ? {
                            backgroundColor: `${teamColor}22`,
                            boxShadow: `inset 3px 0 0 0 ${teamColor}`,
                          }
                        : undefined
                    }
                  >
                    <td className="py-3 pr-0.5 text-sm font-semibold">
                      <div className="flex items-center gap-1">
                        <span className={podiumClass(index)}>{displayPosition}</span>
                        <PositionDelta delta={positionDeltaMap.get(driver.id)} />
                      </div>
                    </td>
                    <td className="py-3 pr-0.5 text-center">
                      <FlagIcon nacionalidade={driver.nacionalidade} className="mx-auto" />
                    </td>
                    <td className="py-3 pr-1 text-center text-sm font-medium text-text-secondary">
                      {driver.idade ?? "—"}
                    </td>
                    <td className="py-3 pr-1">
                      <div className="flex items-center gap-2">
                        <span
                          className={[
                            "block truncate text-sm whitespace-nowrap",
                            driver.is_jogador ? "font-semibold text-accent-primary" : "text-text-primary",
                          ].join(" ")}
                          title={driver.nome}
                        >
                          {driver.is_jogador ? "▸ " : ""}
                          {driver.nome}
                          {driver.is_jogador ? " ◂" : ""}
                        </span>
                        <DriverStatusMarkers driver={driver} />
                        {driver.id === previousChampionId ? (
                          <span className="shrink-0 text-sm" title={t("standings.badge.prevChampion")}>
                            🏆
                          </span>
                        ) : null}
                      </div>
                    </td>
                    <td className="py-3 pr-1 pl-0 text-sm font-semibold uppercase tracking-[0.02em]">
                      <div className="flex items-center gap-1.5">
                        <TeamLogoMarkShared
                          teamName={driver.equipe_nome}
                          color={driver.equipe_cor}
                          size="sm"
                        />
                        <span
                          className="block truncate"
                          style={{ color: getReadableTeamColor(driver.equipe_cor) }}
                          title={driver.equipe_nome_curto ?? driver.equipe_nome ?? "—"}
                        >
                          {driver.equipe_nome_curto ?? driver.equipe_nome ?? "—"}
                        </span>
                      </div>
                    </td>
                    {(driver.results ?? []).map((result, rodadaIndex) => (
                      <td key={`${driver.id}-r${rodadaIndex + 1}`} className="px-1 py-3 text-center">
                        <ResultBadge result={result} />
                      </td>
                    ))}
                    <td className="kfx py-3 pl-3 text-right font-mono text-sm font-semibold text-text-primary">
                      {driver.pontos}
                    </td>
                  </tr>
                );
              })}
            </Fragment>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export default DriverStandingsTable;
