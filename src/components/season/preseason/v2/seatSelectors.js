// Seletores do layout v2 da pré-temporada.
//
// A regra que rege este arquivo: nada aqui inventa número. Todo campo lido já vem
// do backend no `TeamStanding` (car_level, confiabilidade, pit_crew_quality,
// cash_balance, histórico, flags de contrato/aposentadoria) — o v1 simplesmente
// não os desenhava. O que fazemos é contar, ordenar e classificar em faixas.

import { CATEGORIES, count_team_vacancies } from "../../preSeasonFormatters.js";

// Filtra o grid COMPLETO pela categoria do topo, sem ir ao backend de novo.
// Espelha exatamente a regra de `fetchGridTeams` (dbIds + filterClass) — o v2
// busca todas as categorias uma vez por semana e recorta localmente, o que dá
// os contadores dos chips de graça.
export function filterGridTeamsByCategory(allTeams, selectedCat) {
  if (selectedCat === "all") return allTeams;
  const cfg = CATEGORIES.find((c) => c.id === selectedCat);
  if (!cfg) return allTeams;
  const dbIds = new Set(cfg.dbIds ?? []);
  const byCategory = allTeams.filter((team) => dbIds.has(team._categoria));
  if (!cfg.filterClass) return byCategory;
  return byCategory.filter((team) => {
    if (team.classe === cfg.filterClass) return true;
    if (team._categoria?.startsWith(cfg.filterClass)) return true;
    if (cfg.filterClass === "bmw" && team._categoria === "bmw_m2") return true;
    return false;
  });
}

// Assentos que já estão vazios agora.
export function countOpenSeats(teams) {
  return teams.reduce((total, team) => total + count_team_vacancies(team), 0);
}

// Assentos ocupados que PODEM abrir nesta virada: aposentadoria decidida ou
// contrato vencendo. Nas semanas de fotografia é o único número vivo — sem ele
// os contadores do topo ficam todos em zero justamente quando o jogador está
// tentando entender onde vai sobrar lugar.
export function countSeatsAtRisk(teams) {
  return teams.reduce((total, team) => {
    let n = 0;
    if (team.piloto_1_nome && (team.piloto_1_aposentado || team.piloto_1_contrato_vence)) n += 1;
    if (team.piloto_2_nome && (team.piloto_2_aposentado || team.piloto_2_contrato_vence)) n += 1;
    return total + n;
  }, 0);
}

// Resumo por chip do topo: um número por categoria, com a natureza dele.
// `kind` = "open" quando há assento vazio de fato, "risk" quando só há assento
// em risco. Zero nos dois → o chip não mostra contador nenhum.
export function buildCategoryCounters(allTeams) {
  const counters = {};
  for (const cat of CATEGORIES) {
    if (cat.isSeparator) continue;
    const teams = filterGridTeamsByCategory(allTeams, cat.id);
    const open = countOpenSeats(teams);
    const risk = countSeatsAtRisk(teams);
    counters[cat.id] = {
      open,
      risk,
      count: open > 0 ? open : risk,
      kind: open > 0 ? "open" : "risk",
    };
  }
  return counters;
}
