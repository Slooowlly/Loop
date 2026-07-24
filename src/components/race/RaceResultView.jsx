// i18n-ignore-file — tela V1 APOSENTADA (RaceResultViewV2 é a oficial); código morto,
// não vale traduzir. Se um dia voltar a ser usada, remova esta linha e traduza.
import { useMemo, useState } from "react";
import useCareerStore from "../../stores/useCareerStore";
import CareerReadSection from "./raceresult/CareerReadSection";
import HighlightsColumn from "./raceresult/HighlightsColumn";
import PitTimesTable from "./raceresult/PitTimesTable";
import PositionBreakdownSection from "./raceresult/PositionBreakdownSection";
import RaceAnalysisSection from "./raceresult/RaceAnalysisSection";
import ResultHeader from "./raceresult/ResultHeader";
import ResultsPanel from "./raceresult/ResultsPanel";
import TireStrategySection from "./raceresult/TireStrategySection";
import { computePositionBreakdown } from "./raceresult/helpers";
import { useChampionship } from "./raceresult/useChampionship";

function RaceResultView({ result, evaluation, telemetry, onDismiss }) {
  const careerId = useCareerStore((state) => state.careerId);
  const lastRaceId = useCareerStore((state) => state.lastRaceId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const otherCategoriesResult = useCareerStore((state) => state.otherCategoriesResult);
  // Painel direito: 'results' (tabela oficial) | 'championship' | 'charts'.
  const [rightView, setRightView] = useState("results");
  const hasCharts = !!telemetry?.charts;
  const { championship, teamColors, loadingChampionship, championshipError } = useChampionship(
    careerId,
    playerTeam?.categoria,
  );

  const playerResult = useMemo(
    () => result?.race_results?.find((entry) => entry.is_jogador) ?? null,
    [result],
  );

  const winner = useMemo(
    () => result?.race_results?.find((entry) => entry.finish_position === 1) ?? null,
    [result],
  );

  const poleSitter = useMemo(
    () => result?.qualifying_results?.find((entry) => entry.is_pole) ?? null,
    [result],
  );

  const fastestLap = useMemo(
    () => result?.race_results?.find((entry) => entry.has_fastest_lap) ?? null,
    [result],
  );

  const biggestGainer = useMemo(() => {
    const activeResults = result?.race_results?.filter((entry) => !entry.is_dnf) ?? [];
    if (activeResults.length === 0) return null;
    return activeResults.reduce((best, entry) =>
      entry.positions_gained > best.positions_gained ? entry : best,
    activeResults[0]);
  }, [result]);

  const positionBreakdown = useMemo(
    () => computePositionBreakdown(playerResult, result, telemetry),
    [playerResult, result, telemetry],
  );

  if (!result) return null;

  return (
    <div className="relative z-10 flex h-[calc(100vh-4rem)] w-full flex-col overflow-y-auto custom-scrollbar rounded-[32px] border border-white/5 bg-[#080d14]/40 p-2 animate-fade-in shadow-[0_10px_50px_rgba(0,0,0,0.5)] backdrop-blur-3xl lg:p-4">

      <ResultHeader
        result={result}
        playerResult={playerResult}
        evaluation={evaluation}
        onDismiss={onDismiss}
      />

      <CareerReadSection evaluation={evaluation} playerResult={playerResult} />

      <PositionBreakdownSection positionBreakdown={positionBreakdown} telemetry={telemetry} />

      <RaceAnalysisSection telemetry={telemetry} />

      <TireStrategySection telemetry={telemetry} />

      {/* CONTEÚDO */}
      <div className="grid grid-cols-12 gap-6 min-h-[620px] px-4 pb-4">

        {/* Esquerda: Destaques */}
        <HighlightsColumn
          winner={winner}
          fastestLap={fastestLap}
          poleSitter={poleSitter}
          biggestGainer={biggestGainer}
          otherCategoriesResult={otherCategoriesResult}
        />

        {/* Direita: Tabela de Resultados (100% dinâmica com scroll perfeito) */}
        <ResultsPanel
          result={result}
          telemetry={telemetry}
          teamColors={teamColors}
          championship={championship}
          loadingChampionship={loadingChampionship}
          championshipError={championshipError}
          careerId={careerId}
          lastRaceId={lastRaceId}
          hasCharts={hasCharts}
          rightView={rightView}
          onChangeView={setRightView}
        />

      </div>

      {/* TEMPOS DE PIT — tabela dedicada (ênfase na equipe). Própria, separada da
          estratégia de pneu. Só aparece com paradas capturadas (corrida do iRacing). */}
      <PitTimesTable telemetry={telemetry} teamColors={teamColors} />

    </div>
  );
}

export default RaceResultView;
