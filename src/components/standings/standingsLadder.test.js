import {
  ALL_CATEGORIES,
  CATEGORY_SERIES,
  CATEGORY_TIER_LABEL,
  SPECIAL_STANDING_GROUPS,
  buildSpecialStandingSections,
  getForcedSpecialStandingCategory,
  getSpecialClassRelegationCount,
  getZoneCutoffs,
  hasSpecialStandingResults,
  normalizeClassId,
  orderSpecialGroupsForClass,
  resolveInitialNav,
  resolveSeriesForLane,
} from "./standingsLadder";

// A escada de categorias é a espinha da tela de classificação: ela decide em que linha o
// jogador entra, que setas ▲▼ aparecem e quais equipes a tela pinta como promovidas ou
// rebaixadas. Era lógica pura de 208 linhas sem nenhum teste — um dos dois diretórios de
// componente com cobertura zero apontados na vistoria de 10/08/2026.
//
// O que estes casos travam é a parte que NÃO se enxerga lendo a tela: as categorias
// multiclasse (production/endurance) pertencem a várias séries ao mesmo tempo, e a regra de
// desempate entre elas é a única coisa que impede o jogador de cair na linha errada.

describe("resolveSeriesForLane", () => {
  it("resolve categoria regular para a única série que a contém", () => {
    expect(resolveSeriesForLane("mazda_rookie").id).toBe("mazda");
    expect(resolveSeriesForLane("bmw_m2").id).toBe("bmw");
    expect(resolveSeriesForLane("gt3").id).toBe("gt3");
  });

  it("usa a classe do time para desempatar uma categoria multiclasse", () => {
    // Production é o topo de mazda, toyota e bmw ao mesmo tempo. Sem a pista de classe a
    // tela escolheria sempre a primeira, e o piloto da BMW veria a linha da Mazda.
    expect(resolveSeriesForLane("production_challenger", "bmw").id).toBe("bmw");
    expect(resolveSeriesForLane("production_challenger", "toyota").id).toBe("toyota");
    expect(resolveSeriesForLane("endurance", "gt3").id).toBe("gt3");
    expect(resolveSeriesForLane("endurance", "lmp2").id).toBe("lmp2");
  });

  it("normaliza a pista de classe antes de comparar", () => {
    expect(resolveSeriesForLane("endurance", "  GT4  ").id).toBe("gt4");
  });

  it("cai na primeira candidata quando a pista de classe não bate com nenhuma", () => {
    expect(resolveSeriesForLane("endurance", "formula").id).toBe("gt4");
  });

  it("devolve a primeira série do menu para categoria desconhecida", () => {
    // Categoria fora da escada não pode devolver `undefined`: o chamador lê `.id` direto e
    // a tela quebraria em branco.
    expect(resolveSeriesForLane("categoria_que_nao_existe")).toBe(CATEGORY_SERIES[0]);
  });
});

describe("getForcedSpecialStandingCategory", () => {
  it("não força nada fora do Bloco Especial", () => {
    expect(getForcedSpecialStandingCategory("BlocoRegular", "mazda_rookie", null)).toBeNull();
    expect(getForcedSpecialStandingCategory(null, "gt3", null)).toBeNull();
  });

  it("a oferta aceita tem precedência sobre a categoria do time", () => {
    // O jogador da GT3 que aceitou uma vaga na Production precisa ver a Production, e não a
    // Endurance que a categoria do time dele indicaria.
    const forcada = getForcedSpecialStandingCategory("BlocoEspecial", "gt3", {
      special_category: "production_challenger",
    });
    expect(forcada).toBe("production_challenger");
  });

  it("normaliza a categoria da oferta", () => {
    expect(
      getForcedSpecialStandingCategory("BlocoEspecial", "gt3", { special_category: " ENDURANCE " }),
    ).toBe("endurance");
  });

  it("ignora oferta com categoria que não é especial", () => {
    expect(
      getForcedSpecialStandingCategory("BlocoEspecial", "gt3", { special_category: "gt4" }),
    ).toBe("endurance");
  });

  it("mantém a categoria quando o time já está numa especial", () => {
    expect(getForcedSpecialStandingCategory("BlocoEspecial", "endurance", null)).toBe("endurance");
    expect(getForcedSpecialStandingCategory("BlocoEspecial", "production_challenger", null)).toBe(
      "production_challenger",
    );
  });

  it("mapeia cada alimentadora para a especial correspondente", () => {
    for (const cat of ["mazda_rookie", "toyota_rookie", "mazda_amador", "toyota_amador", "bmw_m2"]) {
      expect(getForcedSpecialStandingCategory("BlocoEspecial", cat, null)).toBe(
        "production_challenger",
      );
    }
    for (const cat of ["lmp2", "gt3", "gt4"]) {
      expect(getForcedSpecialStandingCategory("BlocoEspecial", cat, null)).toBe("endurance");
    }
  });

  it("não força nada para quem está fora das duas escadas alimentadoras", () => {
    expect(getForcedSpecialStandingCategory("BlocoEspecial", null, null)).toBeNull();
  });
});

