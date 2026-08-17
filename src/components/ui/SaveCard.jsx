import { useTranslation } from "react-i18next";

import GlassButton from "./GlassButton";
import GlassCard from "./GlassCard";
import { formatDateTime, formatSurfaceSeasonLabel } from "../../utils/formatters";

// A dificuldade saiu do cartão em 16/08/2026, junto com o passo do wizard que a escolhia:
// sem escolha, todo save nasce em "medio" e a informação deixou de distinguir um do outro.
// Com ela foi embora a borda colorida por faixa, que era a mesma cor em todos os cartões.
function SaveCard({ save, onLoad, onDelete, onBackups, loading = false }) {
  const { t } = useTranslation();

  return (
    <GlassCard hover={false} className="rounded-[28px] border border-white/10">
      <div className="flex flex-col gap-6 lg:flex-row lg:items-start lg:justify-between">
        <div className="space-y-3">
          <p className="text-[11px] uppercase tracking-[0.22em] text-accent-primary">
            {save.career_id}
          </p>
          <h3 className="text-2xl font-semibold text-text-primary">{save.player_name}</h3>
          <p className="text-sm text-text-secondary">{save.category_name}</p>

          <div className="grid gap-3 pt-2 sm:grid-cols-2">
            <div className="glass-light rounded-2xl p-4">
              <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                {t("saveCard.season")}
              </p>
              <p className="mt-2 text-sm text-text-primary">{formatSurfaceSeasonLabel(save)}</p>
            </div>
          </div>

          <div className="flex flex-wrap gap-4 text-sm text-text-secondary">
            <span>{t("saveCard.lastPlayed", { when: formatDateTime(save.last_played) })}</span>
            <span>{t("saveCard.created", { when: formatDateTime(save.created) })}</span>
            <span>{t("saveCard.racesInCalendar", { count: save.total_races })}</span>
          </div>
        </div>

        <div className="flex shrink-0 flex-col gap-3 sm:flex-row lg:flex-col">
          <GlassButton
            variant="primary"
            disabled={loading}
            onClick={() => onLoad(save.career_id)}
            className="min-w-36"
          >
            Carregar
          </GlassButton>
          {onBackups ? (
            <GlassButton
              variant="secondary"
              disabled={loading}
              onClick={() => onBackups(save)}
              className="min-w-36"
            >
              {t("loadSave.backups.open")}
            </GlassButton>
          ) : null}
          <GlassButton
            variant="danger"
            disabled={loading}
            onClick={() => onDelete(save.career_id)}
            className="min-w-36"
          >
            Deletar
          </GlassButton>
        </div>
      </div>
    </GlassCard>
  );
}

export default SaveCard;
