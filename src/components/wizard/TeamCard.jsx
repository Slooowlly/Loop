import GlassCard from "../ui/GlassCard";

function TeamCard({ team, selected, onSelect }) {
  const selectionValue = team.id ?? team.index;
  const name = team.name ?? team.nome;
  const shortName = team.shortName ?? team.nome_curto;
  const primaryColor = team.primaryColor ?? team.cor_primaria ?? "#ffffff";
  const performanceRating = Math.round(team.performanceRating ?? team.car_performance ?? 0);
  const reputationRating = Math.round(team.reputationRating ?? team.reputacao ?? 0);
  const badge = team.country ?? `${reputationRating}/100 rep.`;
  const n1Name = team.n1Name ?? team.n1_nome;
  const n2Name = team.n2Name ?? team.n2_nome;

  return (
    <GlassCard
      selected={selected}
      darkBg
      onClick={() => onSelect(selectionValue)}
      className="min-h-[210px]"
    >
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-3">
          <span
            className="h-10 w-4 rounded-full border border-white/10"
            style={{ backgroundColor: primaryColor }}
          />
          <div>
            <h3 className="text-lg font-semibold text-text-primary">{name}</h3>
            <p className="mt-1 text-xs uppercase tracking-[0.18em] text-text-secondary">
              {shortName}
            </p>
          </div>
        </div>
        <span className="rounded-full bg-white/8 px-3 py-1 text-xs text-text-secondary">
          {badge}
        </span>
      </div>

      <div className="mt-8 space-y-3">
        <div className="flex items-center justify-between text-xs uppercase tracking-[0.16em] text-text-secondary">
          <span>Performance</span>
          <span>{performanceRating}/100</span>
        </div>
        <div className="h-2.5 overflow-hidden rounded-full bg-white/8">
          <div
            className="h-full rounded-full bg-gradient-to-r from-accent-primary via-status-green to-podium-gold"
            style={{ width: `${performanceRating}%` }}
          />
        </div>
      </div>

      {n1Name || n2Name ? (
        <div className="mt-6 grid gap-2 text-sm text-text-secondary">
          <div className="flex justify-between gap-3">
            <span className="text-text-muted">N1 atual</span>
            <span className="text-right text-text-primary">{n1Name ?? "Livre"}</span>
          </div>
          <div className="flex justify-between gap-3">
            <span className="text-text-muted">N2 atual</span>
            <span className="text-right text-text-primary">{n2Name ?? "Livre"}</span>
          </div>
        </div>
      ) : null}
    </GlassCard>
  );
}

export default TeamCard;
