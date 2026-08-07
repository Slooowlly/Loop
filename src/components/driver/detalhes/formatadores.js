import i18n from "../../../i18n/index.js";
import { ordinal } from "../../../i18n/format.js";

export function formatStatValue(value) {
  if (value === null || value === undefined) return "-";
  return value;
}

export function formatAverage(value) {
  if (value === null || value === undefined) return "-";
  return value.toFixed(1);
}

export function formatRank(value) {
  if (value === null || value === undefined) return "";
  return ` (${ordinal(value)})`;
}

export function formatRankedValue(value, rank) {
  return `${value ?? 0}${formatRank(rank)}`;
}

export function formatContractRole(role) {
  if (role === "Numero1" || role === "N1" || role === "Piloto N1") return "N1";
  if (role === "Numero2" || role === "N2" || role === "Piloto N2") return "N2";
  return role || "-";
}

export function formatContractPeriod(contract) {
  if (!contract) return "-";

  const start = contract.ano_inicio ?? contract.temporada_inicio;
  const end = contract.ano_fim ?? contract.temporada_fim;
  return `${start} - ${end}`;
}

export function formatAttributeName(name) {
  const map = {
    skill: i18n.t("driverDetail.attributes.skill"),
    consistencia: i18n.t("driverDetail.attributes.consistencia"),
    racecraft: i18n.t("driverDetail.attributes.racecraft"),
    defesa: i18n.t("driverDetail.attributes.defesa"),
    ritmo_classificacao: i18n.t("driverDetail.attributes.ritmo_classificacao"),
    gestao_pneus: i18n.t("driverDetail.attributes.gestao_pneus"),
    habilidade_largada: i18n.t("driverDetail.attributes.habilidade_largada"),
    adaptabilidade: i18n.t("driverDetail.attributes.adaptabilidade"),
    fator_chuva: i18n.t("driverDetail.attributes.fator_chuva"),
    fitness: i18n.t("driverDetail.attributes.fitness"),
    experiencia: i18n.t("driverDetail.attributes.experiencia"),
    desenvolvimento: i18n.t("driverDetail.attributes.desenvolvimento"),
    aggression: i18n.t("driverDetail.attributes.aggression"),
    smoothness: i18n.t("driverDetail.attributes.smoothness"),
    midia: i18n.t("driverDetail.attributes.midia"),
    mentalidade: i18n.t("driverDetail.attributes.mentalidade"),
    confianca: i18n.t("driverDetail.attributes.confianca"),
  };

  return map[name] || name;
}

export function formatInjuryOccurrence(injury) {
  return injury?.corrida_ocorrida_rotulo || injury?.corrida_ocorrida_id || "-";
}

export function formatInjuryRecovery(injury) {
  const remaining = injury?.corridas_restantes;
  if (!Number.isFinite(remaining)) return "-";
  if (remaining <= 0) return i18n.t("driverDetail.injury.recoveryReady");
  return i18n.t("driverDetail.injury.recovery", { count: remaining });
}

export function formatCategoryLabel(categoryId) {
  const normalized = String(categoryId || "").trim();
  if (normalized.includes(":")) {
    const [baseCategory, className] = normalized.split(":");
    const classLabel = formatSpecialClassLabel(className);
    if (baseCategory === "endurance" && classLabel) return `${classLabel} Endurance`;
    if (baseCategory === "production_challenger" && classLabel) return `${classLabel} Production`;
  }

  const map = {
    mazda_rookie: "Mazda Rookie",
    toyota_rookie: "Toyota Rookie",
    mazda_amador: "Mazda Cup",
    toyota_amador: "Toyota Cup",
    bmw_m2: "BMW",
    production_challenger: "Production",
    gt4: "GT4",
    gt3: "GT3",
    endurance: "Endurance",
  };

  return map[normalized] || normalized || "-";
}

export function formatRaceMilestone(value) {
  if (value === null || value === undefined) return i18n.t("driverDetail.history.milestoneNever");
  return i18n.t("driverDetail.history.milestoneRace", { value });
}

export function formatSpecialClassLabel(className) {
  const map = {
    mazda: "Mazda",
    toyota: "Toyota",
    bmw: "BMW",
    gt4: "GT4",
    gt3: "GT3",
    lmp2: "LMP2",
  };

  return map[className] || className || "";
}

export function formatSpecialCategoryAndClass(event) {
  const category = formatCategoryLabel(event?.categoria);
  const classLabel = formatSpecialClassLabel(event?.classe);
  return [category, classLabel].filter(Boolean).join(" ");
}

