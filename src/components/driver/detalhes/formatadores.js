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
    bmw_m2: "BMW Cup",
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
