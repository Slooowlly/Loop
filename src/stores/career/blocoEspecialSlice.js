import { invoke } from "@tauri-apps/api/core";

import i18n from "../../i18n/index.js";
import { deriveAcceptedSpecialOfferFromWindow, getErrorMessage } from "./helpers";

// Slice do BLOCO ESPECIAL — LEGADO 9D.
//
// Nada aqui roda em carreira nova. O Bloco Especial (semanas 41–50, convocações fora do
// calendário regular) foi retirado do fluxo na v33; estas ações só existem para os saves
// pré-v33 que ainda estejam em voo, que atravessam as fases `BlocoRegular` →
// `JanelaConvocacao` → `BlocoEspecial` → `PosEspecial`.
//
// Mora num arquivo próprio, e não no `seasonSlice`, exatamente por isso: era metade
// daquele arquivo, e metade que só executa para um save que talvez não exista mais. Junto,
// o legado disputava leitura com a virada de temporada e a animação do calendário, que são
// código vivo. Separado, o dia em que os saves antigos puderem morrer é a remoção deste
// arquivo mais três linhas no `useCareerStore` — e não uma cirurgia.
//
// A API pública não muda com a separação: os nomes das ações e a forma do estado
// (`specialWindowState`, `playerSpecialOffers`, `acceptedSpecialOffer`, `isConvocating`,
// `showConvocation`) continuam os mesmos, porque todos os slices dividem um único estado.
export const createBlocoEspecialSlice = (set, get) => ({
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
});

export default createBlocoEspecialSlice;
