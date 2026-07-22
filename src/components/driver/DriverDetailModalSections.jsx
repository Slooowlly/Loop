import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

import i18n from "../../i18n/index.js";
import { ordinal } from "../../i18n/format.js";
import { formatSalary, formatSalaryMonthly } from "../../utils/formatters";
import TeamLogoMark from "../team/TeamLogoMark";

function formatStatValue(value) {
  if (value === null || value === undefined) return "-";
  return value;
}

function formatAverage(value) {
  if (value === null || value === undefined) return "-";
  return value.toFixed(1);
}

function formatRank(value) {
  if (value === null || value === undefined) return "";
  return ` (${ordinal(value)})`;
}

function formatRankedValue(value, rank) {
  return `${value ?? 0}${formatRank(rank)}`;
}

function isCareerDebutantDetail(detail) {
  return (detail.stats_carreira?.corridas ?? 0) === 0;
}

function formatContractPeriod(contract) {
  if (!contract) return "-";

  const start = contract.ano_inicio ?? contract.temporada_inicio;
  const end = contract.ano_fim ?? contract.temporada_fim;
  return `${start} - ${end}`;
}

function formatContractRole(role) {
  if (role === "Numero1" || role === "N1" || role === "Piloto N1") return "N1";
  if (role === "Numero2" || role === "N2" || role === "Piloto N2") return "N2";
  return role || "-";
}

export function formatMoment(momento) {
  const map = {
    forte: { label: i18n.t("driverDetail.momentBuilder.forte"), color: "text-[#3fb950]" },
    estavel: { label: i18n.t("driverDetail.momentBuilder.estavel"), color: "text-[#d29922]" },
    em_baixa: { label: i18n.t("driverDetail.momentBuilder.em_baixa"), color: "text-[#f85149]" },
    sem_dados: { label: i18n.t("driverDetail.momentBuilder.sem_dados"), color: "text-[#7d8590]" },
  };

  return map[momento] || map.sem_dados;
}

function DetailRow({ label, value, valueClassName = "text-[#e6edf3]" }) {
  return (
    <div className="flex items-start justify-between gap-4 border-b border-white/6 py-2 last:border-b-0 last:pb-0">
      <span className="text-[11px] uppercase tracking-[0.16em] text-[#7d8590]">{label}</span>
      <span className={["text-right text-sm font-medium", valueClassName].join(" ")}>{value}</span>
    </div>
  );
}

function StatCard({ label, value, tone = "text-[#e6edf3]" }) {
  return (
    <div className="rounded-lg border border-white/6 bg-black/10 p-2.5">
      <div className={["text-lg font-bold", tone].join(" ")}>{formatStatValue(value)}</div>
      <div className="text-[10px] uppercase tracking-[0.16em] text-[#7d8590]">{label}</div>
    </div>
  );
}

function ProgressRow({ label, value, max = 100, color = "#58a6ff", right = null }) {
  const normalized = Number.isFinite(value) ? Math.max(0, Math.min(value, max)) : 0;
  const width = max > 0 ? (normalized / max) * 100 : 0;

  return (
    <div className="grid gap-2 sm:grid-cols-[120px_minmax(0,1fr)_42px] sm:items-center">
      <div className="text-xs font-medium text-[#c9d1d9]">{label}</div>
      <div className="h-2 overflow-hidden rounded-full bg-[#21262d]">
        <div className="h-full rounded-full" style={{ width: `${width}%`, backgroundColor: color }} />
      </div>
      <div className="text-right font-mono text-xs text-[#7d8590]">{right ?? formatStatValue(value)}</div>
    </div>
  );
}

const technicalToneClass = {
  danger: "text-[#f85149]",
  warning: "text-[#d29922]",
  neutral: "text-[#c9d1d9]",
  info: "text-[#58a6ff]",
  success: "text-[#3fb950]",
  elite: "text-[#bc8cff]",
};

const summaryToneClass = {
  danger: {
    card: "border-[#f85149]/25 bg-[#f85149]/10",
    label: "text-[#f85149]",
  },
  warning: {
    card: "border-[#d29922]/25 bg-[#d29922]/10",
    label: "text-[#d29922]",
  },
  info: {
    card: "border-[#58a6ff]/20 bg-[#58a6ff]/8",
    label: "text-[#58a6ff]",
  },
  success: {
    card: "border-[#3fb950]/25 bg-[#3fb950]/10",
    label: "text-[#3fb950]",
  },
};

function QualityLevelRow({ item }) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-lg border border-white/6 bg-black/10 px-3 py-2.5">
      <span className="text-sm font-medium text-[#c9d1d9]">{item.label}</span>
      <span className={["text-right text-sm font-semibold", technicalToneClass[item.tom] || technicalToneClass.neutral].join(" ")}>
        {item.nivel}
      </span>
    </div>
  );
}

function CareerRankStat({ label, value, rank, tone = "text-[#e6edf3]" }) {
  return (
    <div className="rounded-lg border border-white/6 bg-black/10 p-2.5">
      <div className="flex items-baseline gap-1">
        <span className={["text-lg font-bold", tone].join(" ")}>{formatStatValue(value)}</span>
        {rank ? (
          <span className="text-[11px] font-semibold leading-none text-[#7d8590]">
            {formatRank(rank)}
          </span>
        ) : null}
      </div>
      <div className="text-[10px] uppercase tracking-[0.16em] text-[#7d8590]">{label}</div>
    </div>
  );
}

