// A ESCADA de categorias como a classificação a apresenta: séries (linha de carro),
// tiers dentro da série e as categorias multiclasse. Lógica pura extraída de
// `pages/tabs/StandingsTab.jsx`.
import i18n from "../../i18n/index.js";

export const ALL_CATEGORIES = [
  "mazda_rookie",
  "toyota_rookie",
  "mazda_amador",
  "toyota_amador",
  "bmw_m2",
  "production_challenger",
  "gt4",
  "gt3",
  "endurance",
];

// Escada apresentada como duas dimensões: a "série" (marca/linha de carro) que o
// jogador troca por um dropdown agrupado, e o "tier" dentro dela que sobe/desce
// com as setas ▲▼. As categorias multiclasse (production/endurance) NÃO são
// séries próprias: elas ficam no TOPO de várias séries ao mesmo tempo. Subindo a
// Mazda (rookie → championship) você "cai" na Production; a mesma Production também
// é o topo de Toyota e BMW. Endurance é o topo de GT4, GT3 e LMP2. `classId` diz
// qual classe da categoria multiclasse aparece PRIMEIRO quando você chega por
// aquela linha (ex.: Endurance pela linha GT3 lista GT3 antes de LMP2/GT4).
// `access` divide o conteúdo por licença: as linhas de entrada (Mazda/Toyota/BMW +
// Production) são "pro"; as de topo (GT4/GT3/LMP2 + Endurance) são "elite". O menu
// desenha uma separação física entre os dois grupos.
export const CATEGORY_SERIES = [
  { id: "mazda", label: "Mazda", classId: "mazda", access: "pro", categories: ["mazda_rookie", "mazda_amador", "production_challenger"] },
  { id: "toyota", label: "Toyota", classId: "toyota", access: "pro", categories: ["toyota_rookie", "toyota_amador", "production_challenger"] },
  { id: "bmw", label: "BMW", classId: "bmw", access: "pro", categories: ["bmw_m2", "production_challenger"] },
  { id: "gt4", label: "GT4", classId: "gt4", access: "elite", categories: ["gt4", "endurance"] },
  { id: "gt3", label: "GT3", classId: "gt3", access: "elite", categories: ["gt3", "endurance"] },
  { id: "lmp2", label: "LMP2", classId: "lmp2", access: "elite", categories: ["endurance"] },
];

// `label` (série) + tier reconstroem o nome. Para as multiclasse o tier é só o
// nome da categoria ("Production"/"Endurance"), pois a série já dá a linha.
export const CATEGORY_TIER_LABEL = {
  mazda_rookie: "Rookie",
  mazda_amador: "Cup",
  toyota_rookie: "Rookie",
  toyota_amador: "Cup",
  bmw_m2: "Cup",
  production_challenger: "Production",
  gt4: "Series",
  gt3: "Championship",
  endurance: "Endurance",
};

export const SPECIAL_STANDING_GROUPS = {
  production_challenger: [
    { id: "bmw", label: "BMW", color: "#bc8cff" },
    { id: "toyota", label: "Toyota", color: "#f2cc60" },
    { id: "mazda", label: "Mazda", color: "#c8102e" },
  ],
  endurance: [
    { id: "lmp2", label: "LMP2", color: "#f2cc60" },
    { id: "gt3", label: "GT3", color: "#e73f47" },
    { id: "gt4", label: "GT4", color: "#58a6ff" },
  ],
};

// Categorias especiais movem exatamente uma equipe por classe por temporada
// (production: bmw/toyota/mazda; endurance: gt4/gt3). LMP2 é fixa: não sobe nem
// desce (ver promotion/block3.rs — ENDURANCE_PAIRS exclui lmp2).
const NO_MOVEMENT_SPECIAL_CLASSES = new Set(["endurance:lmp2"]);

const PROMOTION_ONLY_TEAM_ZONE_CATEGORIES = new Set([
  "mazda_rookie",
  "toyota_rookie",
  "bmw_m2",
  "gt4",
  "gt3",
  "lmp2",
]);
const NO_MOVEMENT_TEAM_ZONE_CATEGORIES = new Set(["endurance"]);
const PRODUCTION_SPECIAL_FEEDERS = new Set([
  "mazda_rookie",
  "toyota_rookie",
  "mazda_amador",
  "toyota_amador",
  "bmw_m2",
]);
const ENDURANCE_SPECIAL_FEEDERS = new Set(["lmp2", "gt3", "gt4"]);

