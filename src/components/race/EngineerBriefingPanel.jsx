import { useTranslation } from "react-i18next";

import BreakdownRiskButton from "./BreakdownRiskButton";
import EventInterestCard from "./EventInterestCard";
import WeatherButton from "./WeatherButton";
import WeekendReadingPanel from "./WeekendReadingPanel";
import { getReadableTeamColor } from "./raceGridContext";

// Coluna 1 da Sala de Estratégia: condições da pista, aviso de risco de quebra e a
// narrativa do engenheiro (versão da IA, esqueleto enquanto ela vem, ou o template).
function EngineerBriefingPanel({
  careerId,
  raceId,
  briefing,
  breakdownForecast,
  // Leitura do fim de semana (fase 3). `null`/ausente = o motor ainda não fornece.
  weekendReading,
  weatherGlow,
  breakdownGlow,
  onWeatherOpen,
  onBreakdownOpen,
  effectiveAi,
  usingAi,
  showAiSkeleton,
  aiFallbackError,
  showAiDebug,
  renderNarrative,
}) {
  const { t } = useTranslation();

  return (
    <div className="xl:col-span-4 flex flex-col gap-5 xl:h-[calc(100vh-17rem)] xl:min-h-[650px]">
      {/* Condições Compactas — o CARD INTEIRO abre a previsão do tempo. */}
      <WeatherButton
        careerId={careerId}
        raceId={raceId}
        forecast
        onOpen={onWeatherOpen}
        className={`group ${weatherGlow} w-full text-left bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-5 flex justify-between items-center bg-gradient-to-r from-black/40 to-transparent transition hover:border-[#58a6ff]/40`}
      >
        <div className="flex items-center gap-4">
          <div className="text-4xl">{briefing.weatherIcon}</div>
          <div>
            <p className="text-[10px] uppercase tracking-widest text-[#58a6ff] font-bold">
              {t("nextRaceTab.labels.trackCondition")} <span className="opacity-60 group-hover:opacity-100">›</span>
            </p>
            <p className="text-xl font-bold text-white">
              {briefing.weatherSummary} <span className="text-xs text-gray-400">{briefing.trackTemperatureLabel}</span>
            </p>
            <p className="text-[10px] text-gray-500 group-hover:text-[#58a6ff] transition">
              {t("nextRaceTab.labels.seeForecast")}
            </p>
          </div>
        </div>
      </WeatherButton>

      {/* Interesse do evento (F-07). O público SAIU do canto direito do card de
          clima e virou card próprio: pendurado ali ele era um número sem tier, sem
          escala e sem relação com o jogador, dentro de um botão que abre a previsão
          do tempo — dois assuntos disputando a mesma caixa. Repetir o número nos
          dois lugares diria a mesma coisa duas vezes, então o clima ficou só com o
          clima. */}
      <EventInterestCard
        interestLabel={briefing.interestLabel}
        audienceEstimate={briefing.audienceEstimate}
        audienceRankLabel={briefing.audienceRankLabel}
        fameSharePct={briefing.fameSharePct}
      />

      {/* Risco de quebra (aviso pré-corrida) — card compacto pulsante que abre o
          detalhamento; o glow cala depois de aberto nesta corrida. */}
      <BreakdownRiskButton
        forecast={breakdownForecast}
        onOpen={onBreakdownOpen}
        className={`group ${breakdownGlow} w-full text-left bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-5 transition hover:border-[#58a6ff]/40`}
      />

      {/* Leitura do fim de semana (fase 3) — o TERCEIRO card de condições, junto do clima
          e do risco de quebra, porque é do mesmo gênero: o que este fim de semana reserva.
          Fica ANTES da narrativa de propósito — o engenheiro comenta em cima dela.
          Renderiza `null` enquanto o motor não fornece a leitura, então não muda a tela
          até o fio ser ligado. */}
      <WeekendReadingPanel reading={weekendReading} />

      {/* Narrativa Expandida */}
      <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-6 flex-1 flex flex-col relative overflow-hidden">
        <div className="absolute -right-10 -top-10 h-40 w-40 rounded-full bg-[radial-gradient(circle,rgba(240,195,107,0.1),transparent_65%)] pointer-events-none"></div>
        <div className="flex items-center justify-between mb-4 relative z-10">
          <p className="text-[11px] uppercase tracking-[0.2em] text-[#f5c76d] font-bold flex items-center">
            <span className="mr-2 text-sm">🎧</span>{t("nextRaceTab.labels.trackEngineer")}
            {showAiDebug && effectiveAi ? (
              usingAi ? (
                <span className="ml-2 px-1.5 py-0.5 rounded text-[9px] tracking-normal bg-[#58a6ff]/15 text-[#58a6ff] border border-[#58a6ff]/30">
                  {t("nextRaceTab.debug.aiBadge")}
                </span>
              ) : (
                <span className="ml-2 px-1.5 py-0.5 rounded text-[9px] tracking-normal bg-white/[0.06] text-gray-400 border border-white/10">
                  {t("nextRaceTab.debug.templateBadge")}
                </span>
              )
            ) : null}
          </p>
        </div>

        <div className="flex-1 overflow-y-auto custom-scrollbar pr-2 relative z-10 flex flex-col">
          {showAiSkeleton ? (
            <div className="space-y-5" aria-hidden="true">
              <div className="h-7 w-3/4 rounded-lg bg-white/[0.07] animate-pulse" />
              <div className="space-y-2.5">
                <div className="h-4 w-full rounded bg-white/[0.05] animate-pulse" />
                <div className="h-4 w-[94%] rounded bg-white/[0.05] animate-pulse" />
                <div className="h-4 w-[86%] rounded bg-white/[0.05] animate-pulse" />
              </div>
              <div className="space-y-2.5">
                <div className="h-4 w-[96%] rounded bg-white/[0.05] animate-pulse" />
                <div className="h-4 w-[68%] rounded bg-white/[0.05] animate-pulse" />
              </div>
            </div>
          ) : usingAi ? (
            <>
              {effectiveAi.headline ? (
                <h3 className="text-2xl font-bold text-white leading-snug mb-4">{effectiveAi.headline}</h3>
              ) : null}
              {effectiveAi.narrative
                .split(/\n{2,}/)
                .map((para) => para.trim())
                .filter(Boolean)
                .map((para, index) => (
                  <p key={index} className="text-[15px] text-gray-300 leading-relaxed mb-4">
                    {renderNarrative(para)}
                  </p>
                ))}
            </>
          ) : aiFallbackError ? (
            <p className="text-[15px] text-gray-500 italic leading-relaxed">{aiFallbackError}</p>
          ) : (
            <>
              <h3 className="text-2xl font-bold text-white leading-snug mb-4">{briefing.headline}</h3>
              <p className="text-[15px] text-gray-300 leading-relaxed mb-4">
                {renderNarrative(briefing.paragraphs[0] ?? briefing.attendanceNarrative)}
              </p>
              <p className="text-[15px] text-gray-300 leading-relaxed mb-6">
                {renderNarrative(briefing.paragraphs[1] || briefing.actionHint)}
              </p>
            </>
          )}

          {/* Leitura de Box Expandida */}
          <div className="bg-black/30 border border-white/5 p-4 rounded-2xl relative mt-auto">
            <div className="absolute top-2 right-4 text-[#58a6ff] opacity-20 pointer-events-none">
              <span className="text-6xl font-serif leading-none h-[40px] block overflow-hidden">"</span>
            </div>
            <p className="text-[10px] uppercase tracking-[0.15em] text-[#58a6ff] mb-2 font-bold">
              {t("nextRaceTab.labels.teamVoice")} <span className="text-gray-500 font-semibold normal-case tracking-normal">{t("nextRaceTab.labels.toPress")}</span>
            </p>
            {showAiSkeleton ? (
              <div className="space-y-2" aria-hidden="true">
                <div className="h-3.5 w-full rounded bg-white/[0.05] animate-pulse" />
                <div className="h-3.5 w-[72%] rounded bg-white/[0.05] animate-pulse" />
              </div>
            ) : aiFallbackError ? (
              <p className="text-sm italic text-gray-500 leading-relaxed">{aiFallbackError}</p>
            ) : (
              <p className="text-sm italic text-gray-200 leading-relaxed">"{renderNarrative(usingAi ? effectiveAi.teamVoice : briefing.quote)}"</p>
            )}
            <p className="text-xs font-semibold text-gray-400 mt-3 text-right">
              -{" "}
              <span style={briefing.teamColor ? { color: getReadableTeamColor(briefing.teamColor) } : undefined}>
                {briefing.teamVoiceLabel}
              </span>
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}

export default EngineerBriefingPanel;
