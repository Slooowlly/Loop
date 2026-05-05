import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

const initialState = {
  isLoaded: false,
  isLoading: false,
  isSimulating: false,
  isAdvancing: false,
  isCalendarAdvancing: false,
  isAdvancingWeek: false,
  isEnteringPreseason: false,
  isRespondingProposal: false,
  isConvocating: false,
  isDirty: false,
  lastSaved: null,
  error: null,
  careerId: null,
  difficulty: null,
  player: null,
  playerTeam: null,
  season: null,
  nextRace: null,
  nextRaceBriefing: null,
  temporalSummary: null,
  calendarDisplayDate: null,
  displayDaysUntilNextEvent: null,
  totalDrivers: 0,
  totalTeams: 0,
  lastRaceResult: null,
  otherCategoriesResult: null,
  showResult: false,
  showRaceBriefing: false,
  preseasonState: null,
  preseasonWeeks: [],
  lastMarketWeekResult: null,
  playerProposals: [],
  preseasonFreeAgents: [],
  endOfSeasonResult: null,
  showEndOfSeason: false,
  showPreseason: false,
  convocationResult: null,
  showConvocation: false,
  specialWindowState: null,
  playerSpecialOffers: [],
  acceptedSpecialOffer: null,
};

function getErrorMessage(error, fallback) {
  return typeof error === "string" ? error : error?.toString?.() ?? fallback;
}

function applyCareerData(data) {
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
    otherCategoriesResult: null,
  };
}

