import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import GlassCard from "../../components/ui/GlassCard";
import FlagIcon from "../../components/ui/FlagIcon";
import TeamLogoMark from "../../components/team/TeamLogoMark";
import useCareerStore from "../../stores/useCareerStore";
import i18n from "../../i18n/index.js";
import { ordinal } from "../../i18n/format.js";
import {
  categoryLabel,
  extractNationalityLabel,
  formatMoney,
  formatSignedMoney,
  monthlySalary,
} from "../../utils/formatters";

const TECH_AXES = [
  { id: "development" },
  { id: "reliability" },
  { id: "pit" },
];

// Linhas REAIS de receita/despesa vindas de `get_team_finance_report` (backend
// `team_finance_history`). A chave casa com o campo do payload; o rótulo/cor são do
// front. Fonte única do dossiê financeiro — nada aqui é fabricado.
const INCOME_LINES = [
  { key: "sponsorship_income" },
  { key: "gate_income" },
  { key: "result_bonus" },
  { key: "partial_prize_income" },
  { key: "aid_income" },
  { key: "constructor_prize_income" },
];

const EXPENSE_LINES = [
  { key: "salary_expense", color: "#ff6b6b" },
  { key: "event_operations_cost", color: "#58a6ff" },
  { key: "structural_maintenance_cost", color: "#f59e0b" },
  { key: "technical_investment_cost", color: "#22c55e" },
  { key: "debt_service_cost", color: "#a371f7" },
];

const TEAM_HISTORY_TABS = [
  { id: "records" },
  { id: "sport" },
  { id: "identity" },
  { id: "management" },
  { id: "categories" },
];

const RANKING_TIER_COLORS = ["#f85149", "#f0a45a", "#e3c15a", "#7ee787", "#3fb950"];

function MyTeamTab() {
  const careerId = useCareerStore((state) => state.careerId);
  const player = useCareerStore((state) => state.player);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const [drivers, setDrivers] = useState([]);
  const [teams, setTeams] = useState([]);
  const [financeReport, setFinanceReport] = useState(null);
  const [activeAxis, setActiveAxis] = useState("development");
  const [selectedHistoryTeam, setSelectedHistoryTeam] = useState(null);
  const [activeHistoryTab, setActiveHistoryTab] = useState("records");
  const [error, setError] = useState("");

  useEffect(() => {
    let mounted = true;

    async function load() {
      if (!careerId || !playerTeam?.categoria || !playerTeam?.id) return;
      try {
        setError("");
        const [loadedDrivers, loadedTeams, loadedFinance] = await Promise.all([
          invoke("get_drivers_by_category", { careerId, category: playerTeam.categoria }),
          invoke("get_teams_standings", { careerId, category: playerTeam.categoria }),
          invoke("get_team_finance_report", {
            careerId,
            category: playerTeam.categoria,
            teamId: playerTeam.id,
          }),
        ]);
        if (mounted) {
          setDrivers(Array.isArray(loadedDrivers) ? loadedDrivers : []);
          setTeams(Array.isArray(loadedTeams) ? loadedTeams : []);
          setFinanceReport(loadedFinance ?? null);
        }
      } catch (invokeError) {
        if (mounted) {
          setError(typeof invokeError === "string" ? invokeError : i18n.t("myTeamTab.errors.load"));
        }
      }
    }

    load();
    return () => {
      mounted = false;
    };
  }, [careerId, playerTeam?.categoria, playerTeam?.id]);

  const piloto1 = drivers.find((driver) => driver.id === playerTeam?.piloto_1_id);
  const piloto2 = drivers.find((driver) => driver.id === playerTeam?.piloto_2_id);
  const standing = teams.find((team) => team.id === playerTeam?.id);
  const driverRows = [
    buildDriverRow("N1", piloto1, playerTeam, player?.id),
    buildDriverRow("N2", piloto2, playerTeam, player?.id),
  ];

  return (
    <div className="space-y-5">
      <CommandHeader team={playerTeam} standing={standing} />

      {error ? (
        <div className="rounded-2xl border border-status-red/30 bg-status-red/10 px-4 py-3 text-sm text-status-red">
          {error}
        </div>
      ) : null}

      <div className="grid gap-5 xl:grid-cols-[0.72fr_1.28fr]">
        <div className="space-y-5" data-testid="my-team-side-rail">
          <DriverPanel drivers={driverRows} salaryCeiling={playerTeam?.salary_ceiling ?? 0} />
          <TechPanel team={playerTeam} activeAxis={activeAxis} setActiveAxis={setActiveAxis} />
          <CostChart report={financeReport} />
        </div>
        <FinanceDossier team={playerTeam} drivers={driverRows} report={financeReport} />
      </div>

      <RankingTable
        teams={teams}
        playerTeam={playerTeam}
        historyTeamId={selectedHistoryTeam?.id}
        onTeamHistoryOpen={(team) => {
          setSelectedHistoryTeam(team);
          setActiveHistoryTab("records");
        }}
      />

      {selectedHistoryTeam ? (
        <TeamHistoryDrawer
          careerId={careerId}
          team={selectedHistoryTeam}
          teams={teams}
          playerTeam={playerTeam}
          activeCategory={playerTeam?.categoria}
          activeTab={activeHistoryTab}
          onTabChange={setActiveHistoryTab}
          onSelectTeam={setSelectedHistoryTeam}
          onClose={() => setSelectedHistoryTeam(null)}
        />
      ) : null}
    </div>
  );
}

function CommandHeader({ team, standing }) {
  const { t } = useTranslation();
  return (
    <GlassCard hover={false} className="rounded-[30px]" data-testid="my-team-command-header">
      <div className="grid gap-5 lg:grid-cols-[1.18fr_0.82fr] lg:items-center">
        <div className="flex items-center gap-4">
          <TeamLogoMark
            teamName={team?.nome}
            color={team?.cor_primaria}
            size="lg"
            testId="my-team-command-logo"
          />
          <div>
            <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">{t("myTeamTab.command.eyebrow")}</p>
            <h2 className="mt-2 text-3xl font-semibold text-text-primary">{team?.nome ?? t("myTeamTab.team.fallbackName")}</h2>
          </div>
        </div>
        <HeaderFinanceStat team={team} standing={standing} />
      </div>
    </GlassCard>
  );
}

function HeaderFinanceStat({ team, standing }) {
  const { t } = useTranslation();
  const stateTone = financialStateTone(team?.financial_state);
  return (
    <div
      data-testid="header-finance-stat"
      className="justify-self-stretch text-right lg:justify-self-end"
    >
      <div className="flex min-w-0 flex-col items-end">
        <div className="max-w-full">
          <p className={`break-words font-mono text-5xl font-semibold leading-none ${moneyTone(team?.cash_balance ?? 0)}`}>
            {formatMoney(team?.cash_balance ?? 0)}
          </p>
        </div>
        <div className="mt-3 flex flex-wrap items-center justify-end gap-3">
          <span className={`rounded-full border px-3 py-1 text-xs font-semibold ${stateTone}`}>
            {financialState(team?.financial_state)}
          </span>
          <span className="text-[10px] uppercase tracking-[0.16em] text-text-muted">
            {t("myTeamTab.command.position")} <span className="font-mono text-sm font-bold text-status-yellow">{formatOrdinal(standing?.posicao)}</span>
          </span>
        </div>
      </div>
    </div>
  );
}

function DriverPanel({ drivers, salaryCeiling }) {
  const { t } = useTranslation();
  return (
    <GlassCard hover={false} className="rounded-[28px]">
      <p className="text-[10px] uppercase tracking-[0.22em] text-accent-primary">{t("myTeamTab.drivers.eyebrow")}</p>
      <h3 className="mt-2 text-xl font-semibold text-text-primary">{t("myTeamTab.drivers.title")}</h3>
      <div className="mt-5 space-y-3">
        {drivers.map((driver) => (
          <DriverRow key={driver.role} driver={driver} salaryCeiling={salaryCeiling} />
        ))}
      </div>
    </GlassCard>
  );
}

function DriverRow({ driver, salaryCeiling }) {
  const { t } = useTranslation();
  const weight = salaryCeiling > 0 ? (driver.salary / salaryCeiling) * 100 : 0;
  return (
    <div className={`rounded-[22px] border p-4 ${driver.highlight ? "border-accent-primary/35 bg-accent-primary/10" : "border-white/8 bg-white/[0.03]"}`}>
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-[10px] uppercase tracking-[0.2em] text-text-muted">{driver.role}</p>
          <h4 className="mt-1 text-base font-semibold text-text-primary">{driver.name}</h4>
          <p className="mt-2 inline-flex items-center gap-2 rounded-full border border-white/8 bg-black/10 px-2.5 py-1 text-xs text-text-secondary">
            <FlagIcon nacionalidade={driver.nationality} className="shrink-0" />
            <span>{driver.nationalityLabel}</span>
          </p>
        </div>
        <div className="text-right">
          <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">{t("myTeamTab.drivers.salaryRole", { role: driver.role })}</p>
          <p className="mt-1 font-mono text-sm text-status-green">
            {formatMoney(monthlySalary(driver.salary))}
            <span className="ml-1 font-sans text-[10px] font-normal text-text-muted">{t("myTeamTab.drivers.perMonth")}</span>
          </p>
        </div>
      </div>
      <div className="mt-4">
        <div className="mb-2 flex items-center justify-between text-[10px] uppercase tracking-[0.16em] text-text-muted">
          <span>{t("myTeamTab.drivers.payrollWeight")}</span>
          <span>{formatPercent(weight)}</span>
        </div>
        <div className="h-2 rounded-full bg-white/10">
          <div className="h-2 rounded-full bg-gradient-to-r from-accent-primary to-status-green" style={{ width: `${clamp(weight, 4, 100)}%` }} />
        </div>
      </div>
    </div>
  );
}