export function formatSpecialCampaign(campaign) {
  if (!campaign) return "-";
  return `${campaign.ano}, ${formatSpecialCategoryAndClass(campaign)}`;
}

export function formatSpecialEventEntry(event) {
  if (!event) return "-";
  const base = `${event.ano} ${formatSpecialCategoryAndClass(event)}`;
  return event.equipe ? `${base} - ${event.equipe}` : base;
}

export function formatUnemploymentYears(presence) {
  const years = presence?.anos_desempregado ?? 0;
  const periods = Array.isArray(presence?.periodos_desempregado)
    ? presence.periodos_desempregado.filter(Boolean)
    : [];
  const label = i18n.t("driverDetail.moment.expiresValue", { count: years });

  if (periods.length === 0) return label;
  return `${label} (${periods.join(" | ")})`;
}

export function formatCareerYears(value) {
  const years = value ?? 0;
  return i18n.t("driverDetail.moment.expiresValue", { count: years });
}

export function formatYearsAverage(value) {
  if (value === null || value === undefined) return "-";
  const formatted = Number(value).toFixed(1);
  return i18n.t(
    formatted === "1.0" ? "driverDetail.history.yearsAverage_one" : "driverDetail.history.yearsAverage_other",
    { value: formatted },
  );
}

export function formatBestSeason(season) {
  if (!season) return "-";
  return `${season.ano}, ${formatCategoryLabel(season.categoria)}`;
}

// A temporada com o resultado dela colado. "2009, GT3" diz onde ele estava;
// "2009, GT3 · P11" diz o que aconteceu — e sem isso a pior temporada da carreira
// é indistinguível de uma linha qualquer da escada de categorias.
export function formatSeasonWithResult(season) {
  const base = formatBestSeason(season);
  if (!season?.posicao_campeonato) return base;
  return `${base} · P${season.posicao_campeonato}`;
}

// Um marco que é uma temporada inteira, e não um número de corrida. Sem ele o
// piloto que nunca foi campeão veria "-" numa coluna onde todo o resto diz
// "Nunca". Vai sem o resultado de propósito: a colocação de um título é sempre
// P1, e repetir isso em toda ficha de campeão é ruído.
export function formatSeasonMilestone(season) {
  if (!season) return i18n.t("driverDetail.history.milestoneNever");
  return formatBestSeason(season);
}

// Uma sequência contada em corridas, com o período entre parênteses quando ele é
// conhecido — a mesma forma que `formatUnemploymentYears` usa para os anos sem
// assento. O período importa mais no jejum do que no auge: "24" é um número,
// "24 (2012–2016)" é uma travessia. O valor fica nu quando não há período, para
// não destoar do "Maior sequência de vitórias" logo acima.
export function formatStreakRaces(count, startYear, endYear) {
  const total = Number(count) || 0;
  if (total <= 0 || !startYear) return String(total);
  const period = !endYear || endYear === startYear ? `${startYear}` : `${startYear}–${endYear}`;
  return i18n.t("driverDetail.history.streakWithPeriod", { count: total, period });
}

// Taxa de abandono. `null` (nenhuma largada) vira "-" em vez de "0%", que se
// leria como "nunca abandona" — um estreante que ainda não correu não é o piloto
// mais confiável do grid.
export function formatRetirementRate(value) {
  if (value === null || value === undefined) return "-";
  const rate = Number(value);
  if (!Number.isFinite(rate)) return "-";
  return `${Number.isInteger(rate) ? rate : rate.toFixed(1)}%`;
}

export function formatAverageGrid(value) {
  if (value === null || value === undefined) return "-";
  const grid = Number(value);
  if (!Number.isFinite(grid)) return "-";
  return `P${Number.isInteger(grid) ? grid : grid.toFixed(1)}`;
}

// A média do mundo, para o número que sozinho não diz nada. "1,5%" não responde
// se ele é confiável; "1,5% · média 4,2%" responde. Some quando não há
// referência, em vez de virar "média -".
export function formatWorldAverage(value, formatter = String) {
  if (value === null || value === undefined) return null;
  return i18n.t("driverDetail.history.worldAverage", { value: formatter(value) });
}

// O placar de um duelo com companheiro de equipe: "Fulano 6-2".
export function formatDuel(duel) {
  if (!duel?.nome) return "-";
  const placar = i18n.t("driverDetail.history.duelRecord", {
    wins: duel.vitorias ?? 0,
    losses: duel.derrotas ?? 0,
  });
  return `${duel.nome} ${placar}`;
}
