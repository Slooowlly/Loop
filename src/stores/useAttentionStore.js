import { create } from "zustand";

/**
 * Estado de "já visto nesta corrida" dos cards que pulsam pra chamar atenção
 * (Condição de Pista, Risco de Quebra…). O glow (`attn-glow-delayed`) existe só
 * pra lembrar o jogador de olhar; depois que ele abriu o card NESTA corrida, o
 * pulso cala. Como a chave inclui o `raceId`, na corrida seguinte a chave nova
 * não existe → o glow reaparece sozinho. Nada de limpar estado velho.
 */
export const useAttentionStore = create((set, get) => ({
  seen: {}, // `${raceId}:${cardId}` → true

  isSeen: (raceId, cardId) => !!get().seen[`${raceId}:${cardId}`],

  markSeen: (raceId, cardId) => {
    if (!raceId) return;
    const key = `${raceId}:${cardId}`;
    if (get().seen[key]) return;
    set((s) => ({ seen: { ...s.seen, [key]: true } }));
  },
}));
