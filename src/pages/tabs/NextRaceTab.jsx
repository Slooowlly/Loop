import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import LoadingOverlay from "../../components/ui/LoadingOverlay";
import IracingTutorialModal from "../../components/iracing/IracingTutorialModal";
import ChampionshipTablePanel from "../../components/race/ChampionshipTablePanel";
import EngineerBriefingPanel from "../../components/race/EngineerBriefingPanel";
import NextRaceEmptyState from "../../components/race/NextRaceEmptyState";
import NextRaceExportToasts from "../../components/race/NextRaceExportToasts";
import NextRacePaintPrompt from "../../components/race/NextRacePaintPrompt";
import NextRaceWindowModePrompt from "../../components/race/NextRaceWindowModePrompt";
import PodiumFavoritesPanel from "../../components/race/PodiumFavoritesPanel";
import NextRaceHeader from "../../components/race/nextrace/NextRaceHeader";
import { getDisplayError } from "../../components/race/nextrace/nextRaceHelpers";
import { useBriefingData } from "../../components/race/nextrace/useBriefingData";
import { useIracingExport } from "../../components/race/nextrace/useIracingExport";
import { usePreRaceAi } from "../../components/race/nextrace/usePreRaceAi";
import useCareerStore from "../../stores/useCareerStore";
import { useAttentionStore } from "../../stores/useAttentionStore";
import { renderTextWithDriverMentions } from "../../utils/driverMentions";
import { isLegacySeasonPhase } from "../../utils/seasonPhases";
import { isPortuguese, localizedAiError } from "../../utils/aiFallback";

