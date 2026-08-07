import { invoke } from "@tauri-apps/api/core";

import i18n from "../../i18n/index.js";
import { isFinaleSlot } from "../../utils/postRaceLanding";
import { getErrorMessage } from "./helpers";

// Slice de CORRIDA: simulação do fim de semana, import automático do iRacing,
// reabertura de corridas salvas e a tela de resultado.
export const createRaceSlice = (set, get) => ({
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
        lastRaceRepercussion: result.event_repercussion ?? null,
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
          lastRaceRepercussion: payload.summary?.event_repercussion ?? null,
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
          lastRaceRepercussion: screen.event_repercussion ?? null,
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
      lastRaceMaintenance: null,
      lastRaceRepercussion: null,
      resultIsFresh: false,
    });

    if (!careerId) return null;

    try {
      // Devolve a carreira recarregada: quem fecha o resultado precisa do estado
      // JÁ atualizado (ex.: o Dashboard checa se sobrou corrida na temporada para
      // decidir se abre o pop-up de Campeão da Temporada).
      return await loadCareer(careerId);
    } catch (error) {
      console.error("Erro ao recarregar carreira:", error);
      return null;
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

  updateNextRace: (raceData) => {
    set({ nextRace: raceData });
  },

  closeRaceBriefing: () => {
    set({ showRaceBriefing: false });
  },
});

export default createRaceSlice;
