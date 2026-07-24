import { useTranslation } from "react-i18next";

// Popup "pegar a cor do carro": vincula a pintura do iRacing do jogador a este save.
// Só aparece quando ele já conectou ao iRacing e ainda não vinculou a cor.
function NextRacePaintPrompt({ open, busy, error, onConfirm, onCancel }) {
  const { t } = useTranslation();

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm">
      <div className="w-full max-w-md rounded-2xl border border-white/10 bg-[#0d1117] p-6 shadow-2xl">
        <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-[#58a6ff]">
          <span className="mr-2">🎨</span>{t("nextRaceTab.paint.eyebrow")}
        </p>
        <h2 className="mt-2 text-xl font-extrabold text-white">
          {t("nextRaceTab.paint.promptTitle")}
        </h2>
        <p className="mt-2 text-sm leading-relaxed text-gray-400">
          {t("nextRaceTab.paint.promptBody")}
        </p>

        {error && (
          <p className="mt-3 rounded-lg border border-status-red/30 bg-status-red/10 px-3 py-2 text-xs text-status-red">
            {error}
          </p>
        )}

        <div className="mt-5 flex flex-col gap-2">
          <button
            onClick={onConfirm}
            disabled={busy}
            className="w-full rounded-lg border border-[#58a6ff66] bg-[#58a6ff33] px-4 py-3 text-sm font-bold text-[#58a6ff] transition hover:bg-[#58a6ff55] disabled:cursor-not-allowed disabled:opacity-60"
          >
            {busy ? t("nextRaceTab.paint.painting") : t("nextRaceTab.paint.grab")}
          </button>
          <button
            onClick={onCancel}
            disabled={busy}
            className="w-full rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-xs font-semibold text-gray-400 transition hover:bg-white/10 disabled:opacity-60"
          >
            {t("nextRaceTab.paint.notNow")}
          </button>
        </div>
        <p className="mt-3 text-center text-[10px] text-gray-500">
          {t("nextRaceTab.paint.requiresTradingPaints")}
        </p>
      </div>
    </div>
  );
}

export default NextRacePaintPrompt;
