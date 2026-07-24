import { Fragment, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";

import { CATEGORY_SERIES, CATEGORY_TIER_LABEL } from "./standingsLadder";
import { categoryLabel } from "../../utils/formatters";

// Cabeçalho da classificação: escolhe a SÉRIE (linha de carro) por um dropdown
// agrupado por acesso e o TIER dentro dela pelas setas ▲▼. O estado do menu é local
// — a aba só precisa saber qual série/tier foi escolhido.
function SeriesNavigator({
  currentSeries,
  viewCategory,
  navLocked,
  hasTierAbove,
  hasTierBelow,
  driverCount,
  onSelectSeries,
  onTierUp,
  onTierDown,
}) {
  const { t } = useTranslation();
  const triggerRef = useRef(null);
  const menuRef = useRef(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuPos, setMenuPos] = useState(null);

  function toggleMenu() {
    if (navLocked) return;
    if (menuOpen) {
      setMenuOpen(false);
      return;
    }
    const rect = triggerRef.current?.getBoundingClientRect();
    setMenuPos(
      rect ? { top: rect.bottom + 8, left: rect.left, minWidth: rect.width } : { top: 0, left: 0, minWidth: 0 },
    );
    setMenuOpen(true);
  }

  function selectSeries(nextSeriesIndex) {
    onSelectSeries(nextSeriesIndex);
    setMenuOpen(false);
  }

  // Fecha o menu de linha ao clicar fora, com Escape, ou se a página rolar/mudar
  // de tamanho (a posição é fixa, capturada na abertura, então rolar invalida).
  useEffect(() => {
    if (!menuOpen) return undefined;
    function onPointerDown(event) {
      if (triggerRef.current?.contains(event.target) || menuRef.current?.contains(event.target)) {
        return;
      }
      setMenuOpen(false);
    }
    function onKeyDown(event) {
      if (event.key === "Escape") setMenuOpen(false);
    }
    function dismiss() {
      setMenuOpen(false);
    }
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", dismiss);
    window.addEventListener("scroll", dismiss, true);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", dismiss);
      window.removeEventListener("scroll", dismiss, true);
    };
  }, [menuOpen]);

  return (
    <div className="flex items-center justify-between gap-4">
      <div>
        {/* Série (linha de carro): dropdown agrupado troca de linha. */}
        <button
          type="button"
          ref={triggerRef}
          onClick={toggleMenu}
          disabled={navLocked}
          aria-haspopup="listbox"
          aria-expanded={menuOpen}
          className="flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-[0.22em] text-accent-primary transition-colors hover:enabled:text-text-primary disabled:cursor-default disabled:opacity-70"
          title={t("standings.series.changeLine")}
        >
          <span className="kfx whitespace-nowrap">{currentSeries.label}</span>
          {!navLocked ? (
            <span
              className={[
                "text-[9px] text-text-muted transition-transform",
                menuOpen ? "rotate-180" : "",
              ].join(" ")}
            >
              ▾
            </span>
          ) : null}
        </button>
        {menuOpen && menuPos && !navLocked
          ? createPortal(
              <div
                ref={menuRef}
                role="listbox"
                aria-label={t("standings.series.carLine")}
                className="fixed z-[120] rounded-2xl border border-white/10 bg-[#161b22]/95 p-1.5 shadow-[0_16px_48px_rgba(0,0,0,0.55)] backdrop-blur-md"
                style={{
                  top: menuPos.top,
                  left: menuPos.left,
                  minWidth: Math.max(menuPos.minWidth, 224),
                }}
              >
                {/* Ordem invertida: topo = ápice (LMP2), base = entrada
                    (Mazda), pra deixar clara a "subida" da escada. Os dois
                    grupos de acesso (premium em cima, free embaixo) ganham
                    um cabeçalho e uma separação física entre eles. */}
                {CATEGORY_SERIES.slice().reverse().map((serie, displayIndex, ordered) => {
                  const index = CATEGORY_SERIES.indexOf(serie);
                  const isCurrent = serie.id === currentSeries.id;
                  const previous = ordered[displayIndex - 1];
                  const startsGroup = !previous || previous.access !== serie.access;
                  return (
                    <Fragment key={serie.id}>
                      {startsGroup ? (
                        <div
                          className={[
                            "flex items-center gap-2 px-3 pb-1.5",
                            displayIndex === 0 ? "pt-1" : "mt-1.5 border-t border-white/10 pt-2.5",
                          ].join(" ")}
                        >
                          <span
                            className={[
                              "text-[9px] font-semibold uppercase tracking-[0.22em]",
                              serie.access === "elite" ? "text-accent-primary" : "text-text-muted",
                            ].join(" ")}
                          >
                            {serie.access === "elite" ? "Elite" : "Pro"}
                          </span>
                        </div>
                      ) : null}
                      <button
                        type="button"
                        role="option"
                        aria-selected={isCurrent}
                        onClick={() => selectSeries(index)}
                        className={[
                          "block w-full rounded-xl px-3 py-2 text-left transition-glass",
                          isCurrent ? "bg-accent-primary/15" : "hover:bg-white/5",
                        ].join(" ")}
                      >
                        <span
                          className={[
                            "block text-xs font-semibold uppercase tracking-[0.12em]",
                            isCurrent ? "text-accent-primary" : "text-text-primary",
                          ].join(" ")}
                        >
                          {serie.label}
                        </span>
                        <span className="mt-0.5 block text-[10px] leading-relaxed text-text-muted">
                          {serie.categories.map((cat, catIndex) => (
                            <Fragment key={cat}>
                              {catIndex > 0 ? " · " : ""}
                              <span className={isCurrent && cat === viewCategory ? "text-accent-primary" : ""}>
                                {CATEGORY_TIER_LABEL[cat] ?? cat}
                              </span>
                            </Fragment>
                          ))}
                        </span>
                      </button>
                    </Fragment>
                  );
                })}
              </div>,
              document.body,
            )
          : null}
        {/* Tier dentro da série: setas verticais sobem/descem o nível. */}
        <div className="mt-1.5 flex items-center gap-2">
          <div className="flex flex-col">
            <button
              type="button"
              onClick={onTierUp}
              disabled={!hasTierAbove}
              className="text-[10px] leading-[1.1] transition-colors disabled:cursor-default disabled:opacity-20 text-text-muted hover:enabled:text-text-primary"
              title={t("standings.series.tierUp")}
            >
              ▲
            </button>
            <button
              type="button"
              onClick={onTierDown}
              disabled={!hasTierBelow}
              className="text-[10px] leading-[1.1] transition-colors disabled:cursor-default disabled:opacity-20 text-text-muted hover:enabled:text-text-primary"
              title={t("standings.series.tierDown")}
            >
              ▼
            </button>
          </div>
          <h2 className="kfx text-2xl font-semibold text-text-primary">
            {CATEGORY_TIER_LABEL[viewCategory] ?? categoryLabel(viewCategory)}
          </h2>
        </div>
      </div>
      {/* i18n-ignore */}
      <p className="text-sm text-text-secondary">{driverCount} pilotos</p>
    </div>
  );
}

export default SeriesNavigator;
