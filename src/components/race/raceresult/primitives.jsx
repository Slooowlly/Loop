// i18n-ignore-file — tela V1 APOSENTADA (RaceResultViewV2 é a oficial); código morto,
// não vale traduzir. Se um dia voltar a ser usada, remova esta linha e traduza.
// Blocos visuais pequenos e reutilizados dentro da tela de resultado V1.
import { COMPOUND } from "./constants";

export function ExpStat({ label, value, highlight }) {
  return (
    <div className="rounded-xl border border-white/10 bg-white/[0.03] px-4 py-3">
      <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">{label}</p>
      <p className={`mt-1 font-mono text-lg font-black ${highlight ? "text-white" : "text-gray-300"}`}>
        {value}
      </p>
    </div>
  );
}

// Card de análise da telemetria (título + linhas de stat).
export function AnalysisCard({ title, accent, children }) {
  return (
    <div className={`rounded-2xl border bg-white/[0.02] p-4 ${accent}`}>
      <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold mb-2">{title}</p>
      <div className="space-y-1.5">{children}</div>
    </div>
  );
}

export function StatRow({ label, value, color }) {
  return (
    <div className="flex items-baseline justify-between gap-2">
      <span className="text-[11px] text-gray-400">{label}</span>
      <span className={`font-mono text-sm font-bold ${color || "text-gray-200"}`}>{value}</span>
    </div>
  );
}

// Uma linha do breakdown de posições: seta + rótulo + valor com sinal.
export function FlowRow({ icon, label, value, color }) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-lg bg-white/[0.03] px-3 py-2">
      <span className="flex items-center gap-2 text-[12px] text-gray-300">
        <span className={color}>{icon}</span>
        {label}
      </span>
      <span className={`font-mono text-sm font-black ${color}`}>{value}</span>
    </div>
  );
}

// Aba do painel direito (Resultados / Campeonato / Gráficos).
export function PanelTab({ active, onClick, children }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        "rounded-lg px-3 py-1.5 text-[11px] font-bold uppercase tracking-widest transition",
        active ? "bg-[#58a6ff]/20 text-[#58a6ff]" : "text-gray-400 hover:text-white",
      ].join(" ")}
    >
      {children}
    </button>
  );
}

// Banner de "momento" da corrida (melhor momento / erro mais caro).
export function MomentBanner({ label, card, confidence }) {
  if (!card) return null;
  return (
    <div className={`flex items-start gap-3 rounded-2xl border bg-white/[0.02] p-4 ${card.accent}`}>
      <span className="text-2xl mt-0.5">{card.icon}</span>
      <div className="flex-1">
        <div className="flex items-center gap-2">
          <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">{label}</p>
          {confidence === "media" && (
            <span className="text-[8px] uppercase tracking-widest text-gray-500 border border-white/10 rounded px-1.5 py-0.5">
              estimado
            </span>
          )}
        </div>
        <p className={`mt-1 text-sm font-extrabold ${card.color}`}>{card.title}</p>
        <p className="mt-0.5 text-[13px] leading-relaxed text-gray-400">{card.desc}</p>
      </div>
    </div>
  );
}

export function CompoundChip({ compound }) {
  const c = COMPOUND[compound] || COMPOUND.Unknown;
  return (
    <span
      className={`inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wide border ${c.cls}`}
    >
      {c.label}
    </span>
  );
}

// Trilha visual dos stints: Seco →(V8) Chuva → …
export function StintTrail({ stints }) {
  if (!stints?.length) return null;
  return (
    <div className="mt-2 flex flex-wrap items-center gap-1.5">
      {stints.map((s, i) => (
        <span key={i} className="flex items-center gap-1.5">
          {i > 0 && <span className="text-gray-600">→</span>}
          <CompoundChip compound={s.compound} />
          {s.from_lap > 1 && <span className="text-[9px] text-gray-500">V{s.from_lap}</span>}
        </span>
      ))}
    </div>
  );
}
