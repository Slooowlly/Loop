import { useState } from "react";
import { useTranslation } from "react-i18next";

import GlassCard from "../../ui/GlassCard";
import TeamLogoMark from "../TeamLogoMark";
import { formatMoney } from "../../../utils/formatters";
import {
  RANKING_TIER_COLORS,
  carTierIndex,
  defaultSortDirection,
  qualityTierIndex,
  sortRankingRows,
} from "./teamMetrics";

function RankingTable({ teams, playerTeam, historyTeamId, onTeamHistoryOpen }) {
  const { t } = useTranslation();
  const rows = Array.isArray(teams) ? teams : [];
  const [sort, setSort] = useState({ key: "default", direction: "asc" });
  const sortedRows = sortRankingRows(rows, sort);

  function handleSort(key) {
    setSort((current) => {
      if (current.key === key) {
        return { key, direction: current.direction === "asc" ? "desc" : "asc" };
      }
      return { key, direction: defaultSortDirection(key) };
    });
  }

  return (
    <GlassCard hover={false} className="rounded-[28px]">
      <p className="text-[10px] uppercase tracking-[0.22em] text-accent-primary">{t("myTeamTab.ranking.eyebrow")}</p>
      <h3 className="mt-2 text-xl font-semibold text-text-primary">{t("myTeamTab.ranking.title")}</h3>
      <div className="mt-5 overflow-x-auto">
        <table className="min-w-full text-left text-sm" aria-label={t("myTeamTab.ranking.title")}>
          <thead>
            <tr className="border-b border-white/8 text-[10px] uppercase tracking-[0.18em] text-text-muted">
              <SortableHeader label="#" sortKey="posicao" sort={sort} onSort={handleSort} className="py-3 pr-4" />
              <SortableHeader label={t("myTeamTab.ranking.columns.team")} sortKey="nome" sort={sort} onSort={handleSort} />
              <SortableHeader label={t("myTeamTab.ranking.columns.money")} sortKey="cash_balance" sort={sort} onSort={handleSort} />
              <SortableHeader label={t("myTeamTab.ranking.columns.carLevel")} sortKey="car_level" sort={sort} onSort={handleSort} />
              <SortableHeader label={t("myTeamTab.ranking.columns.reliability")} sortKey="confiabilidade" sort={sort} onSort={handleSort} />
              <SortableHeader label={t("myTeamTab.ranking.columns.pitCrew")} sortKey="pit_crew_quality" sort={sort} onSort={handleSort} />
              <SortableHeader label={t("myTeamTab.ranking.columns.points")} sortKey="pontos" sort={sort} onSort={handleSort} />
            </tr>
          </thead>
          <tbody>
            {sortedRows.slice(0, 10).map((team, index) => (
              <tr
                key={team.id}
                className={[
                  "border-b border-white/6 last:border-0 transition-all duration-200",
                  team.id === historyTeamId
                    ? "bg-status-yellow/10 text-text-primary ring-1 ring-status-yellow/45 shadow-[inset_4px_0_0_rgba(242,196,109,0.95)]"
                    : team.id === playerTeam?.id
                      ? "bg-accent-primary/10 text-text-primary"
                      : "text-text-secondary",
                ].join(" ")}
                data-history-active={team.id === historyTeamId ? "true" : undefined}
              >
                <td className="py-3 pr-4 font-mono text-xs text-text-muted">{String(team.posicao ?? index + 1).padStart(2, "0")}</td>
                <td className="px-4 py-3 font-semibold">
                  <div className="flex items-center gap-3">
                    <TeamLogoMark
                      teamName={team.nome}
                      color={team.cor_primaria}
                      size="sm"
                      testId="ranking-team-logo"
                    />
                    <button
                      type="button"
                      data-testid="ranking-team-name"
                      onDoubleClick={() => onTeamHistoryOpen?.(team)}
                      className="rounded-lg text-left transition-glass hover:brightness-125 focus:outline-none focus:ring-2 focus:ring-accent-primary/45"
                      style={{ color: team.cor_primaria ?? "#f0f6fc" }}
                      title={t("myTeamTab.ranking.doubleClickHint")}
                    >
                      {team.nome}
                    </button>
                  </div>
                </td>
                <td className="px-4 py-3 font-mono">{formatMoney(team.cash_balance ?? 0)}</td>
                <td className="whitespace-nowrap px-4 py-3">
                  <RankingTier
                    testId={`ranking-car-tier-${team.id}`}
                    tier={carTierIndex(team.car_level)}
                    label={t(`myTeamTab.ranking.tiers.car.${carTierIndex(team.car_level)}`)}
                  />
                </td>
                <td className="whitespace-nowrap px-4 py-3">
                  <RankingTier
                    testId={`ranking-reliability-tier-${team.id}`}
                    tier={qualityTierIndex(team.confiabilidade)}
                    label={t(`myTeamTab.ranking.tiers.reliability.${qualityTierIndex(team.confiabilidade)}`)}
                  />
                </td>
                <td className="whitespace-nowrap px-4 py-3">
                  <RankingTier
                    testId={`ranking-pit-crew-tier-${team.id}`}
                    tier={qualityTierIndex(team.pit_crew_quality)}
                    label={t(`myTeamTab.ranking.tiers.pitCrew.${qualityTierIndex(team.pit_crew_quality)}`)}
                  />
                </td>
                <td className="px-4 py-3 font-mono text-text-primary">{team.pontos ?? 0}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </GlassCard>
  );
}

function RankingTier({ testId, tier, label }) {
  const color = RANKING_TIER_COLORS[tier] ?? RANKING_TIER_COLORS[0];
  return (
    <span data-testid={testId} className="inline-flex items-center gap-2 text-xs font-semibold" style={{ color }}>
      <span
        aria-hidden="true"
        className="h-1.5 w-1.5 rounded-full"
        style={{ backgroundColor: color, boxShadow: `0 0 8px ${color}80` }}
      />
      {label}
    </span>
  );
}

function SortableHeader({ label, sortKey, sort, onSort, className = "px-4 py-3" }) {
  const isActive = sort.key === sortKey;
  const indicator = isActive ? (sort.direction === "asc" ? "↑" : "↓") : "↕";

  return (
    <th className={className}>
      <button
        type="button"
        onClick={() => onSort(sortKey)}
        className="inline-flex items-center gap-1 rounded-lg text-left transition-glass hover:text-text-primary"
      >
        <span>{label}</span>
        <span className={isActive ? "text-accent-primary" : "text-text-muted"}>{indicator}</span>
      </button>
    </th>
  );
}

export default RankingTable;
