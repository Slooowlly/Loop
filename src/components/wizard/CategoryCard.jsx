import GlassCard from "../ui/GlassCard";

function CategoryCard({ category, selected, onSelect }) {
  return (
    <GlassCard
      selected={selected}
      darkBg
      onClick={() => onSelect(category.id)}
      className="min-h-[170px]"
    >
      <div className="flex items-center justify-between gap-6">
        <div className="min-w-0 flex-1">
          <p className="text-[11px] uppercase tracking-[0.22em] text-text-secondary">
            Categoria inicial
          </p>
          <h3 className="mt-3 text-2xl font-semibold text-text-primary">
            {category.name}
          </h3>
          <p className="mt-1.5 text-sm text-text-secondary">{category.car}</p>
          <p className="mt-4 text-sm leading-6 text-text-secondary">
            {category.description}
          </p>
        </div>
        {category.logo ? (
          <img
            src={category.logo}
            alt={`${category.name} logo`}
            className="h-28 w-auto max-w-[150px] shrink-0 self-center object-contain"
            draggable={false}
          />
        ) : null}
      </div>
    </GlassCard>
  );
}

export default CategoryCard;
