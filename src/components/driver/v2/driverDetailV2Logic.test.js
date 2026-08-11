import {
  DUEL_LOSS_COLOR,
  DUEL_WIN_COLOR,
  MEDAL_COLORS,
  TRAIT_LEVEL_ORDER,
  agrupaPorTemporada,
  colunasDoConfronto,
  corDoSaldo,
  escalaLog,
  finishColor,
  formataSaldo,
  groupTitlesByTeam,
  listaDeAnos,
  naRegua,
  ordenarPorNivel,
  primeiroNome,
  sequenciaAtual,
  tendenciaDeValor,
} from "./driverDetailV2Logic";

// Teste espelho da lógica pura extraída de DriverDetailModalV2.jsx (5.505 linhas, ~91 funções
// internas, apontado como [Alta] na vistoria de 10/08/2026).
//
// Antes disto tudo aqui era exercitado só através do render do modal inteiro — o teste de
// 4.482 linhas que a própria vistoria descreve como "protege o comportamento e ao mesmo tempo
// trava qualquer refatoração barata". Estes casos rodam em milissegundos e falham apontando a
// função, e não a seção da tela.

describe("finishColor", () => {
  it("dá cor própria ao abandono, independente da posição", () => {
    // Terminar em 20º e NÃO terminar são coisas diferentes. A fita existe para deixar isso
    // visível de relance, e o abandono em posição de pódio é o caso que prova a regra.
    expect(finishColor(true, 1)).toBe(MEDAL_COLORS.dnf);
    expect(finishColor(true, 20)).toBe(MEDAL_COLORS.dnf);
  });

  it("pinta o pódio e apaga o resto", () => {
    expect(finishColor(false, 1)).toBe(MEDAL_COLORS.first);
    expect(finishColor(false, 2)).toBe(MEDAL_COLORS.second);
    expect(finishColor(false, 3)).toBe(MEDAL_COLORS.third);
    expect(finishColor(false, 4)).toBe(MEDAL_COLORS.nearMiss);
    expect(finishColor(false, 25)).toBe(MEDAL_COLORS.nearMiss);
  });

  it("sempre devolve uma cor, mesmo sem posição", () => {
    // Uma cor `undefined` deixa a marca invisível na fita, sem erro nenhum.
    for (const p of [null, undefined, 0, NaN]) {
      expect(finishColor(false, p)).toBeTruthy();
    }
  });
});

describe("naRegua", () => {
  it("passa o valor dentro da faixa", () => {
    expect(naRegua(0)).toBe(0);
    expect(naRegua(63)).toBe(63);
    expect(naRegua(100)).toBe(100);
  });

  it("grampeia acima de 100 e abaixo de 0", () => {
    // O backend soma bônus e pode entregar 118. Uma barra de 118% vaza do painel.
    expect(naRegua(130)).toBe(100);
    expect(naRegua(-5)).toBe(0);
  });

  it("aceita número em texto", () => {
    expect(naRegua("72")).toBe(72);
  });

  it("devolve null, e não zero, para o que não é número", () => {
    // Barra vazia e barra ausente contam coisas diferentes: uma diz "atributo zerado", a
    // outra diz "o backend não mandou este eixo".
    for (const v of [undefined, "n/a", NaN, {}]) {
      expect(naRegua(v)).toBeNull();
    }
  });

  it("ARMADILHA: null e string vazia viram ZERO, não null", () => {
    // `Number(null)` é 0 e `Number("")` é 0 — só `undefined` e texto não numérico dão NaN.
    // Na prática: um eixo que o backend manda como `null` desenha barra ZERADA em vez de
    // sumir, e a tela afirma "atributo 0" onde o dado não existe.
    //
    // Documentado como comportamento, e não corrigido aqui: mudar isso muda o desenho da
    // leitura técnica, que tem guard visual próprio. Fica registrado para quem for decidir.
    expect(naRegua(null)).toBe(0);
    expect(naRegua("")).toBe(0);
  });
});

