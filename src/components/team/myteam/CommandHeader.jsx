import { useTranslation } from "react-i18next";

import GlassCard from "../../ui/GlassCard";
import TeamLogoMark from "../TeamLogoMark";
import { financialState } from "../teamFinanceLabels";
import { formatMoney } from "../../../utils/formatters";
import { financialStateTone, formatOrdinal, moneyTone } from "./teamMetrics";

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

export default CommandHeader;
