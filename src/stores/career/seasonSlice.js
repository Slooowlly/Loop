import { invoke } from "@tauri-apps/api/core";

import i18n from "../../i18n/index.js";
import {
  buildCalendarAdvanceTiming,
  buildDateSequence,
  buildTemporalUiState,
  deriveAcceptedSpecialOfferFromWindow,
  getErrorMessage,
  loadTemporalSummary,
  sleep,
} from "./helpers";

// Slice de TEMPORADA: virada de temporada, Bloco Especial (legado 9D) e a
// animação de avanço do calendário até a próxima etapa.
export const createSeasonSlice = (set, get) => ({
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

  updateSeason: (seasonData) => {
    set({ season: seasonData });
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
});

export default createSeasonSlice;
