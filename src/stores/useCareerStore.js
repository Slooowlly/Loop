import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { isFinaleSlot } from "../utils/postRaceLanding";
import i18n from "../i18n/index.js";
import { buildBriefingContext } from "../pages/tabs/nextRaceContext";

import { initialState } from "./career/state";
import { createCareerSlice } from "./career/careerSlice";
import {
  applyCareerData,
  buildCalendarAdvanceTiming,
  buildDateSequence,
  buildTemporalUiState,
  buildWeeksFromNews,
  deriveAcceptedSpecialOfferFromWindow,
  getErrorMessage,
  loadTemporalSummary,
  sleep,
} from "./career/helpers";

export { buildCalendarAdvanceTiming } from "./career/helpers";

const useCareerStore = create((set, get) => ({
  ...initialState,

  ...createCareerSlice(set, get),

  simulateRace: async () => {
    const { careerId, nextRace, isSimulating } = get();
    if (isSimulating) {
      return null;
    }
    if (!careerId || !nextRace?.id) {
      throw new Error(i18n.t("storeErrors.noPendingRace"));
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
        lastRaceFromIracing: false, // simulou: não pisou na pista
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
      const message = getErrorMessage(error, i18n.t("storeErrors.simulateRace"));
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
          lastRaceFromIracing: true, // correu de verdade: resultado importado do sim
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
          lastRaceFromIracing: false, // reabertura de corrida antiga, não é evento novo
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

  advanceSeason: async () => {
    const { careerId } = get();
    if (!careerId) {
      throw new Error(i18n.t("storeErrors.careerNotLoaded"));
    }

    set({ isAdvancing: true, error: null });

    try {
      const result = await invoke("advance_season", { careerId });
      // A tela de "Resumo da Temporada" (EndOfSeasonView) está ocultada do
      // fluxo: guardamos o resultado (endOfSeasonResult) caso volte a ser
      // usada no futuro, mas seguimos direto para o mercado (enterPreseason),
      // que também sobrescreve o resume_context para "preseason".
      set({
        isAdvancing: false,
        endOfSeasonResult: result,
        showEndOfSeason: false,
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
      await get().enterPreseason();
      return result;
    } catch (error) {
      set({
        isAdvancing: false,
        error: getErrorMessage(error, i18n.t("storeErrors.advanceSeason")),
      });
      throw error;
    }
  },

  skipAllPendingRaces: async () => {
    const { careerId } = get();
    if (!careerId) throw new Error(i18n.t("storeErrors.careerNotLoaded"));
    set({ isAdvancing: true, error: null });
    try {
      await invoke("skip_all_pending_races", { careerId });
    } catch (error) {
      set({ isAdvancing: false, error: getErrorMessage(error, i18n.t("storeErrors.skipRaces")) });
      throw error;
    }
    // Após simular todas as corridas, avança a temporada normalmente.
    return get().advanceSeason();
  },

  // API mínima preservada para reativar o overlay quando houver dados reais.
  showChampionOverlay: (data = null) => set({ championOverlay: data ?? { demo: true } }),
  hideChampionOverlay: () => set({ championOverlay: null }),

  // DEBUG: vai direto ao mercado num cenário (agente livre + posição forçada), pra testar
  // as propostas por mérito. scenario ∈ "no_team" | "first" | "fifth".
  debugGoToMarket: async (scenario) => {
    const { careerId } = get();
    if (!careerId) throw new Error(i18n.t("storeErrors.careerNotLoaded"));
    set({ isAdvancing: true, error: null });
    try {
      await invoke("debug_prepare_market_scenario", { careerId, scenario });
    } catch (error) {
      set({ isAdvancing: false, error: getErrorMessage(error, i18n.t("storeErrors.prepMarket")) });
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
    if (!careerId) throw new Error(i18n.t("storeErrors.careerNotLoaded"));
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
    if (!careerId) throw new Error(i18n.t("storeErrors.careerNotLoaded"));

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
        error: getErrorMessage(error, i18n.t("storeErrors.processCallup")),
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
    if (!careerId) throw new Error(i18n.t("storeErrors.careerNotLoaded"));

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
        error: getErrorMessage(error, i18n.t("storeErrors.startSpecialBlock")),
      });
      throw error;
    }
  },

  loadSpecialWindowState: async () => {
    const { careerId } = get();
    if (!careerId) throw new Error(i18n.t("storeErrors.careerNotLoaded"));

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
    if (!careerId) throw new Error(i18n.t("storeErrors.careerNotLoaded"));

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
        error: getErrorMessage(error, i18n.t("storeErrors.setDailyOffer")),
      });
      throw error;
    }
  },

  advanceSpecialWindowDay: async () => {
    const { careerId } = get();
    if (!careerId) throw new Error(i18n.t("storeErrors.careerNotLoaded"));

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
        error: getErrorMessage(error, i18n.t("storeErrors.advanceSpecialDay")),
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
    if (!careerId) throw new Error(i18n.t("storeErrors.careerNotLoaded"));

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
        error: getErrorMessage(error, i18n.t("storeErrors.endSpecialBlock")),
      });
      throw error;
    }
  },

  respondToSpecialOffer: async (offerId, accept) => {
    const { careerId, playerSpecialOffers, acceptedSpecialOffer } = get();
    if (!careerId) {
      throw new Error(i18n.t("storeErrors.careerNotLoaded"));
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
        error: getErrorMessage(error, i18n.t("storeErrors.respondSpecialCallup")),
      });
      throw error;
    }
  },

  enterPreseason: async () => {
    const { careerId } = get();
    if (!careerId) {
      throw new Error(i18n.t("storeErrors.careerNotLoaded"));
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
      const message = getErrorMessage(error, i18n.t("storeErrors.enterPreseason"));
      set({ isEnteringPreseason: false, error: message });
      throw error;
    }
  },

  // `acceptedSeatId` = id da vaga que o jogador aceita nesta semana; `null` = espera.
  advanceMarketWeek: async (acceptedSeatId = null) => {
    const { careerId } = get();
    if (!careerId) {
      throw new Error(i18n.t("storeErrors.careerNotLoaded"));
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
        error: getErrorMessage(error, i18n.t("storeErrors.advancePreseasonWeek")),
      });
      throw error;
    }
  },

  // Quebra de contrato do jogador (Fase 2b.3): resolve a decisão (accept = sair pro
  // pretendente; false = ficar). Aplica no backend, limpa a oferta e recarrega o estado
  // da pré-temporada (o time do jogador mudou).
  resolvePlayerPoachOffer: async (accept) => {
    const { careerId, poachOffer } = get();
    if (!careerId) throw new Error(i18n.t("storeErrors.careerNotLoaded"));
    if (!poachOffer) throw new Error(i18n.t("storeErrors.noActivePoach"));
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
        error: getErrorMessage(error, i18n.t("storeErrors.resolvePoach")),
      });
      throw error;
    }
  },

  // DEBUG: força uma oferta de quebra de contrato pro jogador e a carrega no store,
  // pra testar a tela do leilão. Não é usado no fluxo normal.
  debugForcePlayerPoach: async () => {
    const { careerId } = get();
    if (!careerId) throw new Error(i18n.t("storeErrors.careerNotLoaded"));
    const offer = await invoke("debug_force_player_poach_offer", { careerId });
    set({ poachOffer: offer });
    return offer;
  },

  respondToProposal: async (proposalId, accept) => {
    const { careerId } = get();
    if (!careerId) {
      throw new Error(i18n.t("storeErrors.careerNotLoaded"));
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
        error: getErrorMessage(error, i18n.t("storeErrors.respondProposal")),
      });
      throw error;
    }
  },

  finalizePreseason: async () => {
    const { careerId } = get();
    if (!careerId) {
      throw new Error(i18n.t("storeErrors.careerNotLoaded"));
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
      const message = getErrorMessage(error, i18n.t("storeErrors.startNewSeason"));
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
  // silenciosa (a tela cai no fluxo normal). `buildBriefingContext` vem do módulo puro
  // `nextRaceContext` (não do componente), então o import é estático — sem ciclo.
  prefetchPreRaceBriefing: async () => {
    const { careerId, player, playerTeam, season, nextRace, nextRaceBriefing, preRaceAi, preRaceStandings } =
      get();
    const raceId = nextRace?.id;
    if (!careerId || !raceId || !playerTeam?.categoria) return;
    // Já temos IA E os standings desta etapa em cache → nada a buscar.
    if (preRaceAi?.raceId === raceId && preRaceStandings?.raceId === raceId) return;

    try {
      const [drivers, teams, phraseHistory, forecast] = await Promise.all([
        invoke("get_drivers_by_category", { careerId, category: playerTeam.categoria }),
        invoke("get_teams_standings", { careerId, category: playerTeam.categoria }),
        invoke("get_briefing_phrase_history", { careerId }).catch(() => ({
          season_number: 0,
          entries: [],
        })),
        invoke("get_breakdown_forecast", { careerId }).catch(() => null),
      ]);

      // Outra etapa pode ter virado a corrente enquanto buscávamos — aborta se mudou.
      if (get().nextRace?.id !== raceId) return;

      const driverStandings = Array.isArray(drivers) ? drivers : [];
      const teamStandings = Array.isArray(teams) ? teams : [];
      const normalizedPhraseHistory =
        phraseHistory && Array.isArray(phraseHistory.entries)
          ? phraseHistory
          : { season_number: 0, entries: [] };
      const normalizedForecast = forecast && forecast.available ? forecast : null;

      // Guarda os standings já buscados para a Sala de Estratégia abrir os Favoritos na
      // hora, sem re-disparar get_drivers_by_category/get_teams_standings ao montar.
      set({
        preRaceStandings: {
          raceId,
          driverStandings,
          teamStandings,
          phraseHistory: normalizedPhraseHistory,
          breakdownForecast: normalizedForecast,
        },
      });

      // A IA já pode estar em cache desta etapa (só faltavam os standings): não regenera.
      if (preRaceAi?.raceId === raceId) return;

      const { aiFacts } = buildBriefingContext({
        player,
        playerTeam,
        season,
        nextRace,
        nextRaceBriefing,
        driverStandings,
        teamStandings,
        briefingPhraseHistory: normalizedPhraseHistory,
        playerInterests: get().playerInterests,
        breakdownForecast: normalizedForecast,
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
        error: getErrorMessage(error, i18n.t("storeErrors.advanceCalendar")),
      });
      throw error;
    }
  },
}));

export default useCareerStore;
