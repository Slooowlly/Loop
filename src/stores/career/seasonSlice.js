import { invoke } from "@tauri-apps/api/core";

import i18n from "../../i18n/index.js";
import {
  buildCalendarAdvanceTiming,
  buildDateSequence,
  buildTemporalUiState,
  contextoDeTelaLimpo,
  getErrorMessage,
  loadTemporalSummary,
  sleep,
} from "./helpers";

// Slice de TEMPORADA: virada de temporada, pop-up de Campeão e a animação de avanço do
// calendário até a próxima etapa. O Bloco Especial (legado 9D) mora em
// `blocoEspecialSlice.js` — ver a nota no meio deste arquivo.
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
        ...contextoDeTelaLimpo({ endOfSeasonResult: result }),
        isAdvancing: false,
        showRaceBriefing: false,
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

  // DEBUG: simula tudo menos a última corrida da categoria do jogador, deixando o
  // save a um "Avançar calendário" da final — o atalho para ver a tela de Campeão
  // da Temporada sem correr o ano inteiro. Só o Ctrl+L do Dashboard chama.
  debugSkipToSeasonFinale: async () => {
    const { careerId, loadCareer } = get();
    if (!careerId) throw new Error(i18n.t("storeErrors.careerNotLoaded"));

    set({ isAdvancing: true, error: null });
    try {
      await invoke("debug_skip_to_season_finale", { careerId });
      await loadCareer(careerId);
      set({ isAdvancing: false, isDirty: true });
    } catch (error) {
      set({
        isAdvancing: false,
        error: getErrorMessage(error, i18n.t("storeErrors.skipRaces")),
      });
      throw error;
    }
  },

  // Busca o payload real do pop-up "Campeão da Temporada" e o abre. Sem `seasonNumber`
  // é a temporada ativa; com ele, um ano já encerrado. Sem etapa disputada o backend
  // devolve null e nada acontece — a tela nunca aparece vazia. Best-effort: falhar
  // aqui não pode derrubar o pós-corrida.
  loadSeasonChampionOverlay: async ({ category = null, seasonNumber = null } = {}) => {
    const { careerId } = get();
    if (!careerId) return null;

    try {
      const payload = await invoke("get_season_champion_payload", {
        careerId,
        category,
        seasonNumber,
      });
      if (!payload?.drivers?.length) return null;
      set({ championOverlay: payload });
      return payload;
    } catch (error) {
      console.error("Erro ao carregar o campeão da temporada:", error);
      return null;
    }
  },

  // DEBUG: abre o pop-up de Campeão da Temporada na hora, com o ANO ANTERIOR — uma
  // temporada fechada, com o calendário inteiro e todos os recordes preenchidos. É o
  // atalho para trabalhar na tela sem correr nada. Se não houver ano anterior (ou ele
  // não tiver corrida do jogador), cai na temporada ativa.
  debugShowLastSeasonChampion: async () => {
    const { season, loadSeasonChampionOverlay } = get();
    const current = Number(season?.numero) || 0;

    if (current > 1) {
      const previous = await loadSeasonChampionOverlay({ seasonNumber: current - 1 });
      if (previous) return previous;
    }
    return loadSeasonChampionOverlay();
  },

  // ── Bloco Especial ───────────────────────────────────────────────────────────
  // O BLOCO ESPECIAL (legado 9D) morava aqui e virou `blocoEspecialSlice.js`. Eram cerca
  // de 215 linhas — metade deste arquivo — que só executam para saves pré-v33 em voo,
  // dividindo leitura com a virada de temporada e a animação do calendário, que são
  // código vivo. As ações continuam no mesmo store, com os mesmos nomes; só mudaram de
  // arquivo, para o dia em que os saves antigos puderem morrer ser uma remoção e não uma
  // cirurgia.
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