function RookieFormState() {
  const { t } = useTranslation();
  return (
    <div className="relative overflow-hidden rounded-xl border border-[#58a6ff]/22 bg-[#071120] p-4">
      <div className="absolute inset-x-4 top-4 h-px bg-[#58a6ff]/35" />
      <div className="absolute bottom-4 left-4 right-4 grid grid-cols-5 gap-2 opacity-35">
        {Array.from({ length: 10 }).map((_, index) => (
          <span key={`rookie-slot-${index}`} className="h-7 rounded-sm border border-[#58a6ff]/30 bg-[#58a6ff]/8" />
        ))}
      </div>
      <div className="relative grid gap-4 sm:grid-cols-[130px_minmax(0,1fr)] sm:items-center">
        <div className="rounded-lg border border-[#58a6ff]/25 bg-[#58a6ff]/12 px-4 py-3 text-center shadow-[0_0_30px_rgba(88,166,255,0.12)]">
          <div className="text-[10px] font-bold uppercase tracking-[0.22em] text-[#58a6ff]">
            {t("driverDetail.summary.rookieBadge")}
          </div>
          <div className="mt-2 text-3xl font-bold text-[#e6edf3]">0</div>
          <div className="text-[10px] uppercase tracking-[0.18em] text-[#8b949e]">{t("driverDetail.summary.races")}</div>
        </div>
        <div>
          <div className="text-lg font-semibold text-[#e6edf3]">{t("driverDetail.summary.noFormHistory")}</div>
          <div className="mt-1 text-sm text-[#8b949e]">{t("driverDetail.summary.formStartsAfter")}</div>
        </div>
      </div>
    </div>
  );
}

function InsufficientFormState() {
  const { t } = useTranslation();
  return (
    <div className="rounded-xl border border-white/6 bg-black/10 p-4">
      <div className="text-sm font-semibold text-[#c9d1d9]">{t("driverDetail.summary.insufficientTitle")}</div>
      <div className="mt-1 text-xs text-[#7d8590]">{t("driverDetail.summary.insufficientBody")}</div>
    </div>
  );
}

function InactivePreviousSeasonState({ context }) {
  const { t } = useTranslation();
  const isWithoutTeam = context === "sem_time_temporada_passada";
  const title = isWithoutTeam
    ? t("driverDetail.summary.noTeamLastSeason")
    : t("driverDetail.summary.noRacesLastSeason");
  const body = isWithoutTeam
    ? t("driverDetail.summary.noTeamLastSeasonBody")
    : t("driverDetail.summary.noRacesLastSeasonBody");

  return (
    <div className="relative overflow-hidden rounded-xl border border-[#d29922]/24 bg-[#d29922]/9 p-4">
      <div className="absolute inset-x-4 top-4 h-px bg-[#d29922]/30" />
      <div className="relative flex min-h-[156px] flex-col items-center justify-center text-center">
        <div className="text-[10px] font-bold uppercase tracking-[0.22em] text-[#d29922]">
          {t("driverDetail.summary.offGrid")}
        </div>
        <div className="mt-3 text-2xl font-bold text-[#e6edf3]">{title}</div>
        <div className="mt-2 max-w-md text-sm text-[#8b949e]">{body}</div>
      </div>
    </div>
  );
}

function RookieDossierState({ SectionComponent, title }) {
  const { t } = useTranslation();
  return (
    <SectionComponent title={title ?? t("driverDetail.summary.title")}>
      <div className="flex min-h-[180px] flex-col items-center justify-center text-center">
        <div className="text-[10px] font-bold uppercase tracking-[0.24em] text-[#58a6ff]">
          {t("driverDetail.summary.newOnGrid")}
        </div>
        <div className="mt-3 text-4xl font-bold text-[#e6edf3]">{t("driverDetail.summary.rookie")}</div>
        <div className="mt-3 max-w-sm text-sm font-semibold text-[#c9d1d9]">
          {t("driverDetail.summary.unknownExpectation")}
        </div>
        <div className="mt-1 max-w-sm text-sm text-[#8b949e]">{t("driverDetail.summary.noCompetitivePast")}</div>
      </div>
    </SectionComponent>
  );
}

function RookieUnavailableSection({ SectionComponent, title }) {
  const { t } = useTranslation();
  return (
    <SectionComponent title={title}>
      <div className="flex min-h-[180px] flex-col items-center justify-center text-center">
        <div className="text-[10px] font-bold uppercase tracking-[0.2em] text-[#58a6ff]">
          {t("driverDetail.summary.unavailableForRookie")}
        </div>
        <div className="mt-3 text-3xl font-bold text-[#e6edf3]">{t("driverDetail.summary.rookie")}</div>
        <div className="mt-2 max-w-sm text-sm text-[#8b949e]">
          {t("driverDetail.summary.noCompetitivePastRead")}
        </div>
      </div>
    </SectionComponent>
  );
}

function FormMetric({ label, value, tone = "text-[#e6edf3]" }) {
  return (
    <div className="rounded-lg border border-white/6 bg-white/[0.035] px-3 py-2">
      <div className={["text-sm font-bold", tone].join(" ")}>{value}</div>
      <div className="mt-0.5 text-[10px] uppercase tracking-[0.16em] text-[#7d8590]">{label}</div>
    </div>
  );
}

function resultColor(entry) {
  if (entry?.dnf) return "#f85149";
  const finish = entry?.chegada;
  if (!Number.isFinite(finish)) return "#8b949e";
  if (finish === 1) return "#d29922";
  if (finish <= 3) return "#3fb950";
  if (finish <= 10) return "#58a6ff";
  return "#8b949e";
}

