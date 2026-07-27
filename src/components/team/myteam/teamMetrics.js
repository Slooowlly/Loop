import i18n from "../../../i18n/index.js";
import { ordinal } from "../../../i18n/format.js";
import { financialState } from "../teamFinanceLabels";
import { extractNationalityLabel, formatMoney, formatSignedMoney } from "../../../utils/formatters";

export const TECH_AXES = [
  { id: "development" },
  { id: "reliability" },
  { id: "pit" },
];

// Linhas REAIS de receita/despesa vindas de `get_team_finance_report` (backend
// `team_finance_history`). A chave casa com o campo do payload; o rótulo/cor são do
// front. Fonte única do dossiê financeiro — nada aqui é fabricado.
export const INCOME_LINES = [
  { key: "sponsorship_income" },
  { key: "gate_income" },
  { key: "result_bonus" },
  { key: "partial_prize_income" },
  { key: "aid_income" },
  { key: "constructor_prize_income" },
];

export const EXPENSE_LINES = [
  { key: "salary_expense", color: "#ff6b6b" },
  { key: "event_operations_cost", color: "#58a6ff" },
  { key: "structural_maintenance_cost", color: "#f59e0b" },
  { key: "technical_investment_cost", color: "#22c55e" },
  { key: "debt_service_cost", color: "#a371f7" },
];

export const RANKING_TIER_COLORS = ["#f85149", "#f0a45a", "#e3c15a", "#7ee787", "#3fb950"];

// Divisão REAL dos custos acumulados da temporada (rosca). Deriva percentuais dos
// totais somados por `get_team_finance_report`; sem custos → lista vazia (estado vazio).
export function costDistribution(season) {
  if (!season) return [];
  const rows = EXPENSE_LINES.map((line) => ({
    key: line.key,
    color: line.color,
    value: Math.max(0, season[line.key] ?? 0),
  })).filter((row) => row.value > 0);
  const total = rows.reduce((sum, row) => sum + row.value, 0);
  if (total <= 0) return [];
  return rows.map((row) => ({ ...row, percent: (row.value / total) * 100 }));
}

export function costGradient(rows) {
  let cursor = 0;
  return rows
    .map((row) => {
      const start = cursor;
      cursor += row.percent;
      return `${row.color} ${start}% ${cursor}%`;
    })
    .join(", ");
}

export function buildExecutiveSignals(team, net, payroll) {
  const cash = team?.cash_balance ?? 0;
  const debt = team?.debt_balance ?? 0;
  const spending = team?.spending_power ?? 0;
  const salaryCeiling = team?.salary_ceiling ?? 0;
  const payrollPressure = salaryCeiling > 0 ? (payroll / salaryCeiling) * 100 : 0;
  const debtPressure = cash > 0 ? debt / cash : debt > 0 ? 1 : 0;

  return [
    {
      label: net >= 0 ? i18n.t("myTeamTab.executive.roundPositive") : i18n.t("myTeamTab.executive.roundNegative"),
      detail: net >= 0
        ? i18n.t("myTeamTab.executive.gainDetail", { value: formatMoney(Math.abs(net)) })
        : i18n.t("myTeamTab.executive.lossDetail", { value: formatMoney(Math.abs(net)) }),
      tone: net >= 0 ? "text-status-green" : "text-status-red",
    },
    {
      label: debtPressure > 0.5 ? i18n.t("myTeamTab.executive.debtHigh") : i18n.t("myTeamTab.executive.debtControlled"),
      detail: i18n.t("myTeamTab.executive.debtDetail", { value: formatMoney(debt) }),
      tone: debtPressure > 0.5 ? "text-status-red" : "text-text-primary",
    },
    {
      label: spending < 0 ? i18n.t("myTeamTab.executive.spendRestricted") : i18n.t("myTeamTab.executive.spendMargin"),
      detail: formatSignedMoney(spending),
      tone: spending < 0 ? "text-status-red" : "text-status-green",
    },
    {
      label: i18n.t("myTeamTab.executive.payroll"),
      detail: i18n.t("myTeamTab.executive.payrollDetail", { value: formatPercent(payrollPressure) }),
      tone: payrollPressure > 90 ? "text-status-red" : "text-text-primary",
    },
  ];
}

