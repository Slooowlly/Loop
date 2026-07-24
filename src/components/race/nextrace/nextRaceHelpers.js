import useCareerStore from "../../../stores/useCareerStore";

// Chave de "já vi o tutorial do iRacing" (mostrado só na 1ª ida ao iRacing).
export const IRACING_TUTORIAL_KEY = "loop.iracingTutorialSeen";

// Tempo (ms) na Sala de Estratégia a partir do qual consideramos que o jogador LEU a
// prévia. Abaixo disso (simular/sair antes), conta como "não leu" para o gate de IA.
export const PRE_RACE_READ_MS = 10000;

// Teto de espera (ms) do skeleton da prévia de IA. Enquanto a IA é buscada, mostramos
// um placeholder (não o template) para evitar o flash template→IA. Se estourar esse
// tempo, caímos no template em vez de segurar o skeleton indefinidamente.
export const AI_PREVIEW_MAX_WAIT_MS = 8000;

export function getDisplayError(error, fallback) {
  if (typeof error === "string") {
    return error;
  }

  if (typeof error?.message === "string" && error.message.trim()) {
    return error.message;
  }

  const rendered = error?.toString?.();
  if (typeof rendered === "string" && rendered.trim() && rendered !== "[object Object]") {
    return rendered;
  }

  return fallback;
}

// Carro do iRacing correspondente à categoria da equipe (mazda MX-5 é o padrão).
export function carKeyForCategory(categoria) {
  const cat = (categoria ?? "").toLowerCase();
  return cat.includes("gr86") || cat.includes("toyota")
    ? "gr86"
    : cat.includes("bmw") || cat.includes("m2")
    ? "bmwm2"
    : "mx5"; // mazda e padrão
}

// Lê o cache de standings pré-buscado na store (preenchido pelo prefetch durante a
// animação de avanço), mas SÓ se for da corrida atual. A pré-corrida é estática até a
// corrida rodar, então quando o cache bate a Sala abre os Favoritos na hora — sem
// re-disparar get_drivers_by_category/get_teams_standings (os comandos que faziam o
// "Montando análise" demorar toda vez que se volta à Sala). Cache miss → busca normal.
export function readCachedPreRaceStandings() {
  const state = useCareerStore.getState();
  const cache = state.preRaceStandings;
  return cache && cache.raceId && cache.raceId === state.nextRace?.id ? cache : null;
}
