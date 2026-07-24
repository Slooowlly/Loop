import { useTranslation } from "react-i18next";

import TeamLogoMark from "../../team/TeamLogoMark";
import {
  formatBestSeason,
  formatCareerYears,
  formatCategoryLabel,
  formatRaceMilestone,
  formatRankedValue,
  formatSpecialCampaign,
  formatSpecialEventEntry,
  formatUnemploymentYears,
  formatYearsAverage,
} from "./formatadores.js";

export function CategoryTimeline({ items }) {
  const { t } = useTranslation();
  const timeline = Array.isArray(items) ? items.filter((item) => item?.categoria) : [];

  if (!timeline.length) {
    return <p className="text-xs text-[#7d8590]">{t("driverDetail.history.noCategories")}</p>;
  }

  return (
    <div className="flex flex-wrap items-center gap-2">
      {timeline.map((item, index) => (
        <div key={`${item.categoria}-${item.ano_inicio}-${index}`} className="flex items-center gap-2">
          {index > 0 ? <span className="text-xs font-semibold text-[#7d8590]">-&gt;</span> : null}
          <div className="rounded-lg border border-white/8 bg-black/15 px-3 py-2">
            <div className="text-sm font-semibold text-[#e6edf3]">
              {formatCategoryLabel(item.categoria)} {item.ano_inicio ?? "-"}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

export function DebutTeamLine({ teamName }) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center justify-between gap-4 border-b border-white/6 py-2 last:border-b-0 last:pb-0">
      <span className="text-[11px] uppercase tracking-[0.16em] text-[#7d8590]">
        {t("driverDetail.history.debutTeam")}
      </span>
      {teamName ? (
        <span className="flex min-w-0 items-center justify-end gap-2 text-right text-sm font-medium text-[#e6edf3]">
          <TeamLogoMark teamName={teamName} size="xs" testId="driver-debut-team-logo" />
          <span className="truncate">{teamName}</span>
        </span>
      ) : (
        <span className="text-right text-sm font-medium text-[#e6edf3]">{t("driverDetail.history.notIdentified")}</span>
      )}
    </div>
  );
}

function CareerHistoryLine({ label, value }) {
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_auto] items-baseline gap-4 py-1.5">
      <div className="min-w-0 text-xs font-medium text-[#8b949e]">{label}</div>
      <div className="text-right text-sm font-semibold text-[#e6edf3]">{value}</div>
    </div>
  );
}

function CareerHistoryGroup({ title, rows, first = false }) {
  return (
    <div className={first ? "" : "border-t border-white/8 pt-3"}>
      <div className="mb-2 text-[10px] font-bold uppercase tracking-[0.18em] text-[#58a6ff]">
        {title}
      </div>
      <div className="grid gap-0.5">
        {rows.map((row) => (
          <CareerHistoryLine key={row.label} label={row.label} value={row.value} />
        ))}
      </div>
    </div>
  );
}

function SpecialEventsTimeline({ items }) {
  const { t } = useTranslation();
  const timeline = Array.isArray(items) ? items : [];

  if (!timeline.length) {
    return <p className="mt-2 text-xs text-[#7d8590]">{t("driverDetail.history.noSpecialEvents")}</p>;
  }

  return (
    <div className="mt-3 flex flex-wrap items-center gap-2">
      {timeline.map((event, index) => (
        <div key={`${event.ano}-${event.categoria}-${event.classe}-${index}`} className="flex items-center gap-2">
          {index > 0 ? <span className="text-xs font-semibold text-[#7d8590]">-&gt;</span> : null}
          <span className="rounded-lg border border-white/8 bg-black/15 px-3 py-2 text-xs font-semibold text-[#e6edf3]">
            {formatSpecialEventEntry(event)}
          </span>
        </div>
      ))}
    </div>
  );
}

export function CareerHistoryDossier({ history }) {
  const { t } = useTranslation();
  if (!history) return null;

  const presence = history.presenca ?? {};
  const firstMarks = history.primeiros_marcos ?? {};
  const peak = history.auge ?? {};
  const mobility = history.mobilidade ?? {};
  const injuries = history.lesoes ?? {};
  const specialEvents = history.eventos_especiais ?? {};
  const specialRanks = specialEvents.rankings ?? {};
  const bestSeason = peak.melhor_temporada;

  return (
    <div className="glass-light rounded-xl p-4" data-testid="career-history-dossier">
      <div className="grid gap-3 md:grid-cols-2">
        <CareerHistoryGroup
          title={t("driverDetail.history.groupPresence")}
          first
          rows={[
            { label: t("driverDetail.history.careerTime"), value: formatCareerYears(presence.tempo_carreira) },
            { label: t("driverDetail.history.seasonsPlayed"), value: presence.temporadas_disputadas ?? 0 },
            { label: t("driverDetail.history.yearsUnemployed"), value: formatUnemploymentYears(presence) },
            { label: t("driverDetail.history.categoriesContested"), value: presence.categorias_disputadas ?? 0 },
          ]}
        />
        <CareerHistoryGroup
          title={t("driverDetail.history.groupFirstMarks")}
          first
          rows={[
            { label: t("driverDetail.history.firstPodium"), value: formatRaceMilestone(firstMarks.primeiro_podio_corrida) },
            { label: t("driverDetail.history.firstWin"), value: formatRaceMilestone(firstMarks.primeira_vitoria_corrida) },
            { label: t("driverDetail.history.firstDnf"), value: formatRaceMilestone(firstMarks.primeiro_dnf_corrida) },
          ]}
        />
        <CareerHistoryGroup
          title={t("driverDetail.history.groupPeak")}
          rows={[
            { label: t("driverDetail.history.bestSeason"), value: formatBestSeason(bestSeason) },
            { label: t("driverDetail.history.bestChampionship"), value: bestSeason?.posicao_campeonato ? `P${bestSeason.posicao_campeonato}` : "-" },
            { label: t("driverDetail.history.longestWinStreak"), value: peak.maior_sequencia_vitorias ?? 0 },
          ]}
        />
        <CareerHistoryGroup
          title={t("driverDetail.history.groupMobility")}
          rows={[
            { label: t("driverDetail.history.promotions"), value: mobility.promocoes ?? 0 },
            { label: t("driverDetail.history.relegations"), value: mobility.rebaixamentos ?? 0 },
            { label: t("driverDetail.history.teamsDefended"), value: mobility.equipes_defendidas ?? 0 },
            { label: t("driverDetail.history.avgTimePerTeam"), value: formatYearsAverage(mobility.tempo_medio_por_equipe) },
          ]}
        />
        <CareerHistoryGroup
          title={t("driverDetail.history.groupInjuries")}
          rows={[
            { label: t("driverDetail.history.injuriesLight"), value: injuries.leves ?? 0 },
            { label: t("driverDetail.history.injuriesModerate"), value: injuries.moderadas ?? 0 },
            { label: t("driverDetail.history.injuriesSevere"), value: injuries.graves ?? 0 },
          ]}
        />
        <div className="border-t border-white/8 pt-3 md:col-span-2">
          <CareerHistoryGroup
            title={t("driverDetail.history.groupSpecialEvents")}
            first
            rows={[
              {
                label: t("driverDetail.history.participations"),
                value: formatRankedValue(specialEvents.participacoes, specialRanks.participacoes),
              },
              {
                label: t("driverDetail.history.callUps"),
                value: formatRankedValue(specialEvents.convocacoes, specialRanks.convocacoes),
              },
              { label: t("driverDetail.history.wins"), value: formatRankedValue(specialEvents.vitorias, specialRanks.vitorias) },
              { label: t("driverDetail.history.podiums"), value: formatRankedValue(specialEvents.podios, specialRanks.podios) },
              { label: t("driverDetail.history.bestCampaign"), value: formatSpecialCampaign(specialEvents.melhor_campanha) },
              { label: t("driverDetail.history.lastEvent"), value: formatSpecialEventEntry(specialEvents.ultimo_evento) },
            ]}
          />
          <SpecialEventsTimeline items={specialEvents.timeline} />
        </div>
      </div>
    </div>
  );
}
