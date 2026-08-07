import { describe, expect, it } from "vitest";

import {
  CAR_PART_GROUPS,
  CAR_PART_KEYS,
  RESERVE_ROUNDS,
  carMeterReadings,
  carPartsRadar,
  chartView,
  efficiencyScatter,
  gridSummary,
  lineupDossier,
  radarView,
  rankInGrid,
  roundFlow,
  roundMoneyFlow,
  seasonLedger,
} from "./gridMetrics";

function team(overrides = {}) {
  return {
    id: "t1",
    nome: "Equipe 1",
    cash_balance: 100_000,
    car_level: 2,
    confiabilidade: 50,
    pit_crew_quality: 0,
    pontos: 10,
    posicao: 2,
    ...overrides,
  };
}

const GRID = [
  team({ id: "lider", nome: "Líder", posicao: 1, cash_balance: 200_000, car_level: 4, confiabilidade: 70, pontos: 40 }),
  team({ id: "t1", posicao: 2 }),
  team({ id: "t3", nome: "Equipe 3", posicao: 3, cash_balance: 60_000, car_level: 1, confiabilidade: 30, pontos: 2 }),
];

describe("gridSummary", () => {
  it("se declara sem grid quando há menos de duas equipes", () => {
    expect(gridSummary([]).hasGrid).toBe(false);
    expect(gridSummary([team()]).hasGrid).toBe(false);
  });

  it("elege o LÍDER pela posição no campeonato, não pelo caixa", () => {
    const summary = gridSummary([
      team({ id: "rico", posicao: 5, cash_balance: 900_000 }),
      team({ id: "primeiro", posicao: 1, cash_balance: 10_000 }),
    ]);
    expect(summary.leader.id).toBe("primeiro");
  });

  it("calcula média e máximo por eixo", () => {
    const summary = gridSummary(GRID);
    expect(summary.max.cash_balance).toBe(200_000);
    expect(summary.average.confiabilidade).toBeCloseTo(50, 5);
  });
});

describe("carMeterReadings", () => {
  it("posiciona a régua da média na MESMA escala do valor", () => {
    const [carro] = carMeterReadings(team(), GRID);
    // car_level 2 de 10 = 20%; média (4+2+1)/3 = 2.33 → 23.3%
    expect(carro.percent).toBeCloseTo(20, 5);
    expect(carro.averagePercent).toBeCloseTo(23.33, 1);
  });

  it("omite a régua quando não há grid para comparar", () => {
    const [carro] = carMeterReadings(team(), [team()]);
    expect(carro.average).toBeNull();
    expect(carro.averagePercent).toBeNull();
    expect(carro.tone).toBe("neutral");
  });

  it("marca como ruim quem está abaixo da média e bom quem está acima", () => {
    const meters = carMeterReadings(team({ confiabilidade: 20 }), GRID);
    expect(meters.find((meter) => meter.key === "confiabilidade").tone).toBe("bad");
    const acima = carMeterReadings(team({ confiabilidade: 90 }), GRID);
    expect(acima.find((meter) => meter.key === "confiabilidade").tone).toBe("good");
  });
});

describe("rankInGrid", () => {
  it("devolve a posição da equipe no campo pedido", () => {
    expect(rankInGrid(GRID, "lider", "cash_balance")).toBe(1);
    expect(rankInGrid(GRID, "t3", "cash_balance")).toBe(3);
    expect(rankInGrid(GRID, "inexistente", "cash_balance")).toBe(0);
  });
});

