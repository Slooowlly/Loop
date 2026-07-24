// i18n-ignore-file — tela V1 APOSENTADA (RaceResultViewV2 é a oficial); código morto,
// não vale traduzir. Se um dia voltar a ser usada, remova esta linha e traduza.
import { formatLapTime } from "../../../utils/formatters";
import { getCategorySummaryFit, getCategorySummaryLogo } from "./helpers";

// Coluna esquerda: destaques da prova + resumo das outras categorias.
function HighlightsColumn({ winner, fastestLap, poleSitter, biggestGainer, otherCategoriesResult }) {
  return (
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
  );
}

export default HighlightsColumn;
