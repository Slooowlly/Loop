import { useTranslation } from "react-i18next";

import GlassButton from "../../ui/GlassButton";
import Tooltip from "../../ui/Tooltip";
import { CATEGORY_COLORS, CATEGORY_LABELS, LICENSE_COLORS } from "./constantes.js";
import { formatChampionshipPosition, roleLabel } from "./agrupamentos.js";

export default function PainelCandidatos({
  candidateGroups,
  groupedOffers,
  playerSpecialOffers,
  isConvocating,
  specialWindowState,
  onAcceptOffer,
}) {
  const { t } = useTranslation();

  return (
    <aside className="glass-strong scroll-area animate-edge-rail-in min-h-0 overflow-y-auto rounded-2xl px-3 py-4 lg:px-4 lg:py-5">
      <div className="mb-5">
        <div className="mb-3 flex h-6 items-center justify-between">
          <p className="text-body-sm font-bold uppercase tracking-[0.22em] text-[color:var(--accent-primary)]">
            {t("convocation.candidates.title")}
          </p>
        </div>

        {candidateGroups.length === 0 ? (
          <div className="py-8 text-center text-body text-[color:var(--text-muted)]">
            {t("convocation.candidates.empty")}
          </div>
        ) : (
          <div className="space-y-4">
            {candidateGroups.map((group) => (
              <section key={group.category}>
                <div className="mb-2 flex items-center gap-2">
                  <span
                    className="text-[9px] font-bold uppercase tracking-[0.2em]"
                    style={{ color: group.color }}
                  >
                    {group.label}
                  </span>
                  <div
                    className="h-px flex-1"
                    style={{
                      background: `linear-gradient(to right, ${group.color}66, transparent)`,
                    }}
                  />
                </div>

                <div className="space-y-2">
                  {group.entries.map((candidate) => {
                    const licenseColors =
                      LICENSE_COLORS[candidate.license_sigla] ?? LICENSE_COLORS.R;

                    return (
                    <article
                      key={candidate.driver_id}
                      className="glass-light rounded-xl border px-3 py-3"
                      style={{
                        borderColor: `${group.color}30`,
                        background: `linear-gradient(180deg, ${group.color}12 0%, rgba(255,255,255,0.03) 100%)`,
                      }}
                    >
                      <div className="flex items-center gap-3">
                        <span
                          className="shrink-0 text-[13px] font-black tracking-[-0.02em]"
                          style={{
                            color: group.color,
                          }}
                        >
                          {formatChampionshipPosition(
                            candidate.championship_position,
                            candidate.championship_total_drivers,
                          ) ?? "—"}
                        </span>
                        <p className="min-w-0 flex-1 truncate text-[15px] font-extrabold leading-[1.05] text-[color:var(--text-primary)]">
                          {candidate.driver_name}
                        </p>
                        <Tooltip texto={candidate.license_nivel}>
                          <span
                            className="shrink-0 rounded-md px-2 py-1 text-[11px] font-bold uppercase tracking-[0.08em]"
                            style={{
                              background: licenseColors.bg,
                              color: licenseColors.text,
                            }}
                          >
                            {candidate.license_sigla}
                          </span>
                        </Tooltip>
                      </div>
                    </article>
                    );
                  })}
                </div>
              </section>
            ))}
          </div>
        )}
      </div>

      <div className="border-t border-white/8 pt-4">
        <div className="mb-4 flex h-6 items-center justify-between">
          <p className="text-body-sm font-bold uppercase tracking-[0.22em] text-[color:var(--accent-primary)]">
            {t("convocation.offers.title")}
          </p>
          <span className="text-body-sm text-[color:var(--text-muted)]">
            {playerSpecialOffers.length}
          </span>
        </div>

        {groupedOffers.length === 0 ? (
          <div className="py-6 text-center text-body text-[color:var(--text-muted)]">
            {t("convocation.offers.empty")}
          </div>
        ) : (
          <div className="space-y-5">
            {groupedOffers.map(([category, offers]) => {
              const color = CATEGORY_COLORS[category] ?? "#58a6ff";
              return (
                <section key={category}>
                  <div className="mb-2 flex items-center gap-2">
                    <span
                      className="text-[9px] font-bold uppercase tracking-[0.2em]"
                      style={{ color }}
                    >
                      {CATEGORY_LABELS[category] ?? category}
                    </span>
                    <div
                      className="h-px flex-1"
                      style={{ background: `linear-gradient(to right, ${color}55, transparent)` }}
                    />
                  </div>

                  <div className="space-y-2">
                    {offers.map((offer) => (
                      <article
                        key={offer.id}
                        className="glass-light rounded-xl border border-white/8 px-3 py-3"
                      >
                        <div className="flex items-center justify-between gap-3">
                          <p className="text-body font-bold text-[color:var(--text-primary)]">
                            {offer.team_name}
                          </p>
                          <span className="text-[10px] font-bold uppercase tracking-[0.08em] text-[color:var(--text-muted)]">
                            {t("convocation.offers.day", { day: offer.available_from_day })}
                          </span>
                        </div>
                        <p className="mt-1 text-body-sm text-[color:var(--text-secondary)]">
                          {t("convocation.offers.className", { class: offer.class_name.toUpperCase() })}
                        </p>
                        <p className="text-body-sm text-[color:var(--text-secondary)]">
                          {roleLabel(offer.papel)}
                        </p>
                        <p className="mt-1 text-[11px] font-semibold uppercase tracking-[0.08em] text-[color:var(--text-muted)]">
                          {offer.status}
                        </p>

                        <div className="mt-3">
                          <GlassButton
                            variant="primary"
                            disabled={isConvocating || !offer.is_available_today || specialWindowState?.is_finished}
                            className="min-h-9 w-full rounded-lg px-3 py-2 text-[11px] font-bold tracking-[0.08em]"
                            onClick={() => onAcceptOffer(offer.id)}
                          >
                            {t("convocation.offers.chooseToday")}
                          </GlassButton>
                        </div>
                      </article>
                    ))}
                  </div>
                </section>
              );
            })}
          </div>
        )}
      </div>
    </aside>
  );
}
