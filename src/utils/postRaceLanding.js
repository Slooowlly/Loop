// Comportamento de "onde cair" depois do debrief pós-corrida (regra do jogador):
//
//  - Por padrão, o pós-corrida cai na aba "Notícias".
//  - Se o jogador PULAR a leitura (ficar menos de 15s na tela) 3 vezes seguidas,
//    o comportamento passa a cair direto na "Home", e assim segue pelo resto da
//    temporada.
//  - No começo de CADA temporada, o comportamento volta a cair em "Notícias" e
//    tenta se firmar: se estava vindo da "Home", tolera só 2 pulos seguidos antes
//    de desistir e voltar pra "Home". Assim que o jogador lê uma vez, o modo
//    "Notícias" se firma e volta ao limite normal (3).
//  - Em FINAL DE CAMPEONATO, sempre cai em "Notícias" — sem contar como leitura
//    nem como pulo (é um override, não afeta o aprendizado).
//
// O estado é persistido por carreira no localStorage, então sobrevive a fechar e
// reabrir o app dentro de uma mesma temporada.

// Tempo mínimo (ms) na aba Notícias para considerar que o jogador LEU. Sair antes
// disso conta como "pulo".
export const NEWS_READ_MS = 15000;

export const HOME_TAB = "standings";
export const NEWS_TAB = "news";

const DEFAULT_LIMIT = 3; // pulos seguidos que viram Home no fluxo normal
const RETRY_LIMIT = 2; // tolerância menor na tentativa de início de temporada

// Slots narrativos (thematic_slot) que representam o final de campeonato.
const FINALE_SLOTS = new Set(["FinalDaTemporada", "FinalEspecial"]);

export function isFinaleSlot(slot) {
  return FINALE_SLOTS.has(slot);
}

function storageKey(careerId) {
  return `loop.postRaceLanding.${careerId}`;
}

function defaultState(season) {
  return { mode: NEWS_TAB, streak: 0, limit: DEFAULT_LIMIT, season: season ?? null };
}

export function readLandingState(careerId, season) {
  if (!careerId) return defaultState(season);
  try {
    const raw = localStorage.getItem(storageKey(careerId));
    if (!raw) return defaultState(season);
    const parsed = JSON.parse(raw);
    if (parsed.mode !== NEWS_TAB && parsed.mode !== HOME_TAB) {
      return defaultState(season);
    }
    return {
      mode: parsed.mode,
      streak: Number.isFinite(parsed.streak) ? parsed.streak : 0,
      limit: parsed.limit === RETRY_LIMIT || parsed.limit === DEFAULT_LIMIT ? parsed.limit : DEFAULT_LIMIT,
      season: parsed.season ?? null,
    };
  } catch {
    return defaultState(season);
  }
}

function writeLandingState(careerId, state) {
  if (!careerId) return;
  try {
    localStorage.setItem(storageKey(careerId), JSON.stringify(state));
  } catch {
    /* localStorage indisponível — comportamento só não persiste, sem quebrar */
  }
}

// Aplica a virada de temporada, se `season` mudou desde o último registro.
// Nova temporada → sempre volta pra Notícias; se vinha da Home, é uma tentativa
// com tolerância menor (limite 2).
function applySeasonRollover(state, season) {
  if (season == null || state.season === season) return state;
  return {
    mode: NEWS_TAB,
    streak: 0,
    limit: state.mode === HOME_TAB ? RETRY_LIMIT : DEFAULT_LIMIT,
    season,
  };
}

// Decide a aba pós-corrida e se ela deve ser avaliada quanto à leitura.
// `isFinale` força Notícias sem avaliação. Persiste qualquer virada de temporada.
export function resolvePostRaceLanding(careerId, season, isFinale) {
  const state = applySeasonRollover(readLandingState(careerId, season), season);
  writeLandingState(careerId, state);

  if (isFinale) {
    return { tab: NEWS_TAB, evaluate: false };
  }
  const tab = state.mode === HOME_TAB ? HOME_TAB : NEWS_TAB;
  return { tab, evaluate: tab === NEWS_TAB };
}

// Jogador leu (ficou tempo suficiente): firma o modo Notícias e zera a sequência.
export function recordNewsRead(careerId, season) {
  const state = readLandingState(careerId, season);
  writeLandingState(careerId, {
    ...state,
    mode: NEWS_TAB,
    streak: 0,
    limit: DEFAULT_LIMIT,
    season: season ?? state.season,
  });
}

// Jogador pulou (<15s): incrementa a sequência; ao bater o limite, migra pra Home.
export function recordNewsSkip(careerId, season) {
  const state = readLandingState(careerId, season);
  const streak = state.streak + 1;
  const mode = streak >= state.limit ? HOME_TAB : NEWS_TAB;
  writeLandingState(careerId, {
    ...state,
    mode,
    streak,
    season: season ?? state.season,
  });
}
