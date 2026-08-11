import { afterEach, describe, expect, it } from "vitest";

import i18n from "../../i18n/index.js";
import {
  CATEGORIES,
  CATEGORY_TIER,
  LEVEL_BANDS,
  LICENSE_COLORS,
  MARKET_TIER_BY_CATEGORY,
  PIP_COUNT,
  bandForTier,
  brandOf,
  buildWeeklyClosingGroups,
  championshipColor,
  count_team_vacancies,
  formatLastChampionshipResult,
  formatSafeLastChampionshipResult,
  formatSafeWeeklyClosingPosition,
  formatTenureCounter,
  formatWeeklyClosingPosition,
  getRankStyle,
  getTeamMappingSortValue,
  getTeamMovementBadge,
  getTeamMovementOrder,
  inferWeeklyMovementKind,
  isRealCareerDebutCategory,
  isRookieCategory,
  is_regular_market_category,
  licenseTooltip,
  pipsFilled,
  playerCatToFilter,
  shortCatName,
  shortDestLabel,
  subcatColor,
  subcatLabel,
  subcatLogo,
  subcatLogoFit,
  tierBucket,
  tierColor,
} from "./preSeasonFormatters";

// Teste espelho dos helpers puros da pré-temporada (700 linhas sem teste, apontadas na
// vistoria de 10/08/2026). O que se guarda aqui é o que erra CALADO: um rótulo composto
// que perde a classe, uma faixa de nível que casa a banda errada, e sobretudo o
// classificador de movimento do fechamento semanal — o feed inteiro pendura o selo, a
// cor e a ordem no que ele devolve, e um `null` a mais some com o evento da tela.

afterEach(async () => {
  await i18n.changeLanguage("pt-BR");
});

