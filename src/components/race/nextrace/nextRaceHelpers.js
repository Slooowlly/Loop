import { cacheEhDaEtapaAtual } from "../../../stores/career/helpers";
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

// `carKeyForCategory` foi REMOVIDA em 11/08/2026. Ela adivinhava o carro do iRacing por
// substring da categoria e terminava num `else → mx5`: qualquer categoria não reconhecida
// (GT4, GT3, Production Challenger, Endurance) era exportada como Mazda MX-5 em silêncio.
// Quem decide o carro agora é o backend, em `commands/iracing/exportavel.rs`, a partir da
// identidade da categoria — o frontend manda a categoria e não adivinha nada.

// Lê o cache de standings pré-buscado na store (preenchido pelo prefetch durante a
// animação de avanço), mas SÓ se for da corrida atual DESTA carreira. A pré-corrida é
// estática até a corrida rodar, então quando o cache bate a Sala abre os Favoritos na hora
// — sem re-disparar get_drivers_by_category/get_teams_standings (os comandos que faziam o
// "Montando análise" demorar toda vez que se volta à Sala). Cache miss → busca normal.
//
// A conferência inclui o `careerId` porque R001 é a primeira etapa de qualquer carreira:
// com a chave só na corrida, o cache do save anterior valia aqui.
export function readCachedPreRaceStandings() {
  const state = useCareerStore.getState();
  const cache = state.preRaceStandings;
  return cacheEhDaEtapaAtual(cache, { careerId: state.careerId, raceId: state.nextRace?.id })
    ? cache
    : null;
}
