import { useTranslation } from "react-i18next";

import i18n from "../../i18n/index.js";
import { formatSalary, formatSalaryAnnual } from "../../utils/formatters";
import { CareerHistoryDossier, CategoryTimeline, DebutTeamLine } from "./detalhes/HistoricoCarreira.jsx";
import { InactivePreviousSeasonState } from "./detalhes/estados.jsx";
import { PlayerSkillSection } from "./detalhes/PlayerSkillSection.jsx";
import { StardomSection } from "./detalhes/StardomSection.jsx";
import {
  formatAverage,
  formatContractRole,
  formatRank,
  formatStatValue,
} from "./detalhes/formatadores.js";
import {
  DetailRow,
  ProgressRow,
  StatCard,
  summaryToneClass,
  technicalToneClass,
} from "./detalhes/primitivos.jsx";

export { PlayerSkillSection, StardomSection };

function isCareerDebutantDetail(detail) {
  return (detail.stats_carreira?.corridas ?? 0) === 0;
}

function formatContractPeriod(contract) {
  if (!contract) return "-";

  const start = contract.ano_inicio ?? contract.temporada_inicio;
  const end = contract.ano_fim ?? contract.temporada_fim;
  return `${start} - ${end}`;
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
                <DetailRow label={t("driverDetail.moment.salary")} value={formatSalaryAnnual(contract.salario_anual)} />
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
                <div>{t("driverDetail.market.salaryRange", { value: formatSalaryAnnual(market.salario_estimado) })}</div>
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
