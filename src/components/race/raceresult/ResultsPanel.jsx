// i18n-ignore-file — tela V1 APOSENTADA (RaceResultViewV2 é a oficial); código morto,
// não vale traduzir. Se um dia voltar a ser usada, remova esta linha e traduza.
import RaceCharts from "../RaceCharts";
import ChampionshipStandingsTable from "./ChampionshipStandingsTable";
import OfficialResultsTable from "./OfficialResultsTable";
import WeatherPanel from "./WeatherPanel";
import { PanelTab } from "./primitives";

// Painel direito com abas: Resultados / Campeonato / Gráficos / Clima.
function ResultsPanel({
  result,
  telemetry,
  teamColors,
  championship,
  loadingChampionship,
  championshipError,
  careerId,
  lastRaceId,
  hasCharts,
  rightView,
  onChangeView,
}) {
  return (
    <div className="col-span-12 lg:col-span-9 rounded-3xl p-6 overflow-hidden flex flex-col bg-[#060a10] border border-white/5 shadow-inner relative">

         {/* Gradient glow interno no topo para suavizar */}
         <div className="absolute top-0 left-0 right-0 h-16 bg-gradient-to-b from-[#58a6ff]/5 to-transparent pointer-events-none"></div>

         <div className="flex justify-between items-center mb-4 border-b border-white/10 pb-4 shrink-0 px-2 relative z-10">
             <h3 className="text-sm font-bold text-white uppercase tracking-widest opacity-90 drop-shadow-sm">
                 {rightView === "championship"
                   ? "Classificação Geral do Campeonato"
                   : rightView === "charts"
                     ? "Gráficos da Corrida"
                     : rightView === "clima"
                       ? "Clima da Corrida"
                       : "Tabela Oficial da Prova"}
             </h3>
             <div className="flex items-center gap-1.5 rounded-xl border border-white/10 bg-white/5 p-1">
                 <PanelTab active={rightView === "results"} onClick={() => onChangeView("results")}>Resultados</PanelTab>
                 <PanelTab active={rightView === "championship"} onClick={() => onChangeView("championship")}>Campeonato</PanelTab>
                 {hasCharts && (
                   <PanelTab active={rightView === "charts"} onClick={() => onChangeView("charts")}>Gráficos</PanelTab>
                 )}
                 {lastRaceId && (
                   <PanelTab active={rightView === "clima"} onClick={() => onChangeView("clima")}>Clima</PanelTab>
                 )}
             </div>
         </div>

         <div className="flex-1 overflow-y-auto custom-scrollbar pr-2 relative z-10">
             {rightView === "clima" ? (
                 <WeatherPanel
                     careerId={careerId}
                     raceId={lastRaceId}
                     result={result}
                     telemetry={telemetry}
                 />
             ) : rightView === "charts" ? (
                 <div className="animate-fade-in pr-2">
                     <RaceCharts
                         charts={telemetry.charts}
                         mistakeLap={telemetry?.mistake?.lap ?? 0}
                         bestMomentLap={telemetry?.best_moment?.lap ?? 0}
                     />
                 </div>
             ) : rightView === "championship" ? (
                 <ChampionshipStandingsTable
                     championship={championship}
                     loading={loadingChampionship}
                     error={championshipError}
                 />
             ) : (
                 <OfficialResultsTable result={result} teamColors={teamColors} />
             )}
         </div>
    </div>
  );
}

export default ResultsPanel;
