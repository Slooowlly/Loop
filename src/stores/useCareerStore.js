import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import i18n from "../i18n/index.js";
import { buildBriefingContext } from "../pages/tabs/nextRaceContext";

import { initialState } from "./career/state";
import { createCareerSlice } from "./career/careerSlice";
import { createRaceSlice } from "./career/raceSlice";
import { createMarketSlice } from "./career/marketSlice";
import {
  buildCalendarAdvanceTiming,
  buildDateSequence,
  buildTemporalUiState,
  deriveAcceptedSpecialOfferFromWindow,
  getErrorMessage,
  loadTemporalSummary,
  sleep,
} from "./career/helpers";

export { buildCalendarAdvanceTiming } from "./career/helpers";

const useCareerStore = create((set, get) => ({
  ...initialState,

  ...createCareerSlice(set, get),
  ...createRaceSlice(set, get),
  ...createMarketSlice(set, get),

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
