import { useTranslation } from "react-i18next";

import GlassCard from "../../ui/GlassCard";
import { TECH_AXES, clamp, technicalMetrics } from "./teamMetrics";

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

export default TechPanel;
