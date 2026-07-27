import { useTranslation } from "react-i18next";

import GlassCard from "../../ui/GlassCard";
import { clamp } from "./teamMetrics";

// Clima da garagem: a política interna N1/N2 que o backend (`hierarchy/`) simula a cada
// corrida e que, até aqui, nenhuma tela mostrava. A tensão não é enfeite — acima de 50 ela
// derruba a moral da equipe no fim da temporada, e a moral vira ritmo e confiabilidade na
// pista para os dois carros.
function GaragePanel({ climate, n1Name, n2Name, inverted }) {
  const { t } = useTranslation();

  return (
    <GlassCard hover={false} className="rounded-[28px]" data-testid="garage-panel">
      <p className="text-[10px] uppercase tracking-[0.22em] text-accent-primary">{t("myTeamTab.garage.eyebrow")}</p>
      <div className="mt-2 flex items-baseline justify-between gap-3">
        <h3 className="text-xl font-semibold text-text-primary">{t("myTeamTab.garage.title")}</h3>
        <span className={`text-sm font-semibold ${climate.tone}`} data-testid="garage-climate-label">
          {climate.label}
        </span>
      </div>

      <div className="mt-5">
        <div className="mb-2 flex items-center justify-between text-[10px] uppercase tracking-[0.16em] text-text-muted">
          <span>{t("myTeamTab.garage.tension")}</span>
          <span data-testid="garage-tension-value">{Math.round(climate.tension)}</span>
        </div>
        <div className="h-2 rounded-full bg-white/10">
          <div
            className={`h-2 rounded-full ${climate.barTone}`}
            style={{ width: `${clamp(climate.tension, 4, 100)}%` }}
            data-testid="garage-tension-bar"
          />
        </div>
        {climate.hurtsMorale ? (
          <p className="mt-2 text-xs text-status-red" data-testid="garage-morale-warning">
            {t("myTeamTab.garage.moraleWarning")}
          </p>
        ) : null}
      </div>

      <div className="mt-5 grid grid-cols-2 gap-3">
        <OrderSlot label={t("myTeamTab.garage.n1")} name={n1Name} emphasis />
        <OrderSlot label={t("myTeamTab.garage.n2")} name={n2Name} />
      </div>

      {inverted ? (
        <p className="mt-3 text-xs text-status-yellow" data-testid="garage-inverted-note">
          {t("myTeamTab.garage.invertedNote")}
        </p>
      ) : null}

      {climate.inversions > 0 ? (
        <p className="mt-3 text-xs text-text-secondary" data-testid="garage-inversions">
          {t("myTeamTab.garage.inversions", { count: climate.inversions })}
        </p>
      ) : null}
    </GlassCard>
  );
}

function OrderSlot({ label, name, emphasis = false }) {
  return (
    <div className={`rounded-[18px] border p-3 ${emphasis ? "border-accent-primary/35 bg-accent-primary/10" : "border-white/8 bg-white/[0.03]"}`}>
      <p className="text-[10px] uppercase tracking-[0.2em] text-text-muted">{label}</p>
      <p className="mt-1 truncate text-sm font-semibold text-text-primary">{name}</p>
    </div>
  );
}

export default GaragePanel;