function resultOpacity(entry) {
  if (entry?.dnf) return 1;
  const finish = entry?.chegada;
  if (!Number.isFinite(finish)) return 0.36;
  return finish > 10 ? 0.36 : 1;
}

function resultLabel(entry) {
  if (entry?.dnf) return "DNF";
  if (!Number.isFinite(entry?.chegada)) return "-";
  return `P${entry.chegada}`;
}

function RecentFormChart({ entries, rookie, context }) {
  const { t } = useTranslation();
  if (rookie) return <RookieFormState />;
  if (!entries?.length && context) return <InactivePreviousSeasonState context={context} />;
  if (!entries?.length) return <InsufficientFormState />;

  const width = 760;
  const height = 220;
  const chartLeft = 14;
  const chartRight = 746;
  const chartTop = 34;
  const chartBottom = 156;
  const finishValues = entries.map((entry) => (entry?.dnf ? 24 : entry?.chegada ?? 24));
  const maxPosition = Math.max(20, ...finishValues);
  const xStep = entries.length > 1 ? (chartRight - chartLeft) / (entries.length - 1) : 0;
  const points = entries.map((entry, index) => {
    const finish = entry?.dnf ? maxPosition : entry?.chegada ?? maxPosition;
    const normalized = maxPosition > 1 ? (finish - 1) / (maxPosition - 1) : 0;
    const x = chartLeft + index * xStep;
    const y = chartTop + normalized * (chartBottom - chartTop);
    return { x, y, entry, finish };
  });
  const polyline = points.map((point) => `${point.x},${point.y}`).join(" ");
  const areaPolygon = [
    `${chartLeft},${chartBottom}`,
    ...points.map((point) => `${point.x},${point.y}`),
    `${chartRight},${chartBottom}`,
  ].join(" ");
  const validFinishes = entries.filter((entry) => !entry?.dnf && Number.isFinite(entry?.chegada));
  const bestFinish = validFinishes.length
    ? Math.min(...validFinishes.map((entry) => entry.chegada))
    : null;
  const averageFinish = validFinishes.length
    ? validFinishes.reduce((sum, entry) => sum + entry.chegada, 0) / validFinishes.length
    : null;
  const dnfCount = entries.filter((entry) => entry?.dnf).length;

  return (
    <div className="-m-3.5 overflow-hidden bg-[#070b12]">
      <div className="flex items-center justify-between gap-3 border-b border-white/6 px-4 py-3">
        <div>
          <div className="text-sm font-semibold text-[#e6edf3]">{t("driverDetail.summary.recentTrend")}</div>
          <div className="mt-0.5 text-[11px] text-[#7d8590]">{t("driverDetail.summary.recentLast", { count: entries.length })}</div>
        </div>
        <div className="rounded-full border border-[#58a6ff]/20 bg-[#58a6ff]/10 px-2.5 py-1 text-[10px] font-bold uppercase tracking-[0.16em] text-[#58a6ff]">
          {t("driverDetail.summary.position")}
        </div>
      </div>

      <div className="pb-4 pt-1">
        <svg
          viewBox={`0 0 ${width} ${height}`}
          role="img"
          aria-label={t("driverDetail.summary.chartAria")}
          className="block h-auto w-full"
        >
          <defs>
            <linearGradient id="recentFormArea" x1="0" x2="0" y1="0" y2="1">
              <stop offset="0%" stopColor="#58a6ff" stopOpacity="0.34" />
              <stop offset="100%" stopColor="#58a6ff" stopOpacity="0.02" />
            </linearGradient>
            <linearGradient id="recentFormLine" x1="0" x2="1" y1="0" y2="0">
              <stop offset="0%" stopColor="#58a6ff" />
              <stop offset="100%" stopColor="#58a6ff" />
            </linearGradient>
          </defs>
          <rect x="0" y="0" width={width} height={height} rx="0" fill="#0b111c" />
          {[chartTop, (chartTop + chartBottom) / 2, chartBottom].map((lineY) => (
            <line key={`grid-${lineY}`} x1={chartLeft} y1={lineY} x2={chartRight} y2={lineY} stroke="#30363d" strokeOpacity="0.55" strokeDasharray="3 8" />
          ))}
          <polygon points={areaPolygon} fill="url(#recentFormArea)" />
          <polyline points={polyline} fill="none" stroke="url(#recentFormLine)" strokeWidth="4" strokeLinecap="round" strokeLinejoin="round" />
          {points.map((point) => (
            <g key={`recent-form-${point.entry?.rodada ?? point.x}`}>
              <text
                x={point.x}
                y={Math.max(14, point.y - 12)}
                textAnchor="middle"
                className="text-[10px] font-bold"
                fill={resultColor(point.entry)}
                opacity={resultOpacity(point.entry)}
              >
                {resultLabel(point.entry)}
              </text>
              <circle
                cx={point.x}
                cy={point.y}
                r="6"
                fill="#070b12"
                stroke={resultColor(point.entry)}
                strokeWidth="2.4"
                opacity={resultOpacity(point.entry)}
              />
              <circle
                cx={point.x}
                cy={point.y}
                r="2.5"
                fill={resultColor(point.entry)}
                opacity={resultOpacity(point.entry)}
              />
              <text x={point.x} y="190" textAnchor="middle" className="fill-[#7d8590] text-[10px] font-semibold">
                R{point.entry?.rodada ?? "-"}
              </text>
            </g>
          ))}
          <text x={chartLeft} y="17" className="fill-[#3fb950] text-[10px] font-bold">{t("driverDetail.summary.best")}</text>
          <text x={chartLeft} y="174" className="fill-[#7d8590] text-[10px] font-bold">{t("driverDetail.summary.worst")}</text>
        </svg>

        <div className="grid grid-cols-3 gap-2 px-4">
          <FormMetric label={t("driverDetail.summary.metricBest")} value={bestFinish ? `P${bestFinish}` : "-"} tone="text-[#3fb950]" />
          <FormMetric label={t("driverDetail.summary.metricAverage")} value={averageFinish ? `P${averageFinish.toFixed(1)}` : "-"} />
          <FormMetric label={t("driverDetail.summary.metricDnfs")} value={dnfCount} tone={dnfCount ? "text-[#f85149]" : "text-[#8b949e]"} />
        </div>
      </div>
    </div>
  );
}

