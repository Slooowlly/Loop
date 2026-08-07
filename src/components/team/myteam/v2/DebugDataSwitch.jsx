// i18n-ignore-file — ferramenta de inspeção, some do build de produção junto com o
// `import.meta.env.DEV` que a monta. Não é texto que jogador nenhum lê, então
// traduzir só sujaria os locales com strings de bancada.

import { FAKE_SCENARIOS } from "./debugFixtures";

// Botão de dados falsos: troca o payload que a aba desenha por um cenário sintético.
//
// Nada é gravado — o cenário vive em estado de componente e "Dados reais" volta tudo.
// A faixa fica deliberadamente berrante: a pior coisa que este botão pode causar é
// alguém olhar um número inventado achando que é do save.
function DebugDataSwitch({ active, onSelect }) {
  return (
    <div className="flex flex-wrap items-center gap-2" data-testid="my-team-v2-debug-switch">
      <span className="text-[10px] uppercase tracking-[0.18em] text-text-muted">Bancada</span>
      <button
        type="button"
        onClick={() => onSelect(null)}
        className={`rounded-xl border px-3 py-1.5 text-[11px] transition-glass ${
          active === null
            ? "border-white/20 bg-white/10 text-text-primary"
            : "border-white/8 bg-black/10 text-text-muted hover:text-text-primary"
        }`}
      >
        Dados reais
      </button>
      {FAKE_SCENARIOS.map((scenario) => (
        <button
          key={scenario.id}
          type="button"
          onClick={() => onSelect(scenario.id)}
          className={`rounded-xl border px-3 py-1.5 text-[11px] transition-glass ${
            active === scenario.id
              ? "border-status-yellow/50 bg-status-yellow/15 text-status-yellow"
              : "border-white/8 bg-black/10 text-text-muted hover:text-text-primary"
          }`}
        >
          {scenario.label}
        </button>
      ))}
    </div>
  );
}

export function DebugDataBanner({ active }) {
  if (!active) return null;
  const label = FAKE_SCENARIOS.find((scenario) => scenario.id === active)?.label ?? active;
  return (
    <div
      data-testid="my-team-v2-debug-banner"
      className="rounded-2xl border border-status-yellow/40 bg-status-yellow/10 px-4 py-2 text-xs font-semibold uppercase tracking-[0.16em] text-status-yellow"
    >
      Dados falsos · {label} · nada foi gravado no save
    </div>
  );
}

export default DebugDataSwitch;
