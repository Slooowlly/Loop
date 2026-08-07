// Ordenação, filtro e agregações do ranking global de pilotos (GlobalDriversTab).
//
// Puro: entra a lista de linhas de `get_global_driver_rankings`, sai lista/mapas
// prontos pra render. Os rótulos ficam em [globalDriverFormatters.js].

import i18n from "../../i18n/index.js";
import { currentLang } from "../../i18n/format.js";
import {
  categoryLabel,
  categoryTierOrder,
  nationalityKey,
  parseOptionalNumber,
  titleCategoryLabel,
} from "./globalDriverFormatters";

export const DEFAULT_SORT = { key: "historical_index", direction: "desc" };
export const DEFAULT_FILTERS = {
  status: "Todos",
  category: "Todas",
  nationality: "Todas",
  minAge: "",
  maxAge: "",
  champions: "all",
  injured: "all",
  favorites: "all",
};

// As métricas de carreira que a ficha do piloto sabe apontar para cá. Os ids dos
// cards de recorde da ficha são os mesmos nomes das colunas do ranking, então a
// tradução entre as duas telas é só validar que a métrica existe — mas a lista
// é explícita para que um id desconhecido caia no padrão em vez de ordenar por
// uma coluna que não existe.
export const RANKING_METRICS = ["corridas", "vitorias", "podios", "titulos"];

// A categoria só entra no filtro se ela EXISTE entre as opções montadas a partir
// das linhas. Uma chave de fora dos grupos (categoria agregada, piloto sem
// categoria) deixaria o `select` com um valor que nenhuma opção representa: a
// tela filtraria por algo que o jogador não vê escrito em lugar nenhum e não
// consegue desfazer sem usar o "Limpar filtros".
export function filterableCategory(options, category) {
  if (!category) return null;
  const disponiveis = (options?.categoryGroups ?? []).flatMap((group) =>
    group.options.map(([value]) => value),
  );
  return disponiveis.includes(category) ? category : null;
}

export function sortForMetric(metric) {
  if (!metric || !RANKING_METRICS.includes(metric)) return null;
  return { key: metric, direction: "desc" };
}

const SORTERS = {
  historical_rank: (row) => row.historical_rank ?? 9999,
  nome: (row) => row.nome ?? "",
  status: (row) => row.status ?? "",
  team_category: (row) => (row.status === "Aposentado" ? row.anos_aposentado ?? -1 : -1),
  idade: (row) => row.idade ?? 0,
  anos_carreira: (row) => row.anos_carreira ?? 0,
  salario_anual: (row) => row.salario_anual ?? 0,
  fama: (row) => row.fama ?? 0,
  historical_index: (row) => row.historical_index ?? 0,
  titulos: (row) => row.titulos ?? 0,
  vitorias: (row) => row.vitorias ?? 0,
  podios: (row) => row.podios ?? 0,
  poles: (row) => row.poles ?? 0,
  pontos: (row) => row.pontos ?? 0,
  corridas: (row) => row.corridas ?? 0,
  dnfs: (row) => row.dnfs ?? 0,
  lesoes: (row) => row.lesoes ?? 0,
};

export function sortRows(rows, sort) {
  const getter = SORTERS[sort.key] ?? SORTERS.historical_index;
  return [...rows].sort((a, b) => {
    const aValue = getter(a);
    const bValue = getter(b);
    const direction = sort.direction === "asc" ? 1 : -1;
    if (typeof aValue === "string" || typeof bValue === "string") {
      return String(aValue).localeCompare(String(bValue), currentLang()) * direction;
    }
    return ((aValue > bValue ? 1 : 0) - (aValue < bValue ? 1 : 0)) * direction || a.nome.localeCompare(b.nome, currentLang());
  });
}

export function filterRows(rows, filters) {
  const minAge = parseOptionalNumber(filters.minAge);
  const maxAge = parseOptionalNumber(filters.maxAge);

  return rows.filter((row) => {
    if (filters.status !== "Todos" && row.status !== filters.status) return false;
    if (filters.category !== "Todas" && !rowCategories(row).includes(filters.category)) return false;
    if (filters.nationality !== "Todas" && nationalityKey(row.nacionalidade) !== filters.nationality) return false;
    if (filters.champions === "champions" && (row.titulos ?? 0) <= 0) return false;
    if (filters.injured === "injured" && !row.is_lesionado) return false;
    if (filters.favorites === "only" && !row.is_favorito) return false;
    if (minAge != null && (row.idade ?? 0) < minAge) return false;
    if (maxAge != null && (row.idade ?? 0) > maxAge) return false;
    return true;
  });
}

