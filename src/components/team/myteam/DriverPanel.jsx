import { useTranslation } from "react-i18next";

import GlassCard from "../../ui/GlassCard";
import FlagIcon from "../../ui/FlagIcon";
import { formatMoney } from "../../../utils/formatters";
import { clamp, formatPercent } from "./teamMetrics";

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
            {formatMoney(driver.salary)}
            <span className="ml-1 font-sans text-[10px] font-normal text-text-muted">{t("myTeamTab.drivers.perYear")}</span>
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

export default DriverPanel;
