import { create } from "zustand";

import { initialState } from "./career/state";
import { createCareerSlice } from "./career/careerSlice";
import { createRaceSlice } from "./career/raceSlice";
import { createMarketSlice } from "./career/marketSlice";
import { createSeasonSlice } from "./career/seasonSlice";
import { createBlocoEspecialSlice } from "./career/blocoEspecialSlice";
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
  // LEGADO 9D: só executa para saves pré-v33 em voo. Fica por último de propósito — o dia
  // em que os saves antigos puderem morrer, some esta linha e o arquivo dela.
  ...createBlocoEspecialSlice(set, get),
  ...createPreRaceCacheSlice(set, get),

  // Abertura direta do overlay com um payload já pronto (quem busca do backend é
  // `loadSeasonChampionOverlay`, no slice de temporada). Mora na raiz do store por
  // ser transversal — nenhum domínio reivindica o pop-up. Sem payload, fecha.
  showChampionOverlay: (data = null) => set({ championOverlay: data ?? null }),
  hideChampionOverlay: () => set({ championOverlay: null }),

  // Categoria em exibição na Home → banner do topo acompanha (ver `homeCategory`).
  // Também transversal: quem escreve é a StandingsTab, quem lê é o Header.
  setHomeCategory: (category) => set({ homeCategory: category ?? null }),
}));

export default useCareerStore;
