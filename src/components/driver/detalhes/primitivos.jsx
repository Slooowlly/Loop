import { formatStatValue } from "./formatadores.js";

export function DetailRow({ label, value, valueClassName = "text-[#e6edf3]" }) {
  return (
    <div className="flex items-start justify-between gap-4 border-b border-white/[0.06] py-2 last:border-b-0 last:pb-0">
      <span className="text-[11px] uppercase tracking-[0.16em] text-[#7d8590]">{label}</span>
      <span className={["text-right text-sm font-medium", valueClassName].join(" ")}>{value}</span>
    </div>
  );
}

export function StatCard({ label, value, tone = "text-[#e6edf3]" }) {
  return (
    <div className="rounded-lg border border-white/[0.06] bg-black/10 p-2.5">
      <div className={["text-lg font-bold", tone].join(" ")}>{formatStatValue(value)}</div>
      <div className="text-[10px] uppercase tracking-[0.16em] text-[#7d8590]">{label}</div>
    </div>
  );
}

export function ProgressRow({ label, value, max = 100, color = "#58a6ff", right = null }) {
  const normalized = Number.isFinite(value) ? Math.max(0, Math.min(value, max)) : 0;
  const width = max > 0 ? (normalized / max) * 100 : 0;

  return (
    <div className="grid gap-2 sm:grid-cols-[120px_minmax(0,1fr)_42px] sm:items-center">
      <div className="text-xs font-medium text-[#c9d1d9]">{label}</div>
      <div className="h-2 overflow-hidden rounded-full bg-[#21262d]">
        <div className="h-full rounded-full" style={{ width: `${width}%`, backgroundColor: color }} />
      </div>
      <div className="text-right font-mono text-xs text-[#7d8590]">{right ?? formatStatValue(value)}</div>
    </div>
  );
}

export function TagLine({ tag }) {
  return (
    <div className="flex items-center gap-2 rounded-lg border border-white/[0.06] bg-black/10 px-3 py-2">
      <span className="h-2 w-2 rounded-full" style={{ backgroundColor: tag.color }} />
      <span className="min-w-0 flex-1 text-sm text-[#e6edf3]">{tag.tag_text}</span>
      <span className="text-[10px] uppercase tracking-[0.12em] text-[#7d8590]">{tag.level}</span>
    </div>
  );
}

export const technicalToneClass = {
  danger: "text-[#f85149]",
  warning: "text-[#d29922]",
  neutral: "text-[#c9d1d9]",
  info: "text-[#58a6ff]",
  success: "text-[#3fb950]",
  elite: "text-[#bc8cff]",
};

export const summaryToneClass = {
  danger: {
    card: "border-[#f85149]/25 bg-[#f85149]/10",
    label: "text-[#f85149]",
  },
  warning: {
    card: "border-[#d29922]/25 bg-[#d29922]/10",
    label: "text-[#d29922]",
  },
  info: {
    card: "border-[#58a6ff]/20 bg-[#58a6ff]/8",
    label: "text-[#58a6ff]",
  },
  success: {
    card: "border-[#3fb950]/25 bg-[#3fb950]/10",
    label: "text-[#3fb950]",
  },
};
