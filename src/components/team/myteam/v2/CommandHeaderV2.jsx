import { useTranslation } from "react-i18next";
import { CreditCard, Fuel, TrendingDown, TrendingUp, Users } from "lucide-react";

import GlassCard from "../../../ui/GlassCard";
import TeamLogoMark from "../../TeamLogoMark";
import { formatMoney, formatSignedMoney } from "../../../../utils/formatters";
import { formatOrdinal, formatPercent, moneyTone, operationalRunway } from "../teamMetrics";
import { rankInGrid } from "./gridMetrics";

// Cabeçalho da v2: uma faixa de comando de uma linha só.
//
// A versão anterior era um bloco de três andares — identidade e caixa nas pontas
// opostas de uma tela larga (com um vazio no meio), cinco chips de peso idêntico
// sob o caixa e uma frase de veredito numa faixa própria. O veredito repetia
// literalmente o chip da rodada, e chips que diziam "nada acontecendo"
// (`Dívida $0`, `Fôlego Estável`) gastavam a mesma atenção que os que pediam
// ação.
//
// Aqui a informação de rotina vira duas sublinhas — uma sob o nome, uma sob o
// caixa — e o chip fica reservado à exceção: dívida que existe, folha que aperta,
// fôlego em risco. Quando nada está fora do lugar o centro da faixa fica vazio de
// propósito; é o alarme aparecendo onde antes havia buraco.
function CommandHeaderV2({ team, teams, standing, gridSize, roundNet, projectedAnnual, hasProjection, payroll, salaryCeiling }) {
  const { t } = useTranslation();
  const cash = team?.cash_balance ?? 0;
  const debt = team?.debt_balance ?? 0;
  const position = standing?.posicao ?? 0;
  const cashRank = rankInGrid(teams, team?.id, "cash_balance");
  const runway = operationalRunway(cash, roundNet);
  const payrollPressure = salaryCeiling > 0 ? (payroll / salaryCeiling) * 100 : 0;
  const RoundIcon = roundNet >= 0 ? TrendingUp : TrendingDown;

  const identity = [
    position > 0 && gridSize > 0
      ? t("myTeamTabV2.command.standing", { position: formatOrdinal(position), grid: gridSize })
      : t("myTeamTabV2.command.standingUnknown"),
  ];
  if (standing?.pontos !== undefined) {
    identity.push(t("myTeamTabV2.command.points", { count: standing.pontos ?? 0 }));
  }
  if (cashRank > 0 && teams?.length > 1) {
    identity.push(t("myTeamTabV2.command.cashRank", { position: formatOrdinal(cashRank), grid: teams.length }));
  }

  // Quando há projeção ela vale mais que o fôlego — o fôlego é derivado do
  // resultado da rodada, que já está no primeiro segmento da mesma linha.
  const outlook = hasProjection
    ? { text: t("myTeamTabV2.command.projectionLine", { value: formatSignedMoney(projectedAnnual) }), tone: moneyTone(projectedAnnual) }
    : { text: t("myTeamTabV2.command.runwayLine", { value: runway.value }), tone: runway.tone };

  return (
    <GlassCard hover={false} className="rounded-[28px] px-6 py-4" data-testid="my-team-v2-command-header">
      <div className="flex flex-wrap items-center gap-x-6 gap-y-4">
        <div className="flex min-w-0 items-center gap-3.5">
          <TeamLogoMark teamName={team?.nome} color={team?.cor_primaria} size="lg" testId="my-team-v2-logo" />
          <div className="min-w-0">
            <h2 className="truncate text-xl font-semibold text-text-primary">{team?.nome ?? t("myTeamTabV2.team.fallbackName")}</h2>
            <p className="mt-1 text-xs text-text-secondary" data-testid="my-team-v2-identity-line">
              {identity.join(" · ")}
            </p>
          </div>
        </div>

        {/* Os alertas ocupam o centro da faixa: é o único lugar da linha que fica
            vazio quando a equipe está em ordem, então qualquer chip aqui já é
            sinal de que algo mudou. */}
        <div className="flex flex-1 flex-wrap items-center justify-center gap-2" data-testid="my-team-v2-alerts">
          {debt > 0 ? (
            <Alert Icon={CreditCard} label={t("myTeamTabV2.command.debtChip", { value: formatMoney(debt) })} tone="red" />
          ) : null}
          {salaryCeiling > 0 && payrollPressure >= 80 ? (
            <Alert
              Icon={Users}
              testId="my-team-v2-payroll-chip"
              label={t("myTeamTabV2.command.payrollChip", {
                percent: formatPercent(payrollPressure),
                ceiling: formatMoney(salaryCeiling),
              })}
              tone={payrollPressure >= 95 ? "red" : "yellow"}
            />
          ) : null}
          {/* O fôlego só vira chip quando está em risco e a sublinha do caixa foi
              tomada pela projeção — sem isso o risco sumiria da faixa. */}
          {hasProjection && runway.tone === "text-status-red" ? (
            <Alert Icon={Fuel} label={t("myTeamTabV2.command.runwayLine", { value: runway.value })} tone="red" />
          ) : null}
        </div>

        <div className="text-right" data-testid="my-team-v2-cash">
          <p className={`font-mono text-4xl font-semibold leading-none ${moneyTone(cash)}`}>{formatMoney(cash)}</p>
          <p className="mt-2 flex flex-wrap items-center justify-end gap-x-1.5 text-xs" data-testid="my-team-v2-cash-line">
            <RoundIcon size={14} strokeWidth={1.8} aria-hidden="true" className={roundNet >= 0 ? "text-status-green" : "text-status-red"} />
            <span className={roundNet >= 0 ? "text-status-green" : "text-status-red"}>
              {t("myTeamTabV2.command.lastRound", { value: formatSignedMoney(roundNet) })}
            </span>
            <span className="text-text-muted">·</span>
            <span className={outlook.tone}>{outlook.text}</span>
          </p>
        </div>
      </div>
    </GlassCard>
  );
}

const ALERT_TONES = {
  red: "border-status-red/40 bg-status-red/10 text-status-red",
  yellow: "border-status-yellow/40 bg-status-yellow/10 text-status-yellow",
};

function Alert({ label, tone = "yellow", Icon = null, testId = undefined }) {
  return (
    <span
      data-testid={testId}
      className={`inline-flex items-center gap-1.5 rounded-full border px-3 py-1 text-[11px] font-semibold ${ALERT_TONES[tone]}`}
    >
      {Icon ? <Icon size={14} strokeWidth={1.8} aria-hidden="true" /> : null}
      {label}
    </span>
  );
}

export default CommandHeaderV2;
