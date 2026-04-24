import GlassCard from "../ui/GlassCard";

function DifficultyCard({ difficulty, selected, onSelect }) {
  return (
    <GlassCard
      selected={selected}
      darkBg
      onClick={() => onSelect(difficulty.id)}
      className="min-h-[210px] justify-between"
    >
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-[11px] uppercase tracking-[0.2em] text-text-secondary">
            Dificuldade
          </p>
          <h3 className="mt-3 text-2xl font-semibold text-text-primary">
            {difficulty.name}
          </h3>
        </div>
        <div className="text-4xl">{difficulty.emoji}</div>
      </div>

      <div className="mt-8 space-y-4">
        <div className="h-2 overflow-hidden rounded-full bg-white/8">
          <div
            className="h-full rounded-full transition-glass"
            style={{
              width:
                difficulty.id === "facil"
                  ? "45%"
                  : difficulty.id === "medio"
                    ? "62%"
                    : difficulty.id === "dificil"
                      ? "78%"
                      : "94%",
              backgroundColor: difficulty.accent,
            }}
          />
        </div>
        <p className="text-sm text-text-secondary">{difficulty.desc}</p>
      </div>
    </GlassCard>
  );
}

export default DifficultyCard;