describe("resolveInitialNav", () => {
  it("abre na linha e na categoria do time no bloco regular", () => {
    expect(resolveInitialNav("BlocoRegular", "toyota_amador", "toyota", null)).toEqual({
      seriesId: "toyota",
      viewCategory: "toyota_amador",
    });
  });

  it("cai na primeira categoria da escada quando o jogador não tem equipe", () => {
    const nav = resolveInitialNav("BlocoRegular", null, null, null);
    expect(nav.viewCategory).toBe(ALL_CATEGORIES[0]);
    expect(nav.seriesId).toBe("mazda");
  });

  it("mantém a linha do jogador quando a especial forçada pertence a ela", () => {
    // O piloto da BMW no Bloco Especial vê a Production DENTRO da linha BMW: a Production é
    // o topo daquela linha, então trocar de série aqui tiraria o contexto dele.
    expect(resolveInitialNav("BlocoEspecial", "bmw_m2", "bmw", null)).toEqual({
      seriesId: "bmw",
      viewCategory: "production_challenger",
    });
  });

  it("troca de linha quando a categoria forçada não cabe na do jogador", () => {
    // A GT3 é linha "elite" e a Production é "pro": não existe série que contenha as duas,
    // então a tela precisa mudar de linha em vez de mostrar uma categoria fora do menu.
    const nav = resolveInitialNav("BlocoEspecial", "gt3", "gt3", {
      special_category: "production_challenger",
    });
    expect(nav.viewCategory).toBe("production_challenger");
    expect(nav.seriesId).toBe("mazda");
    const serie = CATEGORY_SERIES.find((s) => s.id === nav.seriesId);
    expect(serie.categories).toContain(nav.viewCategory);
  });
});

describe("zonas de promoção e rebaixamento", () => {
  it("categoria de entrada só promove, não rebaixa", () => {
    // Rookie e o topo de cada linha não têm degrau abaixo: pintar zona de rebaixamento ali
    // seria mentir para o jogador sobre um risco que não existe.
    for (const cat of ["mazda_rookie", "toyota_rookie", "bmw_m2", "gt4", "gt3", "lmp2"]) {
      expect(getZoneCutoffs(cat)).toEqual({ promotionCount: 1, relegationCount: 0 });
    }
  });

  it("endurance não move ninguém", () => {
    expect(getZoneCutoffs("endurance")).toEqual({ promotionCount: 0, relegationCount: 0 });
  });

  it("o meio da escada move nos dois sentidos", () => {
    expect(getZoneCutoffs("mazda_amador")).toEqual({ promotionCount: 1, relegationCount: 1 });
    expect(getZoneCutoffs("production_challenger")).toEqual({ promotionCount: 1, relegationCount: 1 });
  });

  it("a LMP2 dentro da endurance é fixa, as demais classes rebaixam uma", () => {
    // Espelha ENDURANCE_PAIRS em promotion/block3.rs, que exclui a LMP2 do movimento.
    expect(getSpecialClassRelegationCount("endurance", "lmp2")).toBe(0);
    expect(getSpecialClassRelegationCount("endurance", "gt3")).toBe(1);
    expect(getSpecialClassRelegationCount("production_challenger", "mazda")).toBe(1);
  });
});

describe("orderSpecialGroupsForClass", () => {
  const grupos = SPECIAL_STANDING_GROUPS.endurance;

  it("traz a classe da linha atual para o topo sem embaralhar o resto", () => {
    const ordenado = orderSpecialGroupsForClass(grupos, "gt4");
    expect(ordenado.map((g) => g.id)).toEqual(["gt4", "lmp2", "gt3"]);
  });

  it("devolve o mesmo arranjo quando a classe já lidera ou não existe", () => {
    expect(orderSpecialGroupsForClass(grupos, "lmp2")).toBe(grupos);
    expect(orderSpecialGroupsForClass(grupos, "formula")).toBe(grupos);
  });

  it("não muta a lista original", () => {
    const antes = grupos.map((g) => g.id);
    orderSpecialGroupsForClass(grupos, "gt3");
    expect(grupos.map((g) => g.id)).toEqual(antes);
  });

  it("tolera ausência de grupos", () => {
    expect(orderSpecialGroupsForClass(null, "gt3")).toBeNull();
  });
});