describe("lineupDossier", () => {
  function driver(overrides = {}) {
    return { id: "d1", skill: 60, midia: 40, idade: 25, pontos: 20, vitorias: 1, podios: 3, posicao_campeonato: 4, ...overrides };
  }
  const POOL = [
    driver({ id: "d1", skill: 60, midia: 40 }),
    driver({ id: "d2", skill: 80, midia: 70 }),
    driver({ id: "d3", skill: 40, midia: 10 }),
  ];
  const rows = [
    { role: "N1", salary: 100_000, driver: POOL[0] },
    { role: "N2", salary: 50_000, driver: null },
  ];

  it("tira a régua da média da CATEGORIA, não da equipe", () => {
    const dossier = lineupDossier({ drivers: POOL, rows });
    expect(dossier.averages.skill).toBeCloseTo(60, 5);
    expect(dossier.averages.midia).toBeCloseTo(40, 5);
    expect(dossier.hasGrid).toBe(true);
  });

  it("se declara sem régua quando a categoria tem poucos pilotos", () => {
    const dossier = lineupDossier({ drivers: POOL.slice(0, 2), rows });
    expect(dossier.hasGrid).toBe(false);
  });

  it("calcula o custo por ponto e admite quando ele não existe", () => {
    const dossier = lineupDossier({
      drivers: POOL,
      rows: [
        { role: "N1", salary: 100_000, driver: driver({ pontos: 25 }) },
        { role: "N2", salary: 80_000, driver: driver({ id: "d9", pontos: 0 }) },
      ],
    });
    expect(dossier.drivers[0].costPerPoint).toBe(4_000);
    // Zero ponto não vira divisão por zero disfarçada de informação.
    expect(dossier.drivers[1].costPerPoint).toBeNull();
  });

  it("marca o assento sem piloto na lista em vez de inventar atributos", () => {
    const dossier = lineupDossier({ drivers: POOL, rows });
    expect(dossier.drivers[1].hasDetail).toBe(false);
    expect(dossier.drivers[1].skill).toBe(0);
    expect(dossier.drivers[1].costPerPoint).toBeNull();
  });

  it("posiciona o piloto na categoria por habilidade e por mídia", () => {
    const dossier = lineupDossier({ drivers: POOL, rows });
    expect(dossier.drivers[0].skillRank).toBe(2);
    expect(dossier.drivers[0].midiaRank).toBe(2);
  });
});

describe("chartView", () => {
  it("a altura é decisão de layout e NÃO acompanha a largura", () => {
    const estreito = chartView(400, 180);
    const largo = chartView(1600, 180);
    expect(estreito.height).toBe(180);
    expect(largo.height).toBe(180);
    // Só a área útil horizontal cresce — foi o que faltava quando o SVG usava
    // viewBox fixo com width:100% e virava um gráfico de 470px numa tela larga.
    expect(largo.right - largo.left).toBeGreaterThan(estreito.right - estreito.left);
  });

  it("tem piso para não colapsar antes da primeira medição", () => {
    expect(chartView(0, 0).width).toBeGreaterThanOrEqual(320);
    expect(chartView(0, 0).height).toBeGreaterThanOrEqual(120);
  });
});

