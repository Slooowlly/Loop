import {
  BEST_RANK_COLORS,
  MEDAL_COLORS,
  PLACEMENT_COLORS,
  bestDriversRanking,
  campanhaTemDados,
  corDeTextoSobre,
  curvaTemDados,
  placementInk,
  placementTone,
  temporadasDisputadas,
} from "./teamHistoryV2Logic";

// Teste espelho da lógica pura extraída de TeamHistoryDrawerV2.jsx (4.525 linhas, um export
// só, apontado como [Alta] na vistoria de 10/08/2026).
//
// Antes disto, `bestDriversRanking` era exercitado só de raspão pelo teste do componente
// inteiro — e é a função com mais regra de negócio do dossiê: ela decide quem a equipe
// considera o melhor piloto da história dela, com quatro critérios de desempate encadeados e
// uma consolidação por piloto que não aparece em lugar nenhum na tela.

/// Um mandato como o dossiê entrega.
function mandato(extra = {}) {
  return {
    driverId: "D1",
    name: "Piloto",
    nationality: "BR",
    isPlayer: false,
    stillHere: false,
    races: 10,
    titles: 0,
    wins: 0,
    podiums: 0,
    bestPosition: 0,
    firstYear: 2020,
    lastYear: 2021,
    ...extra,
  };
}

const nomes = (lista) => lista.map((p) => p.name);

describe("bestDriversRanking: consolidação por piloto", () => {
  it("soma os mandatos de quem saiu e voltou numa linha só", () => {
    // Na galeria de sucessão são duas passagens; no ranking é um currículo. Sem a soma, a
    // mesma pessoa apareceria duas vezes, cada metade abaixo de quem ela na verdade supera.
    const rank = bestDriversRanking([
      mandato({ driverId: "D1", name: "Voltou", races: 20, wins: 3, firstYear: 2018, lastYear: 2019 }),
      mandato({ driverId: "D1", name: "Voltou", races: 15, wins: 2, firstYear: 2022, lastYear: 2023 }),
    ]);
    expect(rank).toHaveLength(1);
    expect(rank[0]).toMatchObject({ races: 35, wins: 5, firstYear: 2018, lastYear: 2023 });
  });

  it("a melhor colocação é a MENOR das passagens, e zero não conta", () => {
    // Zero significa "nunca teve colocação registrada". Tratado como número, ele venceria
    // qualquer pódio de verdade.
    const rank = bestDriversRanking([
      mandato({ driverId: "D1", bestPosition: 0 }),
      mandato({ driverId: "D1", bestPosition: 4 }),
      mandato({ driverId: "D1", bestPosition: 9 }),
    ]);
    expect(rank[0].bestPosition).toBe(4);
  });

  it("continua na casa se QUALQUER passagem continua", () => {
    const rank = bestDriversRanking([
      mandato({ driverId: "D1", stillHere: false }),
      mandato({ driverId: "D1", stillHere: true }),
    ]);
    expect(rank[0].stillHere).toBe(true);
  });

  it("preenche a nacionalidade a partir da passagem que a tem", () => {
    const rank = bestDriversRanking([
      mandato({ driverId: "D1", nationality: null }),
      mandato({ driverId: "D1", nationality: "IT" }),
    ]);
    expect(rank[0].nationality).toBe("IT");
  });

  it("descarta contrato que nunca virou pista", () => {
    // O titular anunciado aparece na galeria (é onde se confere quem está no carro), mas um
    // ranking de quem correu não tem o que fazer com quem não correu.
    const rank = bestDriversRanking([
      mandato({ driverId: "D1", name: "Correu", races: 5 }),
      mandato({ driverId: "D2", name: "Anunciado", races: 0, titles: 3 }),
    ]);
    expect(nomes(rank)).toEqual(["Correu"]);
  });

  it("tolera elenco ausente ou vazio", () => {
    expect(bestDriversRanking([])).toEqual([]);
    expect(bestDriversRanking(null)).toEqual([]);
    expect(bestDriversRanking(undefined)).toEqual([]);
  });
});

