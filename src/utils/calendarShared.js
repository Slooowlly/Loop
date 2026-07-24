// Helpers puros compartilhados pelo calendário.
import i18n from "../i18n/index.js";

// Imagens por track_id (ids reais do iRacing).
const TRACK_IMAGES = {
  9: "/utilities/tracks/summitpoint.webp",
  353: "/utilities/tracks/limerock.jpeg",
  586: "/utilities/tracks/lagunaseca.webp",
  166: "/utilities/tracks/okayama.webp",
  180: "/utilities/tracks/oultonpark.jpeg",
  181: "/utilities/tracks/oultonpark.jpeg",
  182: "/utilities/tracks/oultonpark.jpeg",
  324: "/utilities/tracks/Tsukuba.webp",
  449: "/utilities/tracks/motorsport arena.webp",
  451: "/utilities/tracks/rudskogen.jpeg",
  489: "/utilities/tracks/ledenon.webp",
  202: "/utilities/tracks/oranpark.webp",
  440: "/utilities/tracks/winton.jpeg",
  515: "/utilities/tracks/Navarra.webp",
  554: "/utilities/tracks/charlotte.webp",
  465: "/utilities/tracks/virginia.jpeg",
};

const TRACK_IMAGE_FILES = [
  { match: ["charlotte"], file: "charlotte.webp" },
  { match: ["laguna seca"], file: "lagunaseca.webp" },
  { match: ["lime rock"], file: "limerock.jpeg" },
  { match: ["okayama"], file: "okayama.webp" },
  { match: ["oulton"], file: "oultonpark.jpeg" },
  { match: ["snetterton"], file: "snetterton.jpeg" },
  { match: ["summit point", "jefferson"], file: "summitpoint.webp" },
  { match: ["tsukuba"], file: "Tsukuba.webp" },
  { match: ["virginia international raceway", "vir full", "vir patriot"], file: "virginia.jpeg" },
  { match: ["ledenon"], file: "ledenon.webp" },
  { match: ["oschersleben", "motorsport arena"], file: "motorsport arena.webp" },
  { match: ["navarra"], file: "Navarra.webp" },
  { match: ["oran park"], file: "oranpark.webp" },
  { match: ["rudskogen"], file: "rudskogen.jpeg" },
  { match: ["winton"], file: "winton.jpeg" },
];

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
  if (value === "HeavyRain") return i18n.t("weather.heavyRain");
  if (value === "Wet") return i18n.t("weather.wet");
  if (value === "Damp") return i18n.t("weather.damp");
  return i18n.t("weather.dry");
}

function getTrackAssetPath(file) {
  if (!file) return null;
  if (file.startsWith("/utilities/tracks/")) {
    return `/utilities/tracks/${encodeURIComponent(file.slice("/utilities/tracks/".length))}`;
  }
  return `/utilities/tracks/${encodeURIComponent(file)}`;
}

function normalizeTrackName(trackName) {
  return (trackName ?? "")
    .normalize("NFD")
    .replace(new RegExp("[\\u0300-\\u036f]", "g"), "")
    .toLowerCase();
}

export function getTrackImageSrc(race) {
  const normalizedName = normalizeTrackName(race?.track_name);
  const entry = TRACK_IMAGE_FILES.find(({ match }) =>
    match.some((candidate) => normalizedName.includes(candidate)),
  );
  if (entry) return getTrackAssetPath(entry.file);
  return getTrackAssetPath(TRACK_IMAGES[race?.track_id]);
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
