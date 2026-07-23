import { useState } from "react";
import { useTranslation } from "react-i18next";

/**
 * Card "Risco de Quebra" da Sala de Estratégia. Espelha o WeatherButton: um card
 * COMPACTO e clicável (cabeçalho + medidor de consequência) que abre um modal com
 * o detalhamento — ponto fraco, lista de peças e a dica de poupar.
 *
 * A cor encoda CONSEQUÊNCIA, não probabilidade crua: confiável / custa tempo /
 * pode abandonar. O número que saturava (`any_prob` ~100%) sai de cena.
 */

const TIER_COLOR = {
  pode_abandonar: "#f87171",
  custa_tempo: "#f0b37a",
  confiavel: "#34d399",
};
const TIER_LABEL_KEY = {
  pode_abandonar: "breakdownConsequenceDnf",
  custa_tempo: "breakdownConsequenceCostsTime",
  confiavel: "breakdownConsequenceReliable",
};

// Nível geral → cor, rótulo e posição do marcador no medidor (0–100%).
const OVERALL = {
  alto: { color: "#f87171", labelKey: "breakdownLevelHigh", marker: 84 },
  médio: { color: "#f0b37a", labelKey: "breakdownLevelMedium", marker: 55 },
  baixo: { color: "#34d399", labelKey: "breakdownLevelLow", marker: 20 },
};
function overallOf(level) {
  return OVERALL[level] || OVERALL.baixo;
}

/** Medidor de 3 zonas (confiável / custa tempo / risco de abandono) + marcador. */
function RiskMeter({ marker, t }) {
  return (
    <div>
      <div className="flex h-[9px] overflow-hidden rounded-[5px] bg-[#0d1117]">
        <div style={{ width: "45%", background: "#34d399" }} />
        <div style={{ width: "25%", background: "#f0b37a" }} />
        <div style={{ width: "30%", background: "#f87171" }} />
      </div>
      <div className="relative h-3.5">
        <div
          className="absolute top-0 h-[11px] w-0.5 bg-[#f5f5f5]"
          style={{ left: `${marker}%`, transform: "translateX(-50%)" }}
        />
      </div>
      <div className="flex justify-between text-[10px] text-[#6e7681]">
        <span>{t("nextRaceTab.labels.breakdownMeterReliable")}</span>
        <span>{t("nextRaceTab.labels.breakdownMeterCostsTime")}</span>
        <span>{t("nextRaceTab.labels.breakdownMeterDnf")}</span>
      </div>
    </div>
  );
}

export default function BreakdownRiskButton({
  forecast,
  className,
  onOpen,
  children,
}) {
  const { t } = useTranslation("common");
  const [open, setOpen] = useState(false);
  if (!forecast?.available) return children || null;

  const overall = overallOf(forecast.overall_level);
  const parts = forecast.parts ?? [];
  // Ponto fraco = a peça mais perigosa (o backend já manda a lista ordenada), e só
  // vira destaque quando de fato preocupa (não é "confiável").
  const weakest = parts.find((p) => p.consequencia !== "confiavel");
  const tierColor = (c) => TIER_COLOR[c] || TIER_COLOR.confiavel;
  const tierLabel = (c) =>
    t(`nextRaceTab.labels.${TIER_LABEL_KEY[c] || TIER_LABEL_KEY.confiavel}`);

  const handleOpen = () => {
    onOpen?.();
    setOpen(true);
  };

  return (
    <>
      {/* Gatilho: card compacto (cabeçalho + medidor). */}
      <button
        type="button"
        onClick={handleOpen}
        className={
          className ||
          "group w-full text-left bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-5 transition hover:border-[#58a6ff]/40"
        }
      >
        <div className="mb-3.5 flex items-center justify-between">
          <span className="flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-widest text-[#58a6ff]">
            {t("nextRaceTab.labels.breakdownRisk")}
            <span className="opacity-60 group-hover:opacity-100">›</span>
          </span>
          <span
            className="text-xs font-bold uppercase"
            style={{ color: overall.color }}
          >
            {t(`nextRaceTab.labels.${overall.labelKey}`)}
          </span>
        </div>
        <RiskMeter marker={overall.marker} t={t} />
      </button>

      {open && (
        <div
          className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
          onClick={() => setOpen(false)}
        >
          <div
            className="w-full max-w-xl rounded-2xl border border-white/10 bg-[#161b22] p-6 shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="mb-3.5 flex items-center justify-between">
              <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-[#58a6ff]">
                {t("nextRaceTab.labels.breakdownRisk")}
              </p>
              <div className="flex items-center gap-3">
                <span
                  className="text-xs font-bold uppercase"
                  style={{ color: overall.color }}
                >
                  {t(`nextRaceTab.labels.${overall.labelKey}`)}
                </span>
                <button
                  type="button"
                  onClick={() => setOpen(false)}
                  className="rounded-lg border border-white/10 bg-white/5 px-2.5 py-1 text-sm text-gray-400 transition hover:bg-white/10 hover:text-white"
                >
                  ✕
                </button>
              </div>
            </div>

            <div className="mb-4">
              <RiskMeter marker={overall.marker} t={t} />
            </div>

            {weakest && (
              <div
                className="mb-3.5 flex items-center justify-between rounded-xl px-3.5 py-3"
                style={{
                  background: `${tierColor(weakest.consequencia)}17`,
                  border: `1px solid ${tierColor(weakest.consequencia)}59`,
                }}
              >
                <div className="flex items-center gap-2.5">
                  <span
                    className="text-lg"
                    style={{ color: tierColor(weakest.consequencia) }}
                  >
                    ⚙️
                  </span>
                  <div>
                    <div
                      className="text-[10px] font-bold uppercase tracking-wider"
                      style={{ color: tierColor(weakest.consequencia) }}
                    >
                      {t("nextRaceTab.labels.breakdownWeakPoint")}
                    </div>
                    <div className="text-[15px] font-semibold text-[#f0f6fc]">
                      {weakest.part_name}
                    </div>
                  </div>
                </div>
                <span
                  className="rounded-lg px-2.5 py-1.5 text-xs font-semibold"
                  style={{
                    color: tierColor(weakest.consequencia),
                    background: `${tierColor(weakest.consequencia)}24`,
                  }}
                >
                  {tierLabel(weakest.consequencia)}
                </span>
              </div>
            )}

            <div className="flex flex-col gap-2">
              {parts.map((p) => {
                const dim = p.consequencia === "confiavel";
                return (
                  <div
                    key={p.part}
                    className="flex items-center gap-2.5 text-[13px]"
                    style={{ opacity: dim ? 0.7 : 1 }}
                  >
                    <span
                      className="h-2 w-2 flex-shrink-0 rounded-full"
                      style={{ background: tierColor(p.consequencia) }}
                    />
                    <span className="flex-1 text-[#c9d1d9]">{p.part_name}</span>
                    <span
                      className="font-semibold"
                      style={{ color: tierColor(p.consequencia) }}
                    >
                      {tierLabel(p.consequencia)}
                    </span>
                  </div>
                );
              })}
            </div>

            {weakest && weakest.consequencia === "pode_abandonar" && (
              <div className="mt-3.5 flex items-center gap-1.5 border-t border-white/5 pt-3 text-xs text-[#8b949e]">
                <span className="text-[#f5c76d]">💡</span>
                {t("nextRaceTab.labels.breakdownSpareHint", {
                  part: weakest.part_name.toLowerCase(),
                })}
              </div>
            )}
          </div>
        </div>
      )}
    </>
  );
}