describe("bestDriversRanking: ordem", () => {
  it("o título vale mais que a vitória, em qualquer quantidade", () => {
    // A regra que a lista dizia ao contrário: um campeão da casa com seis vitórias vale mais
    // para a história dela que um piloto de quinze vitórias que nunca levou o campeonato.
    const rank = bestDriversRanking([
      mandato({ driverId: "A", name: "Quinze vitórias", wins: 15, titles: 0 }),
      mandato({ driverId: "B", name: "Campeão", wins: 6, titles: 1 }),
    ]);
    expect(nomes(rank)).toEqual(["Campeão", "Quinze vitórias"]);
  });

  it("com títulos iguais, decide a vitória; depois o pódio", () => {
    const rank = bestDriversRanking([
      mandato({ driverId: "A", name: "Pódios", titles: 1, wins: 2, podiums: 20 }),
      mandato({ driverId: "B", name: "Vitórias", titles: 1, wins: 5, podiums: 6 }),
      mandato({ driverId: "C", name: "Poucos", titles: 1, wins: 2, podiums: 3 }),
    ]);
    expect(nomes(rank)).toEqual(["Vitórias", "Pódios", "Poucos"]);
  });

  it("no fundo da tabela a melhor colocação desempata, e quem nunca pontuou fica atrás", () => {
    // Sem este critério o fundo da lista sairia em ordem alfabética, que não diz nada.
    const rank = bestDriversRanking([
      mandato({ driverId: "A", name: "Sem colocação", bestPosition: 0 }),
      mandato({ driverId: "B", name: "Décimo", bestPosition: 10 }),
      mandato({ driverId: "C", name: "Quarto", bestPosition: 4 }),
    ]);
    expect(nomes(rank)).toEqual(["Quarto", "Décimo", "Sem colocação"]);
  });

  it("empatado em tudo, mais corridas na frente e o nome fecha a ordem", () => {
    const rank = bestDriversRanking([
      mandato({ driverId: "A", name: "Zeta", races: 10 }),
      mandato({ driverId: "B", name: "Alfa", races: 10 }),
      mandato({ driverId: "C", name: "Veterano", races: 40 }),
    ]);
    expect(nomes(rank)).toEqual(["Veterano", "Alfa", "Zeta"]);
  });

  it("não muta o elenco recebido", () => {
    const elenco = [mandato({ driverId: "A", races: 5 }), mandato({ driverId: "A", races: 7 })];
    const copia = JSON.parse(JSON.stringify(elenco));
    bestDriversRanking(elenco);
    expect(elenco).toEqual(copia);
  });
});

describe("placementTone e placementInk", () => {
  it("pinta as três medalhas e as três faixas", () => {
    expect(placementTone(1)).toBe(MEDAL_COLORS.first);
    expect(placementTone(2)).toBe(MEDAL_COLORS.second);
    expect(placementTone(3)).toBe(MEDAL_COLORS.third);
    expect(placementTone(4)).toBe(PLACEMENT_COLORS.nearMiss);
    expect(placementTone(5)).toBe(PLACEMENT_COLORS.nearMiss);
    expect(placementTone(6)).toBe(PLACEMENT_COLORS.topTen);
    expect(placementTone(10)).toBe(PLACEMENT_COLORS.topTen);
    expect(placementTone(11)).toBe(PLACEMENT_COLORS.outside);
    expect(placementTone(30)).toBe(PLACEMENT_COLORS.outside);
  });

  it("colocação fora da escala cai no tom de fora, sem cor indefinida", () => {
    // Uma cor `undefined` não pinta nada e o quadrado some do gráfico sem erro.
    for (const p of [0, -1, null, undefined, NaN]) {
      expect(placementTone(p)).toBe(PLACEMENT_COLORS.outside);
    }
  });

  it("a tinta escurece só sobre as três medalhas", () => {
    expect(placementInk(1)).toBe("#0b1524");
    expect(placementInk(3)).toBe("#0b1524");
    expect(placementInk(4)).not.toBe("#0b1524");
    expect(placementInk(0)).not.toBe("#0b1524");
  });
});

