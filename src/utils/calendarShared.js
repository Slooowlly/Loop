// Helpers puros compartilhados pelo calendário.
import { getTrackThumbnailSrc } from "./trackImages";
import { CLIMA_CALENDARIO, weatherLabel as climaLabel } from "./weather";

export const CATEGORY_LOGOS = {
  mazda_rookie: "/utilities/categorias/MX5%20ROOKIE.webp",
  toyota_rookie: "/utilities/categorias/GR%20ROOKIE.webp",
  mazda_amador: "/utilities/categorias/MX5%20CUP.webp",
  toyota_amador: "/utilities/categorias/GR%20CUP.webp",
  bmw_m2: "/utilities/categorias/M2%20CUP.webp",
  production_challenger: "/utilities/categorias/PRODUCTION.webp",
  gt4: "/utilities/categorias/GT4.webp",
  gt3: "/utilities/categorias/GT3.webp",
  lmp2: "/utilities/categorias/LMP2.webp",
  endurance: "/utilities/categorias/ENDURANCE.webp",
};

export const ALL_CALENDAR_CATEGORIES = [
  "mazda_rookie",
  "toyota_rookie",
  "mazda_amador",
  "toyota_amador",
  "bmw_m2",
  "production_challenger",
  "gt4",
  "gt3",
  "endurance",
];

// Tipo de fase por mês, seguindo as mesmas regras do calendário legado.
// Não-legado: Jan = pré-temporada, Fev–Nov = temporada, Dez = encerramento.
// Legado: Jan = mercado, Fev–Ago = temporada, Set–Dez = bloco especial.
export function getMonthPhaseType(monthIndex, isLegacyCalendar = false) {
  if (isLegacyCalendar) {
    if (monthIndex === 0) return "mercado";
    if (monthIndex <= 7) return "regular";
    return "especial";
  }
  if (monthIndex === 0) return "mercado";
  if (monthIndex >= 1 && monthIndex <= 10) return "regular";
  return "encerramento";
}

export function parseDisplayDate(dateStr) {
  if (!dateStr) return null;
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(dateStr);
  if (!match) return null;
  return { year: Number(match[1]), month: Number(match[2]) - 1, day: Number(match[3]) };
}

export function formatIsoDateKey(year, month, day) {
  return `${year}-${String(month + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}

// Grade fixa de 6 linhas (42 células) com os dias vizinhos preenchidos e marcados
// `outside` — para exibi-los esmaecidos (acabamento de calendário padrão) em vez de
// deixar buracos. `day` é sempre um número; `outside` distingue mês atual vs vizinho.
export function buildMonthGrid(year, month) {
  const firstWeekday = new Date(year, month, 1).getDay();
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const prevMonthDays = new Date(year, month, 0).getDate();
  const cells = [];
  for (let i = firstWeekday - 1; i >= 0; i -= 1) {
    cells.push({ day: prevMonthDays - i, outside: true });
  }
  for (let day = 1; day <= daysInMonth; day += 1) {
    cells.push({ day, outside: false });
  }
  let nextDay = 1;
  while (cells.length < 42) {
    cells.push({ day: nextDay, outside: true });
    nextDay += 1;
  }
  return cells;
}

export function weatherLabel(value) {
  return climaLabel(value, CLIMA_CALENDARIO);
}

// Adaptador do calendário para o resolvedor de miniatura: recebe a corrida inteira
// e, ao NÃO achar imagem, devolve null — o EventRow depende disso para desenhar o
// placeholder colorido no lugar de uma <img> quebrada. Não é açúcar sintático: a
// política de "miss" é o que diferencia esta chamada da do resto do app.
export function getTrackImageSrc(race) {
  return getTrackThumbnailSrc(race?.track_name, race?.track_id, { aoFalhar: "nulo" });
}

// Posiciona o tooltip de corrida ancorado na célula do dia: prefere abrir acima,
// cai para baixo quando não há espaço, e sempre respeita as bordas da viewport.
export function getRaceTooltipStyle(cellRect, viewport = {}, tooltipSize = {}, options = {}) {
  const tooltipWidth = tooltipSize.width ?? 208;
  const tooltipHeight = tooltipSize.height ?? 176;
  const margin = 12;
  const gap = 8;
  const verticalOffset = options.verticalOffset ?? 0;
  const viewportWidth = viewport.width
    ?? document.documentElement?.clientWidth
    ?? window.innerWidth;
  const viewportHeight = viewport.height
    ?? document.documentElement?.clientHeight
    ?? window.innerHeight;

  const minLeft = margin;
  const maxLeft = Math.max(margin, viewportWidth - tooltipWidth - margin);
  const centeredLeft = cellRect.left + (cellRect.width / 2) - (tooltipWidth / 2);
  const spaceLeft = cellRect.left - margin;
  const spaceRight = viewportWidth - (cellRect.left + cellRect.width) - margin;

  let left = centeredLeft;
  if (spaceRight < tooltipWidth && spaceLeft >= tooltipWidth) {
    left = cellRect.left + cellRect.width - tooltipWidth;
  } else if (spaceLeft < tooltipWidth && spaceRight >= tooltipWidth) {
    left = cellRect.left;
  } else if (centeredLeft + tooltipWidth > viewportWidth - margin) {
    left = cellRect.left + cellRect.width - tooltipWidth;
  } else if (centeredLeft < margin) {
    left = cellRect.left;
  }
  left = Math.min(Math.max(left, minLeft), maxLeft);

  const hasRoomAbove = cellRect.top >= tooltipHeight + gap + margin;
  const belowTop = cellRect.top + cellRect.height + gap - verticalOffset;
  const aboveTop = cellRect.top - tooltipHeight - gap - verticalOffset;
  const maxTop = Math.max(margin, viewportHeight - tooltipHeight - margin);
  const preferredTop = hasRoomAbove ? aboveTop : belowTop;
  const top = Math.min(Math.max(preferredTop, margin), maxTop);

  return {
    position: "fixed",
    left,
    top,
    transform: "translate(0, 0)",
    zIndex: 9999,
    pointerEvents: "none",
  };
}