function TimelineItem({ item }) {
  return (
    <div className="relative pl-5">
      <span className="absolute left-0 top-1.5 h-2.5 w-2.5 rounded-full bg-[#58a6ff]" />
      <div className="text-xs font-semibold uppercase tracking-[0.16em] text-[#7d8590]">
        {item.tipo}
      </div>
      <div className="mt-1 text-sm font-semibold text-[#e6edf3]">{item.titulo}</div>
      <div className="mt-1 text-xs text-[#7d8590]">{item.descricao}</div>
    </div>
  );
}

function CategoryTimeline({ items }) {
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

function DebutTeamLine({ teamName }) {
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

function formatCategoryLabel(categoryId) {
  const normalized = String(categoryId || "").trim();
  if (normalized.includes(":")) {
    const [baseCategory, className] = normalized.split(":");
    const classLabel = formatSpecialClassLabel(className);
    if (baseCategory === "endurance" && classLabel) return `${classLabel} Endurance`;
    if (baseCategory === "production_challenger" && classLabel) return `${classLabel} Production`;
  }

  const map = {
    mazda_rookie: "Mazda Rookie",
    toyota_rookie: "Toyota Rookie",
    mazda_amador: "Mazda Cup",
    toyota_amador: "Toyota Cup",
    bmw_m2: "BMW Cup",
    production_challenger: "Production",
    gt4: "GT4",
    gt3: "GT3",
    endurance: "Endurance",
  };

  return map[normalized] || normalized || "-";
}

function formatRaceMilestone(value) {
  if (value === null || value === undefined) return i18n.t("driverDetail.history.milestoneNever");
  return i18n.t("driverDetail.history.milestoneRace", { value });
}

function formatSpecialClassLabel(className) {
  const map = {
    mazda: "Mazda",
    toyota: "Toyota",
    bmw: "BMW",
    gt4: "GT4",
    gt3: "GT3",
    lmp2: "LMP2",
  };

  return map[className] || className || "";
}

function formatSpecialCategoryAndClass(event) {
  const category = formatCategoryLabel(event?.categoria);
  const classLabel = formatSpecialClassLabel(event?.classe);
  return [category, classLabel].filter(Boolean).join(" ");
}

function formatSpecialCampaign(campaign) {
  if (!campaign) return "-";
  return `${campaign.ano}, ${formatSpecialCategoryAndClass(campaign)}`;
}

function formatSpecialEventEntry(event) {
  if (!event) return "-";
  const base = `${event.ano} ${formatSpecialCategoryAndClass(event)}`;
  return event.equipe ? `${base} - ${event.equipe}` : base;
}

function formatUnemploymentYears(presence) {
  const years = presence?.anos_desempregado ?? 0;
  const periods = Array.isArray(presence?.periodos_desempregado)
    ? presence.periodos_desempregado.filter(Boolean)
    : [];
  const label = i18n.t("driverDetail.moment.expiresValue", { count: years });

  if (periods.length === 0) return label;
  return `${label} (${periods.join(" | ")})`;
}

function formatCareerYears(value) {
  const years = value ?? 0;
  return i18n.t("driverDetail.moment.expiresValue", { count: years });
}

function formatYearsAverage(value) {
  if (value === null || value === undefined) return "-";
  const formatted = Number(value).toFixed(1);
  return i18n.t(
    formatted === "1.0" ? "driverDetail.history.yearsAverage_one" : "driverDetail.history.yearsAverage_other",
    { value: formatted },
  );
}

function formatBestSeason(season) {
  if (!season) return "-";
  return `${season.ano}, ${formatCategoryLabel(season.categoria)}`;
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

function CareerHistoryDossier({ history }) {
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

function TagLine({ tag }) {
  return (
    <div className="flex items-center gap-2 rounded-lg border border-white/6 bg-black/10 px-3 py-2">
      <span className="h-2 w-2 rounded-full" style={{ backgroundColor: tag.color }} />
      <span className="min-w-0 flex-1 text-sm text-[#e6edf3]">{tag.tag_text}</span>
      <span className="text-[10px] uppercase tracking-[0.12em] text-[#7d8590]">{tag.level}</span>
    </div>
  );
}

export function SummarySection({ SectionComponent, detail, moment }) {
  const { t } = useTranslation();
  const resumo = detail.resumo_atual ?? {};
  const stats = detail.performance?.temporada ?? {};
  const form = detail.forma ?? {};
  const rookie = isCareerDebutantDetail(detail);
  const summaryTone = summaryToneClass[resumo.tom] ?? summaryToneClass.info;

  if (rookie) return <RookieDossierState SectionComponent={SectionComponent} />;

  return (
    <>
      <SectionComponent title={t("driverDetail.summary.title")}>
        <div className="grid gap-4 lg:grid-cols-[180px_minmax(0,1fr)]">
          <div
            className={[
              "flex min-h-[156px] flex-col items-center justify-center rounded-xl border p-4 text-center",
              summaryTone.card,
            ].join(" ")}
            data-summary-tone={resumo.tom || "info"}
            data-testid="current-summary-verdict-card"
          >
            <div className={["text-[10px] font-bold uppercase tracking-[0.18em]", summaryTone.label].join(" ")}>
              {t("driverDetail.summary.now")}
            </div>
            <div className="mt-4 text-3xl font-bold text-[#e6edf3]">
              {resumo.veredito || moment.label}
            </div>
            <div className="mt-3 text-xs text-[#7d8590]">
              {t("driverDetail.summary.seasonRead")}
            </div>
          </div>

          <div className="grid gap-3">
            <div className="rounded-xl border border-white/6 bg-black/10 p-3">
              <DetailRow
                label={t("driverDetail.summary.championship")}
                value={resumo.posicao_campeonato ? `P${resumo.posicao_campeonato}` : "-"}
              />
              <DetailRow label={t("driverDetail.moment.formStatus")} value={moment.label} valueClassName={moment.color} />
              <DetailRow label={t("driverDetail.summary.recentAverage")} value={formatAverage(resumo.media_recente)} />
              <DetailRow label={t("driverDetail.moment.trend")} value={resumo.tendencia || form.tendencia || "->"} />
            </div>
            <div className="grid grid-cols-2 gap-3 xl:grid-cols-4">
              <StatCard label={t("driverDetail.summary.wins")} value={resumo.vitorias ?? stats.vitorias} />
              <StatCard label={t("driverDetail.summary.podiums")} value={resumo.podios ?? stats.podios} />
              <StatCard label={t("driverDetail.summary.top10")} value={resumo.top_10 ?? stats.top_10} />
              <StatCard label={t("driverDetail.summary.points")} value={resumo.pontos ?? detail.stats_temporada?.pontos} />
            </div>
          </div>
        </div>
      </SectionComponent>

      <SectionComponent title={t("driverDetail.summary.recentFormTitle")}>
        <RecentFormChart entries={form.ultimas_10 ?? form.ultimas_5 ?? []} rookie={rookie} context={form.contexto} />
      </SectionComponent>
    </>
  );
}

export function QualitySection({ SectionComponent, detail }) {
  const { t } = useTranslation();
  const technicalReadings = detail.leitura_tecnica?.itens ?? [];

  return (
    <SectionComponent title={t("driverDetail.quality.title")}>
      <div className="grid gap-4 lg:grid-cols-[1fr_1fr]">
        <div className="glass-light rounded-xl p-4">
          <div className="mb-3 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
            {t("driverDetail.quality.technicalRead")}
          </div>
          <div className="grid gap-3">
            {technicalReadings.length ? (
              technicalReadings.map((item) => (
                <QualityLevelRow key={item.chave || item.label} item={item} />
              ))
            ) : (
              <p className="text-xs text-[#7d8590]">{t("driverDetail.quality.noTechnicalRead")}</p>
            )}
          </div>
        </div>
        <div className="glass-light rounded-xl p-4">
          <div className="mb-3 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
            {t("driverDetail.quality.driverBase")}
          </div>
          <div className="grid grid-cols-2 gap-3">
            <StatCard label={t("driverDetail.quality.careerRaces")} value={detail.stats_carreira?.corridas} />
            <StatCard label={t("driverDetail.quality.careerWins")} value={detail.stats_carreira?.vitorias} />
            <StatCard label={t("driverDetail.quality.careerPodiums")} value={detail.stats_carreira?.podios} />
            <StatCard label={t("driverDetail.quality.titles")} value={detail.trajetoria?.titulos ?? 0} />
          </div>
        </div>
      </div>
    </SectionComponent>
  );
}

export function PerformanceReadSection({ SectionComponent, detail }) {
  const { t } = useTranslation();
  if (isCareerDebutantDetail(detail)) return <RookieUnavailableSection SectionComponent={SectionComponent} title={t("driverDetail.performance.title")} />;

  const read = detail.leitura_desempenho ?? {};
  const delta = read.delta_posicao;
  const deltaLabel = delta === null || delta === undefined ? "-" : delta > 0 ? `+${delta}` : `${delta}`;

  return (
    <SectionComponent title={t("driverDetail.performance.title")}>
      <div className="grid gap-4 lg:grid-cols-[0.9fr_1.1fr]">
        <div className="glass-light rounded-xl p-4">
          <div className="mb-3 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
            {t("driverDetail.performance.vsExpected")}
          </div>
          <div className="grid gap-2">
            <DetailRow label={t("driverDetail.performance.delivered")} value={read.entregue_posicao ? `P${read.entregue_posicao}` : "-"} />
            <DetailRow label={t("driverDetail.performance.expectedByPackage")} value={read.esperado_posicao ? `P${read.esperado_posicao}` : "-"} />
            <DetailRow label={t("driverDetail.performance.difference")} value={deltaLabel} valueClassName={delta >= 0 ? "text-[#3fb950]" : "text-[#f85149]"} />
            <DetailRow label={t("driverDetail.performance.carTeam")} value={formatAverage(read.car_performance)} />
          </div>
        </div>

        <div className="glass-light rounded-xl p-4">
          <div className="mb-3 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
            {t("driverDetail.performance.internalCompare")}
          </div>
          <div className="grid gap-3">
            <ProgressRow label={detail.nome} value={read.piloto_pontos ?? 0} max={Math.max(read.piloto_pontos ?? 0, read.companheiro_pontos ?? 0, 1)} right={read.piloto_pontos ?? 0} color="#58a6ff" />
            <ProgressRow label={read.companheiro_nome || t("driverDetail.performance.teammate")} value={read.companheiro_pontos ?? 0} max={Math.max(read.piloto_pontos ?? 0, read.companheiro_pontos ?? 0, 1)} right={read.companheiro_pontos ?? "-"} color="#d29922" />
            <div className="rounded-xl border border-white/6 bg-black/10 p-3 text-sm text-[#c9d1d9]">
              {read.leitura || t("driverDetail.performance.noComparisonContext")}
            </div>
          </div>
        </div>
      </div>
    </SectionComponent>
  );
}

export function HistorySection({ SectionComponent, detail, trajetoria }) {
  const { t } = useTranslation();
  if (isCareerDebutantDetail(detail)) return <RookieUnavailableSection SectionComponent={SectionComponent} title={t("driverDetail.history.title")} />;

  const ranks = detail.rankings_carreira ?? {};

  return (
    <>
      <SectionComponent title={t("driverDetail.history.title")}>
        <div className="grid gap-4">
          <div className="grid grid-cols-2 gap-3 xl:grid-cols-4">
          <CareerRankStat label={t("driverDetail.history.races")} value={detail.stats_carreira?.corridas ?? 0} rank={ranks.corridas} />
          <CareerRankStat label={t("driverDetail.history.wins")} value={detail.stats_carreira?.vitorias ?? 0} rank={ranks.vitorias} />
          <CareerRankStat label={t("driverDetail.history.podiums")} value={detail.stats_carreira?.podios ?? 0} rank={ranks.podios} />
          <CareerRankStat label={t("driverDetail.history.titles")} value={trajetoria?.titulos ?? 0} rank={ranks.titulos} tone="text-[#d29922]" />
          </div>
          <CareerHistoryDossier history={trajetoria?.historico} />
        </div>
      </SectionComponent>

      <SectionComponent title={t("driverDetail.history.trajectory")}>
        <div className="grid gap-4 lg:grid-cols-[0.9fr_1.1fr]">
          <div className="glass-light rounded-xl p-4">
            <div className="grid gap-2">
              <DetailRow label={t("driverDetail.history.debutYear")} value={trajetoria?.ano_estreia ?? "-"} />
              <DebutTeamLine teamName={trajetoria?.equipe_estreia} />
              <DetailRow label={t("driverDetail.history.status")} value={trajetoria?.foi_campeao ? t("driverDetail.history.champion") : t("driverDetail.history.noTitle")} />
            </div>
          </div>
          <div className="glass-light rounded-xl p-4">
            <div className="mb-3 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
              {t("driverDetail.history.timeline")}
            </div>
            <CategoryTimeline items={trajetoria?.categorias_timeline} />
          </div>
        </div>
      </SectionComponent>
    </>
  );
}

export function RivalsSection({ SectionComponent, detail }) {
  const { t } = useTranslation();
  if (isCareerDebutantDetail(detail)) return <RookieUnavailableSection SectionComponent={SectionComponent} title={t("driverDetail.rivals.title")} />;

  const rivals = detail.rivais?.itens ?? [];
  const primary = rivals[0] ?? null;

  return (
    <SectionComponent title={t("driverDetail.rivals.title")}>
      <div className="grid gap-4 lg:grid-cols-[1fr_1fr]">
        <div className="grid gap-2">
          {rivals.length ? (
            rivals.map((rival) => (
              <div key={rival.driver_id} className="rounded-xl border border-white/6 bg-black/10 p-3">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <div className="text-sm font-semibold text-[#e6edf3]">{rival.nome}</div>
                    <div className="mt-1 text-[11px] uppercase tracking-[0.12em] text-[#7d8590]">
                      {rival.tipo}
                    </div>
                  </div>
                  <div className="font-mono text-lg font-bold text-[#f85149]">{rival.intensidade}</div>
                </div>
              </div>
            ))
          ) : (
            <div className="rounded-xl border border-white/6 bg-black/10 p-4 text-sm text-[#7d8590]">
              {t("driverDetail.rivals.noRivals")}
            </div>
          )}
        </div>

        <div className="glass-light rounded-xl p-4">
          <div className="mb-3 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
            {t("driverDetail.rivals.mainRival")}
          </div>
          {primary ? (
            <div className="grid gap-3">
              <DetailRow label={t("driverDetail.rivals.name")} value={primary.nome} />
              <DetailRow label={t("driverDetail.rivals.type")} value={primary.tipo} />
              <ProgressRow label={t("driverDetail.rivals.historyLabel")} value={primary.intensidade_historica} />
              <ProgressRow label={t("driverDetail.rivals.recent")} value={primary.atividade_recente} color="#f85149" />
            </div>
          ) : (
            <p className="text-sm text-[#7d8590]">{t("driverDetail.rivals.noMainRival")}</p>
          )}
        </div>
      </div>
    </SectionComponent>
  );
}

const toneHex = {
  danger: "#f85149",
  warning: "#d29922",
  neutral: "#8b949e",
  info: "#58a6ff",
  success: "#3fb950",
  elite: "#bc8cff",
};

function StardomMeter({ label, value, nivel, tone }) {
  const color = toneHex[tone] || toneHex.neutral;
  const width = Math.max(0, Math.min(Number(value) || 0, 100));

  return (
    <div className="grid gap-1.5">
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-xs font-semibold uppercase tracking-[0.16em] text-[#7d8590]">
          {label}
        </span>
        <span className="text-sm font-bold" style={{ color }}>
          {nivel}
        </span>
      </div>
      <div className="h-2 overflow-hidden rounded-full bg-[#21262d]">
        <div className="h-full rounded-full" style={{ width: `${width}%`, backgroundColor: color }} />
      </div>
      <div className="text-right font-mono text-[11px] text-[#7d8590]">{width}/100</div>
    </div>
  );
}

export function StardomSection({ SectionComponent, detail }) {
  const { t } = useTranslation();
  const stardom = detail.estrelato;
  if (!stardom) return null;

  return (
    <SectionComponent title={t("driverDetail.stardom.title")}>
      <div className="glass-light rounded-xl p-4">
        <div className="grid gap-4 sm:grid-cols-2">
          <StardomMeter label={t("driverDetail.stardom.fame")} value={stardom.fama} nivel={stardom.nivel_fama} tone={stardom.tom_fama} />
          <StardomMeter
            label={t("driverDetail.stardom.charisma")}
            value={stardom.carisma}
            nivel={stardom.nivel_carisma}
            tone={stardom.tom_carisma}
          />
        </div>
        {stardom.resumo ? (
          <div className="mt-4 rounded-xl border border-white/6 bg-black/10 p-3 text-sm text-[#c9d1d9]">
            {stardom.resumo}
          </div>
        ) : null}
        <div className="mt-2 text-[11px] leading-relaxed text-[#7d8590]">
          {t("driverDetail.stardom.blurbPre")}<span className="text-[#e6edf3]">{t("driverDetail.stardom.blurbFame")}</span>{t("driverDetail.stardom.blurbMid")}{" "}
          <span className="text-[#e6edf3]">{t("driverDetail.stardom.blurbCharisma")}</span>{t("driverDetail.stardom.blurbPost")}
        </div>
      </div>
    </SectionComponent>
  );
}

// ── Dossiê de Habilidade do JOGADOR (atributos inferidos do desempenho real) ──

const PLAYER_SKILL_LABELS = {
  skill: "driverDetail.attributes.skill",
  ritmo_classificacao: "driverDetail.attributes.ritmo_classificacao",
  racecraft: "driverDetail.attributes.racecraft",
  consistencia: "driverDetail.attributes.consistencia",
  habilidade_largada: "driverDetail.attributes.habilidade_largada",
  aggression: "driverDetail.attributes.aggression",
  fator_chuva: "driverDetail.playerSkill.labels.fator_chuva",
  adaptabilidade: "driverDetail.attributes.adaptabilidade",
  experiencia: "driverDetail.playerSkill.labels.experiencia",
  midia: "driverDetail.playerSkill.labels.midia",
};

function playerSkillToneHex(value) {
  const v = Number(value) || 0;
  if (v >= 85) return "#bc8cff"; // elite
  if (v >= 75) return "#3fb950"; // qualidade
  if (v >= 40) return "#58a6ff"; // médio
  if (v >= 26) return "#d29922"; // fraco
  return "#f85149"; // defeito
}

function playerSkillUnlockMessage(attr) {
  const n = attr.remaining;
  if (attr.unlock_kind === "wet_races") {
    return i18n.t("driverDetail.playerSkill.unlock.wetRaces", { count: n });
  }
  if (attr.unlock_kind === "seasons") {
    return i18n.t("driverDetail.playerSkill.unlock.seasons", { count: n });
  }
  if (attr.unlock_kind === "telemetry_races") {
    return i18n.t("driverDetail.playerSkill.unlock.telemetryRaces", { count: n });
  }
  return i18n.t("driverDetail.playerSkill.unlock.races", { count: n });
}

function PlayerSkillLockedRow({ attr }) {
  const { t } = useTranslation();
  const label = t(PLAYER_SKILL_LABELS[attr.key] || attr.key);
  const progress = attr.unlock_threshold > 0
    ? Math.min(100, (attr.sample_count / attr.unlock_threshold) * 100)
    : 0;

  return (
    <div className="grid gap-1.5 rounded-lg border border-white/6 bg-black/10 px-3 py-2.5">
      <div className="flex items-center justify-between gap-3">
        <span className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.14em] text-[#6e7681]">
          <span aria-hidden="true">🔒</span>
          {label}
        </span>
        <span className="text-[10px] font-mono text-[#6e7681]">
          {attr.sample_count}/{attr.unlock_threshold}
        </span>
      </div>
      <div className="h-1.5 overflow-hidden rounded-full bg-[#161b22]">
        <div className="h-full rounded-full bg-[#30363d]" style={{ width: `${progress}%` }} />
      </div>
      <span className="text-[11px] text-[#7d8590]">{playerSkillUnlockMessage(attr)}</span>
    </div>
  );
}

function PlayerSkillRow({ attr }) {
  const { t } = useTranslation();
  if (!attr.unlocked) return <PlayerSkillLockedRow attr={attr} />;

  const label = t(PLAYER_SKILL_LABELS[attr.key] || attr.key);
  const value = Number(attr.value) || 0;
  const color = playerSkillToneHex(value);
  const firming = attr.confidence < 0.5;

  return (
    <div className="grid gap-1.5">
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-xs font-semibold uppercase tracking-[0.14em] text-[#c9d1d9]">
          {label}
        </span>
        <span className="flex items-baseline gap-2">
          {attr.tag ? (
            <span className="text-sm font-bold" style={{ color }}>
              {attr.tag}
            </span>
          ) : null}
          <span className="font-mono text-xs text-[#7d8590]">{value}</span>
        </span>
      </div>
      <div className="h-2 overflow-hidden rounded-full bg-[#21262d]">
        <div
          className="h-full rounded-full transition-all duration-500"
          style={{ width: `${value}%`, backgroundColor: color, opacity: firming ? 0.55 : 1 }}
        />
      </div>
      {firming ? (
        <span className="text-[10px] italic text-[#6e7681]">
          {t("driverDetail.playerSkill.firming")}
        </span>
      ) : null}
    </div>
  );
}

export function PlayerSkillSection({ SectionComponent, careerId }) {
  const { t } = useTranslation();
  const [dossier, setDossier] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    let active = true;

    async function fetchDossier() {
      if (!careerId) {
        if (active) {
          setLoading(false);
          setDossier(null);
        }
        return;
      }
      setLoading(true);
      setError("");
      try {
        const data = await invoke("get_player_dossier", { careerId });
        if (active) setDossier(data);
      } catch (fetchError) {
        if (active) {
          setError(
            typeof fetchError === "string"
              ? fetchError
              : fetchError?.toString?.() ?? t("driverDetail.playerSkill.loadError"),
          );
        }
      } finally {
        if (active) setLoading(false);
      }
    }

    fetchDossier();
    return () => {
      active = false;
    };
  }, [careerId]);

  return (
    <SectionComponent title={t("driverDetail.playerSkill.title")}>
      <div className="grid gap-4">
        <div className="rounded-xl border border-[#58a6ff]/18 bg-[#58a6ff]/[0.06] p-3 text-[11px] leading-relaxed text-[#8b949e]">
          {t("driverDetail.playerSkill.introPre")}<span className="text-[#e6edf3]">{t("driverDetail.playerSkill.introPerf")}</span>{t("driverDetail.playerSkill.introMid")}{" "}
          <span className="text-[#e6edf3]">{t("driverDetail.playerSkill.introNot")}</span>{t("driverDetail.playerSkill.introEnd")}
        </div>

        {loading ? (
          <div className="rounded-xl border border-white/6 bg-black/10 p-4 text-sm text-[#7d8590]">
            {t("driverDetail.playerSkill.loading")}
          </div>
        ) : error ? (
          <div className="rounded-xl border border-[#f85149]/25 bg-[#f85149]/10 p-4 text-sm text-[#f85149]">
            {error}
          </div>
        ) : dossier ? (
          <>
            <div className="glass-light rounded-xl p-4">
              <div className="grid gap-4">
                {dossier.attributes.map((attr) => (
                  <PlayerSkillRow key={attr.key} attr={attr} />
                ))}
              </div>
            </div>
            <div className="text-[11px] text-[#6e7681]">
              {t("driverDetail.playerSkill.racesCount", { count: dossier.total_races })} ·{" "}
              {t("driverDetail.playerSkill.seasonsBase", { count: dossier.total_seasons })}
            </div>
          </>
        ) : (
          <div className="rounded-xl border border-white/6 bg-black/10 p-4 text-sm text-[#7d8590]">
            {t("driverDetail.playerSkill.noHistory")}
          </div>
        )}
      </div>
    </SectionComponent>
  );
}

export function MarketSection({ SectionComponent, detail, market }) {
  const { t } = useTranslation();
  const contract = detail.contrato_mercado?.contrato;
  const teamColor = detail.equipe_cor_primaria || detail.perfil?.equipe_cor_primaria || "#58a6ff";

  return (
    <>
      <StardomSection SectionComponent={SectionComponent} detail={detail} />
      <SectionComponent title={t("driverDetail.market.title")}>
        <div className="grid gap-4">
          {contract ? (
            <div className="glass-light rounded-xl p-4">
              <div className="mb-3 text-sm font-semibold" style={{ color: teamColor }}>
                {contract.equipe_nome}
              </div>
              <div className="grid gap-x-4 gap-y-2 text-sm sm:grid-cols-2">
                <DetailRow label={t("driverDetail.market.role")} value={formatContractRole(contract.papel)} />
                <DetailRow label={t("driverDetail.moment.salary")} value={formatSalaryMonthly(contract.salario_anual)} />
                <DetailRow label={t("driverDetail.moment.term")} value={formatContractPeriod(contract)} />
                <DetailRow
                  label={t("driverDetail.market.remaining")}
                  value={t("driverDetail.moment.expiresValue", { count: contract.anos_restantes })}
                />
              </div>
            </div>
          ) : (
            <div className="glass-light rounded-xl p-4 text-sm text-[#7d8590]">
              {t("driverDetail.moment.noContract")}
            </div>
          )}

          {market ? (
            <div className="glass-light rounded-xl p-4">
              <div className="mb-2 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
                {t("driverDetail.market.market")}
              </div>
              <div className="grid gap-2 text-sm text-[#e6edf3] sm:grid-cols-3">
                <div>{t("driverDetail.market.marketValue", { value: formatSalary(market.valor_mercado) })}</div>
                <div>{t("driverDetail.market.salaryRange", { value: formatSalaryMonthly(market.salario_estimado) })}</div>
                <div>{t("driverDetail.market.transferChance", { value: market.chance_transferencia ?? "-" })}</div>
              </div>
            </div>
          ) : (
            <div className="glass-light rounded-xl p-4 text-sm text-[#7d8590]">
              {t("driverDetail.market.noMarketSignals")}
            </div>
          )}
        </div>
      </SectionComponent>
      <QualitySection SectionComponent={SectionComponent} detail={detail} />
      <PerformanceReadSection SectionComponent={SectionComponent} detail={detail} />
    </>
  );
}
