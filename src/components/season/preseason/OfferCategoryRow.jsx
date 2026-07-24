import { useTranslation } from "react-i18next";
import { subcatColor } from "../preSeasonFormatters.js";

// Fileira DENSA (uma linha) para os andares 2 e 3: borda colorida + rótulo +
// contagem de vagas discreta. Sem o chip numérico grande (era só ruído).
export default function OfferCategoryRow({ group, onSelect }) {
  const { t } = useTranslation();
  const n = group.n1.length + group.n2.length;
  const accent = subcatColor(group.cat);
  return (
    <button
      type="button"
      onClick={() => onSelect?.(group.cat)}
      data-testid={`offer-category-row-${group.cat}`}
      className="transition-glass glass-light hover:glass group flex w-full items-center gap-3 rounded-lg py-2 pl-3 pr-2.5 text-left"
      style={{ borderLeft: `3px solid ${accent}` }}
    >
      <span
        className="min-w-0 flex-1 truncate text-[11px] font-black uppercase tracking-[0.12em]"
        style={{ color: accent }}
      >
        {group.label}
      </span>
      <span className="shrink-0 text-[10px] font-semibold uppercase tracking-[0.14em] text-[color:var(--text-muted)]">
        {t("preSeason.offers.vacancies", { count: n })}
      </span>
      <span className="shrink-0 text-[color:var(--text-muted)] transition-transform group-hover:translate-x-0.5">›</span>
    </button>
  );
}
