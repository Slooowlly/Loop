import { useTranslation } from "react-i18next";
import FreeAgentCardV2 from "./FreeAgentCardV2";
import { subcatColor, shortDestLabel } from "../../preSeasonFormatters.js";

// Coluna esquerda do v2. A estrutura (bandas de nível → marcas → pilotos) é a
// mesma do v1 e por bom motivo: ela já resolve o embolamento de Toyota e Mazda,
// que têm a mesma cor. O que muda é o cabeçalho fixo e a ficha de cada piloto.
export default function FreeAgentsPanelV2({
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
    <aside className="glass-strong animate-edge-rail-in flex min-h-0 flex-col rounded-2xl">
      {/* O contador é a manchete da coluna: quantos pilotos estão disponíveis para
          a categoria filtrada. Como rodapé de cabeçalho ele não competia com nada
          e passava batido — aqui ele é o número maior da coluna. */}
      <div className="flex items-center gap-3 border-b border-white/[0.07] px-3.5 py-3 lg:px-4">
        <div className="min-w-0 flex-1">
          <p className="text-[9px] font-black uppercase tracking-[0.22em] text-[color:var(--accent-primary)]">
            {t("preSeason.market.title")}
          </p>
          <p className="mt-0.5 truncate text-[10px] text-[color:var(--text-muted)]">
            {t("preSeason.v2.market.hoverHint")}
          </p>
        </div>
        {(preseasonFreeAgents ?? []).length > 0 && (
          <div className="shrink-0 text-right">
            <p className="text-[26px] font-black leading-none tabular-nums text-[color:var(--text-primary)]">
              {visibleFreeAgentCount}
            </p>
            <p className="mt-0.5 text-[8.5px] font-black uppercase tracking-[0.12em] text-[color:var(--text-muted)]">
              {selectedCat === "all"
                ? t("preSeason.v2.market.countFreeLabel")
                : t("preSeason.v2.market.countEligibleLabel")}
            </p>
          </div>
        )}
      </div>

      <div ref={freeAgentContainerRef} className="scroll-area min-h-0 flex-1 overflow-y-auto px-2.5 py-3 lg:px-3">
        {(preseasonFreeAgents ?? []).length === 0 ? (
          <div className="py-10 text-center text-body text-[color:var(--text-muted)]">
            {t("preSeason.market.emptyAllSigned")}
          </div>
        ) : freeAgentBandOrder.length === 0 ? (
          <div className="py-10 text-center text-body text-[color:var(--text-muted)]">
            {t("preSeason.market.emptyNoneEligible")}
          </div>
        ) : (
          <div className="space-y-3.5">
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
                    <span className="text-[9px] font-bold tabular-nums text-[color:var(--text-muted)]">
                      {drivers.length}
                    </span>
                  </div>
                  <div className="space-y-2">
                    {(() => {
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
                              <div
                                className="h-px flex-1"
                                style={{ background: `linear-gradient(to right, transparent, ${subcatColor(cat)}55)` }}
                              />
                              <span
                                className="text-[9px] font-black uppercase tracking-[0.18em]"
                                style={{ color: subcatColor(cat) }}
                              >
                                {shortDestLabel(cat)}
                              </span>
                              <div
                                className="h-px flex-1"
                                style={{ background: `linear-gradient(to left, transparent, ${subcatColor(cat)}55)` }}
                              />
                            </div>
                          )}
                          {list.map((d) => (
                            <FreeAgentCardV2
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
      </div>
    </aside>
  );
}
