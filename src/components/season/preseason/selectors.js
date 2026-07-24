import { invoke } from "@tauri-apps/api/core";
import {
  CATEGORIES,
  CLASS_LABELS,
  CLASS_PRIORITY,
  MULTICLASS_ORDER,
  FREE_AGENT_ORDER,
  CATEGORY_TIER,
  bandForTier,
  shortCatName,
  subcatLabel,
  subcatColor,
  brandOf,
  is_regular_market_category,
} from "../preSeasonFormatters.js";

// Ofertas agrupadas por categoria (N1/N2 dentro). Ordem: MARCA do jogador primeiro
// (ex.: Mazda antes de Toyota) e, dentro de cada marca, tier maior primeiro (Cup antes
// de Rookie). As demais marcas vêm depois, agrupadas, também por tier decrescente.
export function buildOffersByCategory(playerOffers, playerBrand, playerTier) {
  const groups = new Map();
  for (const offer of playerOffers) {
    const baseCat = offer.category || "outras";
    // Production/Endurance dividem por CLASSE (carro): chave "categoria:classe".
    const isMulti =
      (baseCat === "production_challenger" || baseCat === "endurance") && offer.class;
    const key = isMulti ? `${baseCat}:${offer.class}` : baseCat;
    if (!groups.has(key)) {
      groups.set(key, {
        cat: key,
        baseCat,
        classe: isMulti ? offer.class : null,
        tier: offer.category_tier ?? CATEGORY_TIER[baseCat] ?? 0,
        label: isMulti
          ? `${shortCatName(baseCat)} · ${CLASS_LABELS[offer.class] ?? offer.class.toUpperCase()}`
          : offer.category_label || subcatLabel(baseCat),
        n1: [],
        n2: [],
      });
    }
    const g = groups.get(key);
    if (offer.role === "N1") g.n1.push(offer);
    else g.n2.push(offer);
  }
  // Bucket de ordenação: 0 = PROMOÇÃO (tier acima do jogador, sempre no topo),
  // 1 = marca do jogador, 2 = demais marcas. Usa a categoria BASE (não a classe).
  // Tier de EXIBIÇÃO usa CATEGORY_TIER (distingue Production=3 de BMW=2, que no
  // backend são ambos tier 2).
  const bucketOf = (g) => {
    if (playerTier != null && g.tier > playerTier) return 0;
    if (playerBrand && brandOf(g.baseCat) === playerBrand) return 1;
    return 2;
  };
  for (const g of groups.values()) g.bucket = bucketOf(g);
  const dispTier = (g) => CATEGORY_TIER[g.baseCat] ?? g.tier;
  return [...groups.values()].sort((a, b) => {
    if (a.bucket !== b.bucket) return a.bucket - b.bucket;
    // Ordena por NÍVEL da categoria: maior no topo, rookies no fundo.
    // (GT3 > GT4 > Production > BMW/Cup > Rookie.)
    const dt = dispTier(b) - dispTier(a);
    if (dt !== 0) return dt;
    // Mesmo nível, mesma categoria multiclasse → ordem MULTICLASS_ORDER das classes.
    if (a.baseCat === b.baseCat && a.classe && b.classe) {
      const order = MULTICLASS_ORDER[a.baseCat] ?? [];
      const ia = order.indexOf(a.classe);
      const ib = order.indexOf(b.classe);
      if (ia !== ib) return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib);
    }
    // Mesmo nível, marcas diferentes (ex.: BMW vs Cups) → desempate por marca
    // (bmw < mazda < toyota), deixando o BMW acima das cups.
    const na = brandOf(a.baseCat) ?? "";
    const nb = brandOf(b.baseCat) ?? "";
    if (na !== nb) return na < nb ? -1 : 1;
    return 0;
  });
}

// Grid de equipes da categoria selecionada ("all" = todas as regulares).
export async function fetchGridTeams(careerId, selectedCat) {
  const dbIds = new Set();
  if (selectedCat === "all") {
    CATEGORIES.filter((c) => !c.isSeparator && c.id !== "all").forEach((c) =>
      c.dbIds?.forEach((id) => dbIds.add(id)),
    );
  } else {
    const cfg = CATEGORIES.find((c) => c.id === selectedCat);
    if (cfg) cfg.dbIds?.forEach((id) => dbIds.add(id));
  }

  // Busca PARALELA por categoria (era sequencial → grid demorava a refletir as
  // assinaturas após avançar a semana). Tag cada equipe com o dbId usado.
  const perCategory = await Promise.all(
    [...dbIds].map((dbId) =>
      invoke("get_teams_standings", { careerId, category: dbId })
        .then((teams) => teams.map((t) => ({ ...t, _categoria: dbId })))
        .catch(() => []),
    ),
  );
  const all = perCategory.flat();

  // Filtrar por classe quando categoria tem filterClass
  let final = all;
  if (selectedCat !== "all") {
    const cfg = CATEGORIES.find((c) => c.id === selectedCat);
    if (cfg?.filterClass) {
      final = all.filter((t) => {
        if (t.classe === cfg.filterClass) return true;
        if (t._categoria?.startsWith(cfg.filterClass)) return true;
        if (cfg.filterClass === "bmw" && t._categoria === "bmw_m2") return true;
        return false;
      });
    }
  }
  return final;
}