describe("CATEGORIES", () => {
  it("o sentinela 'sem recorte' não carrega rótulo em português", () => {
    // O rótulo dele vem do i18n nos dois cabeçalhos que desenham a régua. Tê-lo aqui
    // era português cravado num módulo que o auditor de i18n não varre.
    const todas = CATEGORIES.find((c) => c.id === "all");
    expect(todas).toBeDefined();
    expect(todas.label).toBeUndefined();
  });

  it("as demais têm rótulo e cor, e os separadores não", () => {
    CATEGORIES.filter((c) => c.id !== "all" && !c.isSeparator).forEach((cat) => {
      expect(typeof cat.label).toBe("string");
      expect(cat.color).toMatch(/^#|^rgba/);
      expect(Array.isArray(cat.dbIds)).toBe(true);
    });
  });
});

describe("rótulo e cor de subcategoria", () => {
  it("chave simples cai no dicionário", () => {
    expect(subcatLabel("gt3")).toBe("GT3 Championship");
    expect(subcatLabel("mazda_rookie")).toBe("Mazda Rookie");
  });

  it("chave composta compõe categoria · classe", () => {
    expect(subcatLabel("production_challenger:bmw")).toBe("Production · BMW");
    expect(subcatLabel("endurance:gt3")).toBe("Endurance · GT3");
  });

  it("classe desconhecida sobe para caixa alta em vez de sumir", () => {
    expect(subcatLabel("endurance:gtx")).toBe("Endurance · GTX");
  });

  it("chave desconhecida devolve a própria chave", () => {
    expect(subcatLabel("formula_e")).toBe("formula_e");
    expect(subcatLabel(null)).toBeNull();
  });

  it("a cor de uma chave composta é a da CATEGORIA, não a da classe", () => {
    // Production em roxo mesmo quando a classe é BMW: o divisor puxa a cor do pai.
    expect(subcatColor("production_challenger:bmw")).toBe(subcatColor("production_challenger"));
  });

  it("categoria desconhecida cai no azul neutro", () => {
    expect(subcatColor("formula_e")).toBe("#58a6ff");
  });

  it("shortCatName encurta as multiclasse e deixa o resto no dicionário", () => {
    expect(shortCatName("production_challenger")).toBe("Production");
    expect(shortCatName("endurance")).toBe("Endurance");
    expect(shortCatName("gt4")).toBe("GT4 Championship");
  });

  it("shortDestLabel tira o sufixo longo da etiqueta", () => {
    expect(shortDestLabel("gt3")).toBe("GT3");
    expect(shortDestLabel("lmp2")).toBe("LMP2");
    expect(shortDestLabel("mazda")).toBe("Mazda Cup");
  });

  it("chave composta não tem brasão", () => {
    // O logo é de categoria; uma classe dentro dela não tem arte própria.
    expect(subcatLogo("endurance:gt3")).toBeNull();
    expect(subcatLogo("gt3")).toMatch(/GT3/);
    expect(subcatLogo("formula_e")).toBeNull();
  });

  it("categoria sem ajuste de arte cai no enquadramento padrão", () => {
    expect(subcatLogoFit("formula_e")).toBe(subcatLogoFit("__inexistente__"));
    expect(subcatLogoFit("toyota")).not.toBe(subcatLogoFit("formula_e"));
  });
});

describe("faixas de nível do mercado", () => {
  it("cada tier casa a banda de cima para baixo", () => {
    expect(bandForTier(6).key).toBe("elite");
    expect(bandForTier(5).key).toBe("elite");
    expect(bandForTier(4).key).toBe("master");
    expect(bandForTier(3).key).toBe("superpro");
    expect(bandForTier(2).key).toBe("pro");
    expect(bandForTier(1).key).toBe("amador");
    expect(bandForTier(0).key).toBe("rookie");
  });

  it("tier ausente ou não-numérico cai na banda de base", () => {
    // O jogador não vem com market_tier; sem o piso, o painel ficaria sem banda.
    expect(bandForTier(null).key).toBe("rookie");
    expect(bandForTier(undefined).key).toBe("rookie");
    expect(bandForTier("elite").key).toBe("rookie");
  });

  it("as bandas estão em ordem decrescente de minTier", () => {
    // `find` casa o PRIMEIRO cujo minTier cabe: fora de ordem, tudo viraria elite.
    const tiers = LEVEL_BANDS.map((b) => b.minTier);
    expect([...tiers].sort((a, b) => b - a)).toEqual(tiers);
  });

  it("o tier do jogador espelha as categorias do backend", () => {
    expect(bandForTier(MARKET_TIER_BY_CATEGORY.gt3).key).toBe("master");
    expect(bandForTier(MARKET_TIER_BY_CATEGORY.mazda_rookie).key).toBe("rookie");
  });
});

describe("faixas de atributo e pips", () => {
  it("o balde de tier reparte 0-100 em cinco", () => {
    expect(tierBucket(0)).toBe(0);
    expect(tierBucket(19)).toBe(0);
    expect(tierBucket(20)).toBe(1);
    expect(tierBucket(99)).toBe(4);
    expect(tierBucket(100)).toBe(4);
  });

  it("valor fora da régua é preso nas pontas", () => {
    expect(tierBucket(-30)).toBe(0);
    expect(tierBucket(999)).toBe(4);
    expect(tierBucket(null)).toBe(0);
    expect(tierColor(999)).toBe(tierColor(100));
  });

  it("os pips acendem proporcional e nunca passam do total", () => {
    expect(pipsFilled(0)).toBe(0);
    expect(pipsFilled(1)).toBe(1);
    expect(pipsFilled(50)).toBe(3);
    expect(pipsFilled(100)).toBe(PIP_COUNT);
    expect(pipsFilled(999)).toBe(PIP_COUNT);
    expect(pipsFilled(null)).toBe(0);
  });

  it("o pódio tem cor de medalha e o resto herda o texto", () => {
    expect(championshipColor(1)).toBe("#ffd700");
    expect(championshipColor(3)).toBe("#cd7f32");
    expect(championshipColor(4)).toBe("var(--text-primary)");
    expect(getRankStyle(1).border).toBe("#ffd700");
    expect(getRankStyle(4)).toBeNull();
  });
});

describe("categoria: marca, rookie e filtro inicial", () => {
  it("a marca sai do prefixo da categoria", () => {
    expect(brandOf("mazda_rookie")).toBe("mazda");
    expect(brandOf("toyota_amador")).toBe("toyota");
    expect(brandOf("bmw_m2")).toBe("bmw");
    expect(brandOf("gt3")).toBe("gt3");
    expect(brandOf(null)).toBeNull();
  });

  it("rookie é sufixo, não prefixo", () => {
    expect(isRookieCategory("mazda_rookie")).toBe(true);
    expect(isRookieCategory("gt3")).toBe(false);
    expect(isRookieCategory(null)).toBe(false);
  });

  it("estreia de carreira de verdade é só a base da escada", () => {
    expect(isRealCareerDebutCategory("mazda_rookie")).toBe(true);
    expect(isRealCareerDebutCategory("toyota_rookie")).toBe(true);
    expect(isRealCareerDebutCategory("gt4")).toBe(false);
  });

  it("a categoria do jogador vira o filtro de abertura da tela", () => {
    expect(playerCatToFilter("mazda_amador")).toBe("mazda");
    expect(playerCatToFilter("bmw_m2")).toBe("bmw");
    expect(playerCatToFilter("gt3")).toBe("gt3");
    expect(playerCatToFilter(null)).toBe("all");
    expect(playerCatToFilter("formula_e")).toBe("all");
  });

  it("o filtro de abertura sempre existe na régua de categorias", () => {
    const ids = new Set(CATEGORIES.map((c) => c.id));
    ["mazda_rookie", "toyota_amador", "bmw_m2", "production_challenger", "gt4", "gt3", "endurance"]
      .forEach((cat) => expect(ids.has(playerCatToFilter(cat))).toBe(true));
  });

  it("lmp2 não é categoria de mercado regular", () => {
    // Ela existe como classe dentro do endurance; sozinha, não abre janela.
    expect(is_regular_market_category("gt3")).toBe(true);
    expect(is_regular_market_category("lmp2")).toBe(false);
    expect(is_regular_market_category("outras")).toBe(false);
  });
});

describe("movimento de equipe entre categorias", () => {
  it("subir de tier é promoção e descer é rebaixamento", () => {
    expect(getTeamMovementBadge("gt4", "gt3").kind).toBe("promoted");
    expect(getTeamMovementBadge("gt3", "gt4").kind).toBe("relegated");
  });

  it("ficar na mesma categoria não é movimento", () => {
    expect(getTeamMovementBadge("gt3", "gt3")).toBeNull();
    expect(getTeamMovementBadge(null, "gt3")).toBeNull();
  });

  it("categorias de mesmo tier não geram selo", () => {
    // mazda_amador e toyota_amador estão no mesmo degrau: trocar de marca não é subir.
    expect(CATEGORY_TIER.mazda_amador).toBe(CATEGORY_TIER.toyota_amador);
    expect(getTeamMovementBadge("mazda_amador", "toyota_amador")).toBeNull();
  });

  it("a ordenação põe promovidas antes de rebaixadas, e paradas primeiro", () => {
    expect(getTeamMovementOrder({ categoria_anterior: "gt4", _categoria: "gt3" })).toBe(1);
    expect(getTeamMovementOrder({ categoria_anterior: "gt3", _categoria: "gt4" })).toBe(2);
    expect(getTeamMovementOrder({ categoria_anterior: "gt3", _categoria: "gt3" })).toBe(0);
  });

  it("equipe sem posição de temporada vai para o fim da lista", () => {
    expect(getTeamMappingSortValue({ temp_posicao: 3 })).toBe(3);
    expect(getTeamMappingSortValue({ temp_posicao: 0 })).toBe(999);
    expect(getTeamMappingSortValue({})).toBe(999);
  });

  it("conta as vagas abertas nos dois carros", () => {
    expect(count_team_vacancies({ piloto_1_nome: "A", piloto_2_nome: "B" })).toBe(0);
    expect(count_team_vacancies({ piloto_1_nome: "A" })).toBe(1);
    expect(count_team_vacancies({})).toBe(2);
  });
});

describe("formatos de resultado", () => {
  it("resultado de campeonato só sai completo", () => {
    // Meia informação ("3º/") é pior que nenhuma na etiqueta do piloto.
    expect(formatSafeLastChampionshipResult({ last_championship_position: 3, last_championship_total_drivers: 24 }))
      .toBe("3º/24");
    expect(formatSafeLastChampionshipResult({ last_championship_position: 3 })).toBeNull();
    expect(formatSafeLastChampionshipResult(null)).toBeNull();
  });

  it("a versão ordinalizada acompanha o idioma", async () => {
    const piloto = { last_championship_position: 3, last_championship_total_drivers: 24 };
    expect(formatLastChampionshipResult(piloto)).toBe("3º/24");
    await i18n.changeLanguage("en-US");
    expect(formatLastChampionshipResult(piloto)).toBe("3rd/24");
  });

  it("posição vazia do fechamento vira traço, não zero", () => {
    expect(formatSafeWeeklyClosingPosition(0)).toBe("--");
    expect(formatSafeWeeklyClosingPosition(null)).toBe("--");
    expect(formatSafeWeeklyClosingPosition(5)).toBe("5º");
    expect(formatWeeklyClosingPosition(null)).toBe("--");
    expect(formatWeeklyClosingPosition(5)).toBe("5º");
  });

  it("o contador de casa distingue o recém-chegado", () => {
    expect(formatTenureCounter(0)).toBeNull();
    expect(formatTenureCounter(1).isNewcomer).toBe(true);
    expect(formatTenureCounter(4).isNewcomer).toBe(false);
    expect(formatTenureCounter(4).label).toContain("4");
  });

  it("a dica de licença traduz a sigla e sobrevive a uma desconhecida", () => {
    expect(licenseTooltip("SE")).toContain("Super Elite");
    expect(licenseTooltip("ZZ")).toEqual(expect.any(String));
    expect(Object.keys(LICENSE_COLORS)).toEqual(["R", "A", "P", "SP", "E", "SE"]);
  });
});

describe("inferWeeklyMovementKind", () => {
  it("o tipo já vindo do backend manda, quando é conhecido", () => {
    expect(inferWeeklyMovementKind({ movement_kind: "retirement", event_type: "ContractExpired" }))
      .toBe("retirement");
  });

  it("tipo desconhecido do backend não passa: cai na inferência", () => {
    expect(inferWeeklyMovementKind({ movement_kind: "inventado", event_type: "ContractExpired" }))
      .toBe("departure");
  });

  it("estreia é rookie só na base da escada", () => {
    expect(inferWeeklyMovementKind({ event_type: "RookieSigned", categoria: "mazda_rookie" })).toBe("rookie");
    expect(inferWeeklyMovementKind({ event_type: "RookieSigned", categoria: "gt3" })).toBe("signing");
  });

  it("permanecer na mesma equipe é renovação, não transferência", () => {
    expect(inferWeeklyMovementKind({
      event_type: "TransferCompleted",
      from_team: "Equipe A",
      to_team: "Equipe A",
      from_categoria: "gt4",
      categoria: "gt3",
    })).toBe("renewal");
  });

  it("transferência lê o degrau de origem e destino", () => {
    const base = { event_type: "TransferCompleted", from_team: "A", to_team: "B" };
    expect(inferWeeklyMovementKind({ ...base, from_categoria: "gt4", categoria: "gt3" })).toBe("promotion");
    expect(inferWeeklyMovementKind({ ...base, from_categoria: "gt3", categoria: "gt4" })).toBe("relegation");
    expect(inferWeeklyMovementKind({ ...base, from_categoria: "gt3", categoria: "gt3" })).toBe("lateral");
  });

  it("sem categoria de origem, é contratação e não promoção", () => {
    // Um piloto que veio de fora do grid não subiu degrau nenhum.
    expect(inferWeeklyMovementKind({ event_type: "TransferCompleted", from_team: "A", to_team: "B", categoria: "gt3" }))
      .toBe("signing");
  });

  it("evento fora do vocabulário não vira selo", () => {
    expect(inferWeeklyMovementKind({ event_type: "SeasonArchived" })).toBeNull();
  });
});

describe("buildWeeklyClosingGroups", () => {
  function evento(extra) {
    return {
      event_type: "TransferCompleted",
      driver_name: "Piloto",
      from_team: "A",
      to_team: "B",
      from_categoria: "gt3",
      categoria: "gt3",
      ...extra,
    };
  }

  it("payload vazio não quebra", () => {
    expect(buildWeeklyClosingGroups(null)).toEqual([]);
    expect(buildWeeklyClosingGroups({ events: [] })).toEqual([]);
  });

  it("descarta evento sem piloto, de tipo fora da lista ou de categoria irregular", () => {
    const grupos = buildWeeklyClosingGroups({
      events: [
        evento({ driver_name: null }),
        evento({ event_type: "SeasonArchived" }),
        evento({ categoria: "lmp2", from_categoria: "lmp2" }),
      ],
    });
    expect(grupos).toEqual([]);
  });

  it("agrupa por categoria, na ordem do mercado", () => {
    // A ordem é a do painel: categoria mais alta primeiro, e não alfabética.
    const grupos = buildWeeklyClosingGroups({
      events: [
        evento({ categoria: "mazda_rookie", from_categoria: "mazda_rookie" }),
        evento({ categoria: "gt3", from_categoria: "gt3" }),
        evento({ categoria: "gt4", from_categoria: "gt4" }),
      ],
    });
    expect(grupos.map((g) => g.category)).toEqual(["gt3", "gt4", "mazda_rookie"]);
    expect(grupos[0].label).toBe(subcatLabel("gt3"));
    expect(grupos[0].color).toBe(subcatColor("gt3"));
  });

  it("dentro do grupo, ordena pela posição no campeonato", () => {
    const grupos = buildWeeklyClosingGroups({
      events: [
        evento({ driver_name: "Terceiro", championship_position: 3 }),
        evento({ driver_name: "Primeiro", championship_position: 1 }),
        evento({ driver_name: "Sem posição" }),
      ],
    });
    expect(grupos[0].events.map((e) => e.driver_name)).toEqual(["Primeiro", "Terceiro", "Sem posição"]);
  });

  it("empate de posição desempata pelo nome", () => {
    const grupos = buildWeeklyClosingGroups({
      events: [
        evento({ driver_name: "Zeta", championship_position: 2 }),
        evento({ driver_name: "Alfa", championship_position: 2 }),
      ],
    });
    expect(grupos[0].events.map((e) => e.driver_name)).toEqual(["Alfa", "Zeta"]);
  });

  it("cada evento carrega o selo inferido", () => {
    const grupos = buildWeeklyClosingGroups({
      events: [evento({ from_categoria: "gt4", categoria: "gt3" })],
    });
    expect(grupos[0].events[0].movement_kind).toBe("promotion");
  });
});