describe("buildSpecialStandingSections", () => {
  const item = (id, classe) => ({ id, classe });

  it("agrupa por classe e descarta seção vazia", () => {
    const secoes = buildSpecialStandingSections(
      [item("a", "gt3"), item("b", "GT3"), item("c", "lmp2")],
      SPECIAL_STANDING_GROUPS.endurance,
    );
    expect(secoes.map((s) => s.id)).toEqual(["lmp2", "gt3"]);
    // A classe chega do banco com caixa livre; agrupar por string crua partiria o GT3 em dois.
    expect(secoes.find((s) => s.id === "gt3").items).toHaveLength(2);
  });

  it("junta as classes desconhecidas numa seção de sobra em vez de sumir com elas", () => {
    // Perder uma linha aqui seria pior que mostrá-la fora de lugar: o jogador contaria o
    // grid e acharia que faltou piloto.
    const secoes = buildSpecialStandingSections(
      [item("a", "gt3"), item("b", "formula_e"), item("c", null)],
      SPECIAL_STANDING_GROUPS.endurance,
    );
    const outros = secoes.find((s) => s.id === "outros");
    expect(outros.items.map((i) => i.id)).toEqual(["b", "c"]);
  });

  it("devolve uma seção única quando a categoria não é multiclasse", () => {
    const secoes = buildSpecialStandingSections([item("a", "gt3")], null);
    expect(secoes).toHaveLength(1);
    expect(secoes[0]).toMatchObject({ id: "all", label: null });
  });
});

describe("hasSpecialStandingResults", () => {
  const piloto = (extra) => ({ results: [], pontos: 0, vitorias: 0, podios: 0, ...extra });

  it("é falso quando ninguém correu ainda", () => {
    expect(hasSpecialStandingResults([piloto(), piloto()], [{ pontos: 0, vitorias: 0 }])).toBe(false);
  });

  it("um resultado de rodada já conta, mesmo sem ponto", () => {
    // Chegar em 15º não pontua, e ainda assim o bloco especial ACONTECEU. Sem este caso a
    // tela diria "ainda não aconteceu" depois de uma corrida disputada.
    expect(hasSpecialStandingResults([piloto({ results: [null, { position: 15 }] })], [])).toBe(true);
  });

  it("pódio ou vitória do piloto contam", () => {
    expect(hasSpecialStandingResults([piloto({ podios: 1 })], [])).toBe(true);
    expect(hasSpecialStandingResults([piloto({ vitorias: 1 })], [])).toBe(true);
  });

  it("a tabela de equipes sozinha também conta", () => {
    expect(hasSpecialStandingResults([piloto()], [{ pontos: 12, vitorias: 0 }])).toBe(true);
  });
});

describe("consistência da escada", () => {
  it("toda categoria da escada aparece em alguma série", () => {
    const naSerie = new Set(CATEGORY_SERIES.flatMap((s) => s.categories));
    const foraDoMenu = ALL_CATEGORIES.filter((c) => !naSerie.has(c));
    expect(foraDoMenu).toEqual([]);
  });

  it("toda categoria de série tem rótulo de degrau", () => {
    // Sem o rótulo o menu monta o nome como "Mazda undefined".
    const semRotulo = [...new Set(CATEGORY_SERIES.flatMap((s) => s.categories))].filter(
      (c) => !CATEGORY_TIER_LABEL[c],
    );
    expect(semRotulo).toEqual([]);
  });

  it("cada série tem acesso declarado, e os dois grupos existem", () => {
    // O menu desenha uma separação física entre "pro" e "elite"; uma série sem acesso cairia
    // fora dos dois blocos e sumiria do dropdown.
    for (const serie of CATEGORY_SERIES) {
      expect(["pro", "elite"]).toContain(serie.access);
    }
    expect(CATEGORY_SERIES.some((s) => s.access === "pro")).toBe(true);
    expect(CATEGORY_SERIES.some((s) => s.access === "elite")).toBe(true);
  });

  it("as classes de cada especial batem com as séries que a contêm", () => {
    for (const [categoria, grupos] of Object.entries(SPECIAL_STANDING_GROUPS)) {
      const seriesQueContem = CATEGORY_SERIES.filter((s) => s.categories.includes(categoria));
      expect(new Set(grupos.map((g) => g.id))).toEqual(new Set(seriesQueContem.map((s) => s.classId)));
    }
  });
});

describe("normalizeClassId", () => {
  it("apara e derruba a caixa", () => {
    expect(normalizeClassId("  GT3 ")).toBe("gt3");
  });

  it("devolve string vazia para o que não é texto", () => {
    // O `classe` vem do banco e pode chegar nulo; devolver `undefined` faria o `.includes`
    // do chamador comparar contra nada e agrupar errado.
    expect(normalizeClassId(null)).toBe("");
    expect(normalizeClassId(42)).toBe("");
  });
});