export function groupTeamsByClass(gridData) {
  const grouped = {};
  gridData.forEach((team) => {
    const key = team._categoria === "endurance" || team._categoria === "production_challenger"
      ? team._categoria
      : team.classe || team._categoria || "outras";
    grouped[key] = grouped[key] ?? [];
    grouped[key].push(team);
  });
  return grouped;
}

export function sortTeamClasses(groupedTeams) {
  return Object.keys(groupedTeams).sort((a, b) => {
    const pa = CLASS_PRIORITY.indexOf(a);
    const pb = CLASS_PRIORITY.indexOf(b);
    if (pa !== -1 && pb !== -1) return pa - pb;
    if (pa !== -1) return -1;
    if (pb !== -1) return 1;
    return a.localeCompare(b);
  });
}

// Free agents agrupados por FAIXA DE NÍVEL (onde correm hoje).
// Chave = banda do tier (market_tier), não a categoria/carteira. Dentro da banda,
// pilotos "frescos" primeiro e os "parados" no fim (marcador de inatividade).
export function buildFreeAgentsByBand(preseasonFreeAgents, selectedCat) {
  // Filtro do topo também recorta a coluna: mostra só quem pode pegar vaga na
  // categoria selecionada (interseção com eligible_categories, vindo do backend).
  const filterCfg = selectedCat === "all" ? null : CATEGORIES.find((c) => c.id === selectedCat);
  const filterDbIds = filterCfg?.dbIds ? new Set(filterCfg.dbIds) : null;
  const grouped = {};
  (preseasonFreeAgents ?? []).forEach((d) => {
    const cat = d.categoria || "outras";
    if (!is_regular_market_category(cat)) return;
    if (filterDbIds && !(d.eligible_categories ?? []).some((id) => filterDbIds.has(id))) return;
    const band = bandForTier(d.market_tier);
    (grouped[band.key] = grouped[band.key] ?? []).push(d);
  });
  Object.values(grouped).forEach((list) =>
    list.sort((a, b) => {
      // 1) Agrupa por marca/categoria dentro da banda: Toyota e Mazda têm a mesma cor,
      //    então intercalá-los confunde — cada marca vira uma sequência contígua.
      const pa = FREE_AGENT_ORDER.indexOf(a.categoria);
      const pb = FREE_AGENT_ORDER.indexOf(b.categoria);
      const oa = pa === -1 ? 999 : pa;
      const ob = pb === -1 ? 999 : pb;
      if (oa !== ob) return oa - ob;
      // 2) Dentro da marca: fresco antes do parado.
      const ia = a.seasons_idle ?? 0;
      const ib = b.seasons_idle ?? 0;
      if (ia !== ib) return ia - ib;
      // 3) Por nome.
      return (a.driver_name ?? "").localeCompare(b.driver_name ?? "");
    }),
  );
  return grouped;
}

// Veteranos sem vaga agrupados por categoria (modal de fim de pré-temporada).
export function buildDisplacedByCategory(displacedVeterans) {
  const grouped = {};

  displacedVeterans.forEach((driver) => {
    const category = driver.categoria || "outras";
    if (!is_regular_market_category(category)) return;
    grouped[category] = grouped[category] ?? [];
    grouped[category].push(driver);
  });

  return Object.entries(grouped)
    .sort(([a], [b]) => {
      const pa = FREE_AGENT_ORDER.indexOf(a);
      const pb = FREE_AGENT_ORDER.indexOf(b);
      if (pa !== -1 && pb !== -1) return pa - pb;
      if (pa !== -1) return -1;
      if (pb !== -1) return 1;
      return a.localeCompare(b);
    })
    .map(([category, drivers]) => ({
      category,
      color: subcatColor(category),
      label: subcatLabel(category),
      drivers,
    }));
}
