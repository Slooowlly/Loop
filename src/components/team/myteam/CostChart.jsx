import { useTranslation } from "react-i18next";

import { costDistribution, costGradient, formatPercent } from "./teamMetrics";

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

export default CostChart;
