import { invoke } from "@tauri-apps/api/core";

import { initialState } from "./state";

// Funções puras (e alguns fetches sem estado) compartilhadas pelos slices do
// store de carreira. Nada aqui toca `set`/`get` — os slices é que decidem.

export function getErrorMessage(error, fallback) {
  return typeof error === "string" ? error : error?.toString?.() ?? fallback;
}

// As chaves que descrevem ONDE O JOGADOR ESTÁ: qual tela de sobreposição está aberta e
// com que dados. Toda troca de contexto (carregar outra carreira, virar a temporada,
// entrar/sair do mercado, restaurar um save) precisa zerar o conjunto INTEIRO, senão um
// pedaço do contexto anterior sobrevive à troca e reaparece na tela nova.
//
// A lista existia copiada à mão em quatro lugares, com omissões silenciosas: os ramos de
// `buildResumeUiState` e o fallback de erro do `loadCareer` esqueciam `preseasonFreeAgents`
// (e o fallback esquecia também as ofertas do bloco especial), então a lista de agentes
// livres da carreira ANTERIOR continuava viva depois da troca.
export const CHAVES_DE_CONTEXTO_DE_TELA = [
  "showEndOfSeason",
  "showPreseason",
  "showConvocation",
  "endOfSeasonResult",
  "preseasonState",
  "preseasonWeeks",
  "playerProposals",
  "preseasonFreeAgents",
  "convocationResult",
  "specialWindowState",
  "playerSpecialOffers",
  "acceptedSpecialOffer",
];

// As chaves que guardam DADO DERIVADO DO SAVE e que `applyCareerData` NÃO sobrescreve:
// caches de etapa, calendário, pós-corrida, mercado e o marcador de rivalidade. Elas
// sobrevivem a um `set` de carga porque ninguém as toca, e é aí que a carreira anterior
// vaza para a nova.
//
// O vazamento é invisível porque os IDs se REPETEM entre saves: R001 é a primeira etapa de
// toda carreira e P001 o primeiro piloto. O cache de pré-corrida chaveado só por `raceId`
// batia na carreira nova e a Sala de Estratégia abria com os favoritos e a prévia da
// carreira antiga, sem nada acusar. Por isso os caches de etapa também carregam `careerId`
// (ver `cacheEhDaEtapaAtual`) — a lista abaixo é o cinto, a chave é o suspensório.
export const CHAVES_DE_CACHE_DO_SAVE = [
  "preRaceAi",
  "preRaceStandings",
  "playerInterests",
  "temporalSummary",
  "calendarDisplayDate",
  "displayDaysUntilNextEvent",
  "homeCategory",
  "lastRaceId",
  "lastRaceEvaluation",
  "lastRaceTelemetry",
  "lastRaceMaintenance",
  "lastRaceRepercussion",
  "lastRaceWasFinale",
  "resultIsFresh",
  "iracingRepair",
  "lastMarketWeekResult",
  "transferWindow",
  "poachOffer",
  "championOverlay",
  "lastSaved",
];

/// Caches locais ao save zerados, com os MESMOS valores do boot.
///
/// Mesma mecânica do `contextoDeTelaLimpo`: os valores saem do `initialState`, então chave
/// nova entra de graça e o valor de reset nunca diverge do inicial.
export function cacheDoSaveLimpo(extras = {}) {
  const limpo = {};
  for (const chave of CHAVES_DE_CACHE_DO_SAVE) {
    const valor = initialState[chave];
    limpo[chave] = Array.isArray(valor) ? [] : valor;
  }
  return { ...limpo, ...extras };
}

/// O cache de pré-corrida é da etapa que está na tela AGORA?
///
/// Vale para `preRaceStandings` e `preRaceAi`, que são gravados durante a animação de
/// avanço e lidos pela Sala de Estratégia. A conferência é do par inteiro: `raceId` sozinho
/// aceita o cache de outro save, porque R001 existe em toda carreira.
export function cacheEhDaEtapaAtual(cache, { careerId, raceId } = {}) {
  return Boolean(
    cache && careerId && raceId && cache.careerId === careerId && cache.raceId === raceId,
  );
}

/// Estado de contexto zerado, com os MESMOS valores do boot.
///
/// Os valores saem do `initialState` de propósito: chave nova no estado entra aqui de
/// graça, e o valor de reset nunca diverge do valor inicial. `extras` sobrescreve o que o
/// chamador precisa deixar diferente (por exemplo, a tela que ele está justamente abrindo).
export function contextoDeTelaLimpo(extras = {}) {
  const limpo = {};
  for (const chave of CHAVES_DE_CONTEXTO_DE_TELA) {
    const valor = initialState[chave];
    limpo[chave] = Array.isArray(valor) ? [] : valor;
  }
  return { ...limpo, ...extras };
}

export function applyCareerData(data) {
  return {
    isLoaded: true,
    isLoading: false,
    error: null,
    careerId: data.career_id,
    difficulty: data.difficulty,
    player: data.player,
    playerTeam: data.player_team,
    season: data.season,
    nextRace: data.next_race,
    nextRaceBriefing: data.next_race_briefing ?? null,
    totalDrivers: data.total_drivers,
    totalTeams: data.total_teams,
    isSimulating: false,
    isCalendarAdvancing: false,
    showResult: false,
    showRaceBriefing: false,
    lastRaceResult: null,
    lastRaceOrigem: null,
    otherCategoriesResult: null,
  };
}