function TechPanel({ team, activeAxis, setActiveAxis }) {
  const { t } = useTranslation();
  const metrics = technicalMetrics(team, activeAxis);
  return (
    <GlassCard hover={false} className="rounded-[28px]">
      <p className="text-[10px] uppercase tracking-[0.22em] text-accent-primary">{t("myTeamTab.tech.eyebrow")}</p>
      <h3 className="mt-2 text-xl font-semibold text-text-primary">{t("myTeamTab.tech.title")}</h3>
      <div className="mt-5 grid grid-cols-3 gap-2">
        {TECH_AXES.map((axis) => (
          <button
            key={axis.id}
            type="button"
            onClick={() => setActiveAxis(axis.id)}
            className={`rounded-2xl border px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.13em] transition-glass ${
              axis.id === activeAxis
                ? "border-accent-primary/40 bg-accent-primary/15 text-accent-primary"
                : "border-white/8 bg-black/10 text-text-muted hover:text-text-primary"
            }`}
          >
            {t(`myTeamTab.tech.axes.${axis.id}`)}
          </button>
        ))}
      </div>
      <div className="mt-5 rounded-[24px] border border-white/8 bg-black/10 p-4">
        <div className="mt-5 space-y-4">
          {metrics.map((metric) => (
            <MetricBar key={metric.label} {...metric} />
          ))}
        </div>
      </div>
    </GlassCard>
  );
}

function FinanceDossier({ team, drivers, report }) {
  const { t } = useTranslation();
  const [showSecondaryCashIndicators, setShowSecondaryCashIndicators] = useState(false);
  const net = team?.last_round_net ?? 0;
  const timeline = cashTimelineFromReport(report);
  const hasTimeline = timeline.length > 0;
  const payroll = drivers.reduce((sum, driver) => sum + driver.salary, 0);
  const peakCash = hasTimeline ? Math.max(...timeline.map((point) => point.value)) : team?.cash_balance ?? 0;
  const lowCash = hasTimeline ? Math.min(...timeline.map((point) => point.value)) : team?.cash_balance ?? 0;
  const openingCash = (team?.cash_balance ?? 0) - net;
  const projectedCash = team?.cash_balance ?? 0;
  const strategyLabel = seasonStrategy(team?.season_strategy);
  const debt = team?.debt_balance ?? 0;
  const incomeLedger = ledgerRows(report?.latest, INCOME_LINES);
  // A linha "Salários" da rodada é anual ÷ nº de corridas; ancoramos com a folha mensal
  // para que o valor por rodada não pareça furado ao lado do salário mensal dos pilotos.
  const expenseLedger = ledgerRows(report?.latest, EXPENSE_LINES).map((row) =>
    row.key === "salary_expense" && payroll > 0
      ? { ...row, hint: t("myTeamTab.finance.salaryHint", { value: formatMoney(monthlySalary(payroll)) }) }
      : row,
  );
  // Média REAL por rodada: acumulado líquido da temporada ÷ rodadas registradas
  // (report.season.round guarda a CONTAGEM de rodadas somadas). Sem histórico → cai no
  // resultado da última rodada como aproximação.
  const seasonRoundsCount = report?.season?.round ?? 0;
  const avgRoundNet = seasonRoundsCount > 0 ? (report?.season?.net ?? 0) / seasonRoundsCount : net;
  // Projeção "se a temporada terminasse agora": net acumulado da temporada + prêmio de
  // construtores ESTIMADO pela posição atual no campeonato. Só exibição (o backend nunca
  // credita a expectativa no caixa nem nas decisões da IA). Fecha o loop visual do déficit
  // por rodada — o prêmio de fim de ano é o que traz o resultado pro verde.
  // Net da temporada SEM prêmio já pago (a linha de encerramento entra em season.net);
  // descontamos para não contar o prêmio duas vezes ao somar a expectativa.
  const seasonNetToDate = (report?.season?.net ?? 0) - (report?.season?.constructor_prize_income ?? 0);
  const expectedPrize = report?.expected_constructor_prize ?? 0;
  const currentPosition = report?.current_position ?? 0;
  const gridSize = report?.grid_size ?? 0;
  const projectedAnnual = seasonNetToDate + expectedPrize;
  const hasProjection = currentPosition > 0 && expectedPrize > 0;
  return (
    <GlassCard hover={false} className="rounded-[28px]">
      <p className="text-[10px] uppercase tracking-[0.24em] text-accent-primary">{t("myTeamTab.finance.eyebrow")}</p>
      <h3 className="mt-2 text-2xl font-semibold text-text-primary">{t("myTeamTab.finance.title")}</h3>

      <div className="mt-6 grid gap-3 sm:grid-cols-2 xl:grid-cols-5">
        <Kpi label={t("myTeamTab.finance.kpi.cash")} value={formatMoney(team?.cash_balance ?? 0)} caption={t("myTeamTab.finance.kpi.cashCaption")} period={t("myTeamTab.finance.period.current")} />
        <Kpi label={t("myTeamTab.finance.kpi.roundResult")} value={formatSignedMoney(net)} caption={t("myTeamTab.finance.kpi.roundResultCaption")} period={t("myTeamTab.finance.period.perRound")} tone={net >= 0 ? "text-status-green" : "text-status-red"} />
        <Kpi label={t("myTeamTab.finance.kpi.debt")} value={formatMoney(debt)} caption={t("myTeamTab.finance.kpi.debtCaption")} period={t("myTeamTab.finance.period.current")} tone={debt > 0 ? "text-status-red" : "text-text-primary"} />
        <Kpi label={t("myTeamTab.finance.kpi.salaryCeiling")} value={formatMoney(monthlySalary(team?.salary_ceiling ?? 0))} caption={t("myTeamTab.finance.kpi.salaryCeilingCaption")} period={t("myTeamTab.finance.period.perMonth")} />
        <Kpi label={t("myTeamTab.finance.kpi.spendingPower")} value={formatSignedMoney(team?.spending_power ?? 0)} caption={t("myTeamTab.finance.kpi.spendingPowerCaption")} period={t("myTeamTab.finance.period.season")} tone={(team?.spending_power ?? 0) >= 0 ? "text-status-green" : "text-status-red"} />
      </div>

      <p className="mt-3 text-[11px] leading-5 text-text-muted">
        {t("myTeamTab.finance.legend.intro")}<span className="text-text-secondary">{t("myTeamTab.finance.legend.perRound")}</span>{t("myTeamTab.finance.legend.perRoundDesc")}{" "}
        <span className="text-text-secondary">{t("myTeamTab.finance.legend.perMonth")}</span>{t("myTeamTab.finance.legend.perMonthDesc")}<span className="text-text-secondary">{t("myTeamTab.finance.legend.season")}</span>{t("myTeamTab.finance.legend.seasonDesc")}
      </p>

      <div className="mt-5 grid gap-4 lg:grid-cols-2">
        <Ledger title={t("myTeamTab.finance.incomeTitle")} rows={incomeLedger} positive />
        <Ledger title={t("myTeamTab.finance.expenseTitle")} rows={expenseLedger} />
      </div>

      {hasProjection ? (
        <div className="mt-5 rounded-[24px] border border-white/8 bg-white/[0.03] p-5" data-testid="season-projection">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <p className="text-[10px] uppercase tracking-[0.22em] text-text-muted">{t("myTeamTab.projection.eyebrow")}</p>
              <h4 className="mt-2 text-lg font-semibold text-text-primary">{t("myTeamTab.projection.title")}</h4>
            </div>
            <span className="rounded-full border border-status-yellow/25 bg-status-yellow/10 px-3 py-1 text-[10px] uppercase tracking-[0.16em] text-status-yellow">
              {formatOrdinal(currentPosition)}{gridSize > 0 ? t("myTeamTab.projection.ofGrid", { count: gridSize }) : ""}
            </span>
          </div>
          <div className="mt-5 grid gap-3 sm:grid-cols-3">
            <Kpi compact label={t("myTeamTab.projection.seasonToDate")} value={formatSignedMoney(seasonNetToDate)} tone={moneyTone(seasonNetToDate)} />
            <Kpi compact label={t("myTeamTab.projection.estimatedPrize")} value={`+${formatMoney(expectedPrize)}`} caption={t("myTeamTab.projection.estimatedPrizeCaption")} tone="text-status-green" />
            <Kpi compact label={t("myTeamTab.projection.annualProjection")} value={formatSignedMoney(projectedAnnual)} tone={projectedAnnual >= 0 ? "text-status-green" : "text-status-red"} />
          </div>
          <p className={`mt-4 text-sm font-semibold ${projectedAnnual >= 0 ? "text-status-green" : "text-status-red"}`}>
            {projectedAnnual >= 0
              ? t("myTeamTab.projection.verdictGreen", { value: formatSignedMoney(projectedAnnual) })
              : t("myTeamTab.projection.verdictRed", { value: formatSignedMoney(projectedAnnual) })}
          </p>
        </div>
      ) : null}

      <div className="mt-5 rounded-[24px] border border-white/8 bg-black/10 p-5">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-[10px] uppercase tracking-[0.22em] text-text-muted">{t("myTeamTab.cash.eyebrow")}</p>
            <h4 className="mt-2 text-lg font-semibold text-text-primary">{t("myTeamTab.cash.title")}</h4>
          </div>
          <div className="flex flex-wrap gap-2">
            <span className="rounded-full border border-accent-primary/25 bg-accent-primary/10 px-3 py-1 text-[10px] uppercase tracking-[0.16em] text-accent-primary">{t("myTeamTab.cash.realBadge")}</span>
            <span className="rounded-full border border-white/10 bg-white/[0.04] px-3 py-1 text-[10px] uppercase tracking-[0.16em] text-text-secondary">
              {t("myTeamTab.cash.strategy")} <span className="text-text-primary">{strategyLabel}</span>
            </span>
          </div>
        </div>

        <div className="mt-5 grid gap-3 sm:grid-cols-2 xl:grid-cols-5">
          <Kpi compact label={t("myTeamTab.cash.openingCash")} value={formatMoney(openingCash)} tone={moneyTone(openingCash)} />
          <Kpi compact label={t("myTeamTab.cash.income")} value={`+${formatMoney(team?.last_round_income ?? 0)}`} tone="text-status-green" period={t("myTeamTab.finance.period.perRound")} />
          <Kpi compact label={t("myTeamTab.cash.expenses")} value={`-${formatMoney(team?.last_round_expenses ?? 0)}`} tone="text-status-red" period={t("myTeamTab.finance.period.perRound")} />
          <Kpi compact label={t("myTeamTab.cash.debt")} value={formatMoney(debt)} tone={debt > 0 ? "text-status-red" : "text-text-primary"} />
          <Kpi compact label={t("myTeamTab.cash.currentCash")} value={formatMoney(projectedCash)} tone={moneyTone(projectedCash)} />
        </div>

        {hasTimeline ? (
          <div className="mt-6 flex h-56 items-end gap-2 rounded-[22px] border border-white/6 bg-white/[0.02] px-4 pb-4 pt-8">
            {timeline.map((point, index) => (
              <div key={index} className="flex h-full flex-1 flex-col justify-end gap-2">
                <div
                  className={`min-h-3 rounded-t-xl bg-gradient-to-t ${
                    point.isSeasonClose
                      ? "from-status-yellow/70 to-status-yellow"
                      : point.value < 0
                        ? "from-status-red to-status-red"
                        : "from-accent-primary/70 to-accent-hover"
                  }`}
                  data-testid={
                    point.isSeasonClose
                      ? "cash-timeline-season-close"
                      : point.value < 0
                        ? "cash-timeline-negative"
                        : undefined
                  }
                  style={{ height: `${point.height}%` }}
                  title={
                    point.isSeasonClose
                      ? t("myTeamTab.cash.seasonCloseTitle", { value: formatMoney(point.value) })
                      : t("myTeamTab.cash.barTitle", { label: point.label, value: formatMoney(point.value) })
                  }
                />
                <span className="text-center font-mono text-[10px] text-text-muted">{point.label}</span>
              </div>
            ))}
          </div>
        ) : (
          <p className="mt-6 rounded-[22px] border border-white/6 bg-white/[0.02] px-4 py-8 text-center text-xs leading-5 text-text-secondary">
            {t("myTeamTab.cash.empty")}
          </p>
        )}
        <div className="mt-4 rounded-[22px] border border-white/8 bg-white/[0.025] p-3">
          <button
            type="button"
            onClick={() => setShowSecondaryCashIndicators((value) => !value)}
            className="flex w-full items-center justify-between gap-3 rounded-2xl px-2 py-1 text-left text-[10px] font-semibold uppercase tracking-[0.16em] text-text-muted transition-glass hover:text-text-primary"
          >
            <span>
              {showSecondaryCashIndicators
                ? t("myTeamTab.cash.hideSecondary")
                : t("myTeamTab.cash.showSecondary")}
            </span>
            <span className="text-accent-primary">{showSecondaryCashIndicators ? "−" : "+"}</span>
          </button>

          {showSecondaryCashIndicators ? (
            <>
              <FinancialRiskPanel
                cash={team?.cash_balance ?? 0}
                debt={debt}
                income={team?.last_round_income ?? 0}
                net={net}
              />
              <div className="mt-3 grid gap-3 sm:grid-cols-4">
                <Kpi compact label={t("myTeamTab.cash.peakCash")} value={formatMoney(peakCash)} tone={moneyTone(peakCash)} />
                <Kpi compact label={t("myTeamTab.cash.worstStretch")} value={formatMoney(lowCash)} tone={moneyTone(lowCash)} />
                <Kpi compact label={t("myTeamTab.cash.avgPerRound")} value={formatSignedMoney(avgRoundNet)} tone={moneyTone(avgRoundNet)} />
                <Kpi compact label={t("myTeamTab.cash.monthlyPayroll")} value={formatMoney(monthlySalary(payroll))} />
              </div>
            </>
          ) : null}
        </div>
        {team?.parachute_payment_remaining > 0 ? (
          <p className="mt-4 rounded-2xl border border-accent-primary/20 bg-accent-primary/10 px-4 py-3 text-sm text-accent-primary">
            {t("myTeamTab.cash.parachute", { value: formatMoney(team.parachute_payment_remaining) })}
          </p>
        ) : null}
      </div>

      <div className="mt-5">
        <ExecutiveReading team={team} net={net} payroll={payroll} />
      </div>
    </GlassCard>
  );
}