describe("ordenarPorNivel", () => {
  const tag = (level, nome) => ({ level, nome });

  it("ordena do mais forte para o mais fraco", () => {
    const ordenado = ordenarPorNivel(
      [tag("defeito", "d"), tag("elite", "e"), tag("qualidade", "q")],
      "strength",
    );
    expect(ordenado.map((x) => x.tag.nome)).toEqual(["e", "q", "d"]);
  });

  it("nível desconhecido vai para o FIM do grupo, nunca para a frente", () => {
    // Um payload antigo sem `level` daria `indexOf === -1`, que ordenaria antes de tudo se o
    // valor não fosse trocado pelo comprimento da lista.
    const ordenado = ordenarPorNivel(
      [tag("nivel_que_nao_existe", "novo"), tag("defeito_grave", "grave")],
      "weakness",
    );
    expect(ordenado.map((x) => x.tag.nome)).toEqual(["grave", "novo"]);
  });

  it("empate de nível preserva a ordem de chegada", () => {
    // Sem o desempate por índice a ordem varia entre renders e a fita "pisca" de posição.
    const ordenado = ordenarPorNivel([tag("qualidade", "a"), tag("qualidade", "b")], "strength");
    expect(ordenado.map((x) => x.tag.nome)).toEqual(["a", "b"]);
  });

  it("carrega o tom recebido em cada item", () => {
    const ordenado = ordenarPorNivel([tag("elite", "x")], "strength");
    expect(ordenado[0].tone).toBe("strength");
  });

  it("tolera lista ausente", () => {
    expect(ordenarPorNivel(null, "strength")).toEqual([]);
  });

  it("a escada de níveis vai de elite a defeito grave", () => {
    expect(TRAIT_LEVEL_ORDER[0]).toBe("elite");
    expect(TRAIT_LEVEL_ORDER[TRAIT_LEVEL_ORDER.length - 1]).toBe("defeito_grave");
  });
});

describe("primeiroNome", () => {
  it("pega a primeira palavra", () => {
    expect(primeiroNome("Ayrton Senna da Silva")).toBe("Ayrton");
  });

  it("ignora espaço sobrando", () => {
    expect(primeiroNome("   Nelson   Piquet  ")).toBe("Nelson");
  });

  it("devolve string vazia em vez de undefined", () => {
    // O retorno entra direto numa frase; `undefined` apareceria escrito na tela.
    for (const n of [null, undefined, "", "   "]) {
      expect(primeiroNome(n)).toBe("");
    }
  });
});

describe("groupTitlesByTeam", () => {
  const titulo = (ano, equipe) => ({ ano, equipe });

  it("agrupa por equipe, com os anos em ordem decrescente", () => {
    const grupos = groupTitlesByTeam([
      titulo(2019, "Arclight"),
      titulo(2022, "Waypoint"),
      titulo(2021, "Arclight"),
    ]);
    expect(grupos.map((g) => g.equipe)).toEqual(["Waypoint", "Arclight"]);
    expect(grupos.find((g) => g.equipe === "Arclight").anos).toEqual([2021, 2019]);
  });

  it("ordena os grupos pelo título MAIS RECENTE de cada um", () => {
    // A leitura da linha do tempo pede que a equipe do último título abra a lista, e não a
    // equipe com mais títulos.
    const grupos = groupTitlesByTeam([
      titulo(2010, "Velha"),
      titulo(2011, "Velha"),
      titulo(2012, "Velha"),
      titulo(2024, "Nova"),
    ]);
    expect(grupos[0].equipe).toBe("Nova");
  });

  it("comprime anos consecutivos em bloco", () => {
    const grupos = groupTitlesByTeam([
      titulo(2019, "T"),
      titulo(2020, "T"),
      titulo(2021, "T"),
      titulo(2024, "T"),
    ]);
    expect(grupos[0].blocos.length).toBeLessThan(4);
  });

  it("título sem equipe vira um grupo com chave própria", () => {
    // Título de save antigo pode não ter equipe. Perder a linha seria pior: o total de
    // títulos do cabeçalho deixaria de bater com a soma dos grupos.
    const grupos = groupTitlesByTeam([titulo(2020, null)]);
    expect(grupos).toHaveLength(1);
    expect(grupos[0].key).toBe("sem-equipe");
    expect(grupos[0].equipe).toBeNull();
  });

  it("descarta ano não numérico sem perder o grupo", () => {
    const grupos = groupTitlesByTeam([titulo(null, "T"), titulo(2020, "T")]);
    expect(grupos[0].anos).toEqual([2020]);
  });

  it("tolera lista vazia ou ausente", () => {
    expect(groupTitlesByTeam([])).toEqual([]);
    expect(groupTitlesByTeam(null)).toEqual([]);
  });
});