export function buildWeeksFromNews(newsItems = []) {
  const grouped = new Map();

  for (const item of newsItems) {
    const weekNumber = item.semana_pretemporada;
    if (!weekNumber) continue;

    if (!grouped.has(weekNumber)) {
      grouped.set(weekNumber, {
        week_number: weekNumber,
        events: [],
        remaining_vacancies: 0,
      });
    }

    grouped.get(weekNumber).events.push({
      event_type: item.tipo,
      headline: item.titulo,
      description: item.texto,
    });
  }

  return [...grouped.values()].sort((a, b) => a.week_number - b.week_number);
}

function parseIsoDate(value) {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(value ?? "");
  if (!match) return null;

  return new Date(Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3])));
}

function formatIsoDate(date) {
  const year = date.getUTCFullYear();
  const month = String(date.getUTCMonth() + 1).padStart(2, "0");
  const day = String(date.getUTCDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function buildDateSequence(startDate, endDate) {
  const start = parseIsoDate(startDate);
  const end = parseIsoDate(endDate);
  if (!start || !end) {
    return [];
  }

  if (start > end) {
    return [endDate];
  }

  const dates = [];
  const cursor = new Date(start.getTime());

  while (cursor <= end) {
    dates.push(formatIsoDate(cursor));
    cursor.setUTCDate(cursor.getUTCDate() + 1);
  }

  return dates;
}

export function buildCalendarAdvanceTiming(totalSteps) {
  const steps = Math.max(0, totalSteps);
  if (steps === 0) {
    return {
      totalDurationMs: 0,
      stepMs: 0,
    };
  }

  const minDurationMs = 1500;
  const maxDurationMs = 3000;
  const shortJumpThreshold = 3;
  const longJumpThreshold = 14;

  let totalDurationMs = minDurationMs;

  if (steps >= longJumpThreshold) {
    totalDurationMs = maxDurationMs;
  } else if (steps > shortJumpThreshold) {
    const ratio = (steps - shortJumpThreshold) / (longJumpThreshold - shortJumpThreshold);
    totalDurationMs = Math.round(minDurationMs + ratio * (maxDurationMs - minDurationMs));
  }

  return {
    totalDurationMs,
    stepMs: Math.round(totalDurationMs / steps),
  };
}

export function buildTemporalUiState(temporalSummary) {
  return {
    temporalSummary,
    calendarDisplayDate: temporalSummary?.current_display_date ?? null,
    displayDaysUntilNextEvent: temporalSummary?.days_until_next_event ?? null,
  };
}

export function deriveAcceptedSpecialOffer(data) {
  if (data?.accepted_special_offer) {
    return data.accepted_special_offer;
  }

  if (data?.player?.categoria_especial_ativa && data?.player_team) {
    return {
      id: "accepted-active-special",
      team_id: data.player_team.id,
      team_name: data.player_team.nome,
      special_category: data.player.categoria_especial_ativa,
      class_name: data.player_team.classe ?? "",
      papel: null,
    };
  }

  return null;
}

export function deriveAcceptedSpecialOfferFromWindow(windowState) {
  const selectedOffer = windowState?.player_offers?.find(
    (offer) => offer.status === "Selecionado" || offer.id === windowState?.active_offer_id,
  );

  if (!selectedOffer) {
    return null;
  }

  return {
    id: selectedOffer.id,
    team_id: selectedOffer.team_id,
    team_name: selectedOffer.team_name,
    special_category: selectedOffer.special_category,
    class_name: selectedOffer.class_name,
    papel: selectedOffer.papel,
  };
}

export async function buildResumeUiState(careerId, resumeContext) {
  if (!careerId || !resumeContext?.active_view) {
    return contextoDeTelaLimpo();
  }

  if (resumeContext.active_view === "end_of_season" && resumeContext.end_of_season_result) {
    return contextoDeTelaLimpo({
      showEndOfSeason: true,
      endOfSeasonResult: resumeContext.end_of_season_result,
    });
  }

  if (resumeContext.active_view === "preseason") {
    return buildPreseasonUiState(careerId);
  }

  return contextoDeTelaLimpo();
}

export async function buildPreseasonUiState(careerId) {
  const [state, proposals, freeAgents] = await Promise.all([
    invoke("get_preseason_state", { careerId }),
    invoke("get_player_proposals", { careerId }).catch(() => []),
    invoke("get_preseason_free_agents", { careerId }).catch(() => []),
  ]);
  const news = await invoke("get_news", {
    careerId,
    season: state.season_number,
    tipo: null,
    limit: 400,
  });

  return contextoDeTelaLimpo({
    showPreseason: true,
    preseasonState: state,
    preseasonWeeks: buildWeeksFromNews(news),
    playerProposals: proposals,
    preseasonFreeAgents: freeAgents,
  });
}

export async function loadTemporalSummary(careerId, season, playerTeam) {
  if (!careerId || !season?.id || !playerTeam?.categoria) {
    return null;
  }

  return invoke("get_temporal_summary", {
    careerId,
    seasonId: season.id,
    playerCategory: playerTeam.categoria,
  });
}

export function sleep(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}
