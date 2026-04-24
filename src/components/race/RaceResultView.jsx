import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import TeamLogoMark from "../team/TeamLogoMark";
import useCareerStore from "../../stores/useCareerStore";
import { formatGap, formatLapTime } from "../../utils/formatters";

const CATEGORY_SUMMARY_LOGOS = {
  mazda: "/categorias/recortadas/MX5%20CUP.png",
  mazda_amador: "/categorias/recortadas/MX5%20CUP.png",
  mazda_rookie: "/categorias/recortadas/MX5%20ROOKIE.png",
  toyota: "/categorias/recortadas/GR%20CUP.png",
  toyota_amador: "/categorias/recortadas/GR%20CUP.png",
  toyota_rookie: "/categorias/recortadas/GR%20ROOKIE.png",
  bmw: "/categorias/recortadas/M2%20CUP.png",
  bmw_m2: "/categorias/recortadas/M2%20CUP.png",
  gt4: "/categorias/recortadas/GT4.png",
  gt3: "/categorias/recortadas/GT3.png",
  production_challenger: "/categorias/recortadas/PRODUCTION.png",
  endurance: "/categorias/recortadas/ENDURANCE.png",
};

const CATEGORY_SUMMARY_FITS = {
  mazda: {
    frameClassName: "overflow-hidden",
    imageStyle: {
      clipPath: "inset(0 0 8% 0)",
    },
  },
  mazda_amador: {
    frameClassName: "overflow-hidden",
    imageStyle: {
      clipPath: "inset(0 0 8% 0)",
    },
  },
};

function weatherLabel(value) {
  if (value === "HeavyRain") return "Chuva forte";
  if (value === "Wet") return "Chuva";
  if (value === "Damp") return "Úmido";
  return "Seco";
}

function getCategorySummaryLogo(categoryId) {
  return typeof categoryId === "string" ? CATEGORY_SUMMARY_LOGOS[categoryId] ?? null : null;
}

function getCategorySummaryFit(categoryId) {
  if (typeof categoryId !== "string") {
    return { frameClassName: "", imageStyle: {} };
  }

  return CATEGORY_SUMMARY_FITS[categoryId] ?? { frameClassName: "", imageStyle: {} };
}