describe("corDeTextoSobre", () => {
  it("escurece sobre ouro e prata, clareia sobre os azuis escuros", () => {
    expect(corDeTextoSobre(MEDAL_COLORS.first)).toBe("#0d1622");
    expect(corDeTextoSobre(MEDAL_COLORS.second)).toBe("#0d1622");
    expect(corDeTextoSobre(PLACEMENT_COLORS.outside)).toBe("#eaf1f8");
    expect(corDeTextoSobre(PLACEMENT_COLORS.nearMiss)).toBe("#eaf1f8");
  });

  it("aceita a cor com e sem cerquilha", () => {
    expect(corDeTextoSobre("#f2c46d")).toBe(corDeTextoSobre("f2c46d"));
  });

  it("cai na tinta clara diante de cor malformada", () => {
    // A alternativa é `NaN > 0.6`, que devolve falso e acerta por acidente. Explícito é
    // melhor: a cor pode chegar nula de um tema custom.
    for (const c of ["", "#fff", "azul", null, undefined]) {
      expect(corDeTextoSobre(c)).toBe("#eaf1f8");
    }
  });

  it("pesa o verde muito mais que o azul, como o olho faz", () => {
    // Verde puro e azul puro têm a MESMA média aritmética de canal (85) e brilho percebido
    // completamente diferente. Uma média simples daria a mesma tinta aos dois.
    //
    // Verde puro fica em 0.587, logo abaixo do corte de 0.6 — ainda tinta clara. Um passo de
    // clareamento já o leva para o outro lado, e o azul puro (0.114) continua longe.
    expect(corDeTextoSobre("#40ff40")).toBe("#0d1622");
    expect(corDeTextoSobre("#4040ff")).toBe("#eaf1f8");
    // Branco e preto, as pontas.
    expect(corDeTextoSobre("#ffffff")).toBe("#0d1622");
    expect(corDeTextoSobre("#000000")).toBe("#eaf1f8");
  });
});

describe("cortes de gráfico", () => {
  it("a campanha exige duas rodadas e ao menos uma linha", () => {
    expect(campanhaTemDados({ rounds: [1, 2], lines: [{}] })).toBe(true);
    expect(campanhaTemDados({ rounds: [1], lines: [{}] })).toBe(false);
    expect(campanhaTemDados({ rounds: [1, 2], lines: [] })).toBe(false);
    expect(campanhaTemDados({ rounds: [1, 2] })).toBe(false);
    expect(campanhaTemDados(null)).toBe(false);
    expect(campanhaTemDados({ rounds: "não é lista", lines: [{}] })).toBe(false);
  });

  it("temporada sem corrida não é ponto de curva", () => {
    // A equipe pode existir no banco sem ter entrado no grid daquele ano.
    const seasons = [{ races: 0, position: "P1" }, { races: 12, position: "P3" }];
    expect(temporadasDisputadas(seasons)).toHaveLength(1);
    expect(temporadasDisputadas(null)).toEqual([]);
  });

  it("a curva exige duas temporadas disputadas e uma colocação conhecida", () => {
    expect(curvaTemDados([{ races: 10, position: "P3" }, { races: 10, position: "P5" }])).toBe(true);
    // Duas temporadas, nenhuma colocação numérica: não há o que desenhar.
    expect(curvaTemDados([{ races: 10, position: "—" }, { races: 10, position: null }])).toBe(false);
    // Uma só temporada disputada: um ponto não é curva.
    expect(curvaTemDados([{ races: 10, position: "P3" }, { races: 0, position: "P1" }])).toBe(false);
    expect(curvaTemDados(null)).toBe(false);
  });
});

describe("paleta do ranking", () => {
  it("as três primeiras posições usam a mesma paleta de medalha dos Records", () => {
    // Um número só muda de significado entre os blocos se mudar de cor. Divergir aqui faria
    // o pódio do ranking parecer outra coisa que o pódio dos recordes.
    expect(BEST_RANK_COLORS).toEqual([MEDAL_COLORS.first, MEDAL_COLORS.second, MEDAL_COLORS.third]);
  });
});
