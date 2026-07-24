// Rótulos e formatadores do ranking global de pilotos (GlobalDriversTab).
//
// Tudo puro: recebe uma linha de `get_global_driver_rankings` (ou um pedaço dela) e
// devolve texto. Vive fora do tab porque é usado pelo próprio tab, pelas linhas da
// tabela, pelos diálogos e pelo módulo de ranking — manter aqui evita ciclo de import.

import i18n from "../../i18n/index.js";
import { currentLang } from "../../i18n/format.js";
import { extractNationalityCode, getCategoryTier } from "../../utils/formatters";

export function parseOptionalNumber(value) {
  if (value === "" || value == null) return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

export function defaultDirection(key) {
  return key === "nome" || key === "status" || key === "historical_rank" ? "asc" : "desc";
}

export function statusFilterLabel(status) {
  const labels = {
    Ativo: "pilotos ativos",
    Livre: "pilotos livres",
    Aposentado: "pilotos aposentados",
  };
  return labels[status] ?? "pilotos filtrados";
}

export function statusClass(status) {
  if (status === "Aposentado") return "rounded-full border border-white/10 bg-white/[0.04] px-3 py-1 text-xs text-text-muted";
  if (status === "Livre") return "rounded-full border border-status-yellow/20 bg-status-yellow/10 px-3 py-1 text-xs text-status-yellow";
  return "rounded-full border border-status-green/20 bg-status-green/10 px-3 py-1 text-xs text-status-green";
}

export function statusTitle(row) {
  if (row.status === "Aposentado" && row.temporada_aposentadoria) {
    return `Aposentado em ${row.temporada_aposentadoria}`;
  }
  return undefined;
}

export function teamCategoryLabel(row) {
  const category = categoryLabel(row.categoria_atual);
  if (row.equipe_nome) return `${row.equipe_nome} / ${category}`;
  if (row.status === "Aposentado") {
    const retiredLabel = row.anos_aposentado != null ? i18n.t("globalDrivers.retiredYears", { count: row.anos_aposentado }) : i18n.t("globalDrivers.retired");
    return `${retiredLabel} / ${category}`;
  }
  if (row.status === "Livre" && category !== "-") return `Livre / ${category}`;
  if (row.status === "Livre") return "Livre";
  return category;
}

export function categoryLabel(category) {
  if (!category) return "-";
  const normalized = String(category).trim();
  if (normalized.includes(":")) {
    const [baseCategory, className] = normalized.split(":");
    const classText = classLabel(className);
    if (baseCategory === "endurance" && classText) return `${classText} Endurance`;
    if (baseCategory === "production_challenger" && classText) return `${classText} Production`;
  }
  return normalized
    .split("_")
    .map((part) => {
      const upper = part.toUpperCase();
      if (["GT3", "GT4", "BMW", "M2"].includes(upper)) return upper;
      return part.charAt(0).toUpperCase() + part.slice(1);
    })
    .join(" ");
}

export function titleCategoryLabel(entry) {
  const category = titleBaseCategoryLabel(entry?.categoria);
  const className = classLabel(entry?.classe ?? entry?.class_name);
  return className ? `${category}/${className}` : category;
}

function titleBaseCategoryLabel(category) {
  if (category === "production_challenger") return "Production";
  if (category === "endurance") return "Endurance";
  if (category === "lmp2") return "LMP2";
  if (category === "bmw_m2") return "BMW";
  if (category === "mazda_amador") return "Mazda Cup";
  if (category === "toyota_amador") return "Toyota Cup";
  if (category === "mazda_rookie") return "Mazda Rookie";
  if (category === "toyota_rookie") return "Toyota Rookie";
  return categoryLabel(category);
}

function classLabel(className) {
  if (!className) return "";
  const normalized = String(className).trim().toLowerCase();
  const labels = {
    mazda: "Mazda",
    toyota: "Toyota",
    bmw: "BMW",
    gt4: "GT4",
    gt3: "GT3",
    lmp2: "LMP2",
  };
  return labels[normalized] ?? categoryLabel(normalized);
}

export function categoryTierOrder(category) {
  const tier = getCategoryTier(category);
  return tier > 0 ? tier : 999;
}

export function nationalityKey(nacionalidade) {
  return extractNationalityCode(nacionalidade) ?? nacionalidade ?? "";
}

export function formatRank(rank) {
  return rank ? String(rank).padStart(2, "0") : "--";
}

export function formatIndex(value) {
  return Number(value ?? 0).toLocaleString(currentLang(), {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  });
}

export function formatYears(value) {
  return value == null || value < 0 ? "-" : `${value} anos`;
}

// Quebra o total de pódios em vitória / 2º / 3º usando o detalhe real dos resultados
// (segundos/terceiros vindos da race_results). Ancorado no total OFICIAL (row.podios):
// o que não tem detalhe por posição — caso dos pilotos históricos pré-gerados, que
// nunca tiveram race_results — aparece como "sem detalhe", em vez de sumir.
export function podiumBreakdownTitle(row) {
  const podios = row.podios ?? 0;
  if (podios <= 0) return i18n.t("globalDrivers.podium.none");
  const vitorias = Math.min(Math.max(0, row.vitorias ?? 0), podios);
  const naoVitorias = Math.max(0, podios - vitorias);
  const segundos = Math.max(0, row.segundos ?? 0);
  const terceiros = Math.max(0, row.terceiros ?? 0);
  const detalhados = Math.min(segundos + terceiros, naoVitorias);
  const semDetalhe = Math.max(0, naoVitorias - detalhados);

  const linhas = [i18n.t("globalDrivers.podium.total", { count: podios })];
  if (vitorias > 0) linhas.push(i18n.t("globalDrivers.podium.wins", { count: vitorias }));
  if (segundos > 0) linhas.push(i18n.t("globalDrivers.podium.seconds", { count: segundos }));
  if (terceiros > 0) linhas.push(i18n.t("globalDrivers.podium.thirds", { count: terceiros }));
  if (semDetalhe > 0) {
    linhas.push(i18n.t("globalDrivers.podium.noDetail", { count: semDetalhe }));
  }
  return linhas.join("\n");
}
