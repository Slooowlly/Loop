import i18n from "../../../i18n/index.js";
import { ordinal } from "../../../i18n/format.js";

import {
  CANDIDATE_GROUP_COLORS,
  CANDIDATE_GROUP_LABELS,
  CANDIDATE_GROUP_ORDER,
  CATEGORY_COLORS,
  CATEGORY_LABELS,
  CATEGORY_ORDER,
  CLASS_COLORS,
  DAILY_LOG_CLASS_ORDER,
  TEAM_CLASS_ORDER,
} from "./constantes.js";

export function roleLabel(role) {
  if (role === "Numero1") return i18n.t("convocation.roles.numero1");
  if (role === "Numero2") return i18n.t("convocation.roles.numero2");
  return i18n.t("convocation.roles.default");
}

export function countTeamVacancies(team) {
  let total = 0;
  if (!team.piloto_1_nome) total += 1;
  if (!team.piloto_2_nome) total += 1;
  return total;
}

export function categorySortValue(categoryId) {
  const index = CATEGORY_ORDER.indexOf(categoryId);
  return index === -1 ? 999 : index;
}

export function classAccentColor(className) {
  return CLASS_COLORS[className] ?? "#8b949e";
}

export function sortByConfiguredOrder(items, getValue, preferredOrder = []) {
  return [...items].sort((left, right) => {
    const leftValue = getValue(left);
    const rightValue = getValue(right);
    const leftIndex = preferredOrder.indexOf(leftValue);
    const rightIndex = preferredOrder.indexOf(rightValue);
    const normalizedLeft = leftIndex === -1 ? 999 : leftIndex;
    const normalizedRight = rightIndex === -1 ? 999 : rightIndex;

    if (normalizedLeft !== normalizedRight) {
      return normalizedLeft - normalizedRight;
    }

    return String(leftValue ?? "").localeCompare(String(rightValue ?? ""));
  });
}

export function normalizeCategorySections(entries = []) {
  const grouped = new Map();

  for (const team of entries) {
    const category = team.categoria ?? team._categoria ?? "especial";
    if (!grouped.has(category)) {
      grouped.set(category, []);
    }
    grouped.get(category).push(team);
  }

  return [...grouped.entries()]
    .sort(([left], [right]) => categorySortValue(left) - categorySortValue(right))
    .map(([category, teams]) => ({
      category,
      label: CATEGORY_LABELS[category] ?? category,
      color: CATEGORY_COLORS[category] ?? "#58a6ff",
      teams: [...teams].sort((left, right) => {
        const classDiff = (left.classe ?? "").localeCompare(right.classe ?? "");
        if (classDiff !== 0) return classDiff;
        return (left.nome ?? "").localeCompare(right.nome ?? "");
      }),
    }));
}

export function buildClassGroups(teams = [], category) {
  const grouped = new Map();

  for (const team of teams) {
    const className = team.classe ?? "geral";
    if (!grouped.has(className)) {
      grouped.set(className, []);
    }
    grouped.get(className).push(team);
  }

  return sortByConfiguredOrder(
    [...grouped.entries()],
    ([className]) => className,
    TEAM_CLASS_ORDER[category] ?? [],
  )
    .map(([className, classTeams]) => ({
      className,
      teams: classTeams,
    }));
}

export function filterEligibleCandidates(candidates = [], selectedCategory = "all") {
  if (selectedCategory === "production_challenger") {
    return candidates.filter((candidate) => candidate.production_eligible);
  }
  if (selectedCategory === "endurance") {
    return candidates.filter((candidate) => candidate.endurance_eligible);
  }
  return candidates.filter(
    (candidate) => candidate.production_eligible || candidate.endurance_eligible,
  );
}

export function buildCandidateGroups(candidates = []) {
  const grouped = new Map();

  for (const candidate of candidates) {
    const category = candidate.origin_category || "sem_categoria";
    if (!grouped.has(category)) {
      grouped.set(category, []);
    }
    grouped.get(category).push(candidate);
  }

  return [...grouped.entries()]
    .sort(([left], [right]) => {
      const leftIndex = CANDIDATE_GROUP_ORDER.indexOf(left);
      const rightIndex = CANDIDATE_GROUP_ORDER.indexOf(right);
      const normalizedLeft = leftIndex === -1 ? 999 : leftIndex;
      const normalizedRight = rightIndex === -1 ? 999 : rightIndex;
      if (normalizedLeft !== normalizedRight) {
        return normalizedLeft - normalizedRight;
      }
      return left.localeCompare(right);
    })
    .map(([category, entries]) => ({
      category,
      label: CANDIDATE_GROUP_LABELS[category] ?? category,
      color: CANDIDATE_GROUP_COLORS[category] ?? "rgba(255,255,255,0.35)",
      entries,
    }));
}

export function buildDailyLogGroups(entries = []) {
  const grouped = new Map();

  for (const entry of entries) {
    const specialCategory = entry.special_category || "especial";
    const className = entry.class_name || "geral";
    const key = `${specialCategory}:${className}`;
    if (!grouped.has(key)) {
      grouped.set(key, {
        key,
        specialCategory,
        className,
        label: `${CATEGORY_LABELS[specialCategory] ?? specialCategory} - ${className.toUpperCase()}`,
        color: classAccentColor(className),
        entries: [],
      });
    }
    grouped.get(key).entries.push(entry);
  }

  return [...grouped.values()].sort((left, right) => {
    const categoryDiff =
      categorySortValue(left.specialCategory) - categorySortValue(right.specialCategory);
    if (categoryDiff !== 0) return categoryDiff;

    const leftIndex = DAILY_LOG_CLASS_ORDER.indexOf(left.className);
    const rightIndex = DAILY_LOG_CLASS_ORDER.indexOf(right.className);
    const normalizedLeft = leftIndex === -1 ? 999 : leftIndex;
    const normalizedRight = rightIndex === -1 ? 999 : rightIndex;
    if (normalizedLeft !== normalizedRight) return normalizedLeft - normalizedRight;
    return left.className.localeCompare(right.className);
  });
}

export function formatSafeChampionshipPosition(position, totalDrivers) {
  if (!position) {
    return null;
  }
  return ordinal(position);
}

export function formatChampionshipPosition(position, totalDrivers) {
  if (!position) {
    return null;
  }
  return ordinal(position);
}