describe("seasonLedger", () => {
  const report = {
    season: { season_number: 2, round: 2, net: -10_000, constructor_prize_income: 0, expenses_total: 100_000 },
    cash_timeline: [
      { season_number: 2, round: 1, cash_balance: 100_000, net: -5_000, is_season_close: false },
      { season_number: 2, round: 2, cash_balance: 90_000, net: -5_000, is_season_close: false },
    ],
    expected_constructor_prize: 20_000,
  };
  const season = { rodada_atual: 2, total_rodadas: 6 };

  it("não desenha sem histórico ou sem calendário", () => {
    expect(seasonLedger({ report: null, team: team(), season }).hasData).toBe(false);
    expect(seasonLedger({ report, team: team(), season: { total_rodadas: 0 } }).hasData).toBe(false);
  });

  it("monta uma coluna por rodada corrida, uma por rodada restante e uma do prêmio", () => {
    const ledger = seasonLedger({ report, team: team({ cash_balance: 90_000 }), season });
    expect(ledger.hasData).toBe(true);
    const kinds = ledger.columns.map((column) => column.kind);
    expect(kinds.filter((kind) => kind === "real")).toHaveLength(2);
    expect(kinds.filter((kind) => kind === "projected")).toHaveLength(4);
    expect(kinds.filter((kind) => kind === "prize")).toHaveLength(1);
  });

  it("a linha de encerramento vira a coluna do prêmio, nunca uma rodada do calendário", () => {
    const comEncerramento = {
      ...report,
      cash_timeline: [...report.cash_timeline, { round: 3, cash_balance: 110_000, net: 20_000, is_season_close: true }],
    };
    const ledger = seasonLedger({ report: comEncerramento, team: team(), season });
    expect(ledger.columns.filter((column) => column.kind === "real")).toHaveLength(2);
  });

  it("descarta as rodadas de temporadas anteriores que vêm na janela do timeline", () => {
    // `cash_timeline` traz as últimas 12 rodadas SEM filtrar temporada, enquanto
    // `season` já vem filtrado. Sem o corte, rodadas do ano passado apareciam como
    // colunas deste ano e a média por rodada não batia com o que estava desenhado.
    const comAnoAnterior = {
      ...report,
      cash_timeline: [
        { season_number: 1, round: 9, cash_balance: 40_000, net: 8_000, is_season_close: false },
        { season_number: 1, round: 10, cash_balance: 48_000, net: 8_000, is_season_close: false },
        ...report.cash_timeline,
      ],
    };
    const ledger = seasonLedger({ report: comAnoAnterior, team: team(), season });
    const reais = ledger.columns.filter((column) => column.kind === "real");
    expect(reais).toHaveLength(2);
    expect(reais.map((column) => column.round)).toEqual([1, 2]);
  });

  it("numera as colunas pela rodada do lançamento, não pela posição na janela", () => {
    const janelaCortada = {
      ...report,
      season: { ...report.season, round: 14 },
      cash_timeline: [
        { season_number: 2, round: 13, cash_balance: 100_000, net: -5_000, is_season_close: false },
        { season_number: 2, round: 14, cash_balance: 90_000, net: -5_000, is_season_close: false },
      ],
    };
    const ledger = seasonLedger({ report: janelaCortada, team: team(), season: { rodada_atual: 14, total_rodadas: 16 } });
    const reais = ledger.columns.filter((column) => column.kind === "real");
    expect(reais.map((column) => column.round)).toEqual([13, 14]);
    // E as rodadas que faltam continuam a partir da 14, não a partir da 2.
    expect(ledger.columns.filter((column) => column.kind === "projected").map((column) => column.round)).toEqual([15, 16]);
  });

  it("não conta o prêmio duas vezes: a extrapolação ignora o prêmio já creditado", () => {
    const comPremio = { ...report, season: { ...report.season, net: 10_000, constructor_prize_income: 20_000 } };
    // (10.000 - 20.000) / 2 rodadas = -5.000, e não +5.000.
    expect(seasonLedger({ report: comPremio, team: team(), season }).avgNet).toBe(-5_000);
  });

  it("a reserva marca a COLUNA em que o caixa fura o piso, sem virar linha no eixo", () => {
    const ledger = seasonLedger({ report, team: team({ cash_balance: 90_000 }), season });
    expect(ledger.reserve).toBe(50_000 * RESERVE_ROUNDS);
    expect(ledger.breach.round).toBe(2);
    expect(ledger.columns.filter((column) => column.isBreach)).toHaveLength(1);
  });

  it("sem furo não há coluna marcada", () => {
    const folgado = { ...report, season: { ...report.season, expenses_total: 2_000 } };
    const ledger = seasonLedger({ report: folgado, team: team(), season });
    expect(ledger.breach).toBeNull();
    expect(ledger.columns.some((column) => column.isBreach)).toBe(false);
  });

  it("o prêmio não entra na escala das colunas: ele é clampado para não achatar as rodadas", () => {
    const premioEnorme = { ...report, expected_constructor_prize: 5_000_000 };
    const ledger = seasonLedger({ report: premioEnorme, team: team(), season });
    const prize = ledger.columns.find((column) => column.kind === "prize");
    expect(prize.clamped).toBe(true);
    expect(prize.y).toBeGreaterThanOrEqual(ledger.plotTop);
    // As rodadas continuam com altura própria em vez de virarem um traço.
    const rodada = ledger.columns.find((column) => column.kind === "real");
    expect(rodada.height).toBeGreaterThan(10);
  });

  it("com todas as rodadas negativas e sem prêmio, a linha do zero fica no topo", () => {
    const semPremio = { ...report, expected_constructor_prize: 0 };
    const ledger = seasonLedger({ report: semPremio, team: team(), season });
    expect(ledger.zeroY).toBeCloseTo(ledger.plotTop, 5);
  });

  it("na temporada toda no vermelho o prêmio ainda ganha altura para ser visto", () => {
    // Sem reserva de espaço acima do zero, a coluna do prêmio virava um traço de 2px
    // justamente no cenário em que ela é a notícia mais importante da tela.
    const ledger = seasonLedger({ report, team: team(), season });
    const prize = ledger.columns.find((column) => column.kind === "prize");
    expect(prize.height).toBeGreaterThan(20);
    expect(ledger.zeroY).toBeGreaterThan(ledger.plotTop);
    // E as rodadas negativas continuam ocupando a metade de baixo.
    const rodada = ledger.columns.find((column) => column.kind === "real");
    expect(rodada.y).toBeCloseTo(ledger.zeroY, 5);
    expect(rodada.height).toBeGreaterThan(20);
  });

  it("acomoda positivos e negativos dos dois lados da linha do zero", () => {
    const misto = {
      ...report,
      season: { round: 2, net: 10_000, constructor_prize_income: 0, expenses_total: 100_000 },
      cash_timeline: [
        { round: 1, cash_balance: 100_000, net: 30_000, is_season_close: false },
        { round: 2, cash_balance: 90_000, net: -10_000, is_season_close: false },
      ],
    };
    const ledger = seasonLedger({ report: misto, team: team(), season });
    expect(ledger.zeroY).toBeGreaterThan(ledger.plotTop);
    expect(ledger.zeroY).toBeLessThan(ledger.plotBottom);
  });

});

