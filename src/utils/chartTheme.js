// Tema dos gráficos de corrida (recharts) — fonte única das cores usadas pelo
// pós-corrida da carreira, pelo pós real do iRacing e pelo overlay de "iRacing
// Conectado". Antes cada componente redefinia essas constantes no topo do arquivo
// (7 cópias de GRID/AXIS_TICK, 4 de PALETTE), o que fazia um ajuste visual exigir
// sete edições — e uma delas sempre ficava para trás.
//
// Módulo sem dependências de propósito: pode ser importado de qualquer webview.

/** Linhas da grade e do eixo — branco quase apagado sobre o fundo escuro. */
export const GRID = "rgba(255,255,255,0.07)";

/** Cor dos rótulos de escala (ticks) dos eixos. */
export const AXIS_TICK = "#94a3b8";

/** Carro do jogador. Só é usado como fallback quando o save não traz `player_color`. */
export const PLAYER_COLOR = "#58a6ff";

/** Faixas de bandeira amarela. */
export const YELLOW = "#facc15";

/** Melhor momento (verde) e erro mais caro (vermelho) nas marcações verticais. */
export const GOOD = "#22c55e";
export const BAD = "#ef4444";

/** Paleta distinta e legível no tema escuro para as linhas dos outros carros. */
export const PALETTE = [
  "#f59e0b", "#10b981", "#ec4899", "#8b5cf6", "#06b6d4", "#ef4444",
  "#a3e635", "#f97316", "#14b8a6", "#e879f9", "#facc15", "#34d399",
  "#fb7185", "#c084fc", "#22d3ee", "#fbbf24", "#4ade80", "#f472b6",
];