export function normalizeClassId(value) {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

// A linha (série) de uma categoria. Categorias regulares pertencem a exatamente
// uma série; as multiclasse (production/endurance) pertencem a várias, então a
// pista de classe (do time do jogador) desempata.
export function resolveSeriesForLane(category, classHint) {
  const hint = normalizeClassId(classHint);
  const candidates = CATEGORY_SERIES.filter((serie) => serie.categories.includes(category));
  if (candidates.length === 0) return CATEGORY_SERIES[0];
  if (candidates.length === 1) return candidates[0];
  return candidates.find((serie) => serie.classId === hint || serie.id === hint) ?? candidates[0];
}

// Nav inicial (série + categoria) a partir do time do jogador, respeitando o
// Bloco Especial (que força production/endurance na linha do próprio jogador).
export function resolveInitialNav(phase, playerTeamCategory, classHint, acceptedSpecialOffer) {
  const forced = getForcedSpecialStandingCategory(phase, playerTeamCategory, acceptedSpecialOffer);
  const laneCategory = playerTeamCategory ?? ALL_CATEGORIES[0];
  const series = resolveSeriesForLane(laneCategory, classHint);
  const viewCategory = forced ?? laneCategory;
  if (!series.categories.includes(viewCategory)) {
    return { seriesId: resolveSeriesForLane(viewCategory, classHint).id, viewCategory };
  }
  return { seriesId: series.id, viewCategory };
}

export function getForcedSpecialStandingCategory(phase, playerTeamCategory, acceptedSpecialOffer) {
  if (phase !== "BlocoEspecial") {
    return null;
  }

  const offeredSpecialCategory =
    typeof acceptedSpecialOffer?.special_category === "string"
      ? acceptedSpecialOffer.special_category.trim().toLowerCase()
      : null;

  if (offeredSpecialCategory === "production_challenger" || offeredSpecialCategory === "endurance") {
    return offeredSpecialCategory;
  }

  if (playerTeamCategory === "production_challenger" || playerTeamCategory === "endurance") {
    return playerTeamCategory;
  }

  if (PRODUCTION_SPECIAL_FEEDERS.has(playerTeamCategory)) {
    return "production_challenger";
  }

  if (ENDURANCE_SPECIAL_FEEDERS.has(playerTeamCategory)) {
    return "endurance";
  }

  return null;
}

// Reordena os grupos de classe de uma categoria multiclasse para que a classe da
// linha atual apareça primeiro (mantendo a ordem relativa das demais).
export function orderSpecialGroupsForClass(groups, classId) {
  if (!groups) return null;
  const index = groups.findIndex((group) => group.id === classId);
  if (index <= 0) return groups;
  const reordered = [...groups];
  const [lead] = reordered.splice(index, 1);
  reordered.unshift(lead);
  return reordered;
}

export function getSpecialClassRelegationCount(category, classId) {
  if (NO_MOVEMENT_SPECIAL_CLASSES.has(`${category}:${classId}`)) {
    return 0;
  }
  return 1;
}

export function getZoneCutoffs(categoria) {
  if (PROMOTION_ONLY_TEAM_ZONE_CATEGORIES.has(categoria)) {
    return { promotionCount: 1, relegationCount: 0 };
  }
  if (NO_MOVEMENT_TEAM_ZONE_CATEGORIES.has(categoria)) {
    return { promotionCount: 0, relegationCount: 0 };
  }
  return { promotionCount: 1, relegationCount: 1 };
}

export function buildSpecialStandingSections(items, classGroups) {
  if (!classGroups) {
    return [{ id: "all", label: null, color: "#7d8590", items }];
  }

  const knownIds = new Set(classGroups.map((group) => group.id));
  const sections = classGroups
    .map((group) => ({
      ...group,
      items: items.filter((item) => normalizeClassId(item.classe) === group.id),
    }))
    .filter((section) => section.items.length > 0);

  const unknownItems = items.filter((item) => !knownIds.has(normalizeClassId(item.classe)));
  if (unknownItems.length > 0) {
    sections.push({
      id: "outros",
      label: i18n.t("standings.others"),
      color: "#7d8590",
      items: unknownItems,
    });
  }

  return sections;
}

export function hasSpecialStandingResults(driverStandings, teamStandings) {
  return (
    driverStandings.some((driver) => {
      const hasRoundResult = (driver.results ?? []).some(Boolean);
      return hasRoundResult || driver.pontos > 0 || driver.vitorias > 0 || driver.podios > 0;
    })
    || teamStandings.some((team) => team.pontos > 0 || team.vitorias > 0)
  );
}