describe("carPartsRadar", () => {
  function car(id, levels, overrides = {}) {
    return {
      team_id: id,
      nome: `Equipe ${id}`,
      nome_curto: id.toUpperCase(),
      cor_primaria: "#58a6ff",
      parts: CAR_PART_KEYS.map((key, index) => ({ key, level: levels[index] })),
      ...overrides,
    };
  }

  const FRACO = new Array(11).fill(3);
  const FORTE = new Array(11).fill(8);
  const CARS = [car("t1", FRACO), car("rival", FORTE)];

  it("normaliza pela escala ABSOLUTA da peça, não pelo máximo do grid", () => {
    // Um grid inteiro no nível 2 tem de desenhar polígonos pequenos: normalizar pelo
    // máximo faria o pior carro do jogo tocar a borda e parecer no teto.
    const fraco = carPartsRadar({ cars: [car("a", new Array(11).fill(2)), car("b", new Array(11).fill(2))], playerTeamId: "a" });
    const raio = fraco.view.radius;
    const distancias = fraco.teams[0].polygon.split(" ").map((point) => {
      const [x, y] = point.split(",").map(Number);
      return Math.hypot(x - fraco.view.cx, y - fraco.view.cy);
    });
    for (const distancia of distancias) expect(distancia).toBeLessThan(raio * 0.3);
  });

  it("entrega um polígono por equipe e marca a do jogador", () => {
    const radar = carPartsRadar({ cars: CARS, playerTeamId: "t1" });
    expect(radar.hasData).toBe(true);
    expect(radar.teams).toHaveLength(2);
    expect(radar.player.id).toBe("t1");
    expect(radar.teams.filter((row) => row.isPlayer)).toHaveLength(1);
    expect(radar.axes).toHaveLength(CAR_PART_GROUPS.length);
  });

  it("agrupa as 11 peças em 5 áreas, sem perder nem repetir nenhuma", () => {
    const agrupadas = CAR_PART_GROUPS.flatMap((group) => group.parts);
    expect(agrupadas).toHaveLength(CAR_PART_KEYS.length);
    expect(new Set(agrupadas).size).toBe(CAR_PART_KEYS.length);
    for (const key of CAR_PART_KEYS) expect(agrupadas).toContain(key);
  });

  it("o nível da área é a MÉDIA das peças dela, não o máximo", () => {
    const levels = [...FRACO];
    // Asa dianteira no teto, o resto da aerodinâmica no chão.
    levels[CAR_PART_KEYS.indexOf("front_wing")] = 10;
    levels[CAR_PART_KEYS.indexOf("rear_wing")] = 1;
    levels[CAR_PART_KEYS.indexOf("sidepods")] = 1;
    const radar = carPartsRadar({ cars: [car("t1", levels), car("rival", FORTE)], playerTeamId: "t1" });
    const aero = radar.axes.findIndex((axis) => axis.key === "aero");
    expect(radar.player.levels[aero]).toBeCloseTo(4, 5);
  });

  it("aponta a área mais atrasada contra o melhor do grid", () => {
    const levels = [...FRACO];
    // Motor e câmbio no chão: o trem de força vira o buraco maior.
    levels[CAR_PART_KEYS.indexOf("engine")] = 1;
    levels[CAR_PART_KEYS.indexOf("gearbox")] = 1;
    const radar = carPartsRadar({ cars: [car("t1", levels), car("rival", FORTE)], playerTeamId: "t1" });
    expect(radar.weakest.key).toBe("powertrain");
    expect(radar.weakest.gapToBest).toBe(7);
  });

  it("não desenha sem carro registrado", () => {
    expect(carPartsRadar({ cars: [], playerTeamId: "t1" }).hasData).toBe(false);
    expect(carPartsRadar({ cars: [{ team_id: "x", parts: [] }], playerTeamId: "x" }).hasData).toBe(false);
  });

  it("mantém os rótulos dentro do viewBox", () => {
    const radar = carPartsRadar({ cars: CARS, playerTeamId: "t1", view: radarView(520, { max: 520, min: 520 }) });
    for (const axis of radar.axes) {
      expect(axis.label.x).toBeGreaterThanOrEqual(0);
      expect(axis.label.x).toBeLessThanOrEqual(radar.view.width);
      expect(axis.label.y).toBeGreaterThanOrEqual(0);
      expect(axis.label.y).toBeLessThanOrEqual(radar.view.height);
    }
  });
});

