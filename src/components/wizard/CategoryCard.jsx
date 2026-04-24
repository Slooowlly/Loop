import GlassCard from "../ui/GlassCard";

function CategoryCard({ category, selected, onSelect }) {
  return (
    <GlassCard
      selected={selected}
      darkBg
      onClick={() => onSelect(category.id)}
      className="min-h-[260px]"
    >
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-[11px] uppercase tracking-[0.22em] text-text-secondary">
            Categoria inicial
          </p>
          <h3 className="mt-3 text-2xl font-semibold text-text-primary">
            {category.name}
          </h3>
          <p className="mt-2 text-sm text-text-secondary">{category.car}</p>
        </div>
        <div className="rounded-2xl bg-white/6 px-4 py-3 text-3xl">
          {category.emoji}
        </div>
      </div>

      <p className="mt-6 text-sm leading-6 text-text-secondary">
        {category.description}
      </p>

      <div className="mt-8 grid grid-cols-3 gap-3">
        <div className="glass-light rounded-2xl p-4 text-center">
          <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">Equipes</p>
          <p className="mt-2 text-xl font-semibold text-text-primary">{category.teams}</p>
        </div>
        <div className="glass-light rounded-2xl p-4 text-center">
          <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">Corridas</p>
          <p className="mt-2 text-xl font-semibold text-text-primary">{category.races}</p>
        </div>
        <div className="glass-light rounded-2xl p-4 text-center">
          <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">Pilotos</p>
          <p className="mt-2 text-xl font-semibold text-text-primary">{category.drivers}</p>
        </div>
      </div>
    </GlassCard>
  );
}

export default CategoryCard;
