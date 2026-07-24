import { create } from "zustand";

import { initialState } from "./career/state";
import { createCareerSlice } from "./career/careerSlice";
import { createRaceSlice } from "./career/raceSlice";
import { createMarketSlice } from "./career/marketSlice";
import { createSeasonSlice } from "./career/seasonSlice";
import { createPreRaceCacheSlice } from "./career/preRaceCacheSlice";

// Reexport histórico: os testes importam esse helper direto do store.
export { buildCalendarAdvanceTiming } from "./career/helpers";

// O store é a COMPOSIÇÃO dos slices de domínio de src/stores/career/. Cada slice
// recebe o mesmo par (set, get), então todos leem e escrevem o mesmo estado e uma
// ação pode chamar a de outro domínio via `get()` — é o mesmo objeto de sempre.
// A API pública (chaves de estado e assinaturas das ações) não muda com a divisão.
const useCareerStore = create((set, get) => ({
  ...initialState,

  ...createCareerSlice(set, get),
  ...createRaceSlice(set, get),
  ...createMarketSlice(set, get),
  ...createSeasonSlice(set, get),
  ...createPreRaceCacheSlice(set, get),

  // API mínima preservada para reativar o overlay quando houver dados reais.
  // Mora na raiz do store por ser transversal (nenhum domínio a reivindica).
  showChampionOverlay: (data = null) => set({ championOverlay: data ?? { demo: true } }),
  hideChampionOverlay: () => set({ championOverlay: null }),
}));

export default useCareerStore;
