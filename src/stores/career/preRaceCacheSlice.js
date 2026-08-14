import { invoke } from "@tauri-apps/api/core";

import { buildBriefingContext } from "../../pages/tabs/nextRaceContext";
import { cacheEhDaEtapaAtual } from "./helpers";
import { buscarDadosDaPreCorrida } from "./preRaceFetch";

// Slice de CACHE PRÉ-CORRIDA: prévia por IA (`preRaceAi`) e standings da etapa
// (`preRaceStandings`), ambos chaveados pelo par `careerId` + `raceId`. O `careerId` faz
// parte da chave porque as etapas se chamam R001, R002... em TODA carreira: só com o
// `raceId` o cache da carreira anterior batia na nova e a Sala de Estratégia abria com os
// favoritos e a prévia do outro save.
export const createPreRaceCacheSlice = (set, get) => ({
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
    const chave = { careerId, raceId };
    // Já temos IA E os standings desta etapa em cache → nada a buscar.
    if (cacheEhDaEtapaAtual(preRaceAi, chave) && cacheEhDaEtapaAtual(preRaceStandings, chave)) {
      return;
    }
    // Geração do save no momento do disparo: o prefetch roda durante a animação de avanço e
    // o jogador pode sair para o menu e abrir outra carreira antes de ele terminar.
    const geracao = get().careerGeneration;

    try {
      // A lista de comandos mora em `preRaceFetch` — a Sala de Estratégia lê a mesma, e
      // um guard estrutural impede as duas de divergirem.
      const retrato = await buscarDadosDaPreCorrida({
        careerId,
        categoria: playerTeam.categoria,
      });
      const { driverStandings, teamStandings, phraseHistory, breakdownForecast } = retrato;

      // Outra etapa (ou outra CARREIRA) pode ter virado a corrente enquanto buscávamos.
      if (get().careerGeneration !== geracao || get().nextRace?.id !== raceId) return;

      // Guarda o retrato INTEIRO da etapa para a Sala de Estratégia abrir na hora, sem
      // re-disparar nenhum dos comandos ao montar. Guardar só parte dele era o que
      // deixava o marcador de quebra e o balão de modificadores chegando depois da tela.
      set({ preRaceStandings: { careerId, raceId, ...retrato } });

      // A IA já pode estar em cache desta etapa (só faltavam os standings): não regenera.
      if (cacheEhDaEtapaAtual(preRaceAi, chave)) return;

      const { aiFacts } = buildBriefingContext({
        player,
        playerTeam,
        season,
        nextRace,
        nextRaceBriefing,
        driverStandings,
        teamStandings,
        briefingPhraseHistory: phraseHistory,
        playerInterests: get().playerInterests,
        breakdownForecast,
      });
      if (!aiFacts || !aiFacts.trim()) return;

      const res = await invoke("pre_race_briefing_ai", { careerId, raceId, facts: aiFacts });
      if (get().careerGeneration !== geracao || get().nextRace?.id !== raceId) return;
      if (res?.narrative && res?.team_voice) {
        set({
          preRaceAi: {
            careerId,
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
});

export default createPreRaceCacheSlice;
