import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { buildBriefingContext } from "../pages/tabs/nextRaceContext";

import { initialState } from "./career/state";
import { createCareerSlice } from "./career/careerSlice";
import { createRaceSlice } from "./career/raceSlice";
import { createMarketSlice } from "./career/marketSlice";
import { createSeasonSlice } from "./career/seasonSlice";

// Reexport histórico: os testes importam esse helper direto do store.
export { buildCalendarAdvanceTiming } from "./career/helpers";

const useCareerStore = create((set, get) => ({
  ...initialState,

  ...createCareerSlice(set, get),
  ...createRaceSlice(set, get),
  ...createMarketSlice(set, get),
  ...createSeasonSlice(set, get),

  // API mínima preservada para reativar o overlay quando houver dados reais.
  // Mora na raiz do store por ser transversal (nenhum domínio a reivindica).
  showChampionOverlay: (data = null) => set({ championOverlay: data ?? { demo: true } }),
  hideChampionOverlay: () => set({ championOverlay: null }),

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

}));

export default useCareerStore;
