import { useState } from "react";

// Passos do tutorial (mostrado só na 1ª vez que o jogador vai pro iRacing).
// As imagens ficam em public/utilities/tutorial/ — se faltarem, cai num
// placeholder e o fluxo continua funcionando.
const STEPS = [
  {
    img: "/utilities/tutorial/step1.png",
    title: 'Entre em "AI Single Player"',
    body: 'No menu lateral do iRacing, dentro de Single Player, clique em "AI Single Player".',
  },
  {
    img: "/utilities/tutorial/step2.png",
    title: "Abra a sua AI Season",
    body: 'Em "My Seasons", abra a temporada que acabamos de exportar (a sua "Carreira ...").',
  },
  {
    img: "/utilities/tutorial/step3.png",
    title: "Inicie a simulação",
    body: 'Na tela da temporada, confira a próxima corrida e clique em "Continue" para começar.',
  },
];

function StepImage({ src, alt }) {
  const [failed, setFailed] = useState(false);

  if (failed) {
    return (
      <div className="flex h-[360px] w-full items-center justify-center rounded-2xl border border-dashed border-white/15 bg-white/5">
        <p className="px-6 text-center text-xs text-text-muted">
          Imagem do tutorial não encontrada.
          <br />
          <span className="font-mono text-[11px] text-text-secondary">{src}</span>
        </p>
      </div>
    );
  }

  return (
    <img
      src={src}
      alt={alt}
      onError={() => setFailed(true)}
      className="max-h-[360px] w-auto max-w-full rounded-2xl border border-white/10 object-contain shadow-[0_12px_40px_rgba(0,0,0,0.5)]"
    />
  );
}

function IracingTutorialModal({ onFinish, onClose, message }) {
  const [step, setStep] = useState(0);
  const [busy, setBusy] = useState(false);
  const isLast = step === STEPS.length - 1;
  const current = STEPS[step];

  async function handlePrimary() {
    if (!isLast) {
      setStep((s) => Math.min(s + 1, STEPS.length - 1));
      return;
    }
    setBusy(true);
    try {
      await onFinish?.();
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      className="fixed inset-0 z-[200] flex items-center justify-center bg-black/75 p-4 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="flex w-full max-w-3xl flex-col overflow-hidden rounded-2xl border border-white/10 bg-[#0f141b] shadow-[0_24px_70px_rgba(0,0,0,0.65)]"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Cabeçalho */}
        <div className="flex items-center justify-between border-b border-white/8 px-6 py-4">
          <div className="flex items-center gap-2">
            <span className="text-base">🤖</span>
            <h3 className="text-sm font-bold uppercase tracking-[0.14em] text-accent-primary">
              Como correr no iRacing
            </h3>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-text-muted transition-glass hover:text-text-primary"
            aria-label="Fechar"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M18 6 6 18" />
              <path d="m6 6 12 12" />
            </svg>
          </button>
        </div>

        {/* Corpo */}
        <div className="flex flex-col items-center gap-4 px-6 py-6">
          <div className="text-center">
            <p className="text-[11px] font-bold uppercase tracking-[0.22em] text-text-muted">
              Passo {step + 1} de {STEPS.length}
            </p>
            <h4 className="mt-1 text-xl font-bold tracking-tight text-text-primary">
              {current.title}
            </h4>
            <p className="mx-auto mt-1.5 max-w-lg text-sm text-text-secondary">{current.body}</p>
          </div>

          <div className="flex min-h-[360px] w-full items-center justify-center">
            <StepImage src={current.img} alt={current.title} />
          </div>

          {message && (
            <div className="w-full rounded-xl border border-status-yellow/30 bg-status-yellow/10 px-4 py-2.5 text-center">
              <p className="text-xs font-semibold text-status-yellow">{message}</p>
            </div>
          )}
        </div>

        {/* Rodapé */}
        <div className="flex items-center justify-between border-t border-white/8 px-6 py-4">
          {/* Indicador de passos */}
          <div className="flex items-center gap-2">
            {STEPS.map((_, i) => (
              <span
                key={i}
                className={`h-1.5 rounded-full transition-all ${
                  i === step ? "w-6 bg-accent-primary" : "w-1.5 bg-white/15"
                }`}
              />
            ))}
          </div>

          <div className="flex items-center gap-2">
            {step > 0 && (
              <button
                type="button"
                onClick={() => setStep((s) => Math.max(s - 1, 0))}
                disabled={busy}
                className="rounded-xl px-4 py-2 text-sm font-semibold text-text-secondary transition-glass hover:text-text-primary disabled:opacity-50"
              >
                Voltar
              </button>
            )}
            <button
              type="button"
              onClick={handlePrimary}
              disabled={busy}
              className="rounded-xl bg-[#58a6ff] px-6 py-2 text-sm font-black uppercase tracking-wide text-[#06090e] shadow-[0_0_20px_rgba(88,166,255,0.4)] transition hover:bg-blue-400 disabled:opacity-70"
            >
              {isLast ? (busy ? "Abrindo…" : "Done") : "Next"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export default IracingTutorialModal;
