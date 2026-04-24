import GlassButton from "./GlassButton";
import GlassCard from "./GlassCard";
import {
  difficultyLabel,
  formatDateTime,
  formatSurfaceSeasonLabel,
} from "../../utils/formatters";

const difficultyAccent = {
  facil: "border-status-green/30",
  medio: "border-status-yellow/30",
  dificil: "border-status-orange/30",
  lendario: "border-status-red/30",
};

function SaveCard({ save, onLoad, onDelete, loading = false }) {
  return (
    <GlassCard
      hover={false}
      className={`rounded-[28px] border ${difficultyAccent[save.difficulty] ?? "border-white/10"}`}
    >
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
                Temporada
              </p>
              <p className="mt-2 text-sm text-text-primary">{formatSurfaceSeasonLabel(save)}</p>
            </div>
            <div className="glass-light rounded-2xl p-4">
              <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                Dificuldade
              </p>
              <p className="mt-2 text-sm text-text-primary">{difficultyLabel(save.difficulty)}</p>
            </div>
          </div>

          <div className="flex flex-wrap gap-4 text-sm text-text-secondary">
            <span>Ultimo jogo: {formatDateTime(save.last_played)}</span>
            <span>Criado: {formatDateTime(save.created)}</span>
            <span>{save.total_races} corridas no calendario</span>
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
