import { invoke } from "@tauri-apps/api/core";

import i18n from "../../i18n/index.js";
import { isFinaleSlot } from "../../utils/postRaceLanding";
import { getErrorMessage } from "./helpers";

// Slice de CORRIDA: simulação do fim de semana, import automático do iRacing,
// reabertura de corridas salvas e a tela de resultado.
export const createRaceSlice = (set, get) => ({
  simulateRace: async () => {
    const { careerId, nextRace, isSimulating, careerGeneration: geracao } = get();
    if (isSimulating) {
      return null;
    }
    if (!careerId || !nextRace?.id) {
      throw new Error(i18n.t("storeErrors.noPendingRace"));
    }
    // Fim de semana inteiro simulado no Rust: a chamada demora, e no meio dela o jogador
    // pode voltar ao menu e abrir outro save. Ver o TOKEN DE ABORTO em `career/state.js`.
    const carreiraTrocou = () => get().careerGeneration !== geracao;

    set({ isSimulating: true, error: null });

    try {
      const result = await invoke("simulate_race_weekend", {
        careerId,
        raceId: nextRace.id,
      });

      // Outro save assumiu enquanto a etapa rodava. O resultado é da carreira anterior:
      // devolve a quem pediu e não escreve nada — nem `isSimulating`, que a carga nova
      // já deixou em `false`.
      if (carreiraTrocou()) return result;

      set({
        lastRaceResult: result.player_race,
        lastRaceId: nextRace.id,
        lastRaceEvaluation: result.evaluation ?? null, // mesmo cérebro do import iRacing
        lastRaceTelemetry: null, // sim offline não tem telemetria ao vivo (sem gráficos)
        lastRaceOrigem: "simulada", // não pisou na pista, mas gerou evento de produto
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
      // Falha de uma etapa já superada não pinta erro na carreira que está na tela.
      if (carreiraTrocou()) throw error;
      const message = getErrorMessage(error, i18n.t("storeErrors.simulateRace"));
      set({ isSimulating: false, error: message });
      throw error;
    }
  },

  // GATILHO AUTOMÁTICO do iRacing: pergunta ao backend se o resultado da próxima
  // corrida já foi gravado (jogador terminou/saiu). Se sim, importa e abre a tela
  // de resultado sozinha. Chamado em loop por um poller no Dashboard. Idempotente.
  pollIracingResult: async () => {
    const {
      careerId,
      showResult,
      isSimulating,
      iracingImporting,
      nextRace,
      careerGeneration: geracao,
    } = get();
    if (!careerId || showResult || isSimulating || iracingImporting) return;
    // O poller do Dashboard dispara em loop; um tique pode estar em voo na troca de save.
    // Sem o token, o resultado importado da carreira A abria a tela de resultado na B.
    const carreiraTrocou = () => get().careerGeneration !== geracao;
    set({ iracingImporting: true });
    try {
      const payload = await invoke("iracing_auto_import_if_ready", { careerId });
      if (carreiraTrocou()) return;
      if (payload?.race_result) {
        set({
          lastRaceResult: payload.race_result,
          lastRaceId: payload.summary?.race_id ?? null,
          lastRaceEvaluation: payload.evaluation ?? null,
          lastRaceTelemetry: payload.telemetry ?? null,
          lastRaceOrigem: "iracing", // correu de verdade: resultado importado do sim
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
      // A bandeira é da carreira que abriu o tique. Na carreira nova ela já nasceu
      // limpa, e escrever aqui destravaria um import que a nova nunca começou.
      if (!carreiraTrocou()) set({ iracingImporting: false });
    }
  },

  // Reabre a CLASSIFICAÇÃO FINAL de uma corrida já disputada (pela Home, duplo
  // clique no R{n}). Lê a tela salva no disco (resultado + avaliação + telemetria).
  // Sem tela salva (corrida antiga / outra categoria) → no-op silencioso.
  openSavedRaceScreen: async (category, rodada) => {
    const { careerId, careerGeneration: geracao } = get();
    if (!careerId || !category || !rodada) return false;
    try {
      const screen = await invoke("get_saved_race_screen", { careerId, category, rodada });
      // Save trocado no meio da leitura: a tela salva é da carreira anterior, e abri-la
      // por cima da nova mostraria uma corrida que a carreira na tela nunca disputou.
      if (get().careerGeneration !== geracao) return false;
      if (screen?.race_result) {
        set({
          lastRaceResult: screen.race_result,
          lastRaceId: screen.race_id ?? null,
          lastRaceEvaluation: screen.evaluation ?? null,
          lastRaceTelemetry: screen.telemetry ?? null,
          lastRaceOrigem: "reaberta", // corrida antiga, não é evento novo
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
