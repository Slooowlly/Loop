// i18n-ignore-file — tela V1 APOSENTADA (RaceResultViewV2 é a oficial); código morto,
// não vale traduzir. Se um dia voltar a ser usada, remova esta linha e traduza.
import TeamLogoMark from "../../team/TeamLogoMark";
import { formatGap, formatLapTime } from "../../../utils/formatters";

// Tabela oficial da prova (aba "Resultados" do painel direito).
function OfficialResultsTable({ result, teamColors }) {
  return (
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
  );
}

export default OfficialResultsTable;
