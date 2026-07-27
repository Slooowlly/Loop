// Clima da corrida → rótulo traduzido e emoji.
//
// Três telas consomem isto com contratos DIFERENTES DE PROPÓSITO — o parâmetro
// existe para preservar essa diferença, não para ser configurável à toa:
//
// - Calendário: namespace "weather", copy em Caixa Alta, "Dry" → "Seco".
// - Banner do Header: mesmo namespace, mas "Dry" → "Parcialmente Nublado", para
//   casar com o ⛅ que aparece ao lado do texto.
// - Resultado de corrida: namespace próprio "raceResult.weather" (copy em caixa
//   baixa, porque entra no meio de uma frase) e a chave de chuva se chama
//   "rain", não "wet".
import i18n from "../i18n/index.js";

const CHAVES_PADRAO = {
  heavyRain: "heavyRain",
  wet: "wet",
  damp: "damp",
  dry: "dry",
};

// Presets nomeados: o call site declara QUAL tela é, não remonta o contrato.
export const CLIMA_CALENDARIO = {};
export const CLIMA_BANNER = { chaves: { dry: "partlyCloudy" } };
export const CLIMA_RESULTADO = { ns: "raceResult.weather", chaves: { wet: "rain" } };

export function weatherLabel(value, { ns = "weather", chaves = {} } = {}) {
  const k = { ...CHAVES_PADRAO, ...chaves };
  if (value === "HeavyRain") return i18n.t(`${ns}.${k.heavyRain}`);
  if (value === "Wet") return i18n.t(`${ns}.${k.wet}`);
  if (value === "Damp") return i18n.t(`${ns}.${k.damp}`);
  return i18n.t(`${ns}.${k.dry}`);
}

// Emoji do banner. O default ⛅ pareia com "Parcialmente Nublado" de CLIMA_BANNER —
// mudar um sem o outro deixa ícone e texto se contradizendo.
export function weatherEmoji(value) {
  if (value === "HeavyRain") return "\u{26C8}\u{FE0F}"; // ⛈️
  if (value === "Wet") return "\u{1F327}\u{FE0F}"; // 🌧️
  if (value === "Damp") return "\u{1F326}\u{FE0F}"; // 🌦️
  return "\u{26C5}"; // ⛅
}
