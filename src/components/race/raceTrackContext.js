// Dados da PISTA no briefing pré-corrida: clima, temperatura, condição do asfalto e
// período do dia. Lógica pura (sem React) extraída de `pages/tabs/nextRaceContext.js`,
// que continua sendo o orquestrador que monta o contexto completo.
import i18n from "../../i18n/index.js";

// Climas em que a pista está molhada (muda o tom da narrativa e dos fatos de IA).
export const WET_WEATHER = ["Damp", "Wet", "HeavyRain"];

export function isWetWeather(clima) {
  return WET_WEATHER.includes(clima);
}

export function buildWeatherSummary(clima) {
  if (clima === "HeavyRain") return i18n.t("raceContext.weather.summary.heavyRain");
  if (clima === "Wet") return i18n.t("raceContext.weather.summary.wet");
  if (clima === "Damp") return i18n.t("raceContext.weather.summary.damp");
  return i18n.t("raceContext.weather.summary.dry");
}

export function buildWeatherIcon(clima) {
  if (clima === "HeavyRain") return "⛈";
  if (clima === "Wet") return "🌧";
  if (clima === "Damp") return "🌦";
  return "☀";
}

export function buildWeatherNarrative(clima) {
  if (clima === "HeavyRain") return i18n.t("raceContext.weather.narrative.heavyRain");
  if (clima === "Wet") return i18n.t("raceContext.weather.narrative.wet");
  if (clima === "Damp") return i18n.t("raceContext.weather.narrative.damp");
  return i18n.t("raceContext.weather.narrative.dry");
}

export function buildTemperatureNarrative(temperatura) {
  if (temperatura == null) return i18n.t("raceContext.display.temperature.unknown");
  if (temperatura <= 16) return i18n.t("raceContext.display.temperature.cold");
  if (temperatura <= 28) return i18n.t("raceContext.display.temperature.balanced");
  return i18n.t("raceContext.display.temperature.hot");
}

export function buildTrackTemperatureLabel(temperatura) {
  return temperatura == null ? "-" : `${Math.round(temperatura)}°C`;
}

export function buildTrackConditionLabel(clima) {
  if (clima === "HeavyRain") return i18n.t("raceContext.display.trackCondition.heavyRain");
  if (clima === "Wet") return i18n.t("raceContext.display.trackCondition.wet");
  if (clima === "Damp") return i18n.t("raceContext.display.trackCondition.damp");
  return i18n.t("raceContext.display.trackCondition.dry");
}

export function buildBoxNarrative(clima) {
  if (clima === "HeavyRain") return i18n.t("raceContext.display.box.heavyRain");
  if (clima === "Wet") return i18n.t("raceContext.display.box.wet");
  if (clima === "Damp") return i18n.t("raceContext.display.box.damp");
  return i18n.t("raceContext.display.box.dry");
}

export function buildTimePeriodPrefix(horario) {
  const hour = parseHour(horario);
  if (hour == null) return i18n.t("raceContext.display.timePeriod.prefixDefault");
  if (hour < 6) return i18n.t("raceContext.display.timePeriod.prefixNight");
  if (hour < 12) return i18n.t("raceContext.display.timePeriod.prefixStart");
  if (hour < 18) return i18n.t("raceContext.display.timePeriod.prefixStart");
  return i18n.t("raceContext.display.timePeriod.prefixStart");
}

export function buildTimePeriodHighlight(horario) {
  const hour = parseHour(horario);
  if (hour == null) return i18n.t("raceContext.display.timePeriod.highlightTrack");
  if (hour < 6) return i18n.t("raceContext.display.timePeriod.highlightDawn");
  if (hour < 12) return i18n.t("raceContext.display.timePeriod.highlightMorning");
  if (hour < 18) return i18n.t("raceContext.display.timePeriod.highlightAfternoon");
  return i18n.t("raceContext.display.timePeriod.highlightEvening");
}

function parseHour(horario) {
  if (typeof horario !== "string") return null;
  const [rawHour] = horario.split(":");
  const parsed = Number.parseInt(rawHour, 10);
  return Number.isNaN(parsed) ? null : parsed;
}
