import { useTranslation } from "react-i18next";

// Modal: Iniciar temporada sem equipe
export default function FreeAgentWarningModal({ onClose, onConfirm }) {
  const { t } = useTranslation();
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div className="glass-strong animate-fade-in mx-4 w-full max-w-md rounded-2xl p-6 md:p-7">
        <div className="mb-1 text-body-sm font-bold uppercase tracking-[0.22em] text-[#f85149]">
          {t("preSeason.freeAgentWarning.eyebrow")}
        </div>
        <h2 className="mb-3 text-[18px] font-bold leading-tight text-[color:var(--text-primary)]">
          {t("preSeason.freeAgentWarning.title")}
        </h2>
        <p className="mb-2 text-body text-[color:var(--text-secondary)]">
          {t("preSeason.freeAgentWarning.body1Prefix")}{" "}
          <span className="font-semibold text-[color:var(--text-primary)]">{t("preSeason.freeAgentWarning.freeAgentTerm")}</span>{" "}
          {t("preSeason.freeAgentWarning.body1Suffix")}
        </p>
        <p className="mb-6 text-body text-[color:var(--text-secondary)]">
          {t("preSeason.freeAgentWarning.body2")}
        </p>
        <div className="flex gap-3">
          <button
            onClick={onClose}
            className="transition-glass flex-1 rounded-xl border border-white/15 bg-white/5 px-4 py-2.5 text-body font-semibold text-[color:var(--text-secondary)] hover:bg-white/10"
          >
            {t("preSeason.actions.back")}
          </button>
          <button
            onClick={onConfirm}
            className="transition-glass flex-1 rounded-xl border border-[#f8514999] bg-[#f85149]/20 px-4 py-2.5 text-body font-bold text-[#f85149] hover:bg-[#f85149]/30"
          >
            {t("preSeason.freeAgentWarning.confirmAnyway")}
          </button>
        </div>
      </div>
    </div>
  );
}
