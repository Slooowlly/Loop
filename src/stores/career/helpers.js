import { invoke } from "@tauri-apps/api/core";

// Funções puras (e alguns fetches sem estado) compartilhadas pelos slices do
// store de carreira. Nada aqui toca `set`/`get` — os slices é que decidem.

export function getErrorMessage(error, fallback) {
  return typeof error === "string" ? error : error?.toString?.() ?? fallback;
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
    return {
      showEndOfSeason: false,
      showPreseason: false,
      endOfSeasonResult: null,
      preseasonState: null,
      preseasonWeeks: [],
      playerProposals: [],
      preseasonFreeAgents: [],
      playerSpecialOffers: [],
      acceptedSpecialOffer: null,
    };
  }

  if (resumeContext.active_view === "end_of_season" && resumeContext.end_of_season_result) {
    return {
      showEndOfSeason: true,
      showPreseason: false,
      endOfSeasonResult: resumeContext.end_of_season_result,
      preseasonState: null,
      preseasonWeeks: [],
      playerProposals: [],
      playerSpecialOffers: [],
      acceptedSpecialOffer: null,
    };
  }

  if (resumeContext.active_view === "preseason") {
    return buildPreseasonUiState(careerId);
  }

  return {
    showEndOfSeason: false,
    showPreseason: false,
    endOfSeasonResult: null,
    preseasonState: null,
    preseasonWeeks: [],
    playerProposals: [],
    playerSpecialOffers: [],
    acceptedSpecialOffer: null,
  };
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

  return {
    showEndOfSeason: false,
    showPreseason: true,
    endOfSeasonResult: null,
    preseasonState: state,
    preseasonWeeks: buildWeeksFromNews(news),
    playerProposals: proposals,
    preseasonFreeAgents: freeAgents,
  };
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