describe("listaDeAnos", () => {
  it("comprime consecutivos e separa os saltos", () => {
    expect(listaDeAnos([2018, 2020, 2021, 2022, 2025])).toBe("2018, 2020–2022, 2025");
  });

  it("ordena e remove repetidos", () => {
    expect(listaDeAnos([2021, 2019, 2021, 2020])).toBe("2019–2021");
  });

  it("um ano só sai sem intervalo", () => {
    expect(listaDeAnos([2023])).toBe("2023");
  });

  it("lista vazia vira string vazia", () => {
    expect(listaDeAnos([])).toBe("");
    expect(listaDeAnos(null)).toBe("");
  });
});

describe("sequenciaAtual", () => {
  const enc = (vencedor) => ({ vencedor });

  it("conta a sequência corrente de trás para frente", () => {
    const s = sequenciaAtual([enc("rival"), enc("piloto"), enc("piloto"), enc("piloto")]);
    expect(s).toEqual({ vencedor: "piloto", total: 3 });
  });

  it("o abandono é transparente e NÃO quebra a série", () => {
    // Cinco vitórias com um motor quebrado no meio continuam sendo domínio. Chamar isso de
    // "duas e depois duas" seria deixar o azar reescrever a história.
    const s = sequenciaAtual([
      enc("piloto"),
      enc("piloto"),
      enc("nenhum"),
      enc("piloto"),
      enc("piloto"),
    ]);
    expect(s).toEqual({ vencedor: "piloto", total: 4 });
  });

  it("para na primeira derrota", () => {
    const s = sequenciaAtual([enc("piloto"), enc("piloto"), enc("rival"), enc("piloto")]);
    expect(s).toEqual({ vencedor: "piloto", total: 1 });
  });

  it("devolve null quando nada foi decidido", () => {
    expect(sequenciaAtual([enc("nenhum"), enc("nenhum")])).toBeNull();
    expect(sequenciaAtual([])).toBeNull();
    expect(sequenciaAtual(null)).toBeNull();
  });
});

describe("colunasDoConfronto", () => {
  it("preserva o índice original de cada encontro", () => {
    // É o índice que a legenda usa para saber qual marca está sob o ponteiro.
    const cols = colunasDoConfronto([{ vencedor: "piloto", gap: 1.2 }, { vencedor: "rival" }]);
    expect(cols.map((c) => c.indice)).toEqual([0, 1]);
  });

  it("normaliza gap ausente ou inválido para null", () => {
    const cols = colunasDoConfronto([
      { vencedor: "piloto", gap: undefined },
      { vencedor: "piloto", gap: NaN },
      { vencedor: "piloto", gap: 0 },
    ]);
    expect(cols.map((c) => c.gap)).toEqual([null, null, 0]);
  });

  it("tolera lista ausente", () => {
    expect(colunasDoConfronto(null)).toEqual([]);
  });
});

describe("agrupaPorTemporada", () => {
  const enc = (season_number, ano) => ({ season_number, ano, vencedor: "piloto" });

  it("quebra por temporada mantendo a ordem cronológica", () => {
    const grupos = agrupaPorTemporada([enc(1, 2024), enc(1, 2024), enc(2, 2025)]);
    expect(grupos.map((g) => g.season_number)).toEqual([1, 2]);
    expect(grupos[0].corridas).toHaveLength(2);
  });

  it("guarda o índice ORIGINAL, e não o índice dentro da temporada", () => {
    const grupos = agrupaPorTemporada([enc(1, 2024), enc(2, 2025), enc(2, 2025)]);
    expect(grupos[1].corridas.map((c) => c.indice)).toEqual([1, 2]);
  });

  it("uma temporada que reaparece depois de outra abre um grupo novo", () => {
    // A quebra é por VIZINHANÇA, não por chave. Um encontro fora de ordem cronológica não
    // pode ser costurado de volta a um grupo distante, senão o índice deixa de ser sequência.
    const grupos = agrupaPorTemporada([enc(1, 2024), enc(2, 2025), enc(1, 2024)]);
    expect(grupos).toHaveLength(3);
  });

  it("tolera lista ausente", () => {
    expect(agrupaPorTemporada(null)).toEqual([]);
  });
});

