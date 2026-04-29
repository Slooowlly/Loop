import { formatSalary } from "../../utils/formatters";
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
  return ` (${value}\u00ba)`;
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
    forte: { label: "Em alta", color: "text-[#3fb950]" },
    estavel: { label: "Estavel", color: "text-[#d29922]" },
    em_baixa: { label: "Em baixa", color: "text-[#f85149]" },
    sem_dados: { label: "Sem dados", color: "text-[#7d8590]" },
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
            ESTREANTE
          </div>
          <div className="mt-2 text-3xl font-bold text-[#e6edf3]">0</div>
          <div className="text-[10px] uppercase tracking-[0.18em] text-[#8b949e]">corridas</div>
        </div>
        <div>
          <div className="text-lg font-semibold text-[#e6edf3]">Sem histórico de forma</div>
          <div className="mt-1 text-sm text-[#8b949e]">A leitura começa depois da primeira largada.</div>
        </div>
      </div>
    </div>
  );
}

function InsufficientFormState() {
  return (
    <div className="rounded-xl border border-white/6 bg-black/10 p-4">
      <div className="text-sm font-semibold text-[#c9d1d9]">Dados insuficientes</div>
      <div className="mt-1 text-xs text-[#7d8590]">Ainda falta volume recente para desenhar a tendência.</div>
    </div>
  );
}

function InactivePreviousSeasonState({ context }) {
  const isWithoutTeam = context === "sem_time_temporada_passada";
  const title = isWithoutTeam ? "Sem time na temporada passada" : "Sem corridas na temporada passada";
  const body = isWithoutTeam
    ? "O piloto ficou fora do grid no último ano, então não há forma recente para comparar."
    : "O piloto não disputou provas no último ano, então a forma recente ficou suspensa.";

  return (
    <div className="relative overflow-hidden rounded-xl border border-[#d29922]/24 bg-[#d29922]/9 p-4">
      <div className="absolute inset-x-4 top-4 h-px bg-[#d29922]/30" />
      <div className="relative flex min-h-[156px] flex-col items-center justify-center text-center">
        <div className="text-[10px] font-bold uppercase tracking-[0.22em] text-[#d29922]">
          Fora do grid
        </div>
        <div className="mt-3 text-2xl font-bold text-[#e6edf3]">{title}</div>
        <div className="mt-2 max-w-md text-sm text-[#8b949e]">{body}</div>
      </div>
    </div>
  );
}

function RookieDossierState({ SectionComponent, title = "Resumo Atual" }) {
  return (
    <SectionComponent title={title}>
      <div className="flex min-h-[180px] flex-col items-center justify-center text-center">
        <div className="text-[10px] font-bold uppercase tracking-[0.24em] text-[#58a6ff]">
          Novo no grid
        </div>
        <div className="mt-3 text-4xl font-bold text-[#e6edf3]">Estreante</div>
        <div className="mt-3 max-w-sm text-sm font-semibold text-[#c9d1d9]">
          Expectativa desconhecida
        </div>
        <div className="mt-1 max-w-sm text-sm text-[#8b949e]">Sem passado competitivo para comparar.</div>
      </div>
    </SectionComponent>
  );
}

