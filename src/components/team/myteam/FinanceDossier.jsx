import { useState } from "react";
import { useTranslation } from "react-i18next";

import GlassCard from "../../ui/GlassCard";
import { formatMoney, formatSignedMoney } from "../../../utils/formatters";
import {
  EXPENSE_LINES,
  INCOME_LINES,
  buildExecutiveSignals,
  cashTimelineFromReport,
  formatOrdinal,
  formatPercent,
  ledgerRows,
  moneyTone,
  operationalRunway,
  seasonStrategy,
} from "./teamMetrics";

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
  // A linha "Salários" da rodada é anual ÷ nº de corridas; ancoramos com a folha ANUAL
  // para que o valor por rodada não pareça furado ao lado do salário anual dos pilotos.
  const expenseLedger = ledgerRows(report?.latest, EXPENSE_LINES).map((row) =>
    row.key === "salary_expense" && payroll > 0
      ? { ...row, hint: t("myTeamTab.finance.salaryHint", { value: formatMoney(payroll) }) }
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
        <Kpi label={t("myTeamTab.finance.kpi.salaryCeiling")} value={formatMoney(team?.salary_ceiling ?? 0)} caption={t("myTeamTab.finance.kpi.salaryCeilingCaption")} period={t("myTeamTab.finance.period.perYear")} />
        <Kpi label={t("myTeamTab.finance.kpi.spendingPower")} value={formatSignedMoney(team?.spending_power ?? 0)} caption={t("myTeamTab.finance.kpi.spendingPowerCaption")} period={t("myTeamTab.finance.period.season")} tone={(team?.spending_power ?? 0) >= 0 ? "text-status-green" : "text-status-red"} />
      </div>

      <p className="mt-3 text-[11px] leading-5 text-text-muted">
        {t("myTeamTab.finance.legend.intro")}<span className="text-text-secondary">{t("myTeamTab.finance.legend.perRound")}</span>{t("myTeamTab.finance.legend.perRoundDesc")}{" "}
        <span className="text-text-secondary">{t("myTeamTab.finance.legend.perYear")}</span>{t("myTeamTab.finance.legend.perYearDesc")}<span className="text-text-secondary">{t("myTeamTab.finance.legend.season")}</span>{t("myTeamTab.finance.legend.seasonDesc")}
      </p>

      <div className="mt-5 grid gap-4 lg:grid-cols-2">
        <Ledger title={t("myTeamTab.finance.incomeTitle")} rows={incomeLedger} positive />
        <Ledger title={t("myTeamTab.finance.expenseTitle")} rows={expenseLedger} />
      </div>

      <PublicPresencePanel presence={team?.presenca_publica ?? 0} />

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
                <Kpi compact label={t("myTeamTab.cash.annualPayroll")} value={formatMoney(payroll)} />
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

// PRESENÇA PÚBLICA da equipe (0–100) — vem pronta do backend (`presenca_publica`,
// derivada da mídia do lineup) e é o multiplicador de patrocínio de cada rodada.
// Fica logo abaixo das entradas porque é ali que ela age: sem esse painel, o efeito
// de contratar um companheiro midiático era invisível.
//
// A barra é só a leitura visual do MESMO 0–100 do backend — nada é recalculado aqui.
// `0` (equipe sem lineup lido) esconde o painel em vez de mostrar um zero sem sentido.
function PublicPresencePanel({ presence }) {
  const { t } = useTranslation();
  if (!(presence > 0)) return null;
  return (
    <div className="mt-5 rounded-[24px] border border-white/8 bg-white/[0.03] p-5" data-testid="public-presence">
      <p className="text-[10px] uppercase tracking-[0.2em] text-text-muted">
        {t("myTeamTab.finance.presence.eyebrow")}
      </p>
      <div className="mt-4 flex flex-wrap items-center gap-5">
        <div className="min-w-[150px]">
          <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
            {t("myTeamTab.finance.presence.label")}
          </p>
          <p className="mt-2 font-mono text-lg font-semibold text-accent-primary">{presence.toFixed(1)}</p>
          <p className="mt-1 text-xs text-text-secondary">{t("myTeamTab.finance.presence.caption")}</p>
        </div>
        <div className="h-2 min-w-[120px] flex-1 overflow-hidden rounded-full bg-white/8">
          <div
            className="h-full rounded-full bg-gradient-to-r from-accent-primary/70 to-accent-hover"
            style={{ width: `${Math.min(100, Math.max(0, presence))}%` }}
          />
        </div>
      </div>
      <p className="mt-4 text-xs leading-5 text-text-secondary">
        {t("myTeamTab.finance.presence.explainer")}
      </p>
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

export default FinanceDossier;