describe("radarView", () => {
  it("cresce com a largura medida, com teto e piso", () => {
    expect(radarView(2000).width).toBe(460);
    expect(radarView(10).width).toBe(240);
    expect(radarView(360).width).toBe(360);
  });

  it("devolve caixa mais larga que alta, para os rótulos caberem fora do círculo", () => {
    const view = radarView(400);
    expect(view.width).toBeGreaterThan(view.height);
    expect(view.radius * 2).toBeLessThan(view.height);
  });
});

describe("efficiencyScatter", () => {
  it("exige pelo menos três equipes", () => {
    expect(efficiencyScatter(GRID.slice(0, 2), "t1").hasData).toBe(false);
  });

  it("não desenha antes de a categoria pontuar", () => {
    const zerado = GRID.map((row) => ({ ...row, pontos: 0 }));
    const scatter = efficiencyScatter(zerado, "t1");
    expect(scatter.hasData).toBe(false);
    expect(scatter.reason).toBe("noPoints");
  });

  it("não repete valor nas marcas do eixo de pontos", () => {
    const raso = [
      team({ id: "a", cash_balance: 100_000, pontos: 1 }),
      team({ id: "b", cash_balance: 200_000, pontos: 1 }),
      team({ id: "c", cash_balance: 150_000, pontos: 0 }),
    ];
    const valores = efficiencyScatter(raso, "a").yTicks.map((tick) => tick.value);
    expect(new Set(valores).size).toBe(valores.length);
  });

  it("mede o resíduo contra a reta de conversão do grid", () => {
    const scatter = efficiencyScatter(GRID, "t1");
    expect(scatter.hasData).toBe(true);
    expect(scatter.player.id).toBe("t1");
    // A reta passa pelos três pontos quase exatamente; o desvio é pequeno mas definido.
    expect(Number.isFinite(scatter.player.residual)).toBe(true);
  });

  it("aponta resíduo positivo para quem pontua acima do que o caixa explica", () => {
    const grid = [
      team({ id: "a", cash_balance: 100_000, pontos: 10 }),
      team({ id: "b", cash_balance: 200_000, pontos: 20 }),
      team({ id: "c", cash_balance: 150_000, pontos: 40 }),
    ];
    expect(efficiencyScatter(grid, "c").player.residual).toBeGreaterThan(0);
  });

  it("ordena o grid por conversão e diz onde o jogador entra", () => {
    const grid = [
      team({ id: "a", cash_balance: 100_000, pontos: 10 }),
      team({ id: "b", cash_balance: 200_000, pontos: 20 }),
      team({ id: "c", cash_balance: 150_000, pontos: 40 }),
    ];
    const scatter = efficiencyScatter(grid, "a");
    expect(scatter.ranking.map((point) => point.id)[0]).toBe("c");
    expect(scatter.best.id).toBe("c");
    expect(scatter.worst.residual).toBeLessThanOrEqual(scatter.best.residual);
    expect(scatter.playerRank).toBeGreaterThan(0);
    expect(scatter.playerRank).toBeLessThanOrEqual(grid.length);
  });

  it("só declara acima ou abaixo quem sai da faixa de tolerância do grid", () => {
    const grid = [
      team({ id: "a", cash_balance: 100_000, pontos: 10 }),
      team({ id: "b", cash_balance: 200_000, pontos: 20 }),
      team({ id: "c", cash_balance: 150_000, pontos: 40 }),
    ];
    const scatter = efficiencyScatter(grid, "c");
    expect(scatter.tolerance).toBeGreaterThan(0);
    for (const point of scatter.points) {
      const fora = Math.abs(point.residual) > scatter.tolerance;
      expect(point.verdict === "onPar").toBe(!fora);
      if (point.verdict === "above") expect(point.residual).toBeGreaterThan(0);
      if (point.verdict === "below") expect(point.residual).toBeLessThan(0);
    }
    // O desvio de "c" é o maior do grid — se nem ele sai da faixa, o gráfico não
    // teria veredito nenhum para dar.
    expect(scatter.player.verdict).toBe("above");
  });

  it("entrega faixa e marcas de escala prontas para desenhar", () => {
    const scatter = efficiencyScatter(GRID, "t1");
    // Quatro vértices: a reta engordada pela tolerância para cima e para baixo.
    expect(scatter.band.split(" ")).toHaveLength(4);
    expect(scatter.yTicks.map((tick) => tick.value)).toEqual([0, Math.round(scatter.yMax / 2), Math.round(scatter.yMax)]);
    expect(scatter.xTicks.map((tick) => tick.anchor)).toEqual(["start", "middle", "end"]);
    // O pé do traço de desvio: onde a reta coloca a equipe, dentro da área de plotagem.
    for (const point of scatter.points) {
      expect(point.expectedY).toBeGreaterThanOrEqual(scatter.view.top);
      expect(point.expectedY).toBeLessThanOrEqual(scatter.view.bottom);
    }
  });
});