function RookieUnavailableSection({ SectionComponent, title }) {
  return (
    <SectionComponent title={title}>
      <div className="flex min-h-[180px] flex-col items-center justify-center text-center">
        <div className="text-[10px] font-bold uppercase tracking-[0.2em] text-[#58a6ff]">
          Indisponível para estreante
        </div>
        <div className="mt-3 text-3xl font-bold text-[#e6edf3]">Estreante</div>
        <div className="mt-2 max-w-sm text-sm text-[#8b949e]">
          Sem passado competitivo para sustentar esta leitura.
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
          <div className="text-sm font-semibold text-[#e6edf3]">Tendência recente</div>
          <div className="mt-0.5 text-[11px] text-[#7d8590]">Últimas {entries.length} corridas</div>
        </div>
        <div className="rounded-full border border-[#58a6ff]/20 bg-[#58a6ff]/10 px-2.5 py-1 text-[10px] font-bold uppercase tracking-[0.16em] text-[#58a6ff]">
          Posição
        </div>
      </div>

      <div className="pb-4 pt-1">
        <svg
          viewBox={`0 0 ${width} ${height}`}
          role="img"
          aria-label="Gráfico de forma recente"
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
          <text x={chartLeft} y="17" className="fill-[#3fb950] text-[10px] font-bold">melhor</text>
          <text x={chartLeft} y="174" className="fill-[#7d8590] text-[10px] font-bold">pior</text>
        </svg>

        <div className="grid grid-cols-3 gap-2 px-4">
          <FormMetric label="Melhor" value={bestFinish ? `P${bestFinish}` : "-"} tone="text-[#3fb950]" />
          <FormMetric label="Média" value={averageFinish ? `P${averageFinish.toFixed(1)}` : "-"} />
          <FormMetric label="DNFs" value={dnfCount} tone={dnfCount ? "text-[#f85149]" : "text-[#8b949e]"} />
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
  const timeline = Array.isArray(items) ? items.filter((item) => item?.categoria) : [];

  if (!timeline.length) {
    return <p className="text-xs text-[#7d8590]">Sem categorias visiveis por enquanto.</p>;
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
  return (
    <div className="flex items-center justify-between gap-4 border-b border-white/6 py-2 last:border-b-0 last:pb-0">
      <span className="text-[11px] uppercase tracking-[0.16em] text-[#7d8590]">
        Equipe de estreia
      </span>
      {teamName ? (
        <span className="flex min-w-0 items-center justify-end gap-2 text-right text-sm font-medium text-[#e6edf3]">
          <TeamLogoMark teamName={teamName} size="xs" testId="driver-debut-team-logo" />
          <span className="truncate">{teamName}</span>
        </span>
      ) : (
        <span className="text-right text-sm font-medium text-[#e6edf3]">Não identificada</span>
      )}
    </div>
  );
}

function formatCategoryLabel(categoryId) {
  const map = {
    mazda_rookie: "Mazda Rookie",
    toyota_rookie: "Toyota Rookie",
    mazda_amador: "Mazda Championship",
    toyota_amador: "Toyota Cup",
    bmw_m2: "BMW M2",
    production_challenger: "Production",
    gt4: "GT4",
    gt3: "GT3",
    endurance: "Endurance",
  };

  return map[categoryId] || categoryId || "-";
}

function formatRaceMilestone(value) {
  if (value === null || value === undefined) return "Nunca";
  return `${value}ª corrida`;
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
  const label = `${years} ano${years === 1 ? "" : "s"}`;

  if (periods.length === 0) return label;
  return `${label} (${periods.join(" | ")})`;
}

function formatCareerYears(value) {
  const years = value ?? 0;
  return `${years} ano${years === 1 ? "" : "s"}`;
}

function formatYearsAverage(value) {
  if (value === null || value === undefined) return "-";
  const formatted = Number(value).toFixed(1);
  return `${formatted} ano${formatted === "1.0" ? "" : "s"}`;
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
  const timeline = Array.isArray(items) ? items : [];

  if (!timeline.length) {
    return <p className="mt-2 text-xs text-[#7d8590]">Sem eventos especiais registrados.</p>;
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
  if (!history) return null;

  const presence = history.presenca ?? {};
  const firstMarks = history.primeiros_marcos ?? {};
  const peak = history.auge ?? {};
  const mobility = history.mobilidade ?? {};
  const specialEvents = history.eventos_especiais ?? {};
  const specialRanks = specialEvents.rankings ?? {};
  const bestSeason = peak.melhor_temporada;

  return (
    <div className="glass-light rounded-xl p-4" data-testid="career-history-dossier">
      <div className="grid gap-3 md:grid-cols-2">
        <CareerHistoryGroup
          title="PRESENÇA"
          first
          rows={[
            { label: "Tempo de carreira", value: formatCareerYears(presence.tempo_carreira) },
            { label: "Temporadas disputadas", value: presence.temporadas_disputadas ?? 0 },
            { label: "Anos desempregado", value: formatUnemploymentYears(presence) },
            { label: "Categorias disputadas", value: presence.categorias_disputadas ?? 0 },
          ]}
        />
        <CareerHistoryGroup
          title="PRIMEIROS MARCOS"
          first
          rows={[
            { label: "Primeiro pódio", value: formatRaceMilestone(firstMarks.primeiro_podio_corrida) },
            { label: "Primeira vitória", value: formatRaceMilestone(firstMarks.primeira_vitoria_corrida) },
            { label: "Primeiro DNF", value: formatRaceMilestone(firstMarks.primeiro_dnf_corrida) },
          ]}
        />
        <CareerHistoryGroup
          title="AUGE"
          rows={[
            { label: "Melhor temporada", value: formatBestSeason(bestSeason) },
            { label: "Melhor campeonato", value: bestSeason?.posicao_campeonato ? `P${bestSeason.posicao_campeonato}` : "-" },
            { label: "Maior sequência de vitórias", value: peak.maior_sequencia_vitorias ?? 0 },
          ]}
        />
        <CareerHistoryGroup
          title="MOBILIDADE"
          rows={[
            { label: "Promoções", value: mobility.promocoes ?? 0 },
            { label: "Rebaixamentos", value: mobility.rebaixamentos ?? 0 },
            { label: "Equipes defendidas", value: mobility.equipes_defendidas ?? 0 },
            { label: "Tempo médio por equipe", value: formatYearsAverage(mobility.tempo_medio_por_equipe) },
          ]}
        />
        <div className="border-t border-white/8 pt-3 md:col-span-2">
          <CareerHistoryGroup
            title="EVENTOS ESPECIAIS"
            first
            rows={[
              {
                label: "Participações",
                value: formatRankedValue(specialEvents.participacoes, specialRanks.participacoes),
              },
              {
                label: "Convocações",
                value: formatRankedValue(specialEvents.convocacoes, specialRanks.convocacoes),
              },
              { label: "Vitórias", value: formatRankedValue(specialEvents.vitorias, specialRanks.vitorias) },
              { label: "Pódios", value: formatRankedValue(specialEvents.podios, specialRanks.podios) },
              { label: "Melhor campanha", value: formatSpecialCampaign(specialEvents.melhor_campanha) },
              { label: "Último evento", value: formatSpecialEventEntry(specialEvents.ultimo_evento) },
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
  const resumo = detail.resumo_atual ?? {};
  const stats = detail.performance?.temporada ?? {};
  const form = detail.forma ?? {};
  const rookie = isCareerDebutantDetail(detail);
  const summaryTone = summaryToneClass[resumo.tom] ?? summaryToneClass.info;

  if (rookie) return <RookieDossierState SectionComponent={SectionComponent} />;

  return (
    <>
      <SectionComponent title="Resumo Atual">
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
              Agora
            </div>
            <div className="mt-4 text-3xl font-bold text-[#e6edf3]">
              {resumo.veredito || moment.label}
            </div>
            <div className="mt-3 text-xs text-[#7d8590]">
              leitura da temporada atual
            </div>
          </div>

          <div className="grid gap-3">
            <div className="rounded-xl border border-white/6 bg-black/10 p-3">
              <DetailRow
                label="Campeonato"
                value={resumo.posicao_campeonato ? `P${resumo.posicao_campeonato}` : "-"}
              />
              <DetailRow label="Status de forma" value={moment.label} valueClassName={moment.color} />
              <DetailRow label="Media recente" value={formatAverage(resumo.media_recente)} />
              <DetailRow label="Tendencia" value={resumo.tendencia || form.tendencia || "->"} />
            </div>
            <div className="grid grid-cols-2 gap-3 xl:grid-cols-4">
              <StatCard label="Vitórias" value={resumo.vitorias ?? stats.vitorias} />
              <StatCard label="Pódios" value={resumo.podios ?? stats.podios} />
              <StatCard label="Top 10" value={resumo.top_10 ?? stats.top_10} />
              <StatCard label="Pontos" value={resumo.pontos ?? detail.stats_temporada?.pontos} />
            </div>
          </div>
        </div>
      </SectionComponent>

      <SectionComponent title="Forma Recente">
        <RecentFormChart entries={form.ultimas_10 ?? form.ultimas_5 ?? []} rookie={rookie} context={form.contexto} />
      </SectionComponent>
    </>
  );
}

export function QualitySection({ SectionComponent, detail }) {
  const technicalReadings = detail.leitura_tecnica?.itens ?? [];

  return (
    <SectionComponent title="Mapa de Qualidade">
      <div className="grid gap-4 lg:grid-cols-[1fr_1fr]">
        <div className="glass-light rounded-xl p-4">
          <div className="mb-3 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
            Leitura técnica
          </div>
          <div className="grid gap-3">
            {technicalReadings.length ? (
              technicalReadings.map((item) => (
                <QualityLevelRow key={item.chave || item.label} item={item} />
              ))
            ) : (
              <p className="text-xs text-[#7d8590]">Sem leitura técnica disponível.</p>
            )}
          </div>
        </div>
        <div className="glass-light rounded-xl p-4">
          <div className="mb-3 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
            Base do piloto
          </div>
          <div className="grid grid-cols-2 gap-3">
            <StatCard label="Corridas carreira" value={detail.stats_carreira?.corridas} />
            <StatCard label="Vitórias carreira" value={detail.stats_carreira?.vitorias} />
            <StatCard label="Pódios carreira" value={detail.stats_carreira?.podios} />
            <StatCard label="Títulos" value={detail.trajetoria?.titulos ?? 0} />
          </div>
        </div>
      </div>
    </SectionComponent>
  );
}

export function PerformanceReadSection({ SectionComponent, detail }) {
  if (isCareerDebutantDetail(detail)) return <RookieUnavailableSection SectionComponent={SectionComponent} title="Leitura de Desempenho" />;

  const read = detail.leitura_desempenho ?? {};
  const delta = read.delta_posicao;
  const deltaLabel = delta === null || delta === undefined ? "-" : delta > 0 ? `+${delta}` : `${delta}`;

  return (
    <SectionComponent title="Leitura de Desempenho">
      <div className="grid gap-4 lg:grid-cols-[0.9fr_1.1fr]">
        <div className="glass-light rounded-xl p-4">
          <div className="mb-3 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
            Contra o esperado
          </div>
          <div className="grid gap-2">
            <DetailRow label="Entregue" value={read.entregue_posicao ? `P${read.entregue_posicao}` : "-"} />
            <DetailRow label="Esperado pelo pacote" value={read.esperado_posicao ? `P${read.esperado_posicao}` : "-"} />
            <DetailRow label="Diferença" value={deltaLabel} valueClassName={delta >= 0 ? "text-[#3fb950]" : "text-[#f85149]"} />
            <DetailRow label="Carro/equipe" value={formatAverage(read.car_performance)} />
          </div>
        </div>

        <div className="glass-light rounded-xl p-4">
          <div className="mb-3 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
            Comparativo interno
          </div>
          <div className="grid gap-3">
            <ProgressRow label={detail.nome} value={read.piloto_pontos ?? 0} max={Math.max(read.piloto_pontos ?? 0, read.companheiro_pontos ?? 0, 1)} right={read.piloto_pontos ?? 0} color="#58a6ff" />
            <ProgressRow label={read.companheiro_nome || "Companheiro"} value={read.companheiro_pontos ?? 0} max={Math.max(read.piloto_pontos ?? 0, read.companheiro_pontos ?? 0, 1)} right={read.companheiro_pontos ?? "-"} color="#d29922" />
            <div className="rounded-xl border border-white/6 bg-black/10 p-3 text-sm text-[#c9d1d9]">
              {read.leitura || "Sem contexto suficiente para comparar o desempenho."}
            </div>
          </div>
        </div>
      </div>
    </SectionComponent>
  );
}

export function HistorySection({ SectionComponent, detail, trajetoria }) {
  if (isCareerDebutantDetail(detail)) return <RookieUnavailableSection SectionComponent={SectionComponent} title="Historico de Carreira" />;

  const ranks = detail.rankings_carreira ?? {};

  return (
    <>
      <SectionComponent title="Histórico de Carreira">
        <div className="grid gap-4">
          <div className="grid grid-cols-2 gap-3 xl:grid-cols-4">
          <CareerRankStat label="Corridas" value={detail.stats_carreira?.corridas ?? 0} rank={ranks.corridas} />
          <CareerRankStat label="Vitórias" value={detail.stats_carreira?.vitorias ?? 0} rank={ranks.vitorias} />
          <CareerRankStat label="Pódios" value={detail.stats_carreira?.podios ?? 0} rank={ranks.podios} />
          <CareerRankStat label="Títulos" value={trajetoria?.titulos ?? 0} rank={ranks.titulos} tone="text-[#d29922]" />
          </div>
          <CareerHistoryDossier history={trajetoria?.historico} />
        </div>
      </SectionComponent>

      <SectionComponent title="Trajetória">
        <div className="grid gap-4 lg:grid-cols-[0.9fr_1.1fr]">
          <div className="glass-light rounded-xl p-4">
            <div className="grid gap-2">
              <DetailRow label="Ano de estreia" value={trajetoria?.ano_estreia ?? "-"} />
              <DebutTeamLine teamName={trajetoria?.equipe_estreia} />
              <DetailRow label="Status" value={trajetoria?.foi_campeao ? "Campeão" : "Sem título"} />
            </div>
          </div>
          <div className="glass-light rounded-xl p-4">
            <div className="mb-3 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
              Linha do tempo
            </div>
            <CategoryTimeline items={trajetoria?.categorias_timeline} />
          </div>
        </div>
      </SectionComponent>
    </>
  );
}

export function RivalsSection({ SectionComponent, detail }) {
  if (isCareerDebutantDetail(detail)) return <RookieUnavailableSection SectionComponent={SectionComponent} title="Rivais" />;

  const rivals = detail.rivais?.itens ?? [];
  const primary = rivals[0] ?? null;

  return (
    <SectionComponent title="Rivais">
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
              Sem rivalidades consolidadas para este piloto.
            </div>
          )}
        </div>

        <div className="glass-light rounded-xl p-4">
          <div className="mb-3 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
            Rival principal
          </div>
          {primary ? (
            <div className="grid gap-3">
              <DetailRow label="Nome" value={primary.nome} />
              <DetailRow label="Tipo" value={primary.tipo} />
              <ProgressRow label="Histórico" value={primary.intensidade_historica} />
              <ProgressRow label="Recente" value={primary.atividade_recente} color="#f85149" />
            </div>
          ) : (
            <p className="text-sm text-[#7d8590]">Ainda nao ha rival principal para comparar.</p>
          )}
        </div>
      </div>
    </SectionComponent>
  );
}

export function MarketSection({ SectionComponent, detail, market }) {
  const contract = detail.contrato_mercado?.contrato;
  const teamColor = detail.equipe_cor_primaria || detail.perfil?.equipe_cor_primaria || "#58a6ff";

  return (
    <>
      <SectionComponent title="Contrato e Mercado">
        <div className="grid gap-4">
          {contract ? (
            <div className="glass-light rounded-xl p-4">
              <div className="mb-3 text-sm font-semibold" style={{ color: teamColor }}>
                {contract.equipe_nome}
              </div>
              <div className="grid gap-x-4 gap-y-2 text-sm sm:grid-cols-2">
                <DetailRow label="Papel" value={formatContractRole(contract.papel)} />
                <DetailRow label="Salário anual" value={formatSalary(contract.salario_anual)} />
                <DetailRow label="Vigencia" value={formatContractPeriod(contract)} />
                <DetailRow
                  label="Restante"
                  value={`${contract.anos_restantes} ano${contract.anos_restantes !== 1 ? "s" : ""}`}
                />
              </div>
            </div>
          ) : (
            <div className="glass-light rounded-xl p-4 text-sm text-[#7d8590]">
              Sem contrato ativo no momento.
            </div>
          )}

          {market ? (
            <div className="glass-light rounded-xl p-4">
              <div className="mb-2 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
                Mercado
              </div>
              <div className="grid gap-2 text-sm text-[#e6edf3] sm:grid-cols-3">
                <div>Valor: {formatSalary(market.valor_mercado)}</div>
                <div>Faixa salarial: {formatSalary(market.salario_estimado)}</div>
                <div>Chance de troca: {market.chance_transferencia ?? "-"}%</div>
              </div>
            </div>
          ) : (
            <div className="glass-light rounded-xl p-4 text-sm text-[#7d8590]">
              Sem sinais fortes de mercado no momento.
            </div>
          )}
        </div>
      </SectionComponent>
      <QualitySection SectionComponent={SectionComponent} detail={detail} />
      <PerformanceReadSection SectionComponent={SectionComponent} detail={detail} />
    </>
  );
}