function Kpi({ label, value, caption, tone = "text-text-primary", compact = false, period }) {
  return (
    <div className={`rounded-2xl border border-white/8 bg-white/[0.03] ${compact ? "p-3" : "p-4"}`}>
      <div className="flex items-start justify-between gap-2">
        <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">{label}</p>
        {period ? (
          <span className="shrink-0 rounded-full border border-white/10 bg-black/20 px-1.5 py-0.5 text-[8px] font-semibold uppercase tracking-[0.1em] text-text-muted">
            {period}
          </span>
        ) : null}
      </div>
      <p className={`mt-2 font-mono ${compact ? "text-sm" : "text-lg"} font-semibold ${tone}`}>{value}</p>
      {caption ? <p className="mt-1 text-xs text-text-secondary">{caption}</p> : null}
    </div>
  );
}

function FinancialRiskPanel({ cash, debt, income, net }) {
  const { t } = useTranslation();
  const liquidBalance = cash - debt;
  const margin = income > 0 ? (net / income) * 100 : 0;
  const runway = operationalRunway(cash, net);

  return (
    <div className="mt-5 rounded-[22px] border border-white/8 bg-white/[0.025] p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-[10px] uppercase tracking-[0.2em] text-text-muted">
          {t("myTeamTab.risk.title")}
        </p>
        <span className="rounded-full border border-white/10 bg-black/10 px-3 py-1 text-[10px] uppercase tracking-[0.16em] text-text-secondary">
          {t("myTeamTab.risk.quickRead")}
        </span>
      </div>
      <div className="mt-4 grid gap-3 md:grid-cols-3">
        <RiskCard
          label={t("myTeamTab.risk.liquidBalance")}
          value={formatMoney(liquidBalance)}
          caption={t("myTeamTab.risk.liquidBalanceCaption")}
          tone={moneyTone(liquidBalance)}
        />
        <RiskCard
          label={t("myTeamTab.risk.roundMargin")}
          value={formatPercent(margin)}
          caption={t("myTeamTab.risk.roundMarginCaption")}
          tone={margin >= 0 ? "text-status-green" : "text-status-red"}
        />
        <RiskCard
          label={t("myTeamTab.risk.runway")}
          value={runway.value}
          caption={runway.caption}
          tone={runway.tone}
        />
      </div>
    </div>
  );
}

function RiskCard({ label, value, caption, tone }) {
  return (
    <div className="rounded-2xl border border-white/8 bg-black/10 p-4">
      <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">{label}</p>
      <p className={`mt-2 font-mono text-lg font-semibold ${tone}`}>{value}</p>
      <p className="mt-1 text-xs leading-5 text-text-secondary">{caption}</p>
    </div>
  );
}

function Ledger({ title, rows, positive = false }) {
  const { t } = useTranslation();
  return (
    <div className="rounded-[24px] border border-white/8 bg-white/[0.03] p-4">
      <p className="text-[10px] uppercase tracking-[0.2em] text-text-muted">{title}</p>
      {rows.length === 0 ? (
        <p className="mt-4 text-xs leading-5 text-text-secondary">{t("myTeamTab.finance.ledgerEmpty")}</p>
      ) : (
      <div className="mt-4 space-y-3">
        {rows.map((row) => (
          <div key={row.key} className="flex items-center justify-between gap-3 border-b border-white/6 pb-2 last:border-0 last:pb-0">
            <span className="text-sm text-text-primary">
              {t(`myTeamTab.finance.lines.${row.key}`)}
              {row.hint ? <span className="ml-2 text-[10px] font-normal text-text-muted">{row.hint}</span> : null}
            </span>
            <span className={`font-mono text-sm ${positive ? "text-status-green" : "text-status-red"}`}>{positive ? "+" : "-"}{formatMoney(row.value)}</span>
          </div>
        ))}
      </div>
      )}
    </div>
  );
}

