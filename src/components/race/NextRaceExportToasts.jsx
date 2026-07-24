import { useTranslation } from "react-i18next";

// Stack de toasts pós-exportação: "Dados exportados" e, logo abaixo, o atalho
// "Entrar no iRacing" (que empurra o primeiro pra cima).
function NextRaceExportToasts({ exported, showGoToast, iracingFocusMsg, onGoToIracing }) {
  const { t } = useTranslation();

  if (!exported) return null;

  return (
    <div className="fixed bottom-6 right-6 z-50 flex w-[300px] flex-col items-stretch gap-3">
      {/* Toast 1 — confirmação (empurrado pra cima quando o 2 surge) */}
      <div className="animate-toast-up flex items-center gap-3 rounded-2xl border border-green-300/40 bg-green-500/95 px-4 py-3.5 text-[#06090e] shadow-[0_10px_30px_rgba(34,197,94,0.45)]">
        <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[#06090e]/15">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" className="h-4 w-4">
            <path fillRule="evenodd" d="M19.916 4.626a.75.75 0 01.208 1.04l-9 13.5a.75.75 0 01-1.154.114l-6-6a.75.75 0 011.06-1.06l5.353 5.353 8.493-12.74a.75.75 0 011.04-.207z" clipRule="evenodd" />
          </svg>
        </span>
        <div className="min-w-0">
          <p className="text-sm font-bold leading-tight">{t("nextRaceTab.toast.exportedTitle")}</p>
          <p className="text-[11px] font-medium leading-tight text-[#06090e]/70">{t("nextRaceTab.toast.exportedMsg")}</p>
        </div>
      </div>

      {/* Toast 2 — ação (surge embaixo, empurrando o de cima) */}
      {showGoToast && (
        <div className="animate-toast-up">
          <button
            onClick={onGoToIracing}
            className="group flex w-full items-center gap-3 rounded-2xl border border-[#58a6ff]/40 bg-[#101826]/95 px-4 py-3.5 text-left shadow-[0_10px_30px_rgba(0,0,0,0.45)] backdrop-blur-md transition hover:border-[#58a6ff]/70 hover:bg-[#16223a]/95"
          >
            <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[#58a6ff]/15 text-base">🏁</span>
            <div className="min-w-0 flex-1">
              <p className="text-sm font-bold leading-tight text-text-primary">{t("nextRaceTab.toast.enterIracingTitle")}</p>
              <p className={`text-[11px] font-medium leading-tight ${iracingFocusMsg ? "text-red-400" : "text-text-muted"}`}>
                {iracingFocusMsg || t("nextRaceTab.toast.bringSimForward")}
              </p>
            </div>
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" className="h-4 w-4 shrink-0 text-[#58a6ff] transition-transform group-hover:translate-x-0.5">
              <path d="m9 18 6-6-6-6" />
            </svg>
          </button>
        </div>
      )}
    </div>
  );
}

export default NextRaceExportToasts;
