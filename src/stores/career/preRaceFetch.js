import { invoke } from "@tauri-apps/api/core";

// A busca dos dados ESTÁTICOS da pré-corrida, num lugar só.
//
// Dois caminhos leem exatamente os mesmos comandos: o pré-carregamento durante a animação
// do calendário (`preRaceCacheSlice`) e a própria Sala de Estratégia ao montar
// (`components/race/nextrace/useBriefingData`). A lista vivia copiada à mão nos dois, e o
// modo de falha era silencioso: a Sala passava a pedir um comando novo, o cache continuava
// completo aos olhos de quem o escreveu, e o flash template→IA que o cache existe para
// evitar voltava sem nenhum erro no console.
//
// Aqui a lista é declarada uma vez e a busca também: a Sala chama esta mesma função no
// cache miss, em vez de repetir os `invoke`. O guard
// `scripts/tests/pre-race-cache-espelha-a-sala` confere que a Sala não pede nada que este
// módulo não busque.

/// Os comandos que compõem o retrato estático da etapa. Ordem sem importância.
export const COMANDOS_DA_PRE_CORRIDA = [
  "get_drivers_by_category",
  "get_teams_standings",
  "get_briefing_phrase_history",
  "get_breakdown_forecast",
  "get_grid_breakdown_risk",
  "get_weekend_modifiers",
];

/// Histórico de frases vazio — o formato que a Sala espera quando o comando falha.
const HISTORICO_VAZIO = { season_number: 0, entries: [] };

/// Busca o retrato estático da etapa. Cada comando degrada sozinho: classificação vazia,
/// histórico vazio, previsão ausente. Nenhum deles derruba a tela — exceto as duas
/// classificações, que SÃO a Sala e por isso sobem o erro para quem chamou.
export async function buscarDadosDaPreCorrida({ careerId, categoria }) {
  const [drivers, teams, phraseHistory, forecast, riskTeamIds, modifierRows] = await Promise.all([
    invoke("get_drivers_by_category", { careerId, category: categoria }),
    invoke("get_teams_standings", { careerId, category: categoria }),
    invoke("get_briefing_phrase_history", { careerId }).catch(() => HISTORICO_VAZIO),
    invoke("get_breakdown_forecast", { careerId }).catch(() => null),
    invoke("get_grid_breakdown_risk", { careerId }).catch(() => []),
    // Falhar aqui só custa o tooltip dos modificadores, então não sobe pra tela — mas
    // custar CALADO era pior: comando não registrado, save sem etapa pendente e erro de
    // verdade ficavam todos com a mesma cara de "o balão não abre".
    invoke("get_weekend_modifiers", { careerId }).catch((erro) => {
      console.warn("[Loop] get_weekend_modifiers falhou:", erro);
      return [];
    }),
  ]);

  return {
    driverStandings: Array.isArray(drivers) ? drivers : [],
    teamStandings: Array.isArray(teams) ? teams : [],
    phraseHistory:
      phraseHistory && Array.isArray(phraseHistory.entries) ? phraseHistory : HISTORICO_VAZIO,
    // `available: false` é a previsão dizendo "não tenho o que mostrar" — vira ausência.
    breakdownForecast: forecast && forecast.available ? forecast : null,
    // Arrays crus de propósito: este retrato vai para o store, e Set/Map não sobrevivem
    // a uma serialização. Quem desenha converte (ver `useBriefingData`).
    breakdownRiskTeamIds: Array.isArray(riskTeamIds) ? riskTeamIds : [],
    weekendModifierRows: Array.isArray(modifierRows) ? modifierRows : [],
  };
}

export default buscarDadosDaPreCorrida;