export function sortRankingRows(rows, sort) {
  if (sort.key === "default") return rows;
  const direction = sort.direction === "asc" ? 1 : -1;

  return [...rows].sort((a, b) => {
    const result = compareRankingValues(rankingSortValue(a, sort.key), rankingSortValue(b, sort.key));
    if (result !== 0) return result * direction;
    return compareRankingValues(a.posicao ?? 999, b.posicao ?? 999);
  });
}

function rankingSortValue(team, key) {
  if (key === "nome") return team.nome ?? "";
  return team?.[key] ?? 0;
}

function compareRankingValues(a, b) {
  if (typeof a === "string" || typeof b === "string") {
    return String(a).localeCompare(String(b), "pt-BR");
  }
  return Number(a) - Number(b);
}

export function defaultSortDirection(key) {
  return ["cash_balance", "car_level", "confiabilidade", "pit_crew_quality", "pontos"].includes(key) ? "desc" : "asc";
}

// Política interna da garagem (módulo `hierarchy` do backend). `hierarquia_n1_id` é a
// hierarquia REAL — os slots `piloto_1_id`/`piloto_2_id` guardam só a ordem dos assentos e
// podem discordar dela depois de uma INVERSÃO no meio da temporada. Quem manda aqui é a
// hierarquia; os slots só entram como fallback (save antigo, payload sem os campos).
export function resolveHierarchy(team) {
  const n1Id = team?.hierarquia_n1_id ?? team?.piloto_1_id ?? null;
  const n2Id = team?.hierarquia_n2_id ?? team?.piloto_2_id ?? null;
  return {
    n1Id,
    n2Id,
    // Assento de onde cada um veio — é por ele que se casa nome/salário do payload.
    n1Slot: n1Id && n1Id === team?.piloto_2_id ? 2 : 1,
    n2Slot: n2Id && n2Id === team?.piloto_1_id ? 1 : 2,
    // A ordem da garagem discorda da ordem dos assentos: houve inversão.
    inverted: Boolean(n1Id) && n1Id === team?.piloto_2_id,
    hasData: Boolean(team?.hierarquia_n1_id),
  };
}

export const GARAGE_CLIMATES = ["estavel", "competitivo", "tensao", "reavaliacao", "inversao", "crise"];

// Leitura do clima interno para a UI. Os patamares acompanham o backend:
// `TeamHierarchyClimate::from_tensao` troca de estável para competitivo em 20, e
// `finance::morale::advance_team_morale` só começa a punir a moral ACIMA de 50 — é aí
// que a treta deixa de ser ruído e vira consequência.
export function garageClimate(team) {
  const raw = String(team?.hierarquia_status ?? "estavel");
  const status = GARAGE_CLIMATES.includes(raw) ? raw : "estavel";
  const tension = clamp(Number(team?.hierarquia_tensao) || 0, 0, 100);
  const hurtsMorale = tension > 50;
  return {
    status,
    tension,
    hurtsMorale,
    label: i18n.t(`myTeamTab.garage.climate.${status}`),
    tone: hurtsMorale ? "text-status-red" : tension >= 20 ? "text-status-yellow" : "text-status-green",
    barTone: hurtsMorale ? "bg-status-red" : tension >= 20 ? "bg-status-yellow" : "bg-status-green",
    inversions: Math.max(0, Number(team?.hierarquia_inversoes_temporada) || 0),
  };
}

