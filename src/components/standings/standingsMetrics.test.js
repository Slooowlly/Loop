import {
  buildPositionDeltaMap,
  calculateBestFinish,
  calculatePointsThroughRound,
  pointsForResult,
} from "./standingsMetrics";

// As setas ▲▼ da tabela de classificação. A conta é toda feita no FRONTEND: o backend manda
// a posição de hoje, e a tela reconstrói como a tabela estava na rodada anterior para achar a
// diferença. Isso significa que o critério de desempate daqui precisa bater com o do Rust —
// se divergir, a seta aponta para o lado errado e ninguém percebe, porque não existe um
// número na tela para conferir contra.

const resultado = (position, is_dnf = false) => ({ position, is_dnf });

describe("pointsForResult", () => {
  it("paga a tabela padrão do 1º ao 10º", () => {
    const esperado = [25, 18, 15, 12, 10, 8, 6, 4, 2, 1];
    esperado.forEach((pontos, i) => expect(pointsForResult(resultado(i + 1))).toBe(pontos));
  });

  it("não paga do 11º em diante", () => {
    expect(pointsForResult(resultado(11))).toBe(0);
    expect(pointsForResult(resultado(30))).toBe(0);
  });

  it("abandono não pontua, mesmo classificado em zona de ponto", () => {
    // O DNF pode vir com posição preenchida (a ordem em que parou). Pagar por ela daria
    // ponto a quem não terminou.
    expect(pointsForResult(resultado(3, true))).toBe(0);
  });

  it("tolera rodada não corrida", () => {
    expect(pointsForResult(null)).toBe(0);
    expect(pointsForResult(undefined)).toBe(0);
  });
});

describe("calculatePointsThroughRound", () => {
  const results = [resultado(1), resultado(5), resultado(3, true), resultado(2)];

  it("soma só até a rodada pedida", () => {
    expect(calculatePointsThroughRound(results, 1)).toBe(25);
    expect(calculatePointsThroughRound(results, 2)).toBe(35);
    expect(calculatePointsThroughRound(results, 3)).toBe(35); // a 3ª foi abandono
    expect(calculatePointsThroughRound(results, 4)).toBe(53);
  });

  it("devolve zero para contagem não positiva ou lista ausente", () => {
    expect(calculatePointsThroughRound(results, 0)).toBe(0);
    expect(calculatePointsThroughRound(results, -1)).toBe(0);
    expect(calculatePointsThroughRound(null, 3)).toBe(0);
  });

  it("não estoura quando a contagem passa do número de rodadas corridas", () => {
    expect(calculatePointsThroughRound(results, 99)).toBe(53);
  });
});

describe("calculateBestFinish", () => {
  it("acha a melhor chegada dentro da janela", () => {
    const results = [resultado(7), resultado(2), resultado(9)];
    expect(calculateBestFinish(results, 3)).toBe(2);
    expect(calculateBestFinish(results, 1)).toBe(7);
  });

  it("ignora abandono", () => {
    expect(calculateBestFinish([resultado(1, true), resultado(6)], 2)).toBe(6);
  });

  it("devolve o sentinela quando não há chegada válida", () => {
    // O sentinela é MAX_SAFE_INTEGER porque o valor entra num desempate por MENOR posição:
    // devolver 0 ou null colocaria quem nunca terminou na frente de todo mundo.
    expect(calculateBestFinish([], 3)).toBe(Number.MAX_SAFE_INTEGER);
    expect(calculateBestFinish([resultado(2, true)], 1)).toBe(Number.MAX_SAFE_INTEGER);
    expect(calculateBestFinish(null, 3)).toBe(Number.MAX_SAFE_INTEGER);
  });
});

describe("buildPositionDeltaMap", () => {
  const piloto = (id, nome, posicao, results) => ({
    id,
    nome,
    posicao_campeonato: posicao,
    results,
  });

  it("é vazio antes da segunda rodada", () => {
    // Com uma rodada só não existe "antes" para comparar.
    expect(buildPositionDeltaMap([piloto("a", "A", 1, [resultado(1)])], 1).size).toBe(0);
    expect(buildPositionDeltaMap([], 5).size).toBe(0);
    expect(buildPositionDeltaMap(null, 5).size).toBe(0);
  });

  it("mede o ganho e a perda de posição entre a rodada anterior e a atual", () => {
    // Depois da 1ª: A tem 25, B tem 18 → A é 1º, B é 2º.
    // Na 2ª, B vence e A abandona → hoje B é 1º e A é 2º.
    const drivers = [
      piloto("a", "A", 2, [resultado(1), resultado(1, true)]),
      piloto("b", "B", 1, [resultado(2), resultado(1)]),
    ];
    const delta = buildPositionDeltaMap(drivers, 2);
    expect(delta.get("b")).toBe(1); // subiu do 2º para o 1º
    expect(delta.get("a")).toBe(-1); // caiu do 1º para o 2º
  });

  it("devolve zero para quem não mudou de posição", () => {
    const drivers = [
      piloto("a", "A", 1, [resultado(1), resultado(1)]),
      piloto("b", "B", 2, [resultado(2), resultado(2)]),
    ];
    const delta = buildPositionDeltaMap(drivers, 2);
    expect(delta.get("a")).toBe(0);
    expect(delta.get("b")).toBe(0);
  });

  it("desempata pontos iguais pela melhor chegada, e não pela posição de hoje", () => {
    // Janela anterior = 2 rodadas. Os dois somam 22 pontos:
    //   A: 2º (18) + 8º (4), melhor chegada 2º
    //   B: 4º (12) + 5º (10), melhor chegada 4º
    // Pela melhor chegada, A estava na frente. Na 3ª rodada B passou A e hoje é 1º.
    //
    // Este caso distingue o critério: se o desempate caísse direto na posição ATUAL, B já
    // apareceria como 1º na tabela anterior e as duas setas ficariam zeradas — a tela
    // esconderia justamente a ultrapassagem que acabou de acontecer.
    const drivers = [
      piloto("a", "A", 2, [resultado(2), resultado(8), resultado(6)]),
      piloto("b", "B", 1, [resultado(4), resultado(5), resultado(1)]),
    ];
    const delta = buildPositionDeltaMap(drivers, 3);
    expect(delta.get("b")).toBe(1);
    expect(delta.get("a")).toBe(-1);
  });

  it("cobre todos os pilotos, inclusive quem não tem resultado nenhum", () => {
    // Piloto que entrou no meio da temporada não pode ficar de fora do mapa: a tabela lê
    // `delta.get(id)` para decidir se desenha a seta, e um `undefined` viraria NaN na tela.
    const drivers = [
      piloto("a", "A", 1, [resultado(1), resultado(1)]),
      piloto("novato", "Novato", 2, []),
    ];
    const delta = buildPositionDeltaMap(drivers, 2);
    expect(delta.size).toBe(2);
    expect(Number.isFinite(delta.get("novato"))).toBe(true);
  });

  it("trata posição atual ausente sem quebrar a ordenação", () => {
    const drivers = [
      piloto("a", "A", undefined, [resultado(1), resultado(1)]),
      piloto("b", "B", 1, [resultado(2), resultado(2)]),
    ];
    const delta = buildPositionDeltaMap(drivers, 2);
    expect(delta.size).toBe(2);
    for (const v of delta.values()) expect(Number.isFinite(v)).toBe(true);
  });
});
