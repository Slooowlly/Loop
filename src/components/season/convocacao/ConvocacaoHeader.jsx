import { useTranslation } from "react-i18next";

import GlassButton from "../../ui/GlassButton";
import { CATEGORY_FILTERS } from "./constantes.js";

export default function ConvocacaoHeader({
  selectedCategory,
  onSelectCategory,
  acceptedSpecialOffer,
  specialWindowState,
  season,
  currentDay,
  totalDays,
  isConvocating,
  primaryCtaLabel,
  onPrimaryCta,
  error,
}) {
  const { t } = useTranslation();

  return (
    <header className="glass-strong animate-fade-in mb-3 rounded-2xl px-5 py-2 lg:px-6">
      <div className="grid items-start gap-3 lg:grid-cols-[1fr_auto]">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <p className="text-body-sm font-bold uppercase tracking-[0.28em] text-[color:var(--accent-primary)]">
              {t("convocation.header.eyebrow")}
            </p>
            {acceptedSpecialOffer && specialWindowState?.is_finished && (
              <span className="glass-light rounded-full px-2.5 py-1 text-body-sm font-bold tracking-[0.14em] text-[color:var(--accent-primary)]">
                {t("convocation.header.acceptedBadge")}
              </span>
            )}
          </div>
          <h1 className="mt-1 text-[20px] font-bold leading-[1.05] tracking-[-0.02em] text-[color:var(--text-primary)] lg:text-[26px]">
            {t("convocation.header.title")}
          </h1>

          <div className="mt-2 max-w-full overflow-x-auto">
            <div className="glass inline-flex w-fit items-center gap-0.5 whitespace-nowrap rounded-full p-1">
              {CATEGORY_FILTERS.map((category) => {
                const active = selectedCategory === category.id;
                return (
                  <button
                    key={category.id}
                    onClick={() => onSelectCategory(category.id)}
                    className={`transition-glass cursor-pointer rounded-full border px-2.5 py-1 text-body-sm font-semibold ${
                      active
                        ? "border-white/30 bg-white/14 text-[color:var(--accent-primary)]"
                        : "border-transparent bg-white/3 text-[color:var(--text-secondary)] hover:bg-white/8 hover:text-[color:var(--text-primary)]"
                    }`}
                  >
                    <span
                      className="mr-2 inline-block h-1.5 w-1.5 rounded-full"
                      style={{ backgroundColor: category.color }}
                    />
                    {t(`convocation.categoryFilters.${category.id}`)}
                  </button>
                );
              })}
            </div>
          </div>
        </div>

        <div className="flex items-center gap-3 self-center lg:justify-self-end">
          <span className="rounded-full border border-[#58a6ff66] bg-[#58a6ff1a] px-2.5 py-1 text-body-sm font-bold uppercase tracking-[0.14em] text-[color:var(--accent-primary)]">
            {t("convocation.header.specialBlockBadge")}
          </span>

          <div className="w-[220px] px-1 lg:w-[280px]">
            <div className="mb-1 flex items-center justify-between gap-2">
              <p className="text-body-sm font-bold uppercase tracking-[0.2em] text-[color:var(--text-secondary)]">
                {t("convocation.header.dayLabel")}{" "}
                <span className="text-[color:var(--text-primary)]">
                  {currentDay}/{totalDays}
                </span>
              </p>
              <p className="text-body-sm text-[color:var(--text-secondary)]">
                {specialWindowState?.status ?? season?.fase ?? t("convocation.header.statusFallback")}
              </p>
            </div>
            <div className="h-[3px] w-full rounded-full bg-[#2a3240]">
              <div
                className="h-full rounded-full bg-[color:var(--accent-primary)]"
                style={{ width: `${Math.max(14, Math.round((currentDay / totalDays) * 100))}%` }}
              />
            </div>
          </div>

          <GlassButton
            variant="primary"
            disabled={isConvocating}
            className="rounded-full px-6 py-2.5 text-body-lg font-bold uppercase tracking-[0.16em]"
            onClick={onPrimaryCta}
          >
            {primaryCtaLabel}
          </GlassButton>
        </div>
      </div>

      {error && (
        <p className="mt-2 text-center text-body-sm text-[color:var(--status-red)]">{error}</p>
      )}
    </header>
  );
}