describe("saldo do confronto", () => {
  it("empate não recebe cor", () => {
    // Pintar o zero de verde ou de vermelho seria dar lado a um empate.
    expect(corDoSaldo(0)).toBeUndefined();
    expect(corDoSaldo(3)).toBe(DUEL_WIN_COLOR);
    expect(corDoSaldo(-3)).toBe(DUEL_LOSS_COLOR);
  });

  it("o positivo leva sinal e o negativo leva o menos tipográfico", () => {
    // Numa coluna monoespaçada o hífen fica alto e curto demais e se lê como travessão de
    // intervalo. O caractere é "−" (U+2212), não "-".
    expect(formataSaldo(8)).toBe("+8");
    expect(formataSaldo(-8)).toBe("−8");
    expect(formataSaldo(-8)).not.toBe("-8");
    expect(formataSaldo(0)).toBe("0");
  });
});

describe("tendenciaDeValor", () => {
  const ponto = (ano, valor_mercado, futuro = false) => ({ ano, valor_mercado, futuro });

  it("compara os dois últimos anos medidos", () => {
    const t = tendenciaDeValor([ponto(2023, 100_000), ponto(2024, 150_000)]);
    expect(t.variacao).toBeCloseTo(0.5, 5);
    expect(t.ano).toBe(2023);
    expect(t.base).toBe(100_000);
  });

  it("ignora o ponto futuro", () => {
    // Projeção comparada com medição faria a seta contar o que o modelo acha, e não o que
    // aconteceu.
    const t = tendenciaDeValor([
      ponto(2023, 100_000),
      ponto(2024, 150_000),
      ponto(2025, 900_000, true),
    ]);
    expect(t.base).toBe(100_000);
    expect(t.variacao).toBeCloseTo(0.5, 5);
  });

  it("cala abaixo de 1% de variação", () => {
    // Não foi o piloto que mudou, foi o arredondamento.
    expect(tendenciaDeValor([ponto(2023, 100_000), ponto(2024, 100_500)])).toBeNull();
  });

  it("exige dois pontos medidos e base positiva", () => {
    expect(tendenciaDeValor([ponto(2024, 150_000)])).toBeNull();
    expect(tendenciaDeValor([ponto(2023, 0), ponto(2024, 150_000)])).toBeNull();
    expect(tendenciaDeValor(null)).toBeNull();
  });

  it("descarta valor não numérico", () => {
    expect(tendenciaDeValor([ponto(2023, null), ponto(2024, 150_000)])).toBeNull();
  });
});

describe("escalaLog", () => {
  it("a fração cresce com o valor e cabe em 0..1", () => {
    const e = escalaLog([10_000, 1_000_000]);
    expect(e.fracao(10_000)).toBeLessThan(e.fracao(1_000_000));
    for (const v of [10_000, 100_000, 1_000_000]) {
      expect(e.fracao(v)).toBeGreaterThanOrEqual(0);
      expect(e.fracao(v)).toBeLessThanOrEqual(1);
    }
  });

  it("grampeia valor abaixo do piso em vez de devolver fração negativa", () => {
    // Fração negativa desenha a linha fora da moldura do gráfico.
    const e = escalaLog([100_000, 200_000]);
    expect(e.fracao(1)).toBeGreaterThanOrEqual(0);
  });

  it("usa décadas quando a amplitude é grande", () => {
    const e = escalaLog([1_000, 1_000_000]);
    expect(e.marcas.length).toBeGreaterThan(2);
    // Em escada de décadas toda marca é 1 ou 3 vezes uma potência de 10.
    for (const m of e.marcas) {
      const mantissa = m / 10 ** Math.floor(Math.log10(m));
      expect([1, 3]).toContain(Math.round(mantissa));
    }
  });

  it("cai para régua linear quando a amplitude é pequena", () => {
    // Com meia década de vão a curva log é indistinguível de uma reta, e a marcação em
    // potências de 10 daria uma marca só — um eixo sem referência.
    const e = escalaLog([100_000, 130_000]);
    expect(e.marcas.length).toBeGreaterThanOrEqual(2);
  });

  it("não repete rótulo na régua linear", () => {
    // Duas linhas com a mesma etiqueta são piores que nenhuma.
    const e = escalaLog([12_000, 13_500]);
    expect(new Set(e.marcas).size).toBe(e.marcas.length);
  });

  it("aguenta uma série de valor único", () => {
    const e = escalaLog([50_000]);
    expect(Number.isFinite(e.fracao(50_000))).toBe(true);
  });
});