export function buildRelativeRanks(rows, active) {
  if (!active) return null;

  const currentRankById = new Map();
  [...rows]
    .sort((left, right) => (left.historical_rank ?? 9999) - (right.historical_rank ?? 9999))
    .forEach((row, index) => currentRankById.set(row.id, index + 1));

  const previousRankById = new Map();
  [...rows]
    .sort((left, right) => previousGlobalRank(left) - previousGlobalRank(right))
    .forEach((row, index) => previousRankById.set(row.id, index + 1));

  const map = new Map();
  rows.forEach((row) => {
    const rank = currentRankById.get(row.id);
    const delta = (previousRankById.get(row.id) ?? rank) - rank;
    map.set(row.id, { rank, delta });
  });
  return map;
}

function previousGlobalRank(row) {
  return (row.historical_rank ?? 9999) + Number(row.historical_rank_delta ?? 0);
}

export function buildTableSections(rows, selectedCategory) {
  if (selectedCategory === "Todas") {
    return [{ key: "all", label: null, rows }];
  }

  const currentRows = [];
  const pastRows = [];

  rows.forEach((row) => {
    if (row.status === "Ativo" && row.categoria_atual === selectedCategory) {
      currentRows.push(row);
    } else {
      pastRows.push(row);
    }
  });

  return [
    {
      key: "current",
      label: currentRows.length > 0 ? `Atualmente em ${categoryLabel(selectedCategory)}` : null,
      rows: currentRows,
    },
    {
      key: "past",
      label: pastRows.length > 0 ? `Ja passaram por ${categoryLabel(selectedCategory)}` : null,
      rows: pastRows,
    },
  ].filter((section) => section.rows.length > 0);
}

export function buildFilterOptions(rows) {
  return {
    categoryGroups: buildCategoryGroups(uniqueSortedCategories(rows.flatMap(rowCategories))),
    nationalities: buildNationalityOptions(rows),
  };
}

// Agrupa as categorias por família/classe seguindo a progressão de carreira.
// Marcas de entrada (Mazda/Toyota: rookie -> championship -> production), depois BMW,
// e as classes de carro juntando pista + endurance da mesma classe (GT4, GT3, LMP2).
// Categorias agregadas/genéricas (endurance/production "geral") e SemCategoria ficam de
// fora do filtro de propósito — esses pilotos continuam visíveis em "Todas".
const CATEGORY_GROUP_DEFS = [
  { key: "mazda", label: "Mazda", members: ["mazda_rookie", "mazda_amador", "production_challenger:mazda"] },
  { key: "toyota", label: "Toyota", members: ["toyota_rookie", "toyota_amador", "production_challenger:toyota"] },
  { key: "bmw", label: "BMW", members: ["bmw_m2", "production_challenger:bmw"] },
  { key: "gt4", label: "GT4", members: ["gt4", "endurance:gt4"] },
  { key: "gt3", label: "GT3", members: ["gt3", "endurance:gt3"] },
  { key: "lmp2", label: "LMP2", members: ["lmp2", "endurance:lmp2"] },
];

function buildCategoryGroups(categories) {
  const present = new Set(categories);

  return CATEGORY_GROUP_DEFS.flatMap((def) => {
    const options = def.members
      .filter((member) => present.has(member))
      .map((member) => [member, categoryLabel(member)]);
    return options.length > 0 ? [{ key: def.key, label: def.label, options }] : [];
  });
}

// Os pilotos guardam a nacionalidade com gênero (ex.: "Brasileiro" / "Brasileira"),
// então agrupamos por país para mostrar uma única entrada por nacionalidade no filtro.
function buildNationalityOptions(rows) {
  const byCountry = new Map();
  rows.forEach((row) => {
    const label = row.nacionalidade;
    if (!label) return;
    const code = nationalityKey(label);
    if (!byCountry.has(code)) {
      byCountry.set(code, label);
    }
  });
  return [...byCountry.entries()]
    .map(([code, label]) => ({ code, label }))
    .sort((a, b) => a.label.localeCompare(b.label, currentLang()));
}

export function buildFocusedDriverRanks(rows, focusedDriver) {
  if (!focusedDriver) {
    return {};
  }

  return {
    races: metricRank(rows, "corridas", focusedDriver.id),
    wins: metricRank(rows, "vitorias", focusedDriver.id),
    titles: metricRank(rows, "titulos", focusedDriver.id),
    podiums: metricRank(rows, "podios", focusedDriver.id),
    careerYears: metricRank(rows, "anos_carreira", focusedDriver.id),
  };
}

function metricRank(rows, key, targetId) {
  const sorted = [...rows]
    .filter((row) => Number(row?.[key] ?? 0) > 0)
    .sort((left, right) =>
      Number(right?.[key] ?? 0) - Number(left?.[key] ?? 0)
      || String(left.nome ?? "").localeCompare(String(right.nome ?? ""), currentLang()),
    );
  let rank = 0;
  let previousValue = null;

  for (let index = 0; index < sorted.length; index += 1) {
    const value = Number(sorted[index]?.[key] ?? 0);
    if (previousValue == null || value !== previousValue) {
      rank = index + 1;
      previousValue = value;
    }
    if (sorted[index].id === targetId) {
      return rank;
    }
  }

  return null;
}

