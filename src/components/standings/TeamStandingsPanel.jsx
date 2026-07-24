import { useTranslation } from "react-i18next";

import i18n from "../../i18n/index.js";
import TeamLogoMarkShared from "../team/TeamLogoMark";
import GlassCard from "../ui/GlassCard";
import TrophyBadge from "./TrophyBadge";
import { SpecialClassHeader, SpecialPendingTeamsNotice } from "./SpecialStandingNotices";
import { getReadableTeamColor, formatTeamDriverPair, podiumClass } from "./standingsFormatting";
import { buildSpecialStandingSections, getSpecialClassRelegationCount, getZoneCutoffs } from "./standingsLadder";

function TeamDriverLine({ team }) {
  const driverNames = formatTeamDriverPair(team);

  return (
    <p
      className="block truncate whitespace-nowrap text-xs text-text-secondary"
      title={driverNames}
    >
      {driverNames}
    </p>
  );
}

function ZoneDivider({ label, variant }) {
  const colorClass = variant === "green" ? "text-status-green border-status-green/30" : "text-status-red border-status-red/30";
  const lineClass = variant === "green" ? "border-status-green/20" : "border-status-red/20";
  return (
    <div className="flex items-center gap-3 py-1">
      <div className={["flex-1 border-t border-dashed", lineClass].join(" ")} />
      <span className={["text-[10px] font-semibold uppercase tracking-[0.18em] px-2 py-0.5 rounded border", colorClass].join(" ")}>
        {label}
      </span>
      <div className={["flex-1 border-t border-dashed", lineClass].join(" ")} />
    </div>
  );
}

function TeamStandingCard({
  team,
  position,
  index,
  isRelegationZone = false,
  isHistoryActive = false,
  onTeamDossierOpen,
  onTeamGlobalHistoryOpen,
}) {
  const cardClassName = [
    "flex items-center justify-between rounded-2xl border px-4 py-3 transition-glass",
    isHistoryActive
      ? "border-status-yellow/45 bg-status-yellow/10 shadow-[inset_4px_0_0_rgba(242,196,109,0.95)] hover:bg-status-yellow/15"
      : "",
    isRelegationZone
      ? "border-status-red/35 bg-status-red/[0.12] shadow-[inset_3px_0_0_0_rgba(248,81,73,0.75)] hover:bg-status-red/[0.18]"
      : !isHistoryActive
        ? "border-white/6 bg-white/[0.03] hover:bg-white/[0.05]"
        : "",
  ].join(" ");

  return (
    <div
      onClick={() => onTeamDossierOpen?.(team)}
      onDoubleClick={() => onTeamGlobalHistoryOpen?.(team)}
      className={cardClassName}
      data-relegation-zone={isRelegationZone ? "true" : undefined}
    >
      <div className="flex min-w-0 flex-1 items-center gap-3">
        <span
          className={[
            "w-7 text-center text-sm font-semibold",
            isRelegationZone ? "text-status-red" : podiumClass(index),
          ].join(" ")}
        >
          {position}
        </span>
        <TeamLogoMarkShared teamName={team.nome} color={team.cor_primaria} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <p
              className="block truncate text-sm font-semibold"
              style={{ color: getReadableTeamColor(team.cor_primaria) }}
            >
              {team.nome}
            </p>
            {(team.trofeus ?? []).map((trofeu, trophyIndex) => (
              <TrophyBadge key={`${team.id}-t${trophyIndex}`} trofeu={trofeu} />
            ))}
          </div>
          <TeamDriverLine team={team} />
        </div>
      </div>
      <div className="shrink-0 pl-4 text-right">
        <p className="kfx font-mono text-base font-semibold text-text-primary">{team.pontos}</p>
        <p className="text-xs text-text-secondary">{team.vitorias} {i18n.t("standings.winsShort")}</p>
      </div>
    </div>
  );
}

