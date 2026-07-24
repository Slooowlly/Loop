// i18n-ignore-file — tela V1 APOSENTADA (RaceResultViewV2 é a oficial); código morto,
// não vale traduzir. Se um dia voltar a ser usada, remova esta linha e traduza.

// Classificação geral do campeonato (aba "Campeonato" do painel direito).
function ChampionshipStandingsTable({ championship, loading, error }) {
  return (
    <div className="animate-fade-in pr-2">
        {loading ? (
            <div className="py-10 text-center">
                <p className="text-sm text-gray-400 font-mono tracking-widest uppercase animate-pulse">Consultando Federação...</p>
            </div>
        ) : error ? (
            <div className="bg-red-500/10 border border-red-500/30 text-red-400 px-4 py-3 rounded-xl text-sm font-mono text-center">
                {error}
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
  );
}

export default ChampionshipStandingsTable;
