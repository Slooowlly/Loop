// i18n-ignore-file — tela V1 APOSENTADA (RaceResultViewV2 é a oficial); código morto,
// não vale traduzir. Se um dia voltar a ser usada, remova esta linha e traduza.
import { gradeBox, weatherLabel } from "./helpers";

// Cabeçalho da tela: pista, clima, desempenho do jogador, nota e botão de saída.
function ResultHeader({ result, playerResult, evaluation, onDismiss }) {
  return (
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
        {evaluation && (
          <>
            <div className="h-10 w-[1px] bg-white/10 mx-2"></div>
            <div className={`flex h-14 w-14 flex-col items-center justify-center rounded-xl border ${gradeBox(evaluation.grade)}`}>
              <span className="text-xl font-black leading-none">{evaluation.grade.toFixed(1)}</span>
              <span className="text-[8px] uppercase tracking-widest opacity-70">Nota</span>
            </div>
          </>
        )}
        <div className="h-10 w-[1px] bg-white/10 mx-2"></div>
        <button onClick={onDismiss} className="px-6 py-3 bg-[#58a6ff] hover:bg-blue-400 text-[#05080c] font-black uppercase tracking-widest rounded-xl transition text-xs shadow-[0_0_20px_rgba(88,166,255,0.2)]">
          Voltar Aos Boxes
        </button>
      </div>
    </header>
  );
}

export default ResultHeader;
