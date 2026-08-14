import { invoke } from "@tauri-apps/api/core";

import i18n from "../../i18n/index.js";
import {
  applyCareerData,
  buildTemporalUiState,
  buildWeeksFromNews,
  contextoDeTelaLimpo,
  getErrorMessage,
  loadTemporalSummary,
} from "./helpers";

// Slice de MERCADO: pré-temporada, avanço das semanas de mercado, propostas ao
// jogador, quebra de contrato (poaching) e a virada para a temporada nova.
export const createMarketSlice = (set, get) => ({
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

  // Fecha a pré-temporada e devolve a carreira já na temporada nova.
  //
  // A trava de voo não é enfeite: `finalize_preseason` cria o calendário e promove a fase, e
  // o botão "Iniciar temporada" chega a ela por três caminhos (o avanço de semana e os dois
  // modais de confirmação), nenhum deles desabilitado enquanto a chamada roda. Dois cliques
  // rápidos disparavam duas finalizações contra o mesmo save.
  finalizePreseason: async () => {
    const { careerId, isFinalizingPreseason } = get();
    if (isFinalizingPreseason) {
      return null;
    }
    if (!careerId) {
      throw new Error(i18n.t("storeErrors.careerNotLoaded"));
    }

    set({ isFinalizingPreseason: true, error: null });

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
        ...contextoDeTelaLimpo(),
        lastRaceResult: null,
        otherCategoriesResult: null,
        isAdvancing: false,
        isAdvancingWeek: false,
        isFinalizingPreseason: false,
        isRespondingProposal: false,
        isDirty: false,
      });

      return data;
    } catch (error) {
      const message = getErrorMessage(error, i18n.t("storeErrors.startNewSeason"));
      set({ isFinalizingPreseason: false, error: message });
      throw error;
    }
  },
});

export default createMarketSlice;
