import { useState } from "react";
import { useTranslation } from "react-i18next";
import WeatherTimelineChart from "./WeatherTimelineChart";

/**
 * Botão "Clima" que abre o timeline do clima da corrida num modal.
 * Reutilizável (sala de estratégia = previsão; pós-corrida = revisão com marcações).
 */
export default function WeatherButton({
  careerId,
  raceId,
  markers,
  forecast,
  className,
  onOpen,
  children,
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  if (!raceId) return children || null;

  const handleOpen = () => {
    onOpen?.();
    setOpen(true);
  };

  return (
    <>
      {/* Gatilho: a própria `children` (ex.: card "Condição de Pista") ou o chip padrão. */}
      <button
        type="button"
        onClick={handleOpen}
        className={
          className ||
          (children
            ? "block w-full text-left"
            : "inline-flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-xs font-semibold text-gray-300 transition hover:bg-white/10")
        }
      >
        {children || (
          <>
            <span>☁️</span> {t("weatherButton.trigger")}
          </>
        )}
      </button>

      {open && (
        <div
          className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
          onClick={() => setOpen(false)}
        >
          <div
            className="w-full max-w-4xl rounded-2xl border border-white/10 bg-[#0d1117] p-6 shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="mb-4 flex items-center justify-between">
              <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-[#58a6ff]">
                <span className="mr-2">☁️</span>
                {forecast ? t("weatherButton.forecastTitle") : t("weatherButton.raceTitle")}
              </p>
              <button
                type="button"
                onClick={() => setOpen(false)}
                className="rounded-lg border border-white/10 bg-white/5 px-2.5 py-1 text-sm text-gray-400 transition hover:bg-white/10 hover:text-white"
              >
                ✕
              </button>
            </div>
            <WeatherTimelineChart
              careerId={careerId}
              raceId={raceId}
              markers={markers}
              forecast={forecast}
            />
          </div>
        </div>
      )}
    </>
  );
}