function CostChart({ report }) {
  const { t } = useTranslation();
  const rows = costDistribution(report?.season);
  const seasonRounds = report?.season?.round ?? 0;
  return (
    <div className="rounded-[24px] border border-white/8 bg-white/[0.03] p-5">
      <div className="flex items-center justify-between gap-3">
        <p className="text-[10px] uppercase tracking-[0.2em] text-text-muted">{t("myTeamTab.cost.title")}</p>
        {seasonRounds > 0 ? (
          <span className="text-[9px] uppercase tracking-[0.14em] text-text-muted">{t("myTeamTab.cost.rounds", { count: seasonRounds })}</span>
        ) : null}
      </div>
      {rows.length === 0 ? (
        <p className="mt-5 rounded-2xl border border-white/8 bg-black/10 px-4 py-6 text-center text-xs leading-5 text-text-secondary">
          {t("myTeamTab.cost.empty")}
        </p>
      ) : (
        <div className="mt-5 grid gap-5 sm:grid-cols-[140px_1fr] xl:grid-cols-1 2xl:grid-cols-[150px_1fr]">
          <div className="mx-auto grid h-36 w-36 place-items-center rounded-full 2xl:h-40 2xl:w-40" style={{ background: `conic-gradient(${costGradient(rows)})` }}>
            <div className="grid h-20 w-20 place-items-center rounded-full bg-bg-primary text-[10px] font-semibold uppercase tracking-[0.14em] text-text-primary 2xl:h-24 2xl:w-24">{t("myTeamTab.cost.center")}</div>
          </div>
          <div className="space-y-3 self-center">
            {rows.map((row) => (
              <div key={row.key} className="rounded-2xl border border-white/6 bg-black/10 px-3 py-2 text-xs">
                <div className="flex items-center justify-between gap-2">
                  <span className="flex items-center gap-2 text-text-secondary"><span className="h-2 w-2 rounded-full" style={{ backgroundColor: row.color }} />{t(`myTeamTab.finance.lines.${row.key}`)}</span>
                  <span className="font-mono text-text-primary">{formatPercent(row.percent)}</span>
                </div>
                <div className="mt-2 h-1.5 rounded-full bg-white/10">
                  <div className="h-1.5 rounded-full" style={{ width: `${row.percent}%`, backgroundColor: row.color }} />
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// Divisão REAL dos custos acumulados da temporada (rosca). Deriva percentuais dos
// totais somados por `get_team_finance_report`; sem custos → lista vazia (estado vazio).
function costDistribution(season) {
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

function costGradient(rows) {
  let cursor = 0;
  return rows
    .map((row) => {
      const start = cursor;
      cursor += row.percent;
      return `${row.color} ${start}% ${cursor}%`;
    })
    .join(", ");
}

function ExecutiveReading({ team, net, payroll }) {
  const { t } = useTranslation();
  const signals = buildExecutiveSignals(team, net, payroll);
  return (
    <div className="rounded-[24px] border border-white/8 bg-white/[0.03] p-5">
      <p className="text-[10px] uppercase tracking-[0.2em] text-text-muted">{t("myTeamTab.executive.title")}</p>
      <div className="mt-4 grid gap-3 sm:grid-cols-2">
        {signals.map((signal) => (
          <div key={signal.label} className="rounded-2xl border border-white/8 bg-black/10 p-3">
            <p className={`text-sm font-semibold ${signal.tone}`}>{signal.label}</p>
            <p className="mt-1 text-xs leading-5 text-text-secondary">{signal.detail}</p>
          </div>
        ))}
      </div>
    </div>
  );
}

function buildExecutiveSignals(team, net, payroll) {
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
                    tier={carTierIndex(team.car_level ?? carLevel(team.car_performance))}
                    label={t(`myTeamTab.ranking.tiers.car.${carTierIndex(team.car_level ?? carLevel(team.car_performance))}`)}
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

function TeamNavChevron({ direction }) {
  const path = direction === "up" ? "M2 7.5 6 3.5l4 4" : "M2 4.5l4 4 4-4";
  return (
    <svg
      viewBox="0 0 12 12"
      aria-hidden="true"
      className="h-3.5 w-3.5 flex-shrink-0"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d={path} />
    </svg>
  );
}

function TeamNavigatorButton({ label, direction, disabled, onClick }) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      className={[
        "flex h-10 w-10 items-center justify-center rounded-2xl border backdrop-blur-md transition-all duration-200 ease-out",
        disabled
          ? "cursor-not-allowed border-white/[0.05] bg-[#11151b]/90 text-[#5b616b]"
          : "border-white/[0.12] bg-[#111d31]/95 text-text-secondary shadow-[0_14px_34px_rgba(0,0,0,0.34)] hover:border-white/[0.18] hover:bg-[#18263d] hover:text-text-primary focus-visible:border-white/[0.18] focus-visible:bg-[#18263d] focus-visible:text-text-primary",
      ].join(" ")}
    >
      <TeamNavChevron direction={direction} />
    </button>
  );
}

function TeamHistoryEdgeNavigator({ previousTeam, nextTeam, onSelectTeam, placement = "right" }) {
  const { t } = useTranslation();
  return (
    <div
      className={[
        "pointer-events-auto fixed top-24 z-[91] flex flex-col gap-2 max-lg:hidden sm:top-28",
        placement === "left" ? "animate-edge-rail-in-right" : "animate-edge-rail-in",
      ].join(" ")}
      style={
        placement === "center"
          ? { right: "18px" }
          : placement === "left"
            ? { left: "calc(min(50vw, 720px) + 14px)" }
            : { right: "calc(min(50vw, 720px) + 14px)" }
      }
    >
      <TeamNavigatorButton
        label={t("myTeamTab.history.nav.previous")}
        direction="up"
        disabled={!previousTeam}
        onClick={() => previousTeam && onSelectTeam(previousTeam)}
      />
      <TeamNavigatorButton
        label={t("myTeamTab.history.nav.next")}
        direction="down"
        disabled={!nextTeam}
        onClick={() => nextTeam && onSelectTeam(nextTeam)}
      />
    </div>
  );
}

export function TeamHistoryDrawer({
  careerId,
  team,
  teams,
  playerTeam,
  activeCategory,
  activeTab,
  onTabChange,
  onSelectTeam,
  onClose,
  placement = "right",
}) {
  const { t } = useTranslation();
  const [historyDossier, setHistoryDossier] = useState(null);
  const [historyStatus, setHistoryStatus] = useState("loading");
  const [historyError, setHistoryError] = useState("");
  const dossier = buildTeamHistoryDossier(
    team,
    teams,
    playerTeam,
    activeCategory,
    historyDossier,
    historyStatus,
    historyError,
  );
  const orderedTeams = orderTeamsForHistoryNavigation(teams);
  const currentTeamIndex = orderedTeams.findIndex((entry) => entry.id === team?.id);
  const previousTeam = currentTeamIndex > 0 ? orderedTeams[currentTeamIndex - 1] : null;
  const nextTeam = currentTeamIndex >= 0 && currentTeamIndex < orderedTeams.length - 1
    ? orderedTeams[currentTeamIndex + 1]
    : null;

  useEffect(() => {
    let mounted = true;
    if (!careerId || !team?.id) {
      setHistoryStatus("error");
      setHistoryError(i18n.t("myTeamTab.history.unavailable"));
      return undefined;
    }

    setHistoryStatus("loading");
    setHistoryError("");
    setHistoryDossier(null);
    invoke("get_team_history_dossier", {
      careerId,
      teamId: team.id,
      category: activeCategory ?? playerTeam?.categoria ?? team?.categoria ?? "",
    })
      .then((payload) => {
        if (!mounted) return;
        setHistoryDossier(payload);
        setHistoryStatus("ready");
      })
      .catch((invokeError) => {
        if (!mounted) return;
        setHistoryError(typeof invokeError === "string" ? invokeError : i18n.t("myTeamTab.history.loadError"));
        setHistoryStatus("error");
      });

    return () => {
      mounted = false;
    };
  }, [activeCategory, careerId, team?.id, team?.categoria, playerTeam?.categoria]);

  const drawerLayer = (
    <div className="fixed inset-0 z-[90] flex items-center justify-center" data-testid="team-history-layer" aria-hidden={false}>
      <button
        type="button"
        aria-label={t("myTeamTab.history.closeAria")}
        onClick={onClose}
        className="absolute inset-0 cursor-default bg-black/70 backdrop-blur-[3px]"
      />
      <TeamHistoryEdgeNavigator
        previousTeam={previousTeam}
        nextTeam={nextTeam}
        onSelectTeam={onSelectTeam}
        placement={placement}
      />
      <aside
        role="dialog"
        aria-modal="true"
        aria-labelledby="team-history-title"
        className={[
          "overflow-y-auto border-white/15 bg-[#07101d]",
          placement === "center"
            ? "animate-scale-in relative z-10 max-h-[88vh] w-[min(92vw,760px)] rounded-[24px] border shadow-[0_30px_90px_rgba(0,0,0,0.72)]"
            : [
                placement === "left" ? "animate-drawer-in-left left-0 border-r shadow-[28px_0_80px_rgba(0,0,0,0.72)]" : "animate-drawer-in right-0 border-l shadow-[-28px_0_80px_rgba(0,0,0,0.72)]",
                "absolute inset-y-0 w-[min(50vw,720px)] max-lg:w-full",
              ].join(" "),
        ].join(" ")}
        data-testid="team-history-drawer"
        style={{
          "--team": dossier.color,
          backgroundImage:
            "radial-gradient(circle at 10% 4%, color-mix(in srgb, var(--team) 16%, transparent), transparent 18rem), linear-gradient(180deg, rgba(12,22,38,0.98), rgba(5,11,20,0.995))",
        }}
      >
        <div className="h-1.5 bg-[linear-gradient(90deg,var(--team),rgba(255,255,255,0.1))]" />
        <button
          type="button"
          onClick={onClose}
          aria-label={t("myTeamTab.history.close")}
          className="absolute right-4 top-4 grid h-9 w-9 place-items-center rounded-xl border border-white/15 bg-[#0d1727] text-text-secondary transition-glass hover:bg-[#14233a] hover:text-text-primary"
        >
          x
        </button>

        <div className="px-6 pb-7 pt-6">
          <section className="rounded-[26px] border border-[color-mix(in_srgb,var(--team)_42%,transparent)] bg-[#0c1626]/95 p-5 shadow-[0_18px_55px_rgba(0,0,0,0.32)]">
            <div className="grid min-w-0 gap-5 pr-10 sm:grid-cols-[168px_minmax(0,1fr)] sm:items-center">
              <TeamLogoMark
                teamName={dossier.name}
                color={dossier.color}
                size="hero"
                testId="team-history-logo"
              />
              <div className="min-w-0">
                <h2 id="team-history-title" className="min-w-0 truncate text-3xl font-semibold leading-none tracking-[-0.04em] text-text-primary">
                  {dossier.name}
                </h2>
                <div className="mt-4 flex flex-wrap gap-2">
                  <span className="rounded-full border border-white/15 bg-[#08111f] px-3 py-1 text-xs text-text-primary">
                    {dossier.state}
                  </span>
                  {dossier.founded ? (
                    <span className="rounded-full border border-white/15 bg-[#08111f] px-3 py-1 text-xs text-text-primary">
                      {t("myTeamTab.history.foundedIn", { year: dossier.founded })}
                    </span>
                  ) : null}
                </div>
              </div>
            </div>
          </section>

          <div role="tablist" aria-label={t("myTeamTab.history.tablistAria")} className="mt-4 flex gap-2 overflow-x-auto pb-1">
            {TEAM_HISTORY_TABS.map((tab) => (
              <button
                key={tab.id}
                type="button"
                role="tab"
                aria-selected={activeTab === tab.id}
                onClick={() => onTabChange(tab.id)}
                className={`shrink-0 rounded-full border px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.13em] transition-glass ${
                  activeTab === tab.id
                    ? "border-[color-mix(in_srgb,var(--team)_55%,transparent)] bg-[color-mix(in_srgb,var(--team)_18%,transparent)] text-text-primary"
                    : "border-white/12 bg-[#0b1524] text-text-secondary hover:border-white/20 hover:bg-[#111d31] hover:text-text-primary"
                }`}
              >
                {t(`myTeamTab.history.tabs.${tab.id}`)}
              </button>
            ))}
          </div>

          <div className="mt-4">
            {activeTab === "records" ? <TeamHistoryRecords dossier={dossier} /> : null}
            {activeTab === "sport" ? <TeamHistorySport dossier={dossier} /> : null}
            {activeTab === "identity" ? <TeamHistoryIdentity dossier={dossier} /> : null}
            {activeTab === "management" ? <TeamHistoryManagement dossier={dossier} /> : null}
            {activeTab === "categories" ? <TeamHistoryCategories dossier={dossier} /> : null}
          </div>
        </div>
      </aside>
    </div>
  );

  return createPortal(drawerLayer, document.body);
}

function TeamHistoryRecords({ dossier }) {
  const { t } = useTranslation();
  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">{t("myTeamTab.history.records.title")}</h3>
      <p className="mt-2 rounded-2xl border border-white/12 bg-[#08111f]/95 px-4 py-3 text-xs leading-5 text-text-secondary">
        {t("myTeamTab.history.records.compareIntro")}<strong className="text-text-primary">{dossier.recordScope}</strong>{t("myTeamTab.history.records.compareOutro")}
      </p>
      {dossier.historyStatus !== "ready" ? (
        <HistoryStateMessage dossier={dossier} />
      ) : null}
      <div className="mt-4 space-y-1">
        {dossier.records.map((record) => (
          <div key={record.label} className="flex items-center justify-between gap-3 border-b border-white/8 py-3 last:border-0">
            <span className="text-[11px] font-semibold uppercase tracking-[0.14em] text-text-secondary">
              {record.label} <em className="not-italic font-bold text-accent-primary">({record.rank})</em>
            </span>
            <strong className="font-mono text-lg text-text-primary">{record.value}</strong>
          </div>
        ))}
      </div>
      {dossier.highlights?.length > 0 && (
        <div className="mt-4 grid gap-3 sm:grid-cols-2">
          {dossier.highlights.map((item) => (
            <div key={item.label} className="rounded-[18px] border border-status-yellow/30 bg-[#1c1808]/95 p-4">
              <span className="text-[10px] font-black uppercase tracking-[0.15em] text-status-yellow/80">{item.label}</span>
              <strong className="mt-2 block text-lg font-semibold text-status-yellow">{item.value}</strong>
              <p className="mt-1 text-xs leading-5 text-text-secondary">{item.detail}</p>
            </div>
          ))}
        </div>
      )}
      {dossier.milestones?.length > 0 && (
        <div className="mt-4">
          <span className="text-[10px] font-black uppercase tracking-[0.15em] text-text-secondary">{t("myTeamTab.history.records.milestones")}</span>
          <div className="mt-3 grid gap-2 sm:grid-cols-3">
            {dossier.milestones.map((milestone) => (
              <div key={milestone.label} className="rounded-[14px] border border-white/12 bg-[#0c1626]/95 px-3 py-2.5 text-center">
                <span className="block text-[10px] font-semibold uppercase tracking-[0.12em] text-text-secondary">{milestone.label}</span>
                <strong className="mt-1 block font-mono text-lg text-[color:var(--team)]">{milestone.year}</strong>
              </div>
            ))}
          </div>
        </div>
      )}
      {dossier.titleCategories?.length > 0 && (
        <div className="mt-4">
          <span className="text-[10px] font-black uppercase tracking-[0.15em] text-text-secondary">{t("myTeamTab.history.records.titleGallery")}</span>
          <div className="mt-3 grid gap-2">
            {dossier.titleCategories.map((item) => (
              <div key={`${item.category}-${item.year}`} className="rounded-2xl border border-l-4 border-white/12 bg-[#0c1626]/95 px-4 py-3" style={{ borderLeftColor: item.color }}>
                <div className="flex items-center justify-between gap-3">
                  <strong className="text-sm text-text-primary">{item.category}</strong>
                  <span className="font-mono text-xs font-bold text-status-yellow">{item.year}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}

function TeamHistorySport({ dossier }) {
  const { t } = useTranslation();
  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">{t("myTeamTab.history.sport.title")}</h3>
      {dossier.historyStatus !== "ready" ? (
        <HistoryStateMessage dossier={dossier} />
      ) : null}
      <div className="mt-4 grid gap-3">
        <HistoryInfoCard label={t("myTeamTab.history.sport.seasonsPlayed")} value={dossier.sport.seasons} detail={t("myTeamTab.history.sport.withinScope", { scope: dossier.recordScope })} />
        <HistoryInfoCard label={t("myTeamTab.history.sport.currentStreak")} value={dossier.sport.currentStreak} />
        <HistoryInfoCard label={t("myTeamTab.history.sport.bestStreak")} value={dossier.sport.bestStreak} />
      </div>
      <div className="mt-4 grid grid-cols-2 gap-3">
        <HistoryMiniMetric label={t("myTeamTab.history.sport.podiumRate")} value={dossier.sport.podiumRate} />
        <HistoryMiniMetric label={t("myTeamTab.history.sport.winRate")} value={dossier.sport.winRate} />
      </div>
      {dossier.seasonResults?.length > 0 && (
        <div className="mt-5">
          <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.sport.seasonBySeason")}</span>
          <div className="mt-3 overflow-hidden rounded-[18px] border border-white/10 bg-[#08111f]/95">
            <div className="grid grid-cols-[auto_1fr_auto_auto_auto_auto] gap-x-3 border-b border-white/10 px-4 py-2 text-[9px] font-black uppercase tracking-[0.14em] text-text-muted">
              <span>{t("myTeamTab.history.sport.cols.year")}</span><span>{t("myTeamTab.history.sport.cols.category")}</span><span className="text-right">{t("myTeamTab.history.sport.cols.pos")}</span><span className="text-right">{t("myTeamTab.history.sport.cols.wins")}</span><span className="text-right">{t("myTeamTab.history.sport.cols.podiums")}</span><span className="text-right">{t("myTeamTab.history.sport.cols.points")}</span>
            </div>
            {dossier.seasonResults.map((season) => (
              <div key={season.year} className="grid grid-cols-[auto_1fr_auto_auto_auto_auto] items-center gap-x-3 border-b border-white/6 px-4 py-2 text-xs last:border-0">
                <span className="font-mono font-black text-[color:var(--team)]">{season.year}</span>
                <span className="truncate text-text-secondary">{season.category}</span>
                <span className="text-right font-mono font-semibold text-text-primary">{season.position}</span>
                <span className="text-right font-mono text-status-yellow">{season.wins}</span>
                <span className="text-right font-mono text-text-primary">{season.podiums}</span>
                <span className="text-right font-mono text-text-secondary">{season.points}</span>
              </div>
            ))}
          </div>
        </div>
      )}
      <TimelineBlock items={dossier.timeline} />
    </section>
  );
}

function HistoryStateMessage({ dossier }) {
  const message = dossier.historyStatus === "error"
    ? dossier.historyError
    : i18n.t("myTeamTab.history.loading");
  return (
    <div className="mt-4 rounded-2xl border border-white/10 bg-[#08111f]/95 px-4 py-3 text-xs text-text-secondary">
      {message}
    </div>
  );
}

function TeamHistoryIdentity({ dossier }) {
  const { t } = useTranslation();
  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">{t("myTeamTab.history.identity.title")}</h3>
      <div className="mt-4 grid gap-3">
        <div className="rounded-[22px] border border-[color-mix(in_srgb,var(--team)_38%,transparent)] bg-[#0c1626] bg-[radial-gradient(circle_at_10%_8%,color-mix(in_srgb,var(--team)_20%,transparent),transparent_12rem),linear-gradient(145deg,rgba(14,26,44,0.96),rgba(7,16,29,0.99))] p-4">
          <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.identity.profileLabel")}</span>
          <strong className="mt-2 block text-2xl font-semibold leading-none tracking-[-0.03em] text-text-primary">{dossier.identity.profile}</strong>
          <p className="mt-3 text-xs leading-5 text-text-secondary">{dossier.identity.summary}</p>
        </div>
        <div className="grid items-stretch gap-3 md:grid-cols-[1fr_auto_1fr]">
          <HistoryInfoCard label={t("myTeamTab.history.identity.originLabel")} value={dossier.identity.origin} detail={t("myTeamTab.history.identity.originDetail")} />
          <div className="hidden place-items-center font-mono font-black text-[color:var(--team)] md:grid">-&gt;</div>
          <HistoryInfoCard label={t("myTeamTab.history.identity.currentLabel")} value={dossier.identity.current} detail={t("myTeamTab.history.identity.currentDetail")} />
        </div>
        <div className="grid gap-3 md:grid-cols-2">
          <div className="rounded-[18px] border border-status-yellow/30 bg-[#201a0b]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.identity.rivalLabel")}</span>
            <strong className="mt-2 block text-base font-semibold text-status-yellow">{dossier.identity.rival.name}</strong>
            <p className="mt-2 text-xs leading-5 text-text-secondary">
              {t("myTeamTab.history.identity.rivalToday", { category: dossier.identity.rival.currentCategory })} {dossier.identity.rival.note}
            </p>
          </div>
          <div className="rounded-[18px] border border-[color-mix(in_srgb,var(--team)_35%,transparent)] bg-[#0c1626]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.identity.symbolLabel")}</span>
            <strong className="mt-2 block text-base font-semibold text-text-primary">{dossier.identity.symbolDriver}</strong>
            <p className="mt-2 text-xs leading-5 text-text-secondary">{dossier.identity.symbolDriverDetail}</p>
          </div>
        </div>
        {dossier.ownershipEvents?.length > 0 && (
          <div className="rounded-[18px] border border-[color-mix(in_srgb,var(--team)_35%,transparent)] bg-[#0c1626]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.identity.erasLabel")}</span>
            <ul className="mt-3 grid gap-2.5">
              {dossier.ownershipEvents.map((event, index) => (
                <li key={index} className="flex items-start gap-3">
                  <span className="mt-0.5 font-mono text-xs font-black text-[color:var(--team)]">{event.year}</span>
                  <div>
                    <strong className="block text-sm font-semibold text-text-primary">{event.title}</strong>
                    <p className="text-xs leading-5 text-text-secondary">{event.detail}</p>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </section>
  );
}

function TeamHistoryManagement({ dossier }) {
  const { t } = useTranslation();
  const operationTone = operationHealthTone(dossier.management.operationHealth);

  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">{t("myTeamTab.history.management.title")}</h3>
      <div className="mt-4 grid gap-3">
        <div className={`rounded-[22px] border p-4 ${operationTone.card}`}>
          <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.management.operationHealth")}</span>
          <strong className={`mt-2 block text-2xl font-semibold ${operationTone.text}`}>{dossier.management.operationHealth}</strong>
          <p className="mt-2 text-xs leading-5 text-text-secondary">{dossier.management.summary}</p>
        </div>
        <div className="grid items-stretch gap-3 md:grid-cols-[1fr_auto_1fr]">
          <div className="rounded-[18px] border border-status-green/30 bg-[#0b1d19]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.management.peakCash")}</span>
            <strong className="mt-2 block font-mono text-base text-status-green">{dossier.management.peakCash}</strong>
            <p className="mt-2 text-xs leading-5 text-text-secondary">{dossier.management.peakCashDetail}</p>
          </div>
          <div className="hidden place-items-center font-mono font-black text-text-muted md:grid">&lt;&gt;</div>
          <div className="rounded-[18px] border border-status-red/30 bg-[#241014]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.management.worstCrisis")}</span>
            <strong className="mt-2 block font-mono text-base text-status-red">{dossier.management.worstCrisis}</strong>
            <p className="mt-2 text-xs leading-5 text-text-secondary">{dossier.management.worstCrisisDetail}</p>
          </div>
        </div>
        <div className="grid gap-3 md:grid-cols-2">
          <HistoryInfoCard label={t("myTeamTab.history.management.healthyYears")} value={dossier.management.healthyYears} detail={dossier.management.healthyYearsDetail} />
          <HistoryInfoCard label={t("myTeamTab.history.management.recordBalance")} value={dossier.management.peakCash} detail={t("myTeamTab.history.management.recordBalanceDetail")} />
        </div>
        <HistoryInfoCard label={t("myTeamTab.history.management.biggestInvestment")} value={dossier.management.biggestInvestment} detail={dossier.management.investmentDetail} />
        {dossier.ownershipEvents?.length > 0 && (
          <div className="rounded-[18px] border border-status-yellow/30 bg-[#201a0b]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.management.boardChanges")}</span>
            <ul className="mt-3 grid gap-2.5">
              {dossier.ownershipEvents.map((event, index) => (
                <li key={index} className="flex items-start gap-3">
                  <span className="mt-0.5 font-mono text-xs font-black text-status-yellow">{event.year}</span>
                  <div>
                    <strong className="block text-sm font-semibold text-text-primary">{event.title}</strong>
                    <p className="text-xs leading-5 text-text-secondary">{event.financialNote}</p>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </section>
  );
}

function TeamHistoryCategories({ dossier }) {
  const { t } = useTranslation();
  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">{t("myTeamTab.history.categories.title")}</h3>
      <div className="mt-4 grid grid-cols-2 gap-3">
        <HistoryMiniMetric label={t("myTeamTab.history.categories.promotions")} value={dossier.movement.promotions} />
        <HistoryMiniMetric label={t("myTeamTab.history.categories.relegations")} value={dossier.movement.relegations} />
      </div>
      <div className="mt-4 grid gap-3">
        <HistoryInfoCard label={t("myTeamTab.history.categories.timeByCategory")} value={dossier.movement.timeByCategory} />
        <HistoryInfoCard label={t("myTeamTab.history.categories.bestCategory")} value={dossier.movement.bestCategory} />
        <HistoryInfoCard label={t("myTeamTab.history.categories.hardestCategory")} value={dossier.movement.hardestCategory} />
      </div>
      <span className="mt-5 block text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.categories.ladder")}</span>
      <div className="mt-3 grid gap-2.5">
        {dossier.categoryPath.map((step, index) => {
          const move = categoryMovementBadge(step.movement);
          return (
            <div key={`${step.category}-${index}`} className="rounded-2xl border border-l-4 border-white/12 bg-[#0c1626]/95 p-4" style={{ borderLeftColor: step.color }}>
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-center gap-2">
                  <span className={`font-mono text-sm font-black ${move.tone}`} title={move.label}>{move.icon}</span>
                  <strong className="text-sm text-text-primary">{step.category}</strong>
                </div>
                <span className="font-mono text-xs font-semibold" style={{ color: step.color }}>{step.years}</span>
              </div>
              <p className="mt-2 text-xs leading-5 text-text-secondary">{step.detail}</p>
            </div>
          );
        })}
      </div>
    </section>
  );
}

function categoryMovementBadge(movement) {
  switch (movement) {
    case "promotion":
      return { icon: "▲", tone: "text-status-green", label: i18n.t("myTeamTab.history.categories.movement.promotion") };
    case "relegation":
      return { icon: "▼", tone: "text-status-red", label: i18n.t("myTeamTab.history.categories.movement.relegation") };
    case "start":
      return { icon: "●", tone: "text-[color:var(--team)]", label: i18n.t("myTeamTab.history.categories.movement.start") };
    default:
      return { icon: "—", tone: "text-text-muted", label: i18n.t("myTeamTab.history.categories.movement.same") };
  }
}

function HistoryInfoCard({ label, value, detail = "" }) {
  return (
    <div className="rounded-[18px] border border-white/12 bg-[#0c1626]/95 p-4">
      <div className="flex items-start justify-between gap-3">
        <strong className="text-sm text-text-primary">{label}</strong>
        <span className="text-right font-mono text-xs font-semibold text-status-yellow">{value}</span>
      </div>
      {detail ? <p className="mt-2 text-xs leading-5 text-text-secondary">{detail}</p> : null}
    </div>
  );
}

function HistoryMiniMetric({ label, value }) {
  return (
    <div className="rounded-2xl border border-white/12 bg-[#08111f]/95 p-3">
      <span className="text-[9px] font-black uppercase tracking-[0.15em] text-text-muted">{label}</span>
      <strong className="mt-2 block font-mono text-lg text-text-primary">{value}</strong>
    </div>
  );
}

function TimelineBlock({ items }) {
  const { t } = useTranslation();
  return (
    <div className="mt-5 rounded-[22px] border border-white/12 bg-[#0c1626]/95 p-4">
      <h4 className="text-[10px] uppercase tracking-[0.2em] text-text-muted">{t("myTeamTab.history.timeline.title")}</h4>
      <div className="mt-4 space-y-3">
        {items.map((item) => (
          <div key={item.year} className="grid grid-cols-[52px_1fr] gap-3 border-b border-white/6 pb-3 last:border-0 last:pb-0">
            <span className="font-mono text-xs font-semibold text-accent-primary">{item.year}</span>
            <p className="text-xs leading-5 text-text-secondary">{item.text}</p>
          </div>
        ))}
      </div>
    </div>
  );
}

function sortRankingRows(rows, sort) {
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
  if (key === "car_level") return team.car_level ?? carLevel(team.car_performance);
  return team?.[key] ?? 0;
}

function compareRankingValues(a, b) {
  if (typeof a === "string" || typeof b === "string") {
    return String(a).localeCompare(String(b), "pt-BR");
  }
  return Number(a) - Number(b);
}

function defaultSortDirection(key) {
  return ["cash_balance", "car_level", "confiabilidade", "pit_crew_quality", "pontos"].includes(key) ? "desc" : "asc";
}

function orderTeamsForHistoryNavigation(teams) {
  return [...(Array.isArray(teams) ? teams : [])].sort((a, b) => {
    const positionDiff = (a.posicao ?? 999) - (b.posicao ?? 999);
    if (positionDiff !== 0) return positionDiff;
    return String(a.nome ?? "").localeCompare(String(b.nome ?? ""), "pt-BR");
  });
}

function buildTeamHistoryDossier(
  team,
  teams,
  playerTeam,
  activeCategory,
  historyDossier,
  historyStatus = "ready",
  historyError = "",
) {
  const mergedTeam = team?.id === playerTeam?.id
    ? { ...team, ...playerTeam, posicao: team.posicao, pontos: team.pontos ?? playerTeam.pontos }
    : team;
  const category = activeCategory ?? playerTeam?.categoria ?? mergedTeam?.categoria ?? "gt4";
  const categoryName = categoryLabel(category);
  const rankedTeams = Array.isArray(teams) ? teams : [];
  const realHistory = normalizeTeamHistoryPayload(historyDossier);
  const rival = findHistoricRival(mergedTeam, rankedTeams);
  const founded = resolveTeamFoundedYear(mergedTeam);
  const categoryGroup = categoryGroupLabel(category);

  // Fallbacks NEUTROS: quando o histórico real ainda não carregou (ou o backend não
  // tem dados), mostramos "—"/estado honesto — nunca um número inventado. O histórico
  // real (get_team_history_dossier) substitui tudo isto quando pronto.
  return {
    name: mergedTeam?.nome ?? i18n.t("myTeamTab.team.fallbackName"),
    color: mergedTeam?.cor_primaria ?? "#58a6ff",
    state: realHistory?.identity?.heritage ?? teamHeritageLabel(founded),
    founded,
    currentCategory: categoryName,
    recordScope: realHistory?.recordScope ?? categoryGroup,
    historyStatus,
    historyError,
    hasHistory: realHistory?.hasHistory ?? false,
    records: realHistory?.records ?? [],
    titleCategories: realHistory?.titleCategories ?? [],
    sport: realHistory?.sport ?? emptyRealSport(),
    identity: realHistory?.identity ?? {
      origin: i18n.t("myTeamTab.history.defaults.dash"),
      current: categoryName,
      profile: i18n.t("myTeamTab.history.defaults.profileForming"),
      summary: i18n.t("myTeamTab.history.defaults.identityFormingSummary"),
      rival: {
        name: rival?.nome ?? i18n.t("myTeamTab.history.defaults.noRival"),
        currentCategory: categoryName,
        note: rival
          ? i18n.t("myTeamTab.history.defaults.rivalClosest", { group: categoryGroup })
          : i18n.t("myTeamTab.history.defaults.rivalNoDuel"),
      },
      symbolDriver: mergedTeam?.piloto_1_nome ?? i18n.t("myTeamTab.history.defaults.mainDriver"),
      symbolDriverDetail: i18n.t("myTeamTab.history.defaults.symbolDriverDetail"),
    },
    management: realHistory?.management ?? emptyRealManagement(mergedTeam),
    movement: realHistory?.movement ?? {
      promotions: i18n.t("myTeamTab.history.defaults.dash"),
      relegations: i18n.t("myTeamTab.history.defaults.dash"),
      timeByCategory: i18n.t("myTeamTab.history.defaults.unavailable"),
      bestCategory: categoryName,
      hardestCategory: i18n.t("myTeamTab.history.defaults.dash"),
    },
    categoryPath: realHistory?.categoryPath ?? [],
    timeline: realHistory?.timeline ?? [],
    ownershipEvents: realHistory?.ownershipEvents ?? [],
    highlights: realHistory?.highlights ?? [],
    milestones: realHistory?.milestones ?? [],
    seasonResults: realHistory?.seasonResults ?? [],
  };
}

function normalizeTeamHistoryPayload(payload) {
  if (!payload) return null;
  const sport = payload.sport ?? {};
  const identity = payload.identity ?? {};
  const rival = identity.rival ?? {};
  const management = payload.management ?? {};
  return {
    recordScope: payload.record_scope ?? payload.recordScope ?? i18n.t("myTeamTab.history.defaults.recordScope"),
    hasHistory: Boolean(payload.has_history ?? payload.hasHistory),
    records: (payload.records ?? []).map((record) => ({
      label: record.label,
      rank: record.rank,
      value: String(record.value),
    })),
    sport: {
      seasons: sport.seasons ?? i18n.t("myTeamTab.history.defaults.noSeasons"),
      currentStreak: sport.current_streak ?? sport.currentStreak ?? i18n.t("myTeamTab.history.defaults.noStreak"),
      bestStreak: sport.best_streak ?? sport.bestStreak ?? i18n.t("myTeamTab.history.defaults.noStreak"),
      podiumRate: sport.podium_rate ?? sport.podiumRate ?? "0%",
      winRate: sport.win_rate ?? sport.winRate ?? "0%",
      races: sport.races ?? 0,
      wins: sport.wins ?? 0,
      podiums: sport.podiums ?? 0,
    },
    timeline: payload.timeline ?? [],
    titleCategories: payload.title_categories ?? payload.titleCategories ?? [],
    categoryPath: (payload.category_path ?? payload.categoryPath ?? []).map((step) => ({
      category: step.category,
      years: step.years,
      detail: step.detail,
      color: step.color,
      movement: step.movement ?? "same",
    })),
    movement: payload.movement
      ? {
          promotions: payload.movement.promotions ?? 0,
          relegations: payload.movement.relegations ?? 0,
          timeByCategory: payload.movement.time_by_category ?? payload.movement.timeByCategory ?? "",
          bestCategory: payload.movement.best_category ?? payload.movement.bestCategory ?? i18n.t("myTeamTab.history.defaults.dash"),
          hardestCategory: payload.movement.hardest_category ?? payload.movement.hardestCategory ?? i18n.t("myTeamTab.history.defaults.dash"),
        }
      : null,
    ownershipEvents: (payload.ownership_events ?? payload.ownershipEvents ?? []).map((event) => ({
      year: String(event.year ?? ""),
      eventType: event.event_type ?? event.eventType ?? "sale",
      title: event.title ?? i18n.t("myTeamTab.history.defaults.newBoard"),
      detail: event.detail ?? "",
      financialNote: event.financial_note ?? event.financialNote ?? "",
    })),
    highlights: (payload.highlights ?? []).map((item) => ({
      label: item.label,
      value: String(item.value ?? ""),
      detail: item.detail ?? "",
    })),
    milestones: (payload.milestones ?? []).map((item) => ({
      label: item.label,
      year: String(item.year ?? ""),
    })),
    seasonResults: (payload.season_results ?? payload.seasonResults ?? []).map((item) => ({
      year: String(item.year ?? ""),
      category: item.category ?? "",
      position: String(item.position ?? "—"),
      wins: item.wins ?? 0,
      podiums: item.podiums ?? 0,
      points: String(item.points ?? "0"),
    })),
    identity: {
      origin: identity.origin ?? i18n.t("myTeamTab.history.defaults.noOrigin"),
      current: identity.current ?? i18n.t("myTeamTab.history.defaults.noCurrentCategory"),
      heritage: identity.heritage ?? null,
      profile: identity.profile ?? i18n.t("myTeamTab.history.defaults.profileForming"),
      summary: identity.summary ?? i18n.t("myTeamTab.history.defaults.identityInsufficient"),
      rival: {
        name: rival.name ?? i18n.t("myTeamTab.history.defaults.noRival"),
        currentCategory: rival.current_category ?? rival.currentCategory ?? i18n.t("myTeamTab.history.defaults.noCurrentCategory"),
        note: rival.note ?? i18n.t("myTeamTab.history.defaults.noRivalry"),
      },
      symbolDriver: identity.symbol_driver ?? identity.symbolDriver ?? i18n.t("myTeamTab.history.defaults.noSymbolDriver"),
      symbolDriverDetail: identity.symbol_driver_detail ?? identity.symbolDriverDetail ?? i18n.t("myTeamTab.history.defaults.insufficientResults"),
    },
    management: {
      operationHealth: management.operation_health ?? management.operationHealth ?? i18n.t("myTeamTab.history.defaults.monitored"),
      peakCash: management.peak_cash ?? management.peakCash ?? i18n.t("myTeamTab.history.defaults.noBalance"),
      worstCrisis: management.worst_crisis ?? management.worstCrisis ?? i18n.t("myTeamTab.history.defaults.noCrisis"),
      healthyYears: management.healthy_years ?? management.healthyYears ?? i18n.t("myTeamTab.history.defaults.noSeasons"),
      efficiency: management.efficiency ?? i18n.t("myTeamTab.history.defaults.efficiencyZero"),
      biggestInvestment: management.biggest_investment ?? management.biggestInvestment ?? i18n.t("myTeamTab.history.defaults.noInvestment"),
      summary: management.summary ?? i18n.t("myTeamTab.history.defaults.managementUnread"),
      peakCashDetail: management.peak_cash_detail ?? management.peakCashDetail ?? i18n.t("myTeamTab.history.defaults.noBalanceDetail"),
      worstCrisisDetail: management.worst_crisis_detail ?? management.worstCrisisDetail ?? i18n.t("myTeamTab.history.defaults.noCrisisDetail"),
      healthyYearsDetail: management.healthy_years_detail ?? management.healthyYearsDetail ?? i18n.t("myTeamTab.history.defaults.noHealthDetail"),
      efficiencyDetail: management.efficiency_detail ?? management.efficiencyDetail ?? i18n.t("myTeamTab.history.defaults.noEfficiencyDetail"),
      investmentDetail: management.investment_detail ?? management.investmentDetail ?? i18n.t("myTeamTab.history.defaults.noInvestmentDetail"),
    },
  };
}

// Ano de fundação REAL (payload do backend `founded_year`). Sem valor confiável → null,
// e o front mostra estado neutro em vez de inventar um ano.
function resolveTeamFoundedYear(team) {
  const explicitYear = Number(team?.founded_year ?? team?.ano_fundacao);
  if (Number.isFinite(explicitYear) && explicitYear > 1800) {
    return explicitYear;
  }
  return null;
}

function teamHeritageLabel(founded) {
  if (!founded) return i18n.t("myTeamTab.history.heritage.team");
  if (founded <= 1970) return i18n.t("myTeamTab.history.heritage.historic");
  return i18n.t("myTeamTab.history.heritage.consolidated");
}

function emptyRealSport() {
  return {
    seasons: i18n.t("myTeamTab.history.defaults.loadingReal"),
    currentStreak: i18n.t("myTeamTab.history.defaults.loadingReal"),
    bestStreak: i18n.t("myTeamTab.history.defaults.loadingReal"),
    podiumRate: "0%",
    winRate: "0%",
    races: 0,
    wins: 0,
    podiums: 0,
  };
}

// Fallback NEUTRO de gestão: usa só o estado financeiro atual (real); o resto fica "—"
// até o histórico real (get_team_history_dossier) chegar. Sem estimativas fabricadas.
function emptyRealManagement(team) {
  const debt = team?.debt_balance ?? 0;
  return {
    operationHealth: financialState(team?.financial_state),
    peakCash: i18n.t("myTeamTab.history.defaults.dash"),
    worstCrisis: debt > 0 ? i18n.t("myTeamTab.history.defaults.currentDebt", { value: formatMoney(debt) }) : i18n.t("myTeamTab.history.defaults.noRelevantDebt"),
    healthyYears: i18n.t("myTeamTab.history.defaults.dash"),
    efficiency: i18n.t("myTeamTab.history.defaults.dash"),
    biggestInvestment: i18n.t("myTeamTab.history.defaults.dash"),
    summary: i18n.t("myTeamTab.history.defaults.managementUnconsolidated"),
    peakCashDetail: i18n.t("myTeamTab.history.defaults.noBalanceHistoryDetail"),
    worstCrisisDetail: debt > 0 ? i18n.t("myTeamTab.history.defaults.currentLiability") : i18n.t("myTeamTab.history.defaults.noCrisisRegistered"),
    healthyYearsDetail: i18n.t("myTeamTab.history.defaults.noHistoryRegistered"),
    efficiencyDetail: i18n.t("myTeamTab.history.defaults.noHistoryRegistered"),
    investmentDetail: i18n.t("myTeamTab.history.defaults.noHistoryRegistered"),
  };
}

function findHistoricRival(team, teams) {
  return [...(teams ?? [])]
    .filter((entry) => entry.id !== team?.id)
    .sort((a, b) => Math.abs((a.posicao ?? 99) - (team?.posicao ?? 99)) - Math.abs((b.posicao ?? 99) - (team?.posicao ?? 99)))[0];
}

function categoryGroupLabel(category) {
  if (category?.includes("mazda")) return i18n.t("myTeamTab.history.groups.mazda");
  if (category?.includes("toyota")) return i18n.t("myTeamTab.history.groups.toyota");
  if (category === "bmw_m2") return i18n.t("myTeamTab.history.groups.bmw");
  if (category === "gt4") return i18n.t("myTeamTab.history.groups.gt4");
  if (category === "gt3") return i18n.t("myTeamTab.history.groups.gt3");
  if (category === "lmp2") return i18n.t("myTeamTab.history.groups.lmp2");
  if (category === "endurance") return i18n.t("myTeamTab.history.groups.endurance");
  return i18n.t("myTeamTab.history.groups.default");
}

function MetricBar({ label, value, rawValue }) {
  const clamped = clamp(Math.round(value), 0, 100);
  return (
    <div>
      <div className="mb-2 flex items-center justify-between text-sm text-text-secondary">
        <span>{label}</span>
        <span className="font-mono text-text-primary">{rawValue}</span>
      </div>
      <div className="h-3 rounded-full bg-white/10">
        <div className="h-3 rounded-full bg-gradient-to-r from-accent-primary to-accent-hover transition-glass" style={{ width: `${Math.max(6, clamped)}%` }} />
      </div>
    </div>
  );
}

function buildDriverRow(role, driver, team, playerId) {
  const isN1 = role === "N1";
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

function technicalMetrics(team, axis) {
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
  const level = team?.car_level ?? carLevel(team?.car_performance);
  return [
    { label: i18n.t("myTeamTab.tech.metrics.carPackage"), value: (level / 10) * 100, rawValue: i18n.t("myTeamTab.tech.raw.carLevel", { level }) },
    { label: i18n.t("myTeamTab.tech.metrics.trackPerformance"), value: normalizeCar(team?.car_performance ?? 0), rawValue: `${Math.round(normalizeCar(team?.car_performance ?? 0))}/100` },
    { label: i18n.t("myTeamTab.tech.metrics.reliability"), value: team?.confiabilidade ?? 0, rawValue: Math.round(team?.confiabilidade ?? 0) },
  ];
}

// Linhas REAIS de um ledger (entradas ou saídas) a partir da última rodada do report.
// Oculta linhas zeradas (ex.: sem auxílio / sem serviço de dívida naquela rodada).
function ledgerRows(round, lines) {
  if (!round) return [];
  return lines
    .map((line) => ({ key: line.key, value: Math.max(0, round[line.key] ?? 0) }))
    .filter((row) => row.value >= 1);
}

// Gráfico de caixa REAL: caixa ao fim de cada rodada, vindo de `team_finance_history`.
// Rótulo curto por rodada; prefixa a temporada quando a janela cruza temporadas.
function cashTimelineFromReport(report) {
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

function normalizeCar(value) {
  return clamp(((value + 5) / 21) * 100, 0, 100);
}

function carLevel(value) {
  return clamp(Math.round(value ?? 0), 1, 10);
}

function carTierIndex(value) {
  return Math.min(4, Math.floor((clamp(Math.round(value ?? 1), 1, 10) - 1) / 2));
}

function qualityTierIndex(value) {
  const normalized = clamp(Number(value) || 0, 0, 100);
  if (normalized <= 20) return 0;
  if (normalized <= 40) return 1;
  if (normalized <= 60) return 2;
  if (normalized <= 80) return 3;
  return 4;
}

function formatOrdinal(value) {
  return Number.isFinite(value) ? ordinal(value) : "-";
}

function moneyTone(value) {
  return value < 0 ? "text-status-red" : "text-text-primary";
}

function operationalRunway(cash, net) {
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

function financialState(state) {
  return {
    elite: i18n.t("myTeamTab.finance.states.elite"),
    healthy: i18n.t("myTeamTab.finance.states.healthy"),
    stable: i18n.t("myTeamTab.finance.states.stable"),
    pressured: i18n.t("myTeamTab.finance.states.pressured"),
    crisis: i18n.t("myTeamTab.finance.states.crisis"),
    collapse: i18n.t("myTeamTab.finance.states.collapse"),
  }[state] ?? i18n.t("myTeamTab.finance.states.stable");
}

function operationHealthTone(label) {
  const normalized = String(label ?? "")
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();

  if (normalized.includes("pressionada") || normalized.includes("critica") || normalized.includes("crise") || normalized.includes("colapso")) {
    return {
      card: "border-status-red/30 bg-[#241014]/95 bg-[radial-gradient(circle_at_12%_10%,rgba(255,103,103,0.14),transparent_12rem),linear-gradient(145deg,rgba(45,16,21,0.96),rgba(7,16,29,0.99))]",
      text: "text-status-red",
    };
  }

  if (normalized.includes("estavel") || normalized.includes("monitorada")) {
    return {
      card: "border-status-yellow/30 bg-[#201a0b]/95 bg-[radial-gradient(circle_at_12%_10%,rgba(242,196,109,0.14),transparent_12rem),linear-gradient(145deg,rgba(35,29,12,0.96),rgba(7,16,29,0.99))]",
      text: "text-status-yellow",
    };
  }

  return {
    card: "border-status-green/30 bg-[#0b1d19] bg-[radial-gradient(circle_at_12%_10%,rgba(94,231,168,0.14),transparent_12rem),linear-gradient(145deg,rgba(12,35,30,0.96),rgba(7,16,29,0.99))]",
    text: "text-status-green",
  };
}

function financialStateTone(state) {
  if (state === "elite" || state === "healthy") {
    return "border-status-green/25 bg-status-green/10 text-status-green";
  }
  if (state === "pressured" || state === "crisis" || state === "collapse") {
    return "border-status-red/25 bg-status-red/10 text-status-red";
  }
  return "border-status-yellow/25 bg-status-yellow/10 text-status-yellow";
}

function seasonStrategy(strategy) {
  return {
    expansion: i18n.t("myTeamTab.finance.strategies.expansion"),
    balanced: i18n.t("myTeamTab.finance.strategies.balanced"),
    austerity: i18n.t("myTeamTab.finance.strategies.austerity"),
    all_in: i18n.t("myTeamTab.finance.strategies.all_in"),
    survival: i18n.t("myTeamTab.finance.strategies.survival"),
  }[strategy] ?? i18n.t("myTeamTab.finance.strategies.balanced");
}

function pitRisk(value) {
  if (value <= 20) return i18n.t("myTeamTab.tech.pitRisk.ultraConservative");
  if (value <= 40) return i18n.t("myTeamTab.tech.pitRisk.conservative");
  if (value <= 55) return i18n.t("myTeamTab.tech.pitRisk.balanced");
  if (value <= 75) return i18n.t("myTeamTab.tech.pitRisk.aggressive");
  return i18n.t("myTeamTab.tech.pitRisk.opportunist");
}

function pitCrew(value) {
  if (value <= 20) return i18n.t("myTeamTab.tech.pitCrew.veryWeak");
  if (value <= 40) return i18n.t("myTeamTab.tech.pitCrew.weak");
  if (value <= 60) return i18n.t("myTeamTab.tech.pitCrew.ok");
  if (value <= 80) return i18n.t("myTeamTab.tech.pitCrew.strong");
  return i18n.t("myTeamTab.tech.pitCrew.elite");
}

function formatPercent(value) {
  return `${Math.round(value ?? 0)}%`;
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

export default MyTeamTab;