// Card de construtores: nas categorias multiclasse a lista quebra por classe (cada
// uma com sua zona de rebaixamento); nas regulares, as faixas de promoção/
// rebaixamento entram como divisores no meio da lista.
function TeamStandingsPanel({
  teamStandings,
  viewCategory,
  specialClassGroups,
  showSpecialPendingNotice,
  selectedHistoryTeamId,
  onTeamDossierOpen,
  onTeamGlobalHistoryOpen,
}) {
  const { t } = useTranslation();
  const sections = buildSpecialStandingSections(teamStandings, specialClassGroups);

  function renderRegularList() {
    const { promotionCount, relegationCount } = getZoneCutoffs(viewCategory);
    const total = teamStandings.length;
    const items = [];
    teamStandings.forEach((team, index) => {
      if (index === promotionCount && promotionCount > 0) {
        items.push(<ZoneDivider key="divider-promo" label={i18n.t("standings.zone.promotion")} variant="green" />);
      }
      if (relegationCount > 0 && index === total - relegationCount) {
        items.push(<ZoneDivider key="divider-relego" label={i18n.t("standings.zone.relegation")} variant="red" />);
      }
      items.push(
        <div
          key={team.id}
          onClick={() => onTeamDossierOpen(team)}
          onDoubleClick={() => onTeamGlobalHistoryOpen(team)}
          className={[
            "flex items-center justify-between rounded-2xl border px-4 py-3 transition-glass hover:bg-white/[0.05]",
            selectedHistoryTeamId === team.id
              ? "border-status-yellow/45 bg-status-yellow/10 shadow-[inset_4px_0_0_rgba(242,196,109,0.95)]"
              : "border-white/6 bg-white/[0.03]",
          ].join(" ")}
        >
          <div className="flex min-w-0 flex-1 items-center gap-3">
            <span className={["w-7 text-center text-sm font-semibold", podiumClass(index)].join(" ")}>
              {team.posicao}
            </span>
            <TeamLogoMarkShared teamName={team.nome} color={team.cor_primaria} />
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-1.5">
                <p
                  className="truncate text-sm font-semibold"
                  style={{ color: getReadableTeamColor(team.cor_primaria) }}
                >
                  {team.nome}
                </p>
                {(team.trofeus ?? []).map((trofeu, trophyIndex) => (
                  <TrophyBadge key={`${team.id}-t${trophyIndex}`} trofeu={trofeu} />
                ))}
              </div>
              <TeamDriverLine team={team} />
            </div>
          </div>
          <div className="shrink-0 pl-4 text-right">
            <p className="kfx font-mono text-base font-semibold text-text-primary">{team.pontos}</p>
            <p className="text-xs text-text-secondary">{team.vitorias} {i18n.t("standings.winsShort")}</p>
          </div>
        </div>
      );
    });
    return items;
  }

  return (
    <GlassCard hover={false} className="rounded-[28px]">
      <div className="flex items-center justify-between gap-4">
        <div>
          <p className="kfx text-[11px] uppercase tracking-[0.22em] text-accent-primary">
            {t("standings.constructors")}
          </p>
          <h2 className="kfx mt-2 text-2xl font-semibold text-text-primary">
            {t("standings.teamsTitle")}
          </h2>
        </div>
        <p className="text-sm text-text-secondary">{t("standings.teamsCount", { count: teamStandings.length })}</p>
      </div>

      <div className="mt-6 space-y-2">
        {showSpecialPendingNotice ? (
          <SpecialPendingTeamsNotice />
        ) : specialClassGroups ? (
          sections.map((section) => {
            const relegationCount = getSpecialClassRelegationCount(viewCategory, section.id);
            return (
              <div key={`teams-${section.id}`} className="space-y-2">
                <SpecialClassHeader section={section} />
                {section.items.map((team, index) => {
                  const isRelegationZone =
                    relegationCount > 0
                    && section.items.length > relegationCount
                    && index >= section.items.length - relegationCount;
                  return (
                    <TeamStandingCard
                      key={team.id}
                      team={team}
                      position={index + 1}
                      index={index}
                      isRelegationZone={isRelegationZone}
                      isHistoryActive={selectedHistoryTeamId === team.id}
                      onTeamDossierOpen={onTeamDossierOpen}
                      onTeamGlobalHistoryOpen={onTeamGlobalHistoryOpen}
                    />
                  );
                })}
              </div>
            );
          })
        ) : (
          renderRegularList()
        )}
      </div>
    </GlassCard>
  );
}

export default TeamStandingsPanel;
