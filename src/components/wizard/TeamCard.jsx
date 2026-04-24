import GlassCard from "../ui/GlassCard";

function TeamCard({ team, selected, onSelect }) {
  return (
    <GlassCard
      selected={selected}
      darkBg
      onClick={() => onSelect(team.index)}
      className="min-h-[210px]"
    >
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-3">
          <span
            className="h-10 w-4 rounded-full border border-white/10"
            style={{ backgroundColor: team.primaryColor }}
          />
          <div>
            <h3 className="text-lg font-semibold text-text-primary">{team.name}</h3>
            <p className="mt-1 text-xs uppercase tracking-[0.18em] text-text-secondary">
              {team.shortName}
            </p>
          </div>
        </div>
        <span className="rounded-full bg-white/8 px-3 py-1 text-xs text-text-secondary">
          {team.country}
        </span>
      </div>

      <div className="mt-8 space-y-3">
        <div className="flex items-center justify-between text-xs uppercase tracking-[0.16em] text-text-secondary">
          <span>Performance</span>
          <span>{team.performanceRating}/100</span>
        </div>
        <div className="h-2.5 overflow-hidden rounded-full bg-white/8">
          <div
            className="h-full rounded-full bg-gradient-to-r from-accent-primary via-status-green to-podium-gold"
            style={{ width: `${team.performanceRating}%` }}
          />
        </div>
      </div>
    </GlassCard>
  );
}

export default TeamCard;