export function buildDriverRow(role, driver, team, playerId, slot = role === "N1" ? 1 : 2) {
  const isN1 = slot === 1;
  const fallbackName = isN1 ? team?.piloto_1_nome : team?.piloto_2_nome;
  const fallbackSalary = isN1 ? team?.piloto_1_salario_anual : team?.piloto_2_salario_anual;
  const fallbackId = isN1 ? team?.piloto_1_id : team?.piloto_2_id;
  // Salário REAL (contrato): do payload da equipe ou do piloto. Assento vazio → 0.
  const salary = fallbackSalary ?? driver?.salario_anual ?? 0;
  return {
    role,
    name: driver?.nome ?? fallbackName ?? "-",
    nationality: driver?.nacionalidade ?? "",
    nationalityLabel: extractNationalityLabel(driver?.nacionalidade) || driver?.nacionalidade || i18n.t("myTeamTab.drivers.noDriverData"),
    salary,
    highlight: driver?.id === playerId || fallbackId === playerId,
  };
}

export function technicalMetrics(team, axis) {
  if (axis === "reliability") {
    return [
      { label: i18n.t("myTeamTab.tech.metrics.reliability"), value: team?.confiabilidade ?? 0, rawValue: Math.round(team?.confiabilidade ?? 0) },
      { label: i18n.t("myTeamTab.tech.metrics.financialPressure"), value: 100 - clamp(team?.budget_index ?? team?.budget ?? 0, 0, 100), rawValue: financialState(team?.financial_state) },
      { label: i18n.t("myTeamTab.tech.metrics.operationalRisk"), value: team?.pit_strategy_risk ?? 0, rawValue: pitRisk(team?.pit_strategy_risk ?? 0) },
    ];
  }
  if (axis === "pit") {
    return [
      { label: i18n.t("myTeamTab.tech.metrics.pitCrewQuality"), value: team?.pit_crew_quality ?? 0, rawValue: pitCrew(team?.pit_crew_quality ?? 0) },
      { label: i18n.t("myTeamTab.tech.metrics.pitStrategyRisk"), value: team?.pit_strategy_risk ?? 0, rawValue: pitRisk(team?.pit_strategy_risk ?? 0) },
      { label: i18n.t("myTeamTab.tech.metrics.overallConsistency"), value: ((team?.pit_crew_quality ?? 0) + (team?.confiabilidade ?? 0)) / 2, rawValue: i18n.t("myTeamTab.tech.raw.pitReliability") },
    ];
  }
  // O Nível do Carro é a ÚNICA leitura de pacote que o jogador vê. A barra de "desempenho na
  // pista" que existia aqui lia o escalar `car_performance` — hoje ele é derivado do MESMO
  // nível, então era uma segunda barra que só repetia a primeira (e, antes da correção do
  // payload, repetia a coluna legada e inventava diferença de carro que o sim não aplica).
  const level = team?.car_level ?? 1;
  return [
    { label: i18n.t("myTeamTab.tech.metrics.carPackage"), value: (level / 10) * 100, rawValue: i18n.t("myTeamTab.tech.raw.carLevel", { level }) },
    { label: i18n.t("myTeamTab.tech.metrics.reliability"), value: team?.confiabilidade ?? 0, rawValue: Math.round(team?.confiabilidade ?? 0) },
    { label: i18n.t("myTeamTab.tech.metrics.pitCrewQuality"), value: team?.pit_crew_quality ?? 0, rawValue: pitCrew(team?.pit_crew_quality ?? 0) },
  ];
}

// Linhas REAIS de um ledger (entradas ou saídas) a partir da última rodada do report.
// Oculta linhas zeradas (ex.: sem auxílio / sem serviço de dívida naquela rodada).
export function ledgerRows(round, lines) {
  if (!round) return [];
  return lines
    .map((line) => ({ key: line.key, value: Math.max(0, round[line.key] ?? 0) }))
    .filter((row) => row.value >= 1);
}