describe("seasonLedger — rótulos por coluna", () => {
  const base = {
    season: { season_number: 2, round: 2, net: 0, constructor_prize_income: 0, expenses_total: 40_000 },
    expected_constructor_prize: 0,
  };
  const season = { rodada_atual: 2, total_rodadas: 2 };

  it("na rodada que ganha, o resultado fica ACIMA da barra e o caixa abaixo da linha do zero", () => {
    const report = {
      ...base,
      season: { ...base.season, net: 30_000 },
      cash_timeline: [
        { season_number: 2, round: 1, cash_balance: 110_000, net: 10_000, is_season_close: false },
        { season_number: 2, round: 2, cash_balance: 130_000, net: 20_000, is_season_close: false },
      ],
    };
    const ledger = seasonLedger({ report, team: team({ cash_balance: 130_000 }), season });
    const coluna = ledger.columns[1];
    expect(coluna.positive).toBe(true);
    expect(coluna.deltaY).toBeLessThan(coluna.y);
    expect(coluna.cashY).toBeGreaterThan(ledger.zeroY);
  });

  it("na rodada que perde, o resultado fica ABAIXO da barra e o caixa acima da linha do zero", () => {
    const report = {
      ...base,
      season: { ...base.season, net: -30_000 },
      cash_timeline: [
        { season_number: 2, round: 1, cash_balance: 110_000, net: -10_000, is_season_close: false },
        { season_number: 2, round: 2, cash_balance: 90_000, net: -20_000, is_season_close: false },
      ],
    };
    const ledger = seasonLedger({ report, team: team({ cash_balance: 90_000 }), season });
    const coluna = ledger.columns[1];
    expect(coluna.positive).toBe(false);
    expect(coluna.deltaY).toBeGreaterThan(coluna.y + coluna.height);
    expect(coluna.cashY).toBeLessThan(ledger.zeroY);
  });

  it("sem prêmio não há divisória de encerramento", () => {
    const report = {
      ...base,
      cash_timeline: [{ season_number: 2, round: 1, cash_balance: 110_000, net: 10_000, is_season_close: false }],
    };
    expect(seasonLedger({ report, team: team(), season }).prizeDividerX).toBeNull();
  });
});

