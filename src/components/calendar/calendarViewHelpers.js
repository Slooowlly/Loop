import { currentLang } from "../../i18n/format.js";

// Superfície de painel: separa do fundo por elevação (fundo sutil + rim light no
// topo + sombra suave), sem borda — para aliviar o excesso de contornos. O rim é um
// brilho de 1px no topo (não uma borda fechada) que dá a leitura de "levantado".
export const SURFACE = "bg-white/[0.03] shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_24px_60px_-24px_rgba(0,0,0,0.65)] backdrop-blur-[10px]";

// Cor da fase por mês (barra na régua do ano + tom sutil das células vazias).
export const PHASE_COLOR = {
  mercado: "var(--status-yellow, #d4a017)",
  regular: "var(--accent-primary, #58a6ff)",
  especial: "var(--status-purple, #a371f7)",
  encerramento: "rgba(255,255,255,0.45)",
};

export function monthShortLabels(lang = currentLang()) {
  const fmt = new Intl.DateTimeFormat(lang, { month: "short" });
  return Array.from({ length: 12 }, (_, m) => {
    const s = fmt.format(new Date(2000, m, 1)).replace(".", "");
    return s.charAt(0).toUpperCase() + s.slice(1);
  });
}

export function weekdayShortLabels(lang = currentLang()) {
  const fmt = new Intl.DateTimeFormat(lang, { weekday: "short" });
  // 2023-01-01 foi um domingo → começa no domingo (igual buildMonthCells).
  return Array.from({ length: 7 }, (_, d) => {
    const s = fmt.format(new Date(2023, 0, 1 + d)).replace(".", "");
    return s.charAt(0).toUpperCase() + s.slice(1);
  });
}

export function formatTrackTime(min) {
  if (!min) return "0min";
  const h = Math.floor(min / 60);
  const m = min % 60;
  if (h === 0) return `${m}min`;
  return m === 0 ? `${h}h` : `${h}h${String(m).padStart(2, "0")}`;
}

export function compareMonth(year, month, current) {
  if (!current) return 0;
  if (year !== current.year) return year < current.year ? -1 : 1;
  if (month === current.month) return 0;
  return month < current.month ? -1 : 1;
}