export function buildChampionshipChampionSections(rows) {
  const groups = buildChampionshipChampionGroups(rows);
  const normal = groups.filter((group) => !group.special).sort(compareChampionshipGroups);
  const special = groups.filter((group) => group.special).sort(compareChampionshipGroups);

  return [
    { key: "normal", label: null, groups: normal },
    { key: "special", label: i18n.t("globalDrivers.specialEvents"), groups: special },
  ].filter((section) => section.groups.length > 0);
}

function buildChampionshipChampionGroups(rows) {
  const groups = new Map();

  rows.forEach((row) => {
    const titleEntries = Array.isArray(row.titulos_por_categoria) ? row.titulos_por_categoria : [];
    titleEntries.forEach((entry) => {
      const titles = Number(entry?.titulos ?? 0);
      if (titles <= 0) return;

      const key = titleGroupKey(entry);
      const existing = groups.get(key) ?? {
        key,
        label: titleCategoryLabel(entry),
        category: entry?.categoria ?? "",
        className: entry?.classe ?? entry?.class_name ?? "",
        special: isSpecialTitleEntry(entry),
        totalTitles: 0,
        champions: [],
      };

      const years = titleEntryYears(entry);
      existing.totalTitles += titles;
      existing.champions.push({
        id: row.id,
        name: row.nome,
        rank: row.historical_rank,
        titles,
        years,
        yearTeams: titleEntryYearTeams(entry),
        latestYear: years[0] ?? 0,
      });
      groups.set(key, existing);
    });
  });

  return [...groups.values()]
    .map((group) => ({
      ...group,
      championCount: group.champions.length,
      champions: group.champions.sort((left, right) =>
        right.titles - left.titles
        || right.latestYear - left.latestYear
        || (left.rank ?? 9999) - (right.rank ?? 9999)
        || left.name.localeCompare(right.name, currentLang()),
      ),
    }))
    .sort(compareChampionshipGroups);
}

function titleGroupKey(entry) {
  return `${entry?.categoria ?? "unknown"}::${entry?.classe ?? entry?.class_name ?? ""}`;
}

function titleEntryYears(entry) {
  const years = Array.isArray(entry?.anos) ? entry.anos : [];
  return [...new Set(years.map(Number).filter((year) => Number.isFinite(year) && year > 0))]
    .sort((left, right) => right - left);
}

function titleEntryYearTeams(entry) {
  const entries = Array.isArray(entry?.anos_equipes) ? entry.anos_equipes : [];
  return entries
    .map((item) => ({
      ano: Number(item?.ano),
      equipe: item?.equipe ?? null,
      equipe_cor: item?.equipe_cor ?? null,
    }))
    .filter((item) => Number.isFinite(item.ano) && item.ano > 0)
    .sort((left, right) => right.ano - left.ano);
}

function isSpecialTitleEntry(entry) {
  return ["endurance", "production_challenger"].includes(entry?.categoria);
}

function compareChampionshipGroups(left, right) {
  return championshipGroupOrder(left) - championshipGroupOrder(right)
    || right.championCount - left.championCount
    || right.totalTitles - left.totalTitles
    || String(left.label).localeCompare(String(right.label), currentLang());
}

function championshipGroupOrder(group) {
  if (group.special) {
    const specialOrder = {
      "endurance:lmp2": 10,
      "endurance:gt3": 20,
      "endurance:gt4": 30,
      "production_challenger:bmw": 40,
      "production_challenger:toyota": 50,
      "production_challenger:mazda": 60,
    };
    return specialOrder[`${group.category}:${String(group.className ?? "").toLowerCase()}`] ?? 999;
  }

  const normalOrder = {
    lmp2: 10,
    gt3: 20,
    gt4: 30,
    bmw_m2: 40,
    mazda_amador: 50,
    toyota_amador: 60,
    mazda_rookie: 70,
    toyota_rookie: 80,
  };
  return normalOrder[group.category] ?? 900 + categoryTierOrder(group.category);
}

function rowCategories(row) {
  const categories = Array.isArray(row.categorias_historicas) ? row.categorias_historicas : [];
  return uniqueSortedCategories([...categories, row.categoria_atual].filter(Boolean));
}

function uniqueSortedCategories(values) {
  return [...new Set(values)].sort(compareCategoriesByProgression);
}

function compareCategoriesByProgression(a, b) {
  const aTier = categoryTierOrder(a);
  const bTier = categoryTierOrder(b);
  return aTier - bTier || categoryLabel(a).localeCompare(categoryLabel(b), currentLang());
}