describe("roundMoneyFlow", () => {
  const LINES_IN = [{ key: "sponsorship_income" }, { key: "result_bonus" }];
  const LINES_OUT = [{ key: "event_operations_cost" }, { key: "salary_expense" }];

  function flow(latest, width = 600) {
    return roundMoneyFlow({ latest, incomeLines: LINES_IN, expenseLines: LINES_OUT, width });
  }

  it("não desenha rodada sem nenhuma linha", () => {
    expect(flow({}).hasData).toBe(false);
    expect(flow(null).hasData).toBe(false);
  });

  it("o tronco é a receita e as fitas da esquerda somam exatamente ele", () => {
    const resultado = flow({ sponsorship_income: 60_000, result_bonus: 20_000, event_operations_cost: 50_000, salary_expense: 10_000 });
    expect(resultado.trunk).toBe(80_000);
    const somaEsquerda = resultado.left.reduce((total, node) => total + node.value, 0);
    expect(somaEsquerda).toBe(resultado.trunk);
  });

  it("com sobra, o excedente vira um nó à DIREITA e a conta fecha dos dois lados", () => {
    const resultado = flow({ sponsorship_income: 60_000, result_bonus: 20_000, event_operations_cost: 50_000, salary_expense: 10_000 });
    const saldo = resultado.right.find((node) => node.key === "balance");
    expect(saldo.value).toBe(20_000);
    expect(resultado.right.reduce((total, node) => total + node.value, 0)).toBe(resultado.trunk);
    expect(resultado.left.some((node) => node.key === "coverage")).toBe(false);
  });

  it("com rombo, o que faltou vira um nó à ESQUERDA — o dinheiro saiu do caixa, não do nada", () => {
    const resultado = flow({ sponsorship_income: 30_000, event_operations_cost: 50_000, salary_expense: 10_000 });
    const cobertura = resultado.left.find((node) => node.key === "coverage");
    expect(cobertura.value).toBe(30_000);
    expect(resultado.trunk).toBe(60_000);
    expect(resultado.left.reduce((total, node) => total + node.value, 0)).toBe(resultado.trunk);
    expect(resultado.right.reduce((total, node) => total + node.value, 0)).toBe(resultado.trunk);
    expect(resultado.right.some((node) => node.key === "balance")).toBe(false);
  });

  it("as fitas encostam no tronco sem folga: a última termina onde o tronco termina", () => {
    // Vale mesmo com uma linha irrisória, em que o piso de `minBand` faria a soma das
    // bandas estourar o tronco se as âncoras não fossem reescaladas.
    const resultado = flow({ sponsorship_income: 60_000, result_bonus: 12, event_operations_cost: 50_000, salary_expense: 10_012 });
    const ultima = resultado.left[resultado.left.length - 1];
    expect(ultima.anchorBottom).toBeCloseTo(resultado.trunkTop + resultado.body, 1);
    expect(resultado.left[0].anchorTop).toBeCloseTo(resultado.trunkTop, 5);
  });

  it("a sobra escapa do escalonamento e nasce na borda, não espremida contra o tronco", () => {
    const resultado = flow({ sponsorship_income: 90_000, result_bonus: 10_000, event_operations_cost: 60_000, salary_expense: 10_000 });
    const saldo = resultado.right.find((node) => node.key === "balance");
    const maiorCusto = resultado.right[0];
    // Por ser o menor valor, a ordenação o jogaria para o ponto mais interno; ele é a
    // RESPOSTA da rodada e fica na mesma borda do maior custo.
    expect(saldo.x + saldo.pill).toBeCloseTo(maiorCusto.x + maiorCusto.pill, 1);
    expect(saldo.pill).toBeGreaterThan(maiorCusto.pill);
    expect(saldo.verdict).toBe(true);
  });

  it("a cobertura do rombo também nasce na borda esquerda, com pílula grossa", () => {
    const resultado = flow({ sponsorship_income: 30_000, event_operations_cost: 50_000, salary_expense: 10_000 });
    const cobertura = resultado.left.find((node) => node.key === "coverage");
    expect(cobertura.x).toBe(0);
    expect(cobertura.pill).toBeGreaterThan(resultado.left[0].pill);
    expect(cobertura.verdict).toBe(true);
  });

  it("o nó de veredito recebe respiro extra acima para o rótulo grande", () => {
    const resultado = flow({ sponsorship_income: 90_000, result_bonus: 10_000, event_operations_cost: 60_000, salary_expense: 10_000 });
    const nos = resultado.right;
    const saldo = nos[nos.length - 1];
    const anterior = nos[nos.length - 2];
    const folgaVeredito = saldo.top - anterior.bottom;
    const folgaComum = nos[1].top - nos[0].bottom;
    expect(folgaVeredito).toBeGreaterThan(folgaComum + 20);
  });

  it("escalona na horizontal: quanto menor o nó, mais perto do tronco ele nasce", () => {
    const resultado = flow({
      sponsorship_income: 50_000,
      result_bonus: 20_000,
      event_operations_cost: 40_000,
      salary_expense: 12_000,
    });
    // À esquerda o x CRESCE conforme o valor cai (nasce mais para dentro).
    const xEsquerda = resultado.left.map((node) => node.x);
    expect(xEsquerda[1]).toBeGreaterThan(xEsquerda[0]);
    // À direita o x DIMINUI conforme o valor cai (termina mais para dentro).
    const xDireita = resultado.right.map((node) => node.x);
    expect(xDireita[1]).toBeLessThan(xDireita[0]);
  });

  it("garante espaço vertical por nó mesmo quando a banda é mínima", () => {
    // Quatro linhas, três delas irrisórias: sem o piso de espaçamento elas viravam
    // bandas de 4px encostadas e os rótulos se sobrepunham.
    const resultado = flow({
      sponsorship_income: 90_000,
      result_bonus: 30,
      event_operations_cost: 89_000,
      salary_expense: 1_030,
    });
    for (const lado of [resultado.left, resultado.right]) {
      for (let index = 1; index < lado.length; index += 1) {
        expect(lado[index].top - lado[index - 1].top).toBeGreaterThanOrEqual(20);
      }
    }
  });

  it("a altura do card acompanha a necessidade dos rótulos, não só o tronco", () => {
    const doisNos = flow({ sponsorship_income: 60_000, event_operations_cost: 60_000 });
    const muitosNos = flow({
      sponsorship_income: 40_000,
      result_bonus: 10_000,
      gate_income: 5_000,
      partial_prize_income: 2_000,
      event_operations_cost: 57_000,
    });
    expect(muitosNos.height).toBeGreaterThan(doisNos.height);
  });

  it("a altura cresce com o número de nós em vez de espremer as fitas", () => {
    const poucos = flow({ sponsorship_income: 60_000, event_operations_cost: 60_000 });
    const muitos = flow({ sponsorship_income: 40_000, result_bonus: 20_000, event_operations_cost: 30_000, salary_expense: 30_000 });
    expect(muitos.height).toBeGreaterThanOrEqual(poucos.height);
  });

  it("estica na horizontal com a largura medida, sem mexer na altura", () => {
    const estreito = flow({ sponsorship_income: 60_000, event_operations_cost: 60_000 }, 400);
    const largo = flow({ sponsorship_income: 60_000, event_operations_cost: 60_000 }, 1600);
    expect(largo.height).toBe(estreito.height);
    expect(largo.rightX).toBeGreaterThan(estreito.rightX);
    expect(largo.trunkX).toBeGreaterThan(estreito.trunkX);
  });
});

describe("roundFlow", () => {
  const LINES = [{ key: "salary_expense" }, { key: "event_operations_cost" }];

  it("não desenha rodada sem entrada", () => {
    expect(roundFlow({ income_total: 0 }, LINES).hasData).toBe(false);
    expect(roundFlow(null, LINES).hasData).toBe(false);
  });

  it("ordena as saídas da maior para a menor e mede a fatia de cada uma", () => {
    const flow = roundFlow({ income_total: 100, salary_expense: 20, event_operations_cost: 50 }, LINES);
    expect(flow.expenses.map((expense) => expense.key)).toEqual(["event_operations_cost", "salary_expense"]);
    expect(flow.expenses[0].share).toBeCloseTo(50, 5);
    expect(flow.net).toBe(30);
  });
});
