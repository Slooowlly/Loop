import { useTranslation } from "react-i18next";
import FreeAgentCard from "./FreeAgentCard";
import { subcatColor, shortDestLabel } from "../preSeasonFormatters.js";

export default function FreeAgentsPanel({
  freeAgentContainerRef,
  freeAgentSectionRefs,
  preseasonFreeAgents,
  selectedCat,
  visibleFreeAgentCount,
  freeAgentBandOrder,
  freeAgentsByBand,
  setHoveredFreeAgentCat,
}) {
  const { t } = useTranslation();
  return (
    <aside ref={freeAgentContainerRef} className="glass-strong scroll-area animate-edge-rail-in min-h-0 overflow-y-auto rounded-2xl px-3 py-4 lg:px-4 lg:py-5">
      <div className="mb-4 flex h-6 items-center justify-between">
        <p className="text-body-sm font-bold uppercase tracking-[0.22em] text-[color:var(--accent-primary)]">
          {t("preSeason.market.title")}
        </p>
        {(preseasonFreeAgents ?? []).length > 0 && (
          <span className="text-body-sm text-[color:var(--text-muted)]">
            {selectedCat === "all"
              ? t("preSeason.market.countFree", { count: visibleFreeAgentCount })
              : t("preSeason.market.countEligible", { count: visibleFreeAgentCount })}
          </span>
        )}
      </div>

      {(preseasonFreeAgents ?? []).length === 0 ? (
        <div className="py-10 text-center text-body text-[color:var(--text-muted)]">
          {t("preSeason.market.emptyAllSigned")}
        </div>
      ) : freeAgentBandOrder.length === 0 ? (
        <div className="py-10 text-center text-body text-[color:var(--text-muted)]">
          {t("preSeason.market.emptyNoneEligible")}
        </div>
      ) : (
        <div className="space-y-4">
          {freeAgentBandOrder.map((band) => {
            const drivers = freeAgentsByBand[band.key];
            return (
              <section key={band.key} ref={(el) => { freeAgentSectionRefs.current[band.key] = el; }}>
                <div className="mb-1.5 flex items-center gap-2">
                  <span
                    className="h-2 w-2 shrink-0 rounded-[3px]"
                    style={{ background: band.color, boxShadow: `0 0 8px ${band.color}88` }}
                  />
                  <span
                    className="text-[10px] font-black uppercase tracking-[0.2em]"
                    style={{ color: band.color }}
                  >
                    {t(`preSeason.bands.${band.key}`)}
                  </span>
                  <div
                    className="h-px flex-1"
                    style={{ background: `linear-gradient(to right, ${band.color}55, transparent)` }}
                  />
                  <span className="text-[9px] text-[color:var(--text-muted)]">
                    {drivers.length}
                  </span>
                </div>
                <div className="space-y-2.5">
                  {(() => {
                    // Sub-agrupa por marca/categoria (drivers já ordenado por marca).
                    // Separador físico entre marcas quando a banda tem mais de uma
                    // (Amador/Pro/Rookie) — senão Toyota e Mazda, de mesma cor, embolam.
                    const groups = [];
                    drivers.forEach((d) => {
                      const last = groups[groups.length - 1];
                      if (last && last.cat === d.categoria) last.list.push(d);
                      else groups.push({ cat: d.categoria, list: [d] });
                    });
                    const multiBrand = groups.length > 1;
                    return groups.map(({ cat, list }) => (
                      <div key={cat} className="space-y-1">
                        {multiBrand && (
                          <div className="flex items-center gap-2 px-0.5 pt-0.5">
                            <span
                              className="text-[8px] font-bold uppercase tracking-[0.18em]"
                              style={{ color: subcatColor(cat) }}
                            >
                              {shortDestLabel(cat)}
                            </span>
                            <div
                              className="h-px flex-1"
                              style={{ background: `linear-gradient(to right, ${subcatColor(cat)}44, transparent)` }}
                            />
                          </div>
                        )}
                        {list.map((d) => (
                          <FreeAgentCard
                            key={d.driver_id}
                            driver={d}
                            onHoverCat={setHoveredFreeAgentCat}
                            isRookie={d.is_rookie}
                          />
                        ))}
                      </div>
                    ));
                  })()}
                </div>
              </section>
            );
          })}
        </div>
      )}
    </aside>
  );
}
