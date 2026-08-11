import { useState } from "react";
import { useTranslation } from "react-i18next";

// Cabeçalho da Sala de Estratégia: identificação da etapa + ações (simular e
// exportar para o iRacing).
function NextRaceHeader({
  nextRace,
  season,
  briefing,
  isSimulating,
  onSimulate,
  isExporting,
  exported,
  onExport,
}) {
  const { t } = useTranslation();
  const [confirmSim, setConfirmSim] = useState(false);

  return (
    <header className="flex flex-col md:flex-row justify-between items-start md:items-end mb-4">
      <div>
        <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-[#58a6ff] mb-2">
          <span className="mr-2">🏁</span>{t("nextRaceTab.labels.strategyRoom")}
        </p>
        <h1 className="text-[2.5rem] font-extrabold text-white leading-none">{nextRace.track_name}</h1>
        <div className="flex flex-wrap items-center gap-3 mt-3">
          <span className="border border-white/10 bg-white/5 px-3 py-1.5 rounded-lg text-xs font-bold text-white">
            {t("nextRaceTab.labels.stageOf", {
              round: nextRace.rodada,
              total: season?.total_rodadas ?? "?",
            })}
          </span>
          <span className="text-sm font-medium text-gray-400 capitalize">
            {briefing.eventDateShort} • {briefing.timePeriodHighlight}
          </span>
        </div>
      </div>

      <div className="flex flex-col sm:flex-row items-center gap-4 mt-6 md:mt-0 w-full sm:w-auto">
        <div className="flex flex-col items-center gap-1 w-full sm:w-auto">
          <button
            onClick={() => setConfirmSim(true)}
            disabled={isSimulating || !nextRace}
            className="w-full sm:w-auto px-5 py-2 border border-white/10 bg-white/5 hover:bg-white/10 text-gray-300 font-semibold rounded-lg transition text-xs flex justify-center items-center gap-1.5 opacity-80 hover:opacity-100 disabled:opacity-50"
          >
            {isSimulating ? t("nextRaceTab.actions.simulating") : t("nextRaceTab.actions.simulateRace")}
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" className="w-4 h-4 text-[#58a6ff]">
              <path fillRule="evenodd" d="M14.615 1.595a.75.75 0 01.359.852L12.982 9.75h7.268a.75.75 0 01.548 1.262l-10.5 11.25a.75.75 0 01-1.272-.71l1.992-7.302H3.75a.75.75 0 01-.548-1.262l10.5-11.25a.75.75 0 01.913-.143z" clipRule="evenodd" />
            </svg>
          </button>
          {confirmSim && !isSimulating && (
            <span className="text-[11px] text-gray-400 whitespace-nowrap">
              {t("nextRaceTab.actions.simulateConfirm")}{" "}
              <button
                onClick={() => {
                  setConfirmSim(false);
                  onSimulate();
                }}
                className="text-[#58a6ff] font-semibold hover:underline"
              >
                {t("nextRaceTab.actions.yes")}
              </button>
              {" · "}
              <button onClick={() => setConfirmSim(false)} className="text-gray-500 hover:underline">
                {t("nextRaceTab.actions.cancel")}
              </button>
            </span>
          )}
        </div>
        <button
          onClick={onExport}
          disabled={isExporting}
          className={`w-full sm:w-auto px-10 py-3.5 font-black uppercase rounded-xl transition text-base flex justify-center items-center gap-2 disabled:opacity-70 ${
            exported
              ? "bg-green-500 hover:bg-green-400 text-[#06090e] shadow-[0_0_22px_rgba(34,197,94,0.55)]"
              : "bg-[#58a6ff] hover:bg-blue-400 text-[#06090e] shadow-[0_0_20px_rgba(88,166,255,0.4)]"
          }`}
        >
          {exported ? (
            <>
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" className="w-5 h-5">
                <path fillRule="evenodd" d="M19.916 4.626a.75.75 0 01.208 1.04l-9 13.5a.75.75 0 01-1.154.114l-6-6a.75.75 0 011.06-1.06l5.353 5.353 8.493-12.74a.75.75 0 011.04-.207z" clipRule="evenodd" />
              </svg>
              {t("nextRaceTab.actions.exported")}
            </>
          ) : isExporting ? (
            t("nextRaceTab.actions.exporting")
          ) : (
            t("nextRaceTab.actions.run")
          )}
        </button>
      </div>
    </header>
  );
}

export default NextRaceHeader;