// Gráfico de caixa REAL: caixa ao fim de cada rodada, vindo de `team_finance_history`.
// Rótulo curto por rodada; prefixa a temporada quando a janela cruza temporadas.
export function cashTimelineFromReport(report) {
  const points = report?.cash_timeline ?? [];
  if (!Array.isArray(points) || points.length === 0) return [];
  const values = points.map((point) => point.cash_balance ?? 0);
  const min = Math.min(...values);
  const span = Math.max(1, Math.max(...values) - min);
  // Rótulo sequencial (R1, R2, …) para corridas; a linha de ENCERRAMENTO (prêmio de
  // construtores) ganha um troféu e cor própria — é o ponto onde o resultado do ano
  // fecha. O contador só avança em corridas, então o troféu não "consome" um número.
  let raceIndex = 0;
  return points.map((point) => {
    const value = point.cash_balance ?? 0;
    const isSeasonClose = Boolean(point.is_season_close);
    if (!isSeasonClose) raceIndex += 1;
    return {
      label: isSeasonClose ? "🏆" : `R${raceIndex}`,
      value,
      height: 22 + ((value - min) / span) * 72,
      isSeasonClose,
    };
  });
}

export function carTierIndex(value) {
  return Math.min(4, Math.floor((clamp(Math.round(value ?? 1), 1, 10) - 1) / 2));
}

export function qualityTierIndex(value) {
  const normalized = clamp(Number(value) || 0, 0, 100);
  if (normalized <= 20) return 0;
  if (normalized <= 40) return 1;
  if (normalized <= 60) return 2;
  if (normalized <= 80) return 3;
  return 4;
}

export function formatOrdinal(value) {
  return Number.isFinite(value) ? ordinal(value) : "-";
}

export function moneyTone(value) {
  return value < 0 ? "text-status-red" : "text-text-primary";
}

export function operationalRunway(cash, net) {
  if (net >= 0) {
    return {
      value: i18n.t("myTeamTab.risk.runwayStable"),
      caption: i18n.t("myTeamTab.risk.runwayStableCaption"),
      tone: "text-status-green",
    };
  }

  const rounds = Math.max(0, Math.floor(cash / Math.abs(net)));
  return {
    value: i18n.t("myTeamTab.risk.runwayRounds", { count: rounds }),
    caption: i18n.t("myTeamTab.risk.runwayRoundsCaption"),
    tone: rounds >= 5 ? "text-text-primary" : "text-status-red",
  };
}

export function financialStateTone(state) {
  if (state === "elite" || state === "healthy") {
    return "border-status-green/25 bg-status-green/10 text-status-green";
  }
  if (state === "pressured" || state === "crisis" || state === "collapse") {
    return "border-status-red/25 bg-status-red/10 text-status-red";
  }
  return "border-status-yellow/25 bg-status-yellow/10 text-status-yellow";
}

export function seasonStrategy(strategy) {
  return {
    expansion: i18n.t("myTeamTab.finance.strategies.expansion"),
    balanced: i18n.t("myTeamTab.finance.strategies.balanced"),
    austerity: i18n.t("myTeamTab.finance.strategies.austerity"),
    all_in: i18n.t("myTeamTab.finance.strategies.all_in"),
    survival: i18n.t("myTeamTab.finance.strategies.survival"),
  }[strategy] ?? i18n.t("myTeamTab.finance.strategies.balanced");
}

export function pitRisk(value) {
  if (value <= 20) return i18n.t("myTeamTab.tech.pitRisk.ultraConservative");
  if (value <= 40) return i18n.t("myTeamTab.tech.pitRisk.conservative");
  if (value <= 55) return i18n.t("myTeamTab.tech.pitRisk.balanced");
  if (value <= 75) return i18n.t("myTeamTab.tech.pitRisk.aggressive");
  return i18n.t("myTeamTab.tech.pitRisk.opportunist");
}

export function pitCrew(value) {
  if (value <= 20) return i18n.t("myTeamTab.tech.pitCrew.veryWeak");
  if (value <= 40) return i18n.t("myTeamTab.tech.pitCrew.weak");
  if (value <= 60) return i18n.t("myTeamTab.tech.pitCrew.ok");
  if (value <= 80) return i18n.t("myTeamTab.tech.pitCrew.strong");
  return i18n.t("myTeamTab.tech.pitCrew.elite");
}

export function formatPercent(value) {
  return `${Math.round(value ?? 0)}%`;
}

export function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}
