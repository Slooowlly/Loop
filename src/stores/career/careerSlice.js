import { invoke } from "@tauri-apps/api/core";

import i18n, { applyLanguage } from "../../i18n/index.js";
import { isLegacySeasonPhase } from "../../utils/seasonPhases";
import { initialState } from "./state";
import {
  applyCareerData,
  buildPreseasonUiState,
  buildResumeUiState,
  buildTemporalUiState,
  deriveAcceptedSpecialOffer,
  deriveAcceptedSpecialOfferFromWindow,
  getErrorMessage,
  loadTemporalSummary,
} from "./helpers";

// Slice de CARREIRA/SAVE: carregar e descartar a carreira, idioma, interesses do
// jogador e consolidação do save em disco.
export const createCareerSlice = (set, get) => ({
  loadCareer: async (careerId) => {
    const isSwitchingCareer = Boolean(get().careerId && get().careerId !== careerId);
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
      ...(isSwitchingCareer
        ? { championOverlay: null }
        : {}),
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
      // Idioma escolhido — fire-and-forget; decide fallback PT vs "erro" localizado.
      void get().loadLanguage();
      return data;
    } catch (error) {
      const message = getErrorMessage(error, i18n.t("storeErrors.loadCareer"));
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

  // Carrega o idioma escolhido do config (best-effort; default pt-BR se falhar).
  loadLanguage: async () => {
    try {
      const cfg = await invoke("get_config");
      if (cfg?.language) {
        set({ language: cfg.language });
        applyLanguage(cfg.language); // sincroniza o i18next (UI estática).
      }
    } catch {
      /* mantém o default */
    }
  },

  // Atualização imediata do idioma (o Settings chama ao trocar, sem exigir reload).
  setLanguage: (language) => {
    if (language) {
      set({ language });
      applyLanguage(language); // troca a UI na hora, sem reload.
    }
  },

  setCareerFromCreation: (createResult) => {
    set({
      isLoaded: true,
      careerId: createResult?.career_id ?? null,
    });
  },

  clearCareer: () => {
    set({ ...initialState });
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
});

export default createCareerSlice;
