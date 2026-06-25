/**
 * Confirmação de "Sair para o menu principal?" — reutilizada pelo menu de pausa e
 * pelo menu da equipe. Presentational: o fluxo (salvar/sair/navegar) vem por props.
 */
export default function LeaveToMenuModal({
  open,
  isSaving,
  onSaveAndExit,
  onExitWithoutSave,
  onCancel,
}) {
  if (!open) return null;

  return (
    <div className="fixed inset-0 z-[120] flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onCancel} />
      <div className="glass-strong relative w-[340px] rounded-2xl border border-white/12 p-6 shadow-2xl">
        <h3 className="mb-1 text-[15px] font-semibold text-text-primary">
          Sair para o menu principal?
        </h3>
        <p className="mb-5 text-[13px] text-text-secondary">
          Você pode salvar o progresso antes de voltar ao menu.
        </p>
        <div className="flex flex-col gap-2">
          <button
            type="button"
            disabled={isSaving}
            onClick={onSaveAndExit}
            className="flex h-9 w-full items-center justify-center rounded-xl bg-accent-primary text-[13px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
          >
            {isSaving ? "Salvando..." : "Salvar e sair"}
          </button>
          <button
            type="button"
            disabled={isSaving}
            onClick={onExitWithoutSave}
            className="flex h-9 w-full items-center justify-center rounded-xl border border-white/10 bg-white/6 text-[13px] text-text-secondary transition-colors hover:bg-white/10 hover:text-text-primary disabled:opacity-50"
          >
            Sair sem salvar
          </button>
          <button
            type="button"
            disabled={isSaving}
            onClick={onCancel}
            className="flex h-9 w-full items-center justify-center rounded-xl text-[13px] text-text-secondary transition-colors hover:text-text-primary disabled:opacity-50"
          >
            Cancelar
          </button>
        </div>
      </div>
    </div>
  );
}