function RaceResultView({ result, onDismiss }) {
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const otherCategoriesResult = useCareerStore((state) => state.otherCategoriesResult);
  const [showChampionship, setShowChampionship] = useState(false);
  const [championship, setChampionship] = useState([]);
  const [teamColors, setTeamColors] = useState({});
  const [loadingChampionship, setLoadingChampionship] = useState(false);
  const [championshipError, setChampionshipError] = useState("");

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

  useEffect(() => {
    let mounted = true;
    async function fetchChampionship() {
      if (!careerId || !playerTeam?.categoria) return;
      setLoadingChampionship(true);
      setChampionshipError("");
      try {
        const data = await invoke("get_drivers_by_category", {
          careerId,
          category: playerTeam.categoria,
        });
        if (mounted) {
          setChampionship(data);
          
          const colors = {};
          data.forEach(d => {
            if (d.equipe_nome && d.equipe_cor) {
              colors[d.equipe_nome] = d.equipe_cor;
            }
          });
          setTeamColors(colors);
        }
      } catch (error) {
        if (mounted) {
          setChampionshipError(
            typeof error === "string" ? error : "Não foi possível carregar o campeonato."
          );
        }
      } finally {
        if (mounted) setLoadingChampionship(false);
      }
    }
    fetchChampionship();
    return () => { mounted = false; };
  }, [careerId, playerTeam?.categoria]);

  if (!result) return null;

  return (
    <div className="relative z-10 flex h-[calc(100vh-4rem)] w-full flex-col rounded-[32px] border border-white/5 bg-[#080d14]/40 p-2 animate-fade-in shadow-[0_10px_50px_rgba(0,0,0,0.5)] backdrop-blur-3xl lg:p-4">
      
      {/* HEADER */}
      <header className="flex flex-col lg:flex-row justify-between items-end mb-6 border-b border-white/10 pb-6 shrink-0 px-4 pt-4">
        <div>
          <p className="text-[11px] uppercase font-black text-[#58a6ff] tracking-[0.3em] mb-2 shadow-text">Classificação Final</p>
          <h1 className="text-4xl lg:text-5xl font-extrabold text-white tracking-tight">{result.track_name}</h1>
          <p className="text-gray-400 mt-2 font-mono text-sm capitalize">{weatherLabel(result.weather)} • {result.total_laps} Voltas Completadas</p>
        </div>
        
        <div className="mt-6 lg:mt-0 bg-[#0a0f16]/80 border border-white/10 px-6 py-4 rounded-2xl flex items-center gap-6 shadow-xl">
          <div>
            <p className="text-[10px] uppercase tracking-widest text-[#58a6ff] font-bold">Seu Desempenho</p>
            <p className="text-3xl font-black text-white leading-none mt-1 drop-shadow-md">
              {playerResult ? (playerResult.is_dnf ? "DNF" : `P${playerResult.finish_position}`) : "—"}
            </p>
          </div>
          <div className="text-right">
             <p className={`text-xs font-bold px-2 py-0.5 rounded uppercase tracking-wider shadow-sm ${playerResult && playerResult.positions_gained >= 0 ? 'text-green-400 bg-green-500/10' : 'text-red-400 bg-red-500/10'}`}>
                {playerResult ? (playerResult.positions_gained > 0 ? `+${playerResult.positions_gained}` : playerResult.positions_gained) : "-"} Var
             </p>
             <p className="text-[10px] text-gray-400 mt-1 uppercase tracking-widest font-bold">Grid: {playerResult ? `${playerResult.grid_position}º` : "—"}</p>
          </div>
          <div className="h-10 w-[1px] bg-white/10 mx-2"></div>
          <button onClick={onDismiss} className="px-6 py-3 bg-[#58a6ff] hover:bg-blue-400 text-[#05080c] font-black uppercase tracking-widest rounded-xl transition text-xs shadow-[0_0_20px_rgba(88,166,255,0.2)]">
            Voltar Aos Boxes
          </button>
        </div>
      </header>

      {/* CONTEÚDO */}
      <div className="grid grid-cols-12 gap-6 flex-1 min-h-0 px-4 pb-4">
        
        {/* Esquerda: Destaques */}
        <div className="col-span-12 lg:col-span-3 flex flex-col gap-4 overflow-y-auto pr-2 custom-scrollbar">
            
            {/* Vencedor */}
            <div className="relative rounded-2xl p-6 text-center border border-yellow-500/20 bg-yellow-500/5 shadow-inner">
                <span className="text-yellow-500 text-3xl mb-2 block drop-shadow-[0_0_15px_rgba(234,179,8,0.5)]">🏆</span>
                <p className="text-[10px] uppercase font-bold text-gray-400 tracking-wider">Vencedor</p>
                <p className="text-xl font-bold text-white mt-1 relative">{winner?.pilot_name || "—"}</p>
                <p className="text-[10px] font-black tracking-widest text-yellow-500 uppercase mt-1 opacity-80">{winner?.team_name || "—"}</p>
            </div>
            
            {/* Fastest Lap */}
            <div className="rounded-2xl p-5 border border-purple-500/20 bg-purple-500/5 shadow-inner flex flex-col justify-center">
                <p className="text-[10px] uppercase font-bold text-purple-400 tracking-wider">Volta Mais Rápida</p>
                <div className="flex justify-between items-end mt-1">
                    <p className="text-lg font-bold text-white truncate max-w-[130px] pr-2">{fastestLap?.pilot_name || "—"}</p>
                    <p className="text-sm font-mono font-bold text-purple-300 drop-shadow-md">{fastestLap ? formatLapTime(fastestLap.best_lap_time_ms) : "—"}</p>
                </div>
            </div>

            {/* Pole Position */}
            <div className="rounded-2xl p-5 border border-white/10 bg-white/5 shadow-inner flex flex-col justify-center">
                <p className="text-[10px] uppercase font-bold text-gray-400 tracking-wider">Pole Position</p>
                <div className="flex justify-between items-end mt-1">
                    <p className="text-lg font-bold text-white truncate max-w-[130px] pr-2">{poleSitter?.pilot_name || "—"}</p>
                    <p className="text-sm font-mono text-gray-400">{poleSitter ? formatLapTime(poleSitter.best_lap_time_ms) : "—"}</p>
                </div>
            </div>

            {/* Escalada */}
            <div className="rounded-2xl p-5 border border-green-500/20 bg-green-500/5 shadow-inner flex items-center justify-between">
                <div>
                    <p className="text-[10px] uppercase font-bold text-green-400 tracking-wider">Maior Escalada</p>
                    <p className="text-lg font-bold text-white mt-1 truncate max-w-[120px]">{biggestGainer?.pilot_name || "—"}</p>
                </div>
                {biggestGainer && (
                    <span className="bg-green-500/20 text-green-400 border border-green-500/30 px-3 py-1 rounded font-black text-sm drop-shadow-sm">
                        {biggestGainer.positions_gained > 0 ? `+${biggestGainer.positions_gained}` : biggestGainer.positions_gained}
                    </span>
                )}
            </div>
            
            {/* Outras Categorias Mini-Resumo */}
            {otherCategoriesResult?.total_races_simulated > 0 && (
                <div className="mt-auto rounded-2xl border border-white/5 bg-[#05080c] p-4 relative overflow-hidden group">
                    <div>
                        <div>
                            <p className="text-[10px] uppercase tracking-widest font-bold text-gray-500">Outras Categorias</p>
                            <p className="mt-1 text-sm font-bold text-[#58a6ff]">
                                {otherCategoriesResult.total_races_simulated} Corrida{otherCategoriesResult.total_races_simulated > 1 ? 's' : ''} Processada{otherCategoriesResult.total_races_simulated > 1 ? 's' : ''}
                            </p>
                        </div>
                    </div>
                    <div
                        className="mt-3 flex flex-wrap items-center justify-center gap-x-6 gap-y-4"
                        data-testid="other-categories-logo-strip"
                    >
                        {otherCategoriesResult.categories_simulated.map((cat) => {
                            const logoSrc = getCategorySummaryLogo(cat.category_id);
                            const logoFit = getCategorySummaryFit(cat.category_id);

                            if (!logoSrc) {
                                return (
                                    <span key={cat.category_id} className="text-[9px] uppercase font-bold tracking-widest border border-white/10 bg-white/5 px-2 py-0.5 rounded text-gray-400">
                                        {cat.category_name}
                                    </span>
                                );
                            }

                            return (
                                <span
                                    key={cat.category_id}
                                    className={[
                                        "flex h-24 w-[320px] items-center justify-center sm:h-28 sm:w-[360px]",
                                        logoFit.frameClassName,
                                    ].join(" ").trim()}
                                >
                                    <img
                                        src={logoSrc}
                                        alt={cat.category_name}
                                        className="h-full w-full object-contain"
                                        style={logoFit.imageStyle}
                                        draggable={false}
                                    />
                                </span>
                            );
                        })}
                    </div>
                </div>
            )}
        </div>

        {/* Direita: Tabela de Resultados (100% dinâmica com scroll perfeito) */}
        <div className="col-span-12 lg:col-span-9 rounded-3xl p-6 overflow-hidden flex flex-col bg-[#060a10] border border-white/5 shadow-inner relative">
             
             {/* Gradient glow interno no topo para suavizar */}
             <div className="absolute top-0 left-0 right-0 h-16 bg-gradient-to-b from-[#58a6ff]/5 to-transparent pointer-events-none"></div>

             <div className="flex justify-between items-center mb-4 border-b border-white/10 pb-4 shrink-0 px-2 relative z-10">
                 <h3 className="text-sm font-bold text-white uppercase tracking-widest opacity-90 drop-shadow-sm">
                     {showChampionship ? "Classificação Geral do Campeonato" : "Tabela Oficial da Prova"}
                 </h3>
                 <button 
                     onClick={() => setShowChampionship(!showChampionship)}
                     className="text-[11px] text-[#58a6ff] bg-[#58a6ff]/10 hover:bg-[#58a6ff]/20 border border-[#58a6ff]/30 px-4 py-1.5 rounded-lg uppercase font-bold tracking-widest transition"
                 >
                     {showChampionship ? "Retornar aos Resultados" : "Ver Campeonato Completo"}
                 </button>
             </div>
             
             <div className="flex-1 overflow-y-auto custom-scrollbar pr-2 relative z-10">
                 {showChampionship ? (
                     <div className="animate-fade-in pr-2">
                         {loadingChampionship ? (
                             <div className="py-10 text-center">
                                 <p className="text-sm text-gray-400 font-mono tracking-widest uppercase animate-pulse">Consultando Federação...</p>
                             </div>
                         ) : championshipError ? (
                             <div className="bg-red-500/10 border border-red-500/30 text-red-400 px-4 py-3 rounded-xl text-sm font-mono text-center">
                                 {championshipError}
                             </div>
                         ) : (
                             <table className="w-full text-left">
                                <thead className="text-[10px] uppercase tracking-[0.2em] text-gray-500 sticky top-0 bg-[#060a10] z-10 shadow-sm">
                                    <tr>
                                        <th className="py-4 px-2 w-[80px] text-center border-b border-white/5">POS</th>
                                        <th className="py-4 px-2 border-b border-white/5">PILOTO</th>
                                        <th className="py-4 px-2 w-[180px] border-b border-white/5">EQUIPE</th>
                                        <th className="py-4 px-2 w-24 text-center border-b border-white/5">VITÓRIAS</th>
                                        <th className="py-4 px-2 w-20 text-right pr-4 border-b border-white/5">PTS</th>
                                    </tr>
                                </thead>
                                <tbody className="text-sm font-medium divide-y divide-white/5">
                                    {championship.map((driver) => (
                                        <tr key={driver.id} className={`hover:bg-white/5 transition ${driver.is_jogador ? 'bg-[#58a6ff]/10 relative shadow-[inset_4px_0_0_#58a6ff]' : ''}`}>
                                            <td className={`py-4 px-2 text-center text-lg font-black ${driver.posicao_campeonato === 1 ? 'text-yellow-500' : driver.posicao_campeonato === 2 ? 'text-gray-300' : driver.posicao_campeonato === 3 ? 'text-orange-400' : 'text-gray-500'}`}>
                                                {driver.posicao_campeonato}
                                            </td>
                                            <td className={`py-4 px-2 font-bold ${driver.is_jogador ? 'text-[#58a6ff]' : 'text-gray-200'}`}>
                                                {driver.is_jogador ? `▶ ${driver.nome} ◀` : driver.nome}
                                            </td>
                                            <td className="py-4 px-2 text-[10px] font-bold uppercase tracking-widest text-gray-400 opacity-90">
                                                <div className="flex items-center gap-2">
                                                    {driver.equipe_cor && (
                                                        <div className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: driver.equipe_cor, boxShadow: `0 0 8px ${driver.equipe_cor}80` }}></div>
                                                    )}
                                                    <span className={`truncate max-w-[140px] ${driver.is_jogador ? 'text-[#58a6ff]' : ''}`}>{driver.equipe_nome || "-"}</span>
                                                </div>
                                            </td>
                                            <td className="py-4 px-2 text-center font-mono font-bold text-gray-400">{driver.vitorias}</td>
                                            <td className="py-4 px-2 text-right font-black font-mono text-white text-base pr-4">{driver.pontos}</td>
                                        </tr>
                                    ))}
                                </tbody>
                             </table>
                         )}
                     </div>
                 ) : (
                     <table className="w-full text-left">
                         <thead className="text-[10px] uppercase tracking-[0.16em] text-gray-500 border-b border-white/10 sticky top-0 bg-[#060a10] z-10 shadow-sm">
                             <tr>
                                 <th className="py-4 px-2 w-[110px] text-center">POS (VAR)</th>
                                 <th className="py-4 px-2 w-[240px]">PILOTO</th>
                                 <th className="py-4 px-2 w-[200px]">EQUIPE</th>
                                 <th className="py-4 px-2 text-right pr-6">TEMPO / GAP</th>
                             </tr>
                         </thead>
                         <tbody className="text-[13px] font-medium divide-y divide-white/5">
                             {result.race_results.map((entry) => {
                                 let posColor = "text-gray-500";
                                 let posSize = "text-base";
                                 if (entry.finish_position === 1) { posColor = "text-yellow-500"; posSize = "text-lg"; }
                                 else if (entry.finish_position === 2) { posColor = "text-gray-300"; posSize = "text-[17px]"; }
                                 else if (entry.finish_position === 3) { posColor = "text-orange-400"; posSize = "text-base"; }
                                 
                                 const isJogador = entry.is_jogador;
                                 if (isJogador) posColor = "text-[#58a6ff]";

                                 // Delta ao lado da Posição
                                 const delta = entry.positions_gained;
                                 let deltaStr = delta === 0 ? "-" : (delta > 0 ? `+${delta}` : `${delta}`);
                                 let deltaColor = delta === 0 ? "text-gray-600 font-medium" : (delta > 0 ? "text-green-400 font-bold" : "text-red-400/80 font-bold");

                                 return (
                                     <tr key={entry.pilot_id} className={`hover:bg-white/5 transition ${isJogador ? 'bg-[#58a6ff]/10 relative shadow-[inset_4px_0_0_#58a6ff]' : entry.is_dnf ? 'bg-red-500/5 opacity-80' : 'bg-white/[0.01]'}`}>
                                         
                                         {/* Coluna combinada POS + Delta */}
                                         <td className="py-4 px-2 text-center align-middle">
                                            <div className="flex items-center justify-center gap-2">
                                                <span className={`font-black w-6 text-right ${entry.is_dnf ? 'text-red-500 text-xs tracking-widest uppercase' : posColor + ' ' + posSize}`}>
                                                    {entry.is_dnf ? 'DNF' : entry.finish_position}
                                                </span>
                                                {!entry.is_dnf && (
                                                    <span className={`text-[10px] min-w-[20px] text-left ${deltaColor}`}>
                                                        {delta > 0 ? `▲${deltaStr.replace('+','')}` : delta < 0 ? `▼${deltaStr.replace('-','')}` : '—'}
                                                    </span>
                                                )}
                                            </div>
                                         </td>
                                         
                                         <td className={`py-4 px-2 font-bold flex items-center gap-2 ${entry.is_dnf ? 'line-through text-gray-500' : isJogador ? 'text-[#58a6ff] text-sm' : 'text-gray-200 text-sm'}`}>
                                            {entry.has_fastest_lap && !entry.is_dnf && <span className="animate-pulse drop-shadow-md pb-[2px]" title="Volta mais rápida">⚡</span>}
                                            {isJogador ? `▶ ${entry.pilot_name} ◀` : entry.pilot_name}
                                         </td>
                                         
                                         <td className={`py-4 px-2 text-[11px] uppercase tracking-widest ${isJogador ? 'font-black text-[#58a6ff] opacity-80' : 'text-gray-400 font-bold'}`}>
                                            <div className="flex items-center gap-2">
                                                <TeamLogoMark
                                                    teamName={entry.team_name}
                                                    color={teamColors[entry.team_name] ?? null}
                                                    size="xs"
                                                    testId="official-race-team-logo"
                                                />
                                                <span className="truncate max-w-[170px]">{entry.team_name}</span>
                                            </div>
                                         </td>
                                         
                                         <td className={`py-4 px-2 text-right font-mono pr-6 ${entry.is_dnf ? 'text-red-500 text-[10px] font-bold tracking-widest uppercase' : entry.finish_position === 1 ? 'text-yellow-500 font-bold' : isJogador ? 'text-white font-bold' : 'text-gray-400'}`}>
                                             {entry.is_dnf 
                                                ? "Abandonou" 
                                                : entry.finish_position === 1 
                                                    ? formatLapTime(entry.total_race_time_ms) 
                                                    : formatGap(entry.gap_to_winner_ms)}
                                         </td>

                                     </tr>
                                 );
                             })}
                         </tbody>
                     </table>
                 )}
             </div>
        </div>

      </div>

    </div>
  );
}

export default RaceResultView;
