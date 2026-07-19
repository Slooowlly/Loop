import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { isLegacySeasonPhase } from "../utils/seasonPhases";
import { isFinaleSlot } from "../utils/postRaceLanding";

const initialState = {
  isLoaded: false,
  isLoading: false,
  isSimulating: false,
  isAdvancing: false,
  isCalendarAdvancing: false,
  isAdvancingWeek: false,
  isEnteringPreseason: false,
  isRespondingProposal: false,
  isResolvingPoach: false,
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
  // Prévia por IA pré-buscada durante a animação de avanço (evita o flash template→IA).
  preRaceAi: null,
  temporalSummary: null,
  calendarDisplayDate: null,
  displayDaysUntilNextEvent: null,
  totalDrivers: 0,
  totalTeams: 0,
  // Pilotos de interesse do jogador (1 Nemesis + até 2 Rivais) — decoram os nomes
  // com o marcador de rivalidade (💥/🔥). { nemesis, rivais } ou null enquanto carrega.
  playerInterests: null,
  lastRaceResult: null,
  // ID da corrida do pós-corrida (race_id) — usado para reconstruir o timeline de clima.
  lastRaceId: null,
  // Avaliação de carreira do pós-corrida (expectativa vs resultado, nota, frases).
  // Null para corridas sem avaliação — a tela trata e nunca quebra.
  lastRaceEvaluation: null,
  // Análise de telemetria (ritmo/consistência/rival). Null se não houve.
  lastRaceTelemetry: null,
  // Fatura de manutenção do carro do pós-corrida (gasolina/pneus + conserto). Null se não houve.
  lastRaceMaintenance: null,
  otherCategoriesResult: null,
  showResult: false,
  showRaceBriefing: false,
  // A corrida recém-terminada era o final de campeonato (thematic_slot final)?
  // Usado pelo Dashboard para forçar a aba "Notícias" no pós-corrida.
  lastRaceWasFinale: false,
  // A tela de resultado aberta é de uma corrida ACABADA AGORA (true) ou é uma
  // reabertura de corrida antiga pela Home (false)? Só a fresca aciona a lógica
  // de aba pós-corrida.
  resultIsFresh: false,
  // Conserto do carro a mostrar no pop-up ao abrir o resultado (import do iRacing).
  iracingRepair: null,
  // Trava p/ o poller do iRacing não importar a mesma corrida duas vezes em voo.
  iracingImporting: false,
  preseasonState: null,
  preseasonWeeks: [],
  lastMarketWeekResult: null,
  playerProposals: [],
  transferWindow: null,
  preseasonFreeAgents: [],
  poachOffer: null,
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

async function buildPreseasonUiState(careerId) {
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

      let preseasonPhaseState = {};
      if (data.season?.fase === "PreTemporada" && !resumeUiState.showPreseason) {
        preseasonPhaseState = await buildPreseasonUiState(data.career_id).catch((error) => {
          console.error("Erro ao carregar pré-temporada ativa:", error);
          return {};
        });
      }

      // LEGADO 9D: saves pré-v33 em JanelaConvocacao ainda restauram a tela especial.
      let convocationResumeState = {};
      if (isLegacySeasonPhase(data.season?.fase) && data.season?.fase === "JanelaConvocacao") {
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
        ...preseasonPhaseState,
        ...convocationResumeState,
        isDirty: false,
      });
      // Rivalidades de interesse (Nemesis/Rivais) — fire-and-forget; decora os nomes.
      void get().loadPlayerInterests();
      return data;
    } catch (error) {
      const message = getErrorMessage(error, "Erro ao carregar carreira.");
      set({ isLoading: false, error: message });
      throw error;
    }
  },

  // Carrega os pilotos de interesse (Nemesis + Rivais) do backend. Best-effort:
  // qualquer falha deixa o marcador ausente, sem quebrar a tela.
  loadPlayerInterests: async () => {
    const { careerId } = get();
    if (!careerId) return;
    try {
      const interests = await invoke("get_player_interests", { careerId });
      set({ playerInterests: interests });
    } catch {
      /* sem rivalidades / falha — marcador simplesmente não aparece */
    }
  },

  setCareerFromCreation: (createResult) => {
    set({
      isLoaded: true,
      careerId: createResult?.career_id ?? null,
    });
  },

  simulateRace: async () => {
    const { careerId, nextRace, isSimulating } = get();
    if (isSimulating) {
      return null;
    }
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
        lastRaceId: nextRace.id,
        lastRaceEvaluation: result.evaluation ?? null, // mesmo cérebro do import iRacing
        lastRaceTelemetry: null, // sim offline não tem telemetria ao vivo (sem gráficos)
        lastRaceMaintenance: result.maintenance ?? null,
        lastRaceWasFinale: isFinaleSlot(nextRace.thematic_slot),
        resultIsFresh: true,
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

  // GATILHO AUTOMÁTICO do iRacing: pergunta ao backend se o resultado da próxima
  // corrida já foi gravado (jogador terminou/saiu). Se sim, importa e abre a tela
  // de resultado sozinha. Chamado em loop por um poller no Dashboard. Idempotente.
  pollIracingResult: async () => {
    const { careerId, showResult, isSimulating, iracingImporting, nextRace } = get();
    if (!careerId || showResult || isSimulating || iracingImporting) return;
    set({ iracingImporting: true });
    try {
      const payload = await invoke("iracing_auto_import_if_ready", { careerId });
      if (payload?.race_result) {
        set({
          lastRaceResult: payload.race_result,
          lastRaceId: payload.summary?.race_id ?? null,
          lastRaceEvaluation: payload.evaluation ?? null,
          lastRaceTelemetry: payload.telemetry ?? null,
          lastRaceMaintenance: payload.summary?.maintenance ?? null,
          lastRaceWasFinale: isFinaleSlot(nextRace?.thematic_slot),
          resultIsFresh: true,
          showResult: true,
          showRaceBriefing: false,
          isDirty: true,
          iracingRepair:
            payload.summary?.repair_cost > 0 ? payload.summary : null,
        });
      }
    } catch {
      // silencioso — "ainda não pronto" não é erro; tenta de novo no próximo tick.
    } finally {
      set({ iracingImporting: false });
    }
  },

  // Reabre a CLASSIFICAÇÃO FINAL de uma corrida já disputada (pela Home, duplo
  // clique no R{n}). Lê a tela salva no disco (resultado + avaliação + telemetria).
  // Sem tela salva (corrida antiga / outra categoria) → no-op silencioso.
  openSavedRaceScreen: async (category, rodada) => {
    const { careerId } = get();
    if (!careerId || !category || !rodada) return false;
    try {
      const screen = await invoke("get_saved_race_screen", { careerId, category, rodada });
      if (screen?.race_result) {
        set({
          lastRaceResult: screen.race_result,
          lastRaceId: screen.race_id ?? null,
          lastRaceEvaluation: screen.evaluation ?? null,
          lastRaceTelemetry: screen.telemetry ?? null,
          lastRaceMaintenance: screen.maintenance ?? null,
          iracingRepair: null,
          // Reabertura de corrida antiga: NÃO aciona a aba pós-corrida.
          lastRaceWasFinale: false,
          resultIsFresh: false,
          showResult: true,
          showRaceBriefing: false,
        });
        return true;
      }
    } catch {
      /* sem tela salva desta corrida — ignora */
    }
    return false;
  },

  // Fecha só o pop-up de conserto, mantendo a tela de resultado.
  dismissIracingRepair: () => set({ iracingRepair: null }),

  dismissResult: async () => {
    const { careerId, loadCareer } = get();
    set({
      showResult: false,
      iracingRepair: null,
      lastRaceEvaluation: null,
      lastRaceTelemetry: null,
      resultIsFresh: false,
    });

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

  // DEBUG: vai direto ao mercado num cenário (agente livre + posição forçada), pra testar
  // as propostas por mérito. scenario ∈ "no_team" | "first" | "fifth".
  debugGoToMarket: async (scenario) => {
    const { careerId } = get();
    if (!careerId) throw new Error("Carreira não carregada.");
    set({ isAdvancing: true, error: null });
    try {
      await invoke("debug_prepare_market_scenario", { careerId, scenario });
    } catch (error) {
      set({ isAdvancing: false, error: getErrorMessage(error, "Erro ao preparar mercado (debug).") });
      throw error;
    }
    const result = await get().advanceSeason();
    // O avanço recalcula standings só de quem correu e exclui o jogador agente livre
    // (posição vira null no arquivo). Carimba a posição forçada DEPOIS do avanço, pra
    // o cenário de campeão/mediano habilitar as ofertas de promoção no mercado.
    try {
      await invoke("debug_stamp_player_championship", { careerId, scenario });
    } catch (error) {
      console.error("[debug] falha ao carimbar posição do jogador:", error);
    }
    return result;
  },

  // DEBUG: simula os leilões de poaching (quebra de contrato entre IAs) e devolve o
  // raio-x de cada assédio. NÃO altera o save — o backend desfaz tudo (rollback).
  debugPoachingAuctions: async () => {
    const { careerId } = get();
    if (!careerId) throw new Error("Carreira não carregada.");
    return invoke("debug_poaching_auctions", { careerId });
  },

  // ── Bloco Especial ───────────────────────────────────────────────────────────
  // LEGADO 9D: comandos especiais ficam acessíveis apenas para saves pré-v33 em voo.

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
    const { careerId, loadCareer, isConvocating } = get();
    if (isConvocating) return null;
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
      const [state, proposals, freeAgents, transferWindow, poachOffer] = await Promise.all([
        invoke("get_preseason_state", { careerId }),
        invoke("get_player_proposals", { careerId }).catch(() => []),
        invoke("get_preseason_free_agents", { careerId }).catch(() => []),
        invoke("get_transfer_window_state", { careerId }).catch(() => null),
        invoke("get_player_poach_offer", { careerId }).catch(() => null),
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
        transferWindow,
        preseasonFreeAgents: freeAgents,
        poachOffer,
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

  // `acceptedSeatId` = id da vaga que o jogador aceita nesta semana; `null` = espera.
  advanceMarketWeek: async (acceptedSeatId = null) => {
    const { careerId } = get();
    if (!careerId) {
      throw new Error("Carreira não carregada.");
    }

    set({ isAdvancingWeek: true, error: null });

    try {
      const weekResult = await invoke("advance_market_week", {
        careerId,
        acceptedSeatId,
      });

      const [state, freeAgents, transferWindow, poachOffer] = await Promise.all([
        invoke("get_preseason_state", { careerId }),
        invoke("get_preseason_free_agents", { careerId }).catch((e) => {
          console.error("[preseason] get_preseason_free_agents falhou:", e);
          return get().preseasonFreeAgents ?? [];
        }),
        invoke("get_transfer_window_state", { careerId }).catch(
          () => get().transferWindow,
        ),
        invoke("get_player_poach_offer", { careerId }).catch(() => get().poachOffer ?? null),
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
        transferWindow,
        preseasonFreeAgents: freeAgents,
        poachOffer,
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

  // Quebra de contrato do jogador (Fase 2b.3): resolve a decisão (accept = sair pro
  // pretendente; false = ficar). Aplica no backend, limpa a oferta e recarrega o estado
  // da pré-temporada (o time do jogador mudou).
  resolvePlayerPoachOffer: async (accept) => {
    const { careerId, poachOffer } = get();
    if (!careerId) throw new Error("Carreira não carregada.");
    if (!poachOffer) throw new Error("Nenhuma proposta de quebra ativa.");
    set({ isResolvingPoach: true, error: null });
    try {
      const outcome = await invoke("resolve_player_poach_offer", {
        careerId,
        offer: poachOffer,
        accept,
      });
      const [state, transferWindow] = await Promise.all([
        invoke("get_preseason_state", { careerId }).catch(() => get().preseasonState),
        invoke("get_transfer_window_state", { careerId }).catch(() => get().transferWindow),
      ]);
      set({
        poachOffer: null,
        preseasonState: state,
        transferWindow,
        isResolvingPoach: false,
        isDirty: true,
      });
      return outcome;
    } catch (error) {
      set({
        isResolvingPoach: false,
        error: getErrorMessage(error, "Erro ao resolver a quebra de contrato."),
      });
      throw error;
    }
  },

  // DEBUG: força uma oferta de quebra de contrato pro jogador e a carrega no store,
  // pra testar a tela do leilão. Não é usado no fluxo normal.
  debugForcePlayerPoach: async () => {
    const { careerId } = get();
    if (!careerId) throw new Error("Carreira não carregada.");
    const offer = await invoke("debug_force_player_poach_offer", { careerId });
    set({ poachOffer: offer });
    return offer;
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

  // Pré-busca a prévia por IA da próxima corrida ENQUANTO o calendário avança (a
  // animação dá tempo de sobra). Monta os mesmos fatos da Sala de Estratégia, gera
  // no servidor (cacheado por etapa) e guarda em `preRaceAi` para a tela abrir já com
  // o texto pronto — sem o flash template→IA. Fire-and-forget; qualquer falha é
  // silenciosa (a tela cai no fluxo normal). Import dinâmico evita ciclo de módulos.
  prefetchPreRaceBriefing: async () => {
    const { careerId, player, playerTeam, season, nextRace, nextRaceBriefing, preRaceAi } = get();
    const raceId = nextRace?.id;
    if (!careerId || !raceId || !playerTeam?.categoria) return;
    if (preRaceAi?.raceId === raceId) return; // já temos desta etapa

    try {
      const [{ buildBriefingContext }, drivers, teams, phraseHistory] = await Promise.all([
        import("../pages/tabs/NextRaceTab"),
        invoke("get_drivers_by_category", { careerId, category: playerTeam.categoria }),
        invoke("get_teams_standings", { careerId, category: playerTeam.categoria }),
        invoke("get_briefing_phrase_history", { careerId }).catch(() => ({
          season_number: 0,
          entries: [],
        })),
      ]);

      // Outra etapa pode ter virado a corrente enquanto buscávamos — aborta se mudou.
      if (get().nextRace?.id !== raceId) return;

      const { aiFacts } = buildBriefingContext({
        player,
        playerTeam,
        season,
        nextRace,
        nextRaceBriefing,
        driverStandings: Array.isArray(drivers) ? drivers : [],
        teamStandings: Array.isArray(teams) ? teams : [],
        briefingPhraseHistory:
          phraseHistory && Array.isArray(phraseHistory.entries)
            ? phraseHistory
            : { season_number: 0, entries: [] },
        playerInterests: get().playerInterests,
      });
      if (!aiFacts || !aiFacts.trim()) return;

      const res = await invoke("pre_race_briefing_ai", { careerId, raceId, facts: aiFacts });
      if (get().nextRace?.id !== raceId) return;
      if (res?.narrative && res?.team_voice) {
        set({
          preRaceAi: {
            raceId,
            headline: res.headline ?? null,
            narrative: res.narrative,
            teamVoice: res.team_voice,
          },
        });
      }
    } catch (_error) {
      // silencioso: a Sala de Estratégia gera por conta própria ao abrir.
    }
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

    // Aproveita a animação para gerar a prévia por IA em paralelo (fire-and-forget),
    // assim a Sala de Estratégia abre já com o texto pronto.
    void get().prefetchPreRaceBriefing();

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