function NextRaceTab() {
  const { t } = useTranslation();
  const [error, setError] = useState("");
  const [hasExistingPreseason, setHasExistingPreseason] = useState(false);
  // Piloto realçado ao passar o mouse num nome mencionado no texto do engenheiro: acende
  // o mesmo piloto nos Favoritos e na Tabela do Campeonato.
  const [hoveredDriverId, setHoveredDriverId] = useState(null);

  const player = useCareerStore((state) => state.player);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const nextRace = useCareerStore((state) => state.nextRace);
  const nextRaceBriefing = useCareerStore((state) => state.nextRaceBriefing);
  const preRaceAi = useCareerStore((state) => state.preRaceAi);
  const temporalSummary = useCareerStore((state) => state.temporalSummary);
  const season = useCareerStore((state) => state.season);
  const playerInterests = useCareerStore((state) => state.playerInterests);
  const language = useCareerStore((state) => state.language);
  const isSimulating = useCareerStore((state) => state.isSimulating);
  const isAdvancing = useCareerStore((state) => state.isAdvancing);
  const careerId = useCareerStore((state) => state.careerId);
  const simulateRace = useCareerStore((state) => state.simulateRace);
  const advanceSeason = useCareerStore((state) => state.advanceSeason);
  const skipAllPendingRaces = useCareerStore((state) => state.skipAllPendingRaces);
  const enterPreseason = useCareerStore((state) => state.enterPreseason);
  const runConvocationWindow = useCareerStore((state) => state.runConvocationWindow);
  const finishSpecialBlock = useCareerStore((state) => state.finishSpecialBlock);
  const startCalendarAdvance = useCareerStore((state) => state.startCalendarAdvance);
  const isConvocating = useCareerStore((state) => state.isConvocating);
  const isEnteringPreseason = useCareerStore((state) => state.isEnteringPreseason);
  // Glow de atenção dos cards (Clima, Risco de Quebra): pulsa até o jogador abrir
  // o card NESTA corrida; depois cala. Reaparece sozinho na corrida seguinte.
  // Assino a fatia específica de `seen` pra o card recalcular o glow assim que abre.
  const markAttnSeen = useAttentionStore((state) => state.markSeen);
  const raceId = nextRace?.id;
  const weatherSeen = useAttentionStore(
    (state) => !!raceId && !!state.seen[`${raceId}:weather`]
  );
  const breakdownSeen = useAttentionStore(
    (state) => !!raceId && !!state.seen[`${raceId}:breakdown`]
  );
  const weatherGlow = weatherSeen ? "" : "attn-glow-delayed";
  const breakdownGlow = breakdownSeen ? "" : "attn-glow-delayed";
  const phase = season?.fase;
  const isLegacyPhase = isLegacySeasonPhase(phase);
  const hasPendingRegularRaces =
    isLegacyPhase && phase === "BlocoRegular" && (temporalSummary?.pending_in_phase ?? 0) > 0;

  const {
    briefing,
    isLoadingBriefing,
    breakdownForecast,
    breakdownRiskTeams,
    weekendModifiers,
  } = useBriefingData({
    careerId,
    player,
    playerTeam,
    season,
    nextRace,
    nextRaceBriefing,
    playerInterests,
  });

  const { aiBriefing, aiPending, showTemplate, reportPreRaceEngagement } = usePreRaceAi({
    careerId,
    nextRace,
    preRaceAi,
    briefing,
    isLoadingBriefing,
  });

  const iracing = useIracingExport({ careerId, player, playerTeam, setError });

  useEffect(() => {
    let active = true;

    async function detectPreseason() {
      if (!careerId || nextRace || phase !== "PreTemporada") return;

      try {
        await invoke("get_preseason_state", { careerId });
        if (active) {
          setHasExistingPreseason(true);
          await enterPreseason();
        }
      } catch (_error) {
        if (active) {
          setHasExistingPreseason(false);
        }
      }
    }

    detectPreseason();

    return () => {
      active = false;
    };
  }, [careerId, enterPreseason, nextRace, phase]);

  async function handleSimulate() {
    setError("");
    iracing.setExportNotice("");
    // Registra a leitura desta etapa antes de sair da tela (simular = ação de saída).
    reportPreRaceEngagement();

    try {
      await simulateRace();
    } catch (invokeError) {
      setError(getDisplayError(invokeError, t("nextRaceTab.errors.simulate")));
    }
  }

  async function handleSeasonAdvance() {
    setError("");
    iracing.setExportNotice("");

    try {
      // LEGADO 9D: o bloco especial só segue vivo para saves pré-v33 em voo.
      if (isLegacyPhase && phase === "BlocoEspecial") {
        await finishSpecialBlock();
        return;
      }

      if (!nextRace && hasPendingRegularRaces) {
        await startCalendarAdvance();
        return;
      }

      if (isLegacyPhase && !nextRace && phase === "BlocoRegular") {
        await runConvocationWindow();
        return;
      }

      if (isLegacyPhase && !nextRace && phase === "PosEspecial") {
        await advanceSeason();
        return;
      }

      if (phase === "PreTemporada" || hasExistingPreseason) {
        await enterPreseason();
        return;
      }

      await advanceSeason();
    } catch (invokeError) {
      setError(getDisplayError(invokeError, t("nextRaceTab.errors.advancePreseason")));
    }
  }

  if (!nextRace) {
    const isFreeAgent = !playerTeam;
    return (
      <NextRaceEmptyState
        phase={phase}
        isLegacyPhase={isLegacyPhase}
        isFreeAgent={isFreeAgent}
        hasPendingRegularRaces={hasPendingRegularRaces}
        hasExistingPreseason={hasExistingPreseason}
        busy={{
          open: isAdvancing || isConvocating || isEnteringPreseason,
          isEnteringPreseason,
        }}
        error={error}
        onAdvance={() => void handleSeasonAdvance()}
        onSkipSeason={() => {
          setError("");
          skipAllPendingRaces().catch((e) => {
            setError(getDisplayError(e, t("nextRaceTab.errors.skipSeason")));
          });
        }}
      />
    );
  }

  // Prévia da IA a exibir: o reroll/fetch local (`aiBriefing`) tem prioridade; senão,
  // a versão pré-buscada na animação de avanço (`preRaceAi`), se for desta etapa.
  const prefetchedAi =
    preRaceAi?.raceId && preRaceAi.raceId === nextRace?.id
      ? {
          headline: preRaceAi.headline ?? null,
          narrative: preRaceAi.narrative,
          teamVoice: preRaceAi.teamVoice,
        }
      : null;
  const effectiveAi = aiBriefing ?? prefetchedAi;
  // Exibindo a versão da IA? (há prévia e o debug não forçou o template.)
  const usingAi = Boolean(effectiveAi?.narrative) && !showTemplate;
  // Buscando a IA e ainda sem nada pra mostrar → skeleton (não o template), pra não
  // piscar o template por 1s antes da IA. `showTemplate` (debug) sempre vence.
  const showAiSkeleton = aiPending && !usingAi && !showTemplate;
  // Sem IA e fora do skeleton: em português mostramos o template determinístico; em
  // outro idioma, "erro na geração de texto" (não despejamos PT para quem não é PT).
  const aiFallbackError =
    !usingAi && !showAiSkeleton && !isPortuguese(language) ? localizedAiError(language) : null;
  // Controles de debug da IA (Rerolar / Ver template / badge): só em dev, ou se
  // VITE_AI_DEBUG=true. No build de produção ficam ocultos para o jogador.
  const showAiDebug = import.meta.env.DEV || import.meta.env.VITE_AI_DEBUG === "true";
  // Pilotos cujos nomes o texto do engenheiro pode mencionar (elenco da categoria).
  const mentionDrivers = briefing.championshipTable ?? [];
  const renderNarrative = (text) =>
    renderTextWithDriverMentions(text, mentionDrivers, hoveredDriverId, setHoveredDriverId);

  return (
    <div className="relative min-h-[calc(100vh-100px)]">
      {/* Background glass effect specific to this dashboard */}
      <div className="fixed inset-0 z-0 overflow-hidden pointer-events-none opacity-60">
        <div className="absolute inset-x-0 -top-40 h-[600px] bg-[url('https://images.unsplash.com/photo-1541443131876-44b03de101c5?auto=format&fit=crop&q=80')] bg-cover opacity-15 filter blur-[30px] mix-blend-screen transform scale-110"></div>
        <div className="absolute inset-0 bg-gradient-to-b from-transparent via-[#06090e]/80 to-[#06090e]"></div>
      </div>

      {/* Popup: pegar a cor do carro (1ª corrida da 1ª temporada) */}
      <NextRacePaintPrompt
        open={iracing.showPaintPrompt}
        busy={iracing.paintBusy}
        error={iracing.paintError}
        onConfirm={iracing.handleGrabPaint}
        onCancel={() => {
          iracing.setShowPaintPrompt(false);
          iracing.setPaintError("");
        }}
      />

      {/* Popup: pôr o iRacing em modo janela, logo após exportar a etapa */}
      <NextRaceWindowModePrompt
        open={iracing.showWindowPrompt}
        busy={iracing.windowBusy}
        error={iracing.windowError}
        onConfirm={iracing.handleWindowModeConfirm}
        onCancel={iracing.handleWindowModeSkip}
      />

      {/* Toast de confirmação do modo janela */}
      {iracing.windowToast && (
        <div className="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-xl border border-[#58a6ff44] bg-[#0d1117] px-4 py-2.5 text-sm font-semibold text-white shadow-2xl">
          {iracing.windowToast}
        </div>
      )}

      {/* Toast de confirmação da pintura */}
      {iracing.paintToast && (
        <div className="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-xl border border-[#58a6ff44] bg-[#0d1117] px-4 py-2.5 text-sm font-semibold text-white shadow-2xl">
          {iracing.paintToast}
        </div>
      )}

      <div className="relative z-10 space-y-6">
        <LoadingOverlay
          open={isSimulating}
          title={t("nextRaceTab.loading.simulatingRaceTitle")}
          message={t("nextRaceTab.loading.simulatingRaceMsg")}
        />

        {/* HEADER COM BOTÕES */}
        <NextRaceHeader
          nextRace={nextRace}
          season={season}
          briefing={briefing}
          isSimulating={isSimulating}
          onSimulate={() => void handleSimulate()}
          canPickPaint={iracing.canPickPaint}
          onOpenPaintPrompt={() => {
            iracing.setPaintError("");
            iracing.setShowPaintPrompt(true);
          }}
          isExporting={iracing.isExporting}
          exported={iracing.exported}
          onExport={iracing.handleExport}
        />

        {iracing.exportNotice && <p className="text-right text-sm text-[#58a6ff]">{iracing.exportNotice}</p>}
        {error && <p className="text-right text-sm text-red-500">{error}</p>}

        <NextRaceExportToasts
          exported={iracing.exported}
          showGoToast={iracing.showGoToast}
          iracingFocusMsg={iracing.iracingFocusMsg}
          onGoToIracing={iracing.handleGoToIracing}
        />

        {iracing.showTutorial && (
          <IracingTutorialModal
            onFinish={iracing.handleTutorialDone}
            onClose={iracing.handleTutorialClose}
            message={iracing.tutorialMsg}
          />
        )}

        {/* GRID PRINCIPAL (4-4-4) */}
        <div className="grid grid-cols-1 xl:grid-cols-12 gap-6 items-stretch pb-10">

          {/* 1) NARRATIVA DA ETAPA */}
          <EngineerBriefingPanel
            careerId={careerId}
            raceId={nextRace?.id}
            briefing={briefing}
            breakdownForecast={breakdownForecast}
            weatherGlow={weatherGlow}
            breakdownGlow={breakdownGlow}
            onWeatherOpen={() => markAttnSeen(raceId, "weather")}
            onBreakdownOpen={() => markAttnSeen(raceId, "breakdown")}
            effectiveAi={effectiveAi}
            usingAi={usingAi}
            showAiSkeleton={showAiSkeleton}
            aiFallbackError={aiFallbackError}
            showAiDebug={showAiDebug}
            renderNarrative={renderNarrative}
          />

          {/* 2) METAS HORIZONTAIS E FAVORITOS */}
          <PodiumFavoritesPanel
            goals={briefing.goals}
            favorites={briefing.favorites}
            isLoading={isLoadingBriefing}
            hoveredDriverId={hoveredDriverId}
            contractWarning={nextRaceBriefing?.contract_warning}
            showContractWarning={
              nextRaceBriefing?.contract_warning != null &&
              Math.max(0, (season?.total_rodadas ?? 0) - (nextRace?.rodada ?? 0)) <= 1
            }
          />

          {/* 3) TABELA CAMPEONATO */}
          <ChampionshipTablePanel
            championshipTable={briefing.championshipTable}
            constructorsTable={briefing.constructorsTable}
            playerTeamId={briefing.playerTeamId}
            breakdownRiskTeams={breakdownRiskTeams}
            weekendModifiers={weekendModifiers}
            hoveredDriverId={hoveredDriverId}
          />

        </div>
      </div>
    </div>
  );
}

export default NextRaceTab;
