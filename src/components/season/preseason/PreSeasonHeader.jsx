import { useTranslation } from "react-i18next";
import { CATEGORIES } from "../preSeasonFormatters.js";

// As duas semanas de abertura têm nome próprio: a janela não é uma fila de nove
// semanas iguais, e o jogador precisa saber por que a 1 e a 2 não contratam ninguém.
function openingStageKey(currentWeek, signingsStartWeek) {
  if (currentWeek >= signingsStartWeek) return null;
  return currentWeek <= 1 ? "snapshot" : "departures";
}

export default function PreSeasonHeader({
  isComplete,
  isMarketOpen,
  playerOffers,
  playerProposals,
  selectedCat,
  setSelectedCat,
  currentWeek,
  totalWeeks,
  signingsStartWeek,
  interestForecast,
  weekProgress,
  currentDateLabel,
  isAdvancingWeek,
  handleAdvanceWeek,
  startError,
}) {
  const { t } = useTranslation();
  const stageKey = openingStageKey(currentWeek, signingsStartWeek);
  // A faixa da expectativa vira uma frase só quando os dois extremos coincidem.
  const forecastLabel = interestForecast
    ? interestForecast.min === interestForecast.max
      ? t("preSeason.forecast.exact", { count: interestForecast.max })
      : t("preSeason.forecast.range", {
          min: interestForecast.min,
          max: interestForecast.max,
        })
    : null;
  return (
    <header className="glass-strong animate-fade-in mb-3 rounded-2xl px-5 py-2 lg:px-6">
      <div className="grid items-start gap-3 lg:grid-cols-[1fr_auto]">

        {/* Título + filtros */}
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <p className="text-body-sm font-bold uppercase tracking-[0.28em] text-[color:var(--accent-primary)]">
              {t("preSeason.header.eyebrow")}
            </p>
            {/* O contador é um atalho para a lista, então some junto com ela nas semanas
                de abertura — lá o número mora no painel, sem levar a ficha nenhuma. */}
            {playerOffers.length > 0 && !stageKey && (
              <span className="glass-light rounded-full px-2.5 py-1 text-body-sm font-bold tracking-[0.14em] text-[color:var(--accent-primary)]">
                {t("preSeason.header.offerCount", { count: playerOffers.length })}
              </span>
            )}
            {stageKey && forecastLabel && (
              <span className="glass-light rounded-full px-2.5 py-1 text-body-sm font-semibold text-[color:var(--text-secondary)]">
                {forecastLabel}
              </span>
            )}
          </div>
          <h1 className="mt-1 text-[20px] font-bold leading-[1.05] tracking-[-0.02em] text-[color:var(--text-primary)] lg:text-[26px]">
            {isComplete
              ? t("preSeason.header.titleClosed")
              : stageKey
                ? t(`preSeason.stage.${stageKey}.title`)
                : t("preSeason.header.titleOpen")}
          </h1>
          {stageKey && !isComplete && (
            <p className="mt-1 text-body-sm text-[color:var(--text-secondary)]">
              {t(`preSeason.stage.${stageKey}.subtitle`)}
            </p>
          )}

          {/* Filtros de categoria */}
          <div className="mt-2 max-w-full overflow-x-auto">
            <div className="glass inline-flex w-fit items-center gap-0.5 whitespace-nowrap rounded-full p-1">
              {CATEGORIES.map((cat, i) => {
                if (cat.isSeparator) {
                  return <span key={i} className="mx-1 h-4 w-px bg-white/10" />;
                }
                const active = selectedCat === cat.id;
                return (
                  <button
                    key={cat.id}
                    onClick={() => setSelectedCat(cat.id)}
                    className={`transition-glass cursor-pointer rounded-full border px-2.5 py-1 text-body-sm font-semibold ${
                      active
                        ? "border-white/30 bg-white/[0.14] text-[color:var(--accent-primary)]"
                        : "border-transparent bg-white/[0.03] text-[color:var(--text-secondary)] hover:bg-white/[0.08] hover:text-[color:var(--text-primary)]"
                    }`}
                  >
                    <span
                      className="mr-2 inline-block h-1.5 w-1.5 rounded-full"
                      style={{ backgroundColor: cat.color }}
                    />
                    {cat.id === "all" ? t("preSeason.filters.all") : cat.label}
                  </button>
                );
              })}
            </div>
          </div>
        </div>

        {/* Status + semana + botão */}
        <div className="flex items-center gap-3 self-center lg:justify-self-end">
          <span
            className={`shrink-0 rounded-full border px-2.5 py-1 text-body-sm font-bold uppercase tracking-[0.14em] ${
              isMarketOpen
                ? "border-[#3fb95066] bg-[#3fb9501a] text-[color:var(--status-green)]"
                : "border-[#d2992266] bg-[#d2992218] text-[color:var(--status-yellow)]"
            }`}
          >
            {isMarketOpen ? t("preSeason.header.marketOpen") : t("preSeason.header.marketClosed")}
          </span>

          <div className="w-[220px] px-1 lg:w-[280px]">
            <div className="mb-1 flex items-center justify-between gap-2">
              <p className="text-body-sm font-bold uppercase tracking-[0.2em] text-[color:var(--text-secondary)]">
                {t("preSeason.header.week")}{" "}
                <span className="text-[color:var(--text-primary)]">{currentWeek}</span>
                /{totalWeeks}
              </p>
              <p className="text-body-sm text-[color:var(--text-secondary)]">{currentDateLabel}</p>
            </div>
            <div className="h-[3px] w-full rounded-full bg-[#2a3240]">
              <div
                className="h-full rounded-full bg-[color:var(--accent-primary)] transition-all duration-500"
                style={{ width: `${weekProgress}%` }}
              />
            </div>
          </div>

          <button
            onClick={handleAdvanceWeek}
            disabled={isAdvancingWeek || (isComplete && playerProposals.length > 0)}
            className={`transition-glass rounded-full border px-6 py-2.5 text-body-lg font-bold uppercase tracking-[0.16em] disabled:cursor-not-allowed disabled:opacity-50 ${
              isComplete
                ? "border-[#3fb95099] bg-[#3fb950] text-[#06101f] hover:bg-[#52d16a]"
                : "glow-blue border-[#58a6ff99] bg-[#58a6ff] text-[#06101f] hover:bg-[#79b8ff]"
            }`}
          >
            {isAdvancingWeek
              ? t("preSeason.actions.processing")
              : isComplete
                ? t("preSeason.actions.startSeason")
                : stageKey
                  ? t(`preSeason.stage.${stageKey}.action`)
                  : t("preSeason.actions.advanceWeek")}
          </button>
        </div>
      </div>
      {startError && (
        <p className="mt-2 text-center text-body-sm text-[color:var(--status-red)]">{startError}</p>
      )}
    </header>
  );
}