function buildWeeksFromNews(newsItems = []) {
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

function buildDateSequence(startDate, endDate) {
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

function buildTemporalUiState(temporalSummary) {
  return {
    temporalSummary,
    calendarDisplayDate: temporalSummary?.current_display_date ?? null,
    displayDaysUntilNextEvent: temporalSummary?.days_until_next_event ?? null,
  };
}

function deriveAcceptedSpecialOffer(data) {
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

function deriveAcceptedSpecialOfferFromWindow(windowState) {
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

async function buildResumeUiState(careerId, resumeContext) {
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

async function loadTemporalSummary(careerId, season, playerTeam) {
  if (!careerId || !season?.id || !playerTeam?.categoria) {
    return null;
  }

  return invoke("get_temporal_summary", {
    careerId,
    seasonId: season.id,
    playerCategory: playerTeam.categoria,
  });
}

function sleep(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

const useCareerStore = create((set, get) => ({
  ...initialState,

  loadCareer: async (careerId) => {
    set({
      isLoading: true,
      isSimulating: false,
      isCalendarAdvancing: false,
      isConvocating: false,
      error: null,
      showResult: false,
      showRaceBriefing: false,
      lastRaceResult: null,
      otherCategoriesResult: null,
      showEndOfSeason: false,
      showPreseason: false,
      endOfSeasonResult: null,
      preseasonState: null,
      preseasonWeeks: [],
      playerProposals: [],
      preseasonFreeAgents: [],
      showConvocation: false,
      convocationResult: null,
      specialWindowState: null,
      playerSpecialOffers: [],
      acceptedSpecialOffer: null,
    });

    try {
      const data = await invoke("load_career", { careerId });
      const temporalSummary = await loadTemporalSummary(
        data.career_id,
        data.season,
        data.player_team,
      ).catch((error) => {
        console.error("Erro ao carregar resumo temporal:", error);
        return null;
      });
      const resumeUiState = await buildResumeUiState(
        data.career_id,
        data.resume_context,
      ).catch((error) => {
        console.error("Erro ao restaurar contexto salvo da carreira:", error);
        return {
          showEndOfSeason: false,
          showPreseason: false,
          endOfSeasonResult: null,
          preseasonState: null,
          preseasonWeeks: [],
          playerProposals: [],
        };
      });

      // Se a carreira foi salva no meio da janela de convocação, restaura a tela.
      let convocationResumeState = {};
      if (data.season?.fase === "JanelaConvocacao") {
        const windowState = await invoke("get_special_window_state", {
          careerId: data.career_id,
        }).catch(() => null);

        convocationResumeState = {
          showConvocation: true,
          convocationResult: null,
          specialWindowState: windowState,
          playerSpecialOffers: windowState?.player_offers ?? [],
          acceptedSpecialOffer:
            deriveAcceptedSpecialOfferFromWindow(windowState) ?? deriveAcceptedSpecialOffer(data),
        };
      }

      set({
        ...applyCareerData(data),
        ...buildTemporalUiState(temporalSummary),
        ...resumeUiState,
        ...convocationResumeState,
        isDirty: false,
      });
      return data;
    } catch (error) {
      const message = getErrorMessage(error, "Erro ao carregar carreira.");
      set({ isLoading: false, error: message });
      throw error;
    }
  },

  setCareerFromCreation: (createResult) => {
    set({
      isLoaded: true,
      careerId: createResult?.career_id ?? null,
    });
  },

  simulateRace: async () => {
    const { careerId, nextRace } = get();
    if (!careerId || !nextRace?.id) {
      throw new Error("Não existe corrida pendente para simular.");
    }

    set({ isSimulating: true, error: null });

    try {
      const result = await invoke("simulate_race_weekend", {
        careerId,
        raceId: nextRace.id,
      });

      set({
        lastRaceResult: result.player_race,
        otherCategoriesResult: result.other_categories,
        isSimulating: false,
        showResult: true,
        showRaceBriefing: false,
        isDirty: true,
      });

      return result;
    } catch (error) {
      const message = getErrorMessage(error, "Erro ao simular corrida.");
      set({ isSimulating: false, error: message });
      throw error;
    }
  },

  dismissResult: async () => {
    const { careerId, loadCareer } = get();
    set({ showResult: false });

    if (!careerId) return;

    try {
      await loadCareer(careerId);
      // Ao fechar o resultado da última corrida regular, aciona a janela de convocação.
    } catch (error) {
      console.error("Erro ao recarregar carreira:", error);
    }
  },

  clearLastResult: () => {
    set({
      lastRaceResult: null,
      otherCategoriesResult: null,
      showResult: false,
      showRaceBriefing: false,
    });
  },

  clearCareer: () => {
    set({ ...initialState });
  },

  advanceSeason: async () => {
    const { careerId } = get();
    if (!careerId) {
      throw new Error("Carreira não carregada.");
    }

    set({ isAdvancing: true, error: null });

    try {
      const result = await invoke("advance_season", { careerId });
      set({
        isAdvancing: false,
        endOfSeasonResult: result,
        showEndOfSeason: true,
        showRaceBriefing: false,
        showPreseason: false,
        preseasonState: null,
        preseasonWeeks: [],
        playerProposals: [],
        playerSpecialOffers: [],
        acceptedSpecialOffer: null,
        nextRace: null,
        nextRaceBriefing: null,
        temporalSummary: null,
        calendarDisplayDate: null,
        displayDaysUntilNextEvent: null,
        lastRaceResult: null,
        otherCategoriesResult: null,
        isDirty: true,
      });
      return result;
    } catch (error) {
      set({
        isAdvancing: false,
        error: getErrorMessage(error, "Erro ao avancar temporada."),
      });
      throw error;
    }
  },

  skipAllPendingRaces: async () => {
    const { careerId } = get();
    if (!careerId) throw new Error("Carreira não carregada.");
    set({ isAdvancing: true, error: null });
    try {
      await invoke("skip_all_pending_races", { careerId });
    } catch (error) {
      set({ isAdvancing: false, error: getErrorMessage(error, "Erro ao pular corridas.") });
      throw error;
    }
    // Após simular todas as corridas, avança a temporada normalmente.
    return get().advanceSeason();
  },

  // ── Bloco Especial ───────────────────────────────────────────────────────────

  /**
   * Abre a Janela de Convocação: transiciona BlocoRegular → JanelaConvocacao,
   * executa a convocação e armazena o resultado para exibição.
   */
  runConvocationWindow: async () => {
    const { careerId } = get();
    if (!careerId) throw new Error("Carreira não carregada.");

    set({ isConvocating: true, error: null });

    try {
      await invoke("advance_to_convocation_window", { careerId });
      const result = await invoke("run_convocation_window", { careerId });
      const windowState = await invoke("get_special_window_state", { careerId }).catch(
        () => null,
      );
      set({
        isConvocating: false,
        convocationResult: result,
        showConvocation: true,
        specialWindowState: windowState,
        playerSpecialOffers: windowState?.player_offers ?? [],
        acceptedSpecialOffer: deriveAcceptedSpecialOfferFromWindow(windowState),
        isDirty: true,
      });
      return result;
    } catch (error) {
      set({
        isConvocating: false,
        error: getErrorMessage(error, "Erro ao processar convocação."),
      });
      throw error;
    }
  },

  /**
   * Confirma o início do Bloco Especial: JanelaConvocacao → BlocoEspecial.
   * Gera o calendário especial (semanas 41–50) e recarrega a carreira.
   */
  confirmSpecialBlock: async () => {
    const { careerId, loadCareer } = get();
    if (!careerId) throw new Error("Carreira não carregada.");

    set({ isConvocating: true, error: null });

    try {
      await invoke("iniciar_bloco_especial", { careerId });
      set({
        showConvocation: false,
        convocationResult: null,
        playerSpecialOffers: [],
        acceptedSpecialOffer: null,
      });
      const data = await loadCareer(careerId);
      set({ isConvocating: false });
      return data;
    } catch (error) {
      set({
        isConvocating: false,
        error: getErrorMessage(error, "Erro ao iniciar bloco especial."),
      });
      throw error;
    }
  },

  loadSpecialWindowState: async () => {
    const { careerId } = get();
    if (!careerId) throw new Error("Carreira não carregada.");

    const windowState = await invoke("get_special_window_state", { careerId });
    set({
      specialWindowState: windowState,
      playerSpecialOffers: windowState?.player_offers ?? [],
      acceptedSpecialOffer: deriveAcceptedSpecialOfferFromWindow(windowState),
      showConvocation: true,
      error: null,
    });
    return windowState;
  },

  acceptSpecialOfferForDay: async (offerId) => {
    const { careerId } = get();
    if (!careerId) throw new Error("Carreira não carregada.");

    set({ isConvocating: true, error: null });

    try {
      const windowState = await invoke("accept_special_offer_for_day", {
        careerId,
        offerId,
      });

      set({
        isConvocating: false,
        specialWindowState: windowState,
        playerSpecialOffers: windowState?.player_offers ?? [],
        acceptedSpecialOffer: deriveAcceptedSpecialOfferFromWindow(windowState),
        isDirty: true,
      });

      return windowState;
    } catch (error) {
      set({
        isConvocating: false,
        error: getErrorMessage(error, "Erro ao definir oferta ativa do dia."),
      });
      throw error;
    }
  },

  advanceSpecialWindowDay: async () => {
    const { careerId } = get();
    if (!careerId) throw new Error("Carreira não carregada.");

    set({ isConvocating: true, error: null });

    try {
      const windowState = await invoke("advance_special_window_day", {
        careerId,
      });

      set({
        isConvocating: false,
        specialWindowState: windowState,
        playerSpecialOffers: windowState?.player_offers ?? [],
        acceptedSpecialOffer: deriveAcceptedSpecialOfferFromWindow(windowState),
        isDirty: true,
      });

      return windowState;
    } catch (error) {
      set({
        isConvocating: false,
        error: getErrorMessage(error, "Erro ao avancar dia da janela especial."),
      });
      throw error;
    }
  },

  /**
   * Encerra o Bloco Especial: simula todas as corridas especiais pendentes,
   * transiciona BlocoEspecial → PosEspecial e faz a desmontagem dos contratos.
   * Após isso, advance_season fica disponível normalmente.
   */
  finishSpecialBlock: async () => {
    const { careerId, loadCareer } = get();
    if (!careerId) throw new Error("Carreira não carregada.");

    set({ isConvocating: true, error: null });

    try {
      await invoke("simulate_special_block", { careerId });
      await invoke("encerrar_bloco_especial", { careerId });
      await invoke("run_pos_especial", { careerId });
      const data = await loadCareer(careerId);
      set({ isConvocating: false });
      return data;
    } catch (error) {
      set({
        isConvocating: false,
        error: getErrorMessage(error, "Erro ao encerrar bloco especial."),
      });
      throw error;
    }
  },

  respondToSpecialOffer: async (offerId, accept) => {
    const { careerId, playerSpecialOffers, acceptedSpecialOffer } = get();
    if (!careerId) {
      throw new Error("Carreira não carregada.");
    }

    const selectedOffer =
      playerSpecialOffers.find((offer) => offer.id === offerId) ?? null;

    set({ isConvocating: true, error: null });

    try {
      const response = await invoke("respond_player_special_offer", {
        careerId,
        offerId,
        accept,
      });
      const pendingOffers =
        response.remaining_offers > 0
          ? await invoke("get_player_special_offers", { careerId }).catch(() => [])
          : [];

      set({
        isConvocating: false,
        playerSpecialOffers: pendingOffers,
        acceptedSpecialOffer:
          accept && selectedOffer
            ? {
                ...selectedOffer,
                special_category:
                  response.special_category ?? selectedOffer.special_category,
              }
            : acceptedSpecialOffer,
        isDirty: true,
      });

      return response;
    } catch (error) {
      set({
        isConvocating: false,
        error: getErrorMessage(error, "Erro ao responder convocação especial."),
      });
      throw error;
    }
  },

  enterPreseason: async () => {
    const { careerId } = get();
    if (!careerId) {
      throw new Error("Carreira não carregada.");
    }

    set({ isEnteringPreseason: true, error: null });

    try {
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
      await invoke("set_career_resume_context", {
        careerId,
        activeView: "preseason",
        endOfSeasonResult: null,
      });

      set({
        isEnteringPreseason: false,
        showEndOfSeason: false,
        showRaceBriefing: false,
        showPreseason: true,
        showConvocation: false,
        convocationResult: null,
        specialWindowState: null,
        preseasonState: state,
        preseasonWeeks: buildWeeksFromNews(news),
        lastMarketWeekResult: null,
        playerProposals: proposals,
        preseasonFreeAgents: freeAgents,
        playerSpecialOffers: [],
        acceptedSpecialOffer: null,
        error: null,
      });

      return state;
    } catch (error) {
      const message = getErrorMessage(error, "Erro ao entrar na pré-temporada.");
      set({ isEnteringPreseason: false, error: message });
      throw error;
    }
  },

  advanceMarketWeek: async () => {
    const { careerId } = get();
    if (!careerId) {
      throw new Error("Carreira não carregada.");
    }

    set({ isAdvancingWeek: true, error: null });

    try {
      const weekResult = await invoke("advance_market_week", { careerId });

      let proposals = get().playerProposals;
      if ((weekResult.player_proposals?.length ?? 0) > 0) {
        proposals = await invoke("get_player_proposals", { careerId });
      }

      const [state, freeAgents] = await Promise.all([
        invoke("get_preseason_state", { careerId }),
        invoke("get_preseason_free_agents", { careerId }).catch((e) => {
          console.error("[preseason] get_preseason_free_agents falhou:", e);
          return get().preseasonFreeAgents ?? [];
        }),
      ]);
      const news = await invoke("get_news", {
        careerId,
        season: state.season_number,
        tipo: null,
        limit: 400,
      });

      set({
        preseasonWeeks: buildWeeksFromNews(news),
        preseasonState: state,
        lastMarketWeekResult: weekResult,
        playerProposals: proposals,
        preseasonFreeAgents: freeAgents,
        isAdvancingWeek: false,
        isDirty: true,
      });

      return weekResult;
    } catch (error) {
      set({
        isAdvancingWeek: false,
        error: getErrorMessage(error, "Erro ao avançar semana da pré-temporada."),
      });
      throw error;
    }
  },

  respondToProposal: async (proposalId, accept) => {
    const { careerId } = get();
    if (!careerId) {
      throw new Error("Carreira não carregada.");
    }

    set({ isRespondingProposal: true, error: null });

    try {
      const response = await invoke("respond_to_proposal", {
        careerId,
        proposalId,
        accept,
      });

      const [state, proposals] = await Promise.all([
        invoke("get_preseason_state", { careerId }).catch(() => get().preseasonState),
        response.remaining_proposals === 0
          ? Promise.resolve([])
          : invoke("get_player_proposals", { careerId }),
      ]);

      set({
        preseasonState: state,
        playerProposals: proposals,
        isRespondingProposal: false,
        isDirty: true,
      });

      return response;
    } catch (error) {
      set({
        isRespondingProposal: false,
        error: getErrorMessage(error, "Erro ao responder proposta."),
      });
      throw error;
    }
  },

  finalizePreseason: async () => {
    const { careerId } = get();
    if (!careerId) {
      throw new Error("Carreira não carregada.");
    }

    try {
      await invoke("finalize_preseason", { careerId });
      const data = await invoke("load_career", { careerId });
      const temporalSummary = await loadTemporalSummary(
        data.career_id,
        data.season,
        data.player_team,
      ).catch((error) => {
        console.error("Erro ao carregar resumo temporal:", error);
        return null;
      });

      set({
        ...applyCareerData(data),
        ...buildTemporalUiState(temporalSummary),
        showPreseason: false,
        showEndOfSeason: false,
        preseasonState: null,
        preseasonWeeks: [],
        playerProposals: [],
        endOfSeasonResult: null,
        lastRaceResult: null,
        otherCategoriesResult: null,
        isAdvancing: false,
        isAdvancingWeek: false,
        isRespondingProposal: false,
        isDirty: false,
      });

      return data;
    } catch (error) {
      const message = getErrorMessage(error, "Erro ao iniciar a nova temporada.");
      set({ error: message });
      throw error;
    }
  },

  updateSeason: (seasonData) => {
    set({ season: seasonData });
  },

  updateNextRace: (raceData) => {
    set({ nextRace: raceData });
  },

  closeRaceBriefing: () => {
    set({ showRaceBriefing: false });
  },

  startCalendarAdvance: async () => {
    const {
      careerId,
      season,
      playerTeam,
      nextRace,
      temporalSummary,
      calendarDisplayDate,
      displayDaysUntilNextEvent,
      isCalendarAdvancing,
    } = get();

    if (isCalendarAdvancing) {
      return;
    }

    let effectiveTemporalSummary = temporalSummary;
    if (!effectiveTemporalSummary) {
      effectiveTemporalSummary = await loadTemporalSummary(careerId, season, playerTeam).catch(
        (error) => {
          console.error("Erro ao sincronizar resumo temporal:", error);
          return null;
        },
      );

      if (effectiveTemporalSummary) {
        set(buildTemporalUiState(effectiveTemporalSummary));
      }
    }

    // Se não há próxima corrida do jogador E nada pendente na fase, não avança
    if (!nextRace && (!effectiveTemporalSummary || effectiveTemporalSummary.pending_in_phase === 0)) {
      return;
    }

    const targetDate =
      effectiveTemporalSummary?.next_event_display_date ?? nextRace?.display_date ?? null;
    const startDate =
      calendarDisplayDate ??
      effectiveTemporalSummary?.current_display_date ??
      targetDate;

    if (!targetDate || !startDate) {
      set({
        calendarDisplayDate: targetDate ?? startDate,
        displayDaysUntilNextEvent: 0,
        showRaceBriefing: true,
      });
      return;
    }

    if ((displayDaysUntilNextEvent ?? effectiveTemporalSummary?.days_until_next_event ?? 0) <= 0) {
      set({
        calendarDisplayDate: targetDate,
        displayDaysUntilNextEvent: 0,
        showRaceBriefing: true,
      });
      return;
    }

    const sequence = buildDateSequence(startDate, targetDate);
    if (sequence.length <= 1) {
      set({
        calendarDisplayDate: targetDate,
        displayDaysUntilNextEvent: 0,
        showRaceBriefing: true,
      });
      return;
    }

    const { stepMs } = buildCalendarAdvanceTiming(sequence.length - 1);

    set({
      isCalendarAdvancing: true,
      error: null,
      showRaceBriefing: false,
      calendarDisplayDate: sequence[0],
      displayDaysUntilNextEvent: sequence.length - 1,
    });

    try {
      for (let index = 1; index < sequence.length; index += 1) {
        await sleep(stepMs);
        set({
          calendarDisplayDate: sequence[index],
          displayDaysUntilNextEvent: sequence.length - index - 1,
        });
      }

      set({
        isCalendarAdvancing: false,
        showRaceBriefing: true,
        calendarDisplayDate: targetDate,
        displayDaysUntilNextEvent: 0,
      });
    } catch (error) {
      set({
        isCalendarAdvancing: false,
        error: getErrorMessage(error, "Erro ao avançar calendário."),
      });
      throw error;
    }
  },

  flushSave: async () => {
    const { careerId } = get();
    if (!careerId) return;

    try {
      const result = await invoke("flush_save", { careerId });
      set({ isDirty: false, lastSaved: result.last_saved });
    } catch (error) {
      console.error("Falha ao consolidar save:", error);
      throw error;
    }
  },
}));

export default useCareerStore;
