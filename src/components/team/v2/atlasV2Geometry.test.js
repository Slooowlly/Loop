import {
  BAND_GAP,
  DIVISION_HEADER_HEIGHT,
  LABEL_COLUMN_SHIFT,
  MAX_LABEL_COLUMNS,
  BAND_ACCENT_CHAMPIONSHIP,
  BAND_ACCENT_ENDURANCE,
  BAND_ACCENT_GT3,
  BAND_ACCENT_GT4,
  BAND_ACCENT_PRODUCTION,
  BAND_ACCENT_ROOKIE,
  atlasDivisions,
  bandAccent,
  buildAtlasGeometry,
  buildAtlasVerticalGeometry,
  buildAtlasTracks,
  buildAtlasYears,
  assignCorridorLanes,
  buildTrackSegments,
  crossingCorridorPath,
  crossingIsLong,
  buildRankingCards,
  ensureMinimumLuminance,
  familyPreSeriesBox,
  firstSeriesYear,
  ENTRY_MARKER_RADIUS,
  LABEL_HEIGHT,
  labelConnector,
  labelIsDisplaced,
  labelWidthFor,
  liveColumnBox,
  splitVerticesAtLiveYear,
  splitVerticesByBand,
  intersects,
  placeEntryLabels,
  rectFromRightCenter,
  trackVertices,
} from "./atlasV2Geometry";

const LABEL_HEIGHT = 20;
const GAP = 6;

// ---------------------------------------------------------------------------
// Movimentação entre temporadas
//
// A escada é fechada e ordenada: bands[0] = Production (mais alta), bands[1] =
// Championship, bands[2] = Rookie. Uma equipe promovida NÃO tem ponto do ano
// anterior na faixa nova — era daí que vinha o traço indevido.
// ---------------------------------------------------------------------------
function team(id, points, extra = {}) {
  return {
    team_id: id,
    nome: id,
    nome_curto: id,
    cor_primaria: "#58a6ff",
    cor_secundaria: "#0b2545",
    base_position: 1,
    titles: [],
    is_reigning_champion: false,
    points: points.map(([year, position]) => ({ year, slot: "regular", position, points: 0, wins: 0, titles: 0 })),
    ...extra,
  };
}

const ladder = {
  // Familia sem ano de abertura fixado: vale a regra geral (primeira - 3).
  selected_family: "toyota",
  min_year: 2000,
  max_year: 2025,
  current_year: 2025,
  window_start: 2000,
  window_end: 2025,
  window_size: 26,
  families: [],
  bands: [
    {
      key: "production_mazda",
      label: "Mazda Production",
      category: "production_challenger",
      class_name: "mazda",
      starts_year: 2020,
      is_special: false,
      rows: [
        // Campeão da própria divisão: os 8 troféus são daqui.
        team("KESTREL", [[2024, 2], [2025, 1]], {
          is_reigning_champion: true,
          titles: [{ band_key: "production_mazda", band_label: "Mazda Production", count: 8 }],
        }),
        team("BACKMESA", [[2024, 3], [2025, 4]]),
        // Promovido do Championship, e campeão de LÁ: aqui chega sem troféu.
        team("VELOCITY", [[2025, 3]], {
          titles: [{ band_key: "mazda_amador", band_label: "Mazda Championship", count: 3 }],
        }),
        // Promovido do Rookie, tetracampeão de LÁ: aqui também chega sem troféu.
        team("APEX", [[2025, 6]], {
          titles: [{ band_key: "mazda_rookie", band_label: "Mazda Rookie", count: 4 }],
        }),
        // Voltou depois de duas temporadas fora: não é subida, é reaparecimento.
        team("RETORNO", [[2022, 5], [2025, 5]]),
      ],
    },
    {
      key: "mazda_amador",
      label: "Mazda Championship",
      category: "mazda_amador",
      class_name: null,
      starts_year: 2018,
      is_special: false,
      rows: [
        team("VELOCITY", [[2024, 1]]),
        // Rebaixado da Production: a variação é de divisão, não de posição.
        team("QUEDA", [[2025, 2]]),
      ],
    },
    {
      key: "mazda_rookie",
      label: "Mazda Rookie",
      category: "mazda_rookie",
      class_name: null,
      starts_year: 2018,
      is_special: false,
      rows: [
        team("APEX", [[2024, 1]], {
          titles: [{ band_key: "mazda_rookie", band_label: "Mazda Rookie", count: 4 }],
        }),
      ],
    },
  ],
};

// QUEDA precisa existir na Production em 2024 para o rebaixamento fazer sentido.
ladder.bands[0].rows.push(team("QUEDA", [[2024, 5]]));

// ---------------------------------------------------------------------------
// Troféus da linha do ranking
//
// A conta é da CATEGORIA em que a equipe está agora, nunca da carreira dela na
// família. Subir de divisão zera o troféu ao lado do nome; ser rebaixado de volta
// devolve a conta inteira.
// ---------------------------------------------------------------------------
describe("titulos por categoria", () => {
  const cards = buildRankingCards(ladder);
  const production = cards.find((card) => card.key === "production_mazda");
  const rookie = cards.find((card) => card.key === "mazda_rookie");
  const titlesOf = (card, id) => card.rows.find((row) => row.team_id === id).titles;

  it("conta os titulos ganhos na propria divisao", () => {
    expect(titlesOf(production, "KESTREL")).toBe(8);
  });

  it("nao carrega titulo de outra divisao para quem foi promovido", () => {
    // VELOCITY e tricampeao da Championship, APEX e tetracampeao da Rookie: na
    // Production os dois comecam sem trofeu nenhum.
    expect(titlesOf(production, "VELOCITY")).toBe(0);
    expect(titlesOf(production, "APEX")).toBe(0);
  });

  it("devolve a conta quando a equipe aparece na divisao onde ganhou", () => {
    expect(titlesOf(rookie, "APEX")).toBe(4);
  });

  it("da zero para quem nunca ganhou nada", () => {
    expect(titlesOf(production, "BACKMESA")).toBe(0);
    expect(titlesOf(production, "RETORNO")).toBe(0);
  });

  it("nao emite indicador de variacao em linha alguma", () => {
    cards.flatMap((card) => card.rows).forEach((row) => {
      expect(row.indicator).toBeUndefined();
      expect(Number.isFinite(row.titles)).toBe(true);
    });
  });
});

describe("buildAtlasGeometry", () => {
  it("gasta a altura livre no espacamento entre posicoes antes do respiro das faixas", () => {
    const years = buildAtlasYears(ladder);
    const roomy = buildAtlasGeometry(ladder, years, { width: 1200, height: 900 });
    const cramped = buildAtlasGeometry(ladder, years, { width: 1200, height: 420 });

    expect(roomy.rowHeight).toBeGreaterThan(cramped.rowHeight);
    // O vao entre campeonatos nao cresce com a altura: ele e so o lugar do
    // separador, e vale igual para o grafico e para os cards da coluna lateral.
    expect(roomy.bandGap).toBe(cramped.bandGap);
    expect(roomy.bandGap).toBe(BAND_GAP);
    // E o conteudo preenche a altura disponivel, sem sobra acumulada no rodape.
    expect(roomy.contentHeight).toBeGreaterThan(900 * 0.9);
  });
});

// ---------------------------------------------------------------------------
// Régua horizontal — a única fonte de X do gráfico
//
// `ladder` estreia em 2022 e termina em 2025, então o eixo exibido vai de 2019
// (2022 - PRE_SERIES_YEARS) até 2025: 7 colunas. Com 700px de largura, cada ano
// mede 100px e os limites caem em múltiplos redondos.
// ---------------------------------------------------------------------------
describe("regua da timeline", () => {
  const years = buildAtlasYears(ladder);
  const geometry = buildAtlasGeometry(ladder, years, { width: 700, height: 600 });

  it("mostra tres anos antes da primeira temporada e nenhum ano futuro", () => {
    expect(years[0]).toBe(2019);
    expect(years[years.length - 1]).toBe(2025);
    expect(years).toHaveLength(7);
  });

  it("trata ano como intervalo: N celulas, N+1 limites", () => {
    expect(geometry.yearWidth).toBe(100);
    expect(geometry.getBoundaryX(0)).toBe(0);
    expect(geometry.getBoundaryX(7)).toBe(700);
    expect(geometry.timelineRight).toBe(geometry.getBoundaryX(geometry.yearCount));
  });

  it("poe o rotulo do ano no centro da celula e o ponto na abertura dela", () => {
    const index2022 = years.indexOf(2022);
    expect(geometry.getBoundaryX(index2022)).toBe(300);
    expect(geometry.getCenterX(index2022)).toBe(350);
    // O ponto da temporada usa o limite, nunca o centro.
    expect(geometry.boundaryOfYear(2022)).toBe(300);
  });

  it("hachura exatamente as colunas anteriores a existencia da familia", () => {
    const box = familyPreSeriesBox(geometry, years, firstSeriesYear(ladder));

    expect(box).toEqual({ left: 0, width: 300 });
    // A divisa entre a hachura e o primeiro dado e o limite 2021|2022, que e onde
    // o primeiro ponto das equipes fundadoras tambem cai.
    expect(box.left + box.width).toBe(geometry.boundaryOfYear(2022));
  });

  it("fecha a linha no fim da ultima temporada da equipe", () => {
    const ateOFim = trackVertices(
      { points: [{ year: 2024, position: 2, band_key: "production_mazda" }, { year: 2025, position: 1, band_key: "production_mazda" }] },
      geometry,
      years,
    );

    expect(ateOFim.map((vertex) => vertex.x)).toEqual([500, 600, 700]);
    // Ultimo segmento alcanca a borda direita do grafico.
    expect(ateOFim[ateOFim.length - 1].x).toBe(geometry.timelineRight);
    expect(ateOFim[ateOFim.length - 1].y).toBe(ateOFim[ateOFim.length - 2].y);

    // Quem parou antes fecha na propria ultima temporada, e nao na borda — estender
    // ate 2025 afirmaria que a equipe correu ate hoje.
    const parouEm2023 = trackVertices(
      { points: [{ year: 2022, position: 3, band_key: "production_mazda" }, { year: 2023, position: 4, band_key: "production_mazda" }] },
      geometry,
      years,
    );

    expect(parouEm2023.map((vertex) => vertex.x)).toEqual([300, 400, 500]);
  });
});

describe("cards independem da janela visivel", () => {
  it("da o mesmo resultado com e sem zoom, porque nao recebe os anos exibidos", () => {
    const cheio = buildRankingCards(ladder);
    const comZoom = buildRankingCards(ladder);

    expect(comZoom).toEqual(cheio);
    // A temporada de referencia sai do payload, nao do eixo — e cada card mostra a
    // ULTIMA temporada da sua propria divisao: no fixture, Rookie parou em 2024.
    expect(cheio.map((card) => [card.key, card.referenceYear])).toEqual([
      ["production_mazda", 2025],
      ["mazda_amador", 2025],
      ["mazda_rookie", 2024],
    ]);
  });
});

describe("ano de abertura por familia", () => {
  it("respeita o ano fixado da Mazda em vez da regra geral", () => {
    // Regra geral daria 2022 - 3 = 2019; a Mazda abre em 2014 por configuracao.
    const mazda = { ...ladder, selected_family: "mazda" };
    const years = buildAtlasYears(mazda);

    expect(years[0]).toBe(2014);
    expect(years[years.length - 1]).toBe(2025);
  });

  it("nunca abre depois da primeira temporada, mesmo com override tardio", () => {
    // Um override posterior ao primeiro dado esconderia temporada real.
    const antiga = { ...ladder, selected_family: "mazda" };
    antiga.bands = [{ ...ladder.bands[0], rows: [team("VELHA", [[2008, 1]])] }];

    expect(buildAtlasYears(antiga)[0]).toBe(2008);
  });
});

describe("cor de exibicao", () => {
  it("clareia cores escuras demais para texto miudo sem mexer nas ja legiveis", () => {
    // Cinza-escuro tipico de equipe: some num nome de 11px sobre fundo escuro.
    const escura = ensureMinimumLuminance("#2b3a4a");
    expect(escura).not.toBe("#2b3a4a");
    expect(luminanceOf(escura)).toBeGreaterThanOrEqual(0.5);

    // Cor ja clara passa intacta — a identidade da equipe nao muda a toa.
    expect(ensureMinimumLuminance("#e5e7eb")).toBe("#e5e7eb");
  });

  it("aceita a forma rgb() que a leitura de cor do v1 devolve", () => {
    expect(luminanceOf(ensureMinimumLuminance("rgb(20, 24, 30)"))).toBeGreaterThanOrEqual(0.5);
  });
});

function luminanceOf(color) {
  const [r, g, b] = color.startsWith("#")
    ? [1, 3, 5].map((offset) => parseInt(color.slice(offset, offset + 2), 16))
    : color.match(/\d+/g).map(Number);
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
}

// ---------------------------------------------------------------------------
// Régua vertical — a mesma para o gráfico e para os cards laterais
// ---------------------------------------------------------------------------
describe("regua vertical compartilhada", () => {
  const divisions = [
    { id: "production", rowCount: 6 },
    { id: "championship", rowCount: 10 },
    { id: "rookie", rowCount: 6 },
  ];
  const vertical = buildAtlasVerticalGeometry({ totalHeight: 760, divisions });

  it("da a cada divisao um bloco contiguo separado pelo mesmo vao dos cards", () => {
    const { production, championship, rookie } = vertical.divisions;

    expect(production.top).toBe(0);
    expect(championship.top).toBe(production.bottom + vertical.gap);
    expect(rookie.top).toBe(championship.bottom + vertical.gap);
    expect(vertical.gap).toBe(BAND_GAP);
  });

  it("conta a zona de cabecalho UMA vez, dos dois lados", () => {
    const { championship } = vertical.divisions;

    // O primeiro lugar util da divisao comeca depois do cabecalho — a altura do
    // cabecalho do card e exatamente o respiro que o grafico deixa acima da P1.
    expect(championship.rowsTop).toBe(championship.top + DIVISION_HEADER_HEIGHT);
    expect(championship.headerBottom).toBe(championship.rowsTop);
  });

  it("posiciona o rank pelo mesmo rankY que o card usa para centrar a linha", () => {
    const { championship } = vertical.divisions;

    // Formula unica: rowsTop + (rank - 0.5) * rowHeight.
    expect(vertical.rankY("championship", 1)).toBe(championship.rowsTop + 0.5 * vertical.rowHeight);
    expect(vertical.rankY("championship", 4)).toBe(championship.rowsTop + 3.5 * vertical.rowHeight);

    // O centro relativo dentro do card bate com o Y absoluto do grafico.
    const centroNoCard = vertical.rankY("championship", 4) - championship.top;
    expect(championship.top + centroNoCard).toBe(vertical.rankY("championship", 4));
  });

  it("o grafico consome exatamente essas caixas, sem recalcular altura", () => {
    const years = buildAtlasYears(ladder);
    const geometry = buildAtlasGeometry(ladder, years, { width: 700, height: 0 }, vertical);

    expect(geometry.bands).toBe(vertical.divisions);
    expect(geometry.rowHeight).toBe(vertical.rowHeight);
    expect(geometry.bandGap).toBe(vertical.gap);
  });

  it("fecha a linha de 2025 no Y da classificacao daquela temporada", () => {
    const years = buildAtlasYears(ladder);
    const geometry = buildAtlasGeometry(ladder, years, { width: 700, height: 0 }, verticalDoLadder());
    const vertices = trackVertices(
      { points: [{ year: 2024, position: 2, band_key: "production_mazda" }, { year: 2025, position: 1, band_key: "production_mazda" }] },
      geometry,
      years,
    );
    const terminal = vertices[vertices.length - 1];

    // O terminal herda o Y da posicao de 2025 (P1) — nao o Y anterior, nem o
    // centro da faixa, nem uma aproximacao visual.
    expect(terminal.y).toBe(geometry.vertical.rankY("production_mazda", 1));
    expect(terminal.x).toBe(geometry.timelineRight);
  });
});

function verticalDoLadder() {
  return buildAtlasVerticalGeometry({ totalHeight: 760, divisions: atlasDivisions(ladder) });
}


// ---------------------------------------------------------------------------
// Colocação das etiquetas
//
// A unidade é o GRUPO (campeonato + ano de estreia): borda direita compartilhada,
// só o Y varia, e se não couber é o grupo inteiro que muda de coluna.
// ---------------------------------------------------------------------------
const banda = { top: 0, bottom: 400, height: 400, headerHeight: 44, rowHeight: 30 };
const geometriaDeUmaBanda = { bands: { banda } };

// Por padrao fundadora: e o grupo que fica permanentemente na tela e, por isso,
// o unico que disputa espaco com os demais.
function etiqueta(id, targetY, extra = {}) {
  return {
    key: id,
    team_id: id,
    nome: id,
    band_key: "banda",
    year: 2020,
    isFounder: true,
    pointX: 420,
    idealRight: 406,
    width: 150,
    height: LABEL_HEIGHT,
    targetY,
    ...extra,
  };
}

function rectOf(label) {
  return rectFromRightCenter({
    right: label.right,
    centerY: label.renderY,
    width: label.width,
    height: label.height,
  });
}

function paresSobrepostos(labels) {
  const uteis = labels.filter((label) => !label.unresolved);
  const pares = [];
  for (let i = 0; i < uteis.length; i += 1) {
    for (let j = i + 1; j < uteis.length; j += 1) {
      if (intersects(rectOf(uteis[i]), rectOf(uteis[j]))) pares.push([uteis[i].team_id, uteis[j].team_id]);
    }
  }
  return pares;
}

describe("placeEntryLabels", () => {
  it("mantem o grupo de estreia na MESMA borda direita", () => {
    // Era o caso da Backmesa: uma etiqueta do grupo fundador era empurrada sozinha
    // para a esquerda e saia da fila das outras cinco.
    const grupo = [0, 1, 2, 3, 4, 5].map((index) => etiqueta(`t${index}`, 60 + index * 31));

    const placed = placeEntryLabels(grupo, geometriaDeUmaBanda);

    const bordas = new Set(placed.map((label) => label.right));
    expect(bordas.size).toBe(1);
    expect(placed.every((label) => !label.unresolved)).toBe(true);
  });

  it("move o GRUPO INTEIRO de coluna, nunca uma etiqueta isolada", () => {
    // Um grupo ja ocupa a coluna ideal; o segundo grupo, no mesmo ponto, tem de
    // recuar por completo.
    const primeiro = [0, 1].map((index) => etiqueta(`a${index}`, 80 + index * 34));
    const segundo = [0, 1].map((index) => etiqueta(`b${index}`, 84 + index * 34, { year: 2021 }));

    const placed = placeEntryLabels([...primeiro, ...segundo], geometriaDeUmaBanda);
    const porGrupo = new Map();
    placed.forEach((label) => porGrupo.set(label.year, new Set([...(porGrupo.get(label.year) ?? []), label.right])));

    porGrupo.forEach((bordas) => expect(bordas.size).toBe(1));
    expect(paresSobrepostos(placed)).toEqual([]);
  });

  it("nunca aceita retangulos sobrepostos, nem no ultimo recurso", () => {
    // Muitas etiquetas largas disputando o mesmo ponto numa faixa curta: o que nao
    // couber fica `unresolved` em vez de ser desenhado por cima de outra.
    const apertada = { bands: { banda: { top: 0, bottom: 90, height: 90, headerHeight: 44, rowHeight: 30 } } };
    const grupos = Array.from({ length: 10 }, (_, index) =>
      etiqueta(`t${index}`, 45, { year: 2020 + index }),
    );

    const placed = placeEntryLabels(grupos, apertada);

    expect(paresSobrepostos(placed)).toEqual([]);
    expect(placed.some((label) => label.unresolved)).toBe(true);
  });

  it("mantem a etiqueta inteira dentro da faixa, contando a altura", () => {
    // O clamp antigo usava o Y do centro sem descontar metade da altura, e a ultima
    // etiqueta saia cortada pela base da faixa.
    const grupo = [0, 1, 2].map((index) => etiqueta(`t${index}`, 395 + index * 5));

    const placed = placeEntryLabels(grupo, geometriaDeUmaBanda);

    placed.filter((label) => !label.unresolved).forEach((label) => {
      const rect = rectOf(label);
      expect(rect.top).toBeGreaterThanOrEqual(banda.top);
      expect(rect.bottom).toBeLessThanOrEqual(banda.bottom);
    });
  });

  it("preserva a ordem vertical dos pontos de entrada", () => {
    const grupo = [
      etiqueta("primeiro", 100),
      etiqueta("segundo", 104),
      etiqueta("terceiro", 108),
    ];

    const placed = placeEntryLabels(grupo, geometriaDeUmaBanda);
    const porNome = Object.fromEntries(placed.map((label) => [label.team_id, label.renderY]));

    expect(porNome.primeiro).toBeLessThan(porNome.segundo);
    expect(porNome.segundo).toBeLessThan(porNome.terceiro);
  });

  it("nunca deixa a etiqueta vazar pela beirada esquerda do grafico", () => {
    // Categoria com janela larga: a faixa pre-serie tem menos pixels que o chip, e
    // a etiqueta crescia para a esquerda ate sair cortada pela borda do card.
    const grupo = [0, 1, 2].map((index) => etiqueta(`t${index}`, 60 + index * 34, { pointX: 90, idealRight: 76 }));

    const placed = placeEntryLabels(grupo, geometriaDeUmaBanda);

    placed.filter((label) => !label.unresolved).forEach((label) => {
      expect(label.right - label.width).toBeGreaterThanOrEqual(0);
    });
  });

  it("encolhe a FONTE em vez de estreitar o chip quando o espaco aperta", () => {
    // Contrato: o nome sai inteiro. A largura continua sendo a que o nome pede na
    // escala escolhida — nunca uma largura menor que obrigaria a cortar.
    const nome = "Mercedes-AMG Motorsport";
    const apertado = [etiqueta("t0", 60, { nome, pointX: 150, idealRight: 136 })];
    const folgado = [etiqueta("t1", 60, { nome, pointX: 420, idealRight: 406 })];

    const [comAperto] = placeEntryLabels(apertado, geometriaDeUmaBanda);
    const [semAperto] = placeEntryLabels(folgado, geometriaDeUmaBanda);

    expect(comAperto.fontScale).toBeLessThan(1);
    expect(semAperto.fontScale).toBe(1);
    expect(comAperto.width).toBe(labelWidthFor(nome, comAperto.fontScale));
  });

  it("nao desenha conector quando a etiqueta encosta na beirada e passa do ponto", () => {
    const grupo = [etiqueta("t0", 60, { pointX: 40, idealRight: 26 })];
    const [placed] = placeEntryLabels(grupo, geometriaDeUmaBanda);

    expect(placed.right - placed.width).toBeGreaterThanOrEqual(0);
    expect(labelConnector(placed)).toBeNull();
  });

  it("liga o conector da borda do chip ate a beirada do marcador de estreia", () => {
    const grupo = [etiqueta("a", 100), etiqueta("b", 104)];
    const placed = placeEntryLabels(grupo, geometriaDeUmaBanda);
    const deslocada = placed.find(labelIsDisplaced);

    const connector = labelConnector(deslocada);
    expect(connector.x1).toBe(deslocada.right);
    expect(connector.y1).toBe(deslocada.renderY);
    // Termina na beirada do anel, nao no centro do ponto.
    expect(connector.x2).toBe(deslocada.pointX - ENTRY_MARKER_RADIUS);
    expect(connector.y2).toBe(deslocada.targetY);

    // Quem ficou no lugar ideal nao ganha conector.
    const noLugar = placed.find((label) => !labelIsDisplaced(label));
    if (noLugar) expect(labelConnector(noLugar)).toBeNull();
  });
});

describe("fundadoras x entradas do meio do caminho", () => {
  it("deixa as entradas tardias no lugar ideal, desviando so das fundadoras", () => {
    // Duas entradas tardias no MESMO ponto: como nunca aparecem juntas (cada uma
    // surge no hover da propria linha), elas nao precisam se desviar entre si.
    const tardias = [
      etiqueta("a", 120, { year: 2022, isFounder: false }),
      etiqueta("b", 120, { year: 2023, isFounder: false }),
    ];

    const placed = placeEntryLabels(tardias, geometriaDeUmaBanda);

    placed.forEach((label) => {
      expect(label.unresolved).toBe(false);
      expect(label.renderY).toBe(120);
      expect(label.right).toBe(label.idealRight);
    });
  });

  it("faz a entrada tardia desviar de uma fundadora que ocupa o mesmo espaco", () => {
    const fundadora = etiqueta("fundadora", 120, { year: 2020, isFounder: true });
    const tardia = etiqueta("tardia", 122, { year: 2023, isFounder: false });

    const placed = placeEntryLabels([fundadora, tardia], geometriaDeUmaBanda);
    const colocada = Object.fromEntries(placed.map((label) => [label.team_id, label]));

    // A fundadora manda no lugar ideal; a tardia sai de cima dela.
    expect(colocada.fundadora.renderY).toBe(120);
    expect(intersects(rectOf(colocada.fundadora), rectOf(colocada.tardia))).toBe(false);
  });
});

describe("bandAccent", () => {
  it("cor por degrau da escada, nao por familia", () => {
    // Mazda e Toyota sao familias diferentes e o mesmo degrau tem a mesma cor.
    expect(bandAccent({ key: "production_mazda", category: "production_challenger" })).toBe(BAND_ACCENT_PRODUCTION);
    expect(bandAccent({ key: "production_toyota", category: "production_challenger" })).toBe(BAND_ACCENT_PRODUCTION);
    expect(bandAccent({ key: "mazda_rookie", category: "mazda_rookie" })).toBe(BAND_ACCENT_ROOKIE);
    expect(bandAccent({ key: "toyota_rookie", category: "toyota_rookie" })).toBe(BAND_ACCENT_ROOKIE);
    expect(bandAccent({ key: "mazda_amador", category: "mazda_amador" })).toBe(BAND_ACCENT_CHAMPIONSHIP);
    expect(bandAccent({ key: "bmw_m2", category: "bmw_m2" })).toBe(BAND_ACCENT_CHAMPIONSHIP);
  });

  it("verde e o formato de prova longa, nao a classe que corre nele", () => {
    // As faixas de endurance dividem o verde: o que elas tem em comum e correr
    // longo, e nao a classe.
    expect(bandAccent({ key: "endurance_gt4", category: "endurance" })).toBe(BAND_ACCENT_ENDURANCE);
    expect(bandAccent({ key: "endurance_gt3", category: "endurance" })).toBe(BAND_ACCENT_ENDURANCE);
  });

  it("a sprint da GT3 e a da GT4 nao dividem o vermelho do degrau", () => {
    // As duas sao degrau de championship das suas familias, mas com o vermelho de
    // todo mundo a GT3 saia visualmente identica a GT4 — so o rotulo diferia.
    expect(bandAccent({ key: "gt3", category: "gt3" })).toBe(BAND_ACCENT_GT3);
    expect(bandAccent({ key: "gt4", category: "gt4" })).toBe(BAND_ACCENT_GT4);
    expect(BAND_ACCENT_GT3).not.toBe(BAND_ACCENT_GT4);
    expect(BAND_ACCENT_GT3).not.toBe(BAND_ACCENT_CHAMPIONSHIP);
    expect(BAND_ACCENT_GT4).not.toBe(BAND_ACCENT_CHAMPIONSHIP);
  });

  it("da a LMP2 a cor de endurance, como as outras faixas de resistencia", () => {
    // Ela ja teve rosa proprio por ser a unica faixa da familia dela. Endurance e
    // uma cor so: separar a LMP2 dizia o oposto do que a paleta deve dizer.
    expect(bandAccent({ key: "endurance_lmp2", category: "endurance" })).toBe(BAND_ACCENT_ENDURANCE);
    expect(bandAccent(null)).toBe(BAND_ACCENT_CHAMPIONSHIP);
  });
});

// ---------------------------------------------------------------------------
// Temporada em andamento
//
// A coluna do ano corrente nao e comensuravel com as anteriores: pontuacao
// parcial, posicao provisoria e nenhum titulo decidido. A geometria precisa
// marcar essa diferenca — quem desenha nao pode ter de adivinhar.
// ---------------------------------------------------------------------------
function liveTeam(id, points, extra = {}) {
  return {
    team_id: id,
    nome: id,
    nome_curto: id,
    cor_primaria: "#58a6ff",
    cor_secundaria: "#0b2545",
    base_position: 1,
    titles: [],
    is_reigning_champion: false,
    points: points.map(([year, position, pontos = 0, vitorias = 0]) => ({
      year,
      slot: "regular",
      position,
      points: pontos,
      wins: vitorias,
      titles: 0,
    })),
    ...extra,
  };
}

const emAndamento = {
  selected_family: "mazda",
  min_year: 2000,
  max_year: 2026,
  current_year: 2026,
  in_progress: true,
  last_completed_year: 2025,
  window_start: 2000,
  window_end: 2026,
  window_size: 27,
  families: [],
  bands: [
    {
      key: "mazda_rookie",
      label: "Mazda Rookie",
      category: "mazda_rookie",
      class_name: null,
      starts_year: 2018,
      is_special: false,
      rows: [
        // Subiu de 3o para 1o e lidera com 102 pontos.
        liveTeam("SOBE", [[2025, 3], [2026, 1, 102, 3]]),
        // Caiu de 1o para 4o.
        liveTeam("CAI", [[2025, 1], [2026, 4, 50, 0]]),
        // Nao estava nesta divisao em 2025: estreia, sem variacao.
        liveTeam("NOVA", [[2026, 6, 11, 0]]),
        // Mesma posicao das duas temporadas.
        liveTeam("PARADA", [[2025, 5], [2026, 5, 19, 0]]),
      ],
    },
    {
      // Faixa que acabou em 2024: nao e a temporada em curso, entao nada nela e vivo.
      key: "mazda_amador",
      label: "Mazda Championship",
      category: "mazda_amador",
      class_name: null,
      starts_year: 2018,
      is_special: false,
      rows: [liveTeam("ANTIGA", [[2024, 1, 200, 9]])],
    },
  ],
};

describe("cards da temporada em andamento", () => {
  const cards = buildRankingCards(emAndamento);
  const rookie = cards.find((card) => card.key === "mazda_rookie");
  const amador = cards.find((card) => card.key === "mazda_amador");
  const rowOf = (card, id) => card.rows.find((row) => row.team_id === id);

  it("marca como viva so a faixa que esta disputando o ano corrente", () => {
    expect(rookie.isLive).toBe(true);
    expect(rookie.referenceYear).toBe(2026);
    expect(rookie.baselineYear).toBe(2025);
    // A faixa parada em 2024 continua sendo um campeonato decidido.
    expect(amador.isLive).toBe(false);
    expect(amador.baselineYear).toBeNull();
  });

  it("mede a variacao contra a ultima temporada DECIDIDA, com sinal de posicao", () => {
    // Subir de 3o para 1o e +2, ainda que a diferenca dos numeros seja -2.
    expect(rowOf(rookie, "SOBE").delta).toBe(2);
    expect(rowOf(rookie, "CAI").delta).toBe(-3);
    expect(rowOf(rookie, "PARADA").delta).toBe(0);
  });

  it("trata quem nao estava na divisao como estreia, nao como variacao", () => {
    const nova = rowOf(rookie, "NOVA");
    expect(nova.isNewInBand).toBe(true);
    expect(nova.delta).toBeNull();
  });

  it("expoe o placar parcial so na tabela viva", () => {
    expect(rowOf(rookie, "SOBE").points).toBe(102);
    expect(rowOf(rookie, "SOBE").wins).toBe(3);
    // Num ano fechado quem conta a historia e a posicao final, nao o placar.
    expect(rowOf(amador, "ANTIGA").points).toBeNull();
    expect(rowOf(amador, "ANTIGA").wins).toBeNull();
    expect(rowOf(amador, "ANTIGA").delta).toBeNull();
    expect(rowOf(amador, "ANTIGA").isNewInBand).toBe(false);
  });

  it("sem temporada em curso nada e vivo", () => {
    const decidido = buildRankingCards({ ...emAndamento, in_progress: false });
    decidido.forEach((card) => {
      expect(card.isLive).toBe(false);
      card.rows.forEach((row) => {
        expect(row.points).toBeNull();
        expect(row.delta).toBeNull();
      });
    });
  });
});

describe("recorte da linha na temporada em andamento", () => {
  const vertices = [
    { x: 0, y: 10, point: { year: 2024 } },
    { x: 10, y: 20, point: { year: 2025 } },
    { x: 20, y: 5, point: { year: 2026 } },
    { x: 30, y: 5, point: { year: 2026 }, isClosing: true },
  ];

  it("compartilha o ultimo vertice decidido entre as duas metades", () => {
    const { settled, live } = splitVerticesAtLiveYear(vertices, 2026);
    // A diagonal rumo a coluna viva sai inteira no trecho tracejado — e ela que
    // representa a mudanca de posicao que ainda nao aconteceu de verdade.
    expect(settled.map((vertex) => vertex.point.year)).toEqual([2024, 2025]);
    expect(live.map((vertex) => vertex.point.year)).toEqual([2025, 2026, 2026]);
  });

  it("sem ano vivo devolve a linha inteira como decidida", () => {
    expect(splitVerticesAtLiveYear(vertices, null).live).toEqual([]);
    expect(splitVerticesAtLiveYear(vertices, 2030).settled).toHaveLength(4);
  });

  it("equipe que so existe na temporada em curso nao tem trecho decidido", () => {
    const estreante = [
      { x: 20, y: 5, point: { year: 2026 } },
      { x: 30, y: 5, point: { year: 2026 }, isClosing: true },
    ];
    const { settled, live } = splitVerticesAtLiveYear(estreante, 2026);
    expect(settled).toEqual([]);
    expect(live).toHaveLength(2);
  });
});

describe("coluna da temporada em andamento", () => {
  const geometry = { getBoundaryX: (index) => index * 40, yearWidth: 40 };

  it("cai exatamente sobre a coluna do ano corrente", () => {
    const box = liveColumnBox(emAndamento, geometry, [2024, 2025, 2026]);
    expect(box).toEqual({ left: 80, width: 40, year: 2026 });
  });

  it("nao existe sem temporada em curso nem fora do eixo visivel", () => {
    expect(liveColumnBox({ ...emAndamento, in_progress: false }, geometry, [2024, 2025, 2026])).toBeNull();
    expect(liveColumnBox(emAndamento, geometry, [2023, 2024, 2025])).toBeNull();
  });
});

describe("eixo com temporada em andamento", () => {
  it("nao gasta coluna com o ano em curso, mas mantem o ponto dele na linha", () => {
    const years = buildAtlasYears(emAndamento);

    // A ultima coluna e a ultima temporada DECIDIDA; 2026 chega na borda direita.
    expect(years[years.length - 1]).toBe(2025);
    expect(years).not.toContain(2026);

    const tracks = buildAtlasTracks(emAndamento, years);
    const sobe = tracks.find((track) => track.team_id === "SOBE");
    expect(sobe.points.map((point) => point.year)).toEqual([2025, 2026]);
  });

  it("mantem a coluna quando a temporada em curso e a unica que existe", () => {
    // Carreira sem nada arquivado: sem a coluna do ano corrente nao sobraria eixo.
    const estreia = {
      ...emAndamento,
      last_completed_year: 2025,
      bands: [
        {
          ...emAndamento.bands[0],
          rows: [liveTeam("UNICA", [[2026, 1, 10, 0]])],
        },
      ],
    };

    expect(buildAtlasYears(estreia)).toContain(2026);
  });

  it("sem temporada em curso o eixo termina na ultima temporada com dado", () => {
    const decidido = { ...emAndamento, in_progress: false };
    const years = buildAtlasYears(decidido);
    expect(years[years.length - 1]).toBe(2026);
  });
});

// ---------------------------------------------------------------------------
// Promocao e rebaixamento
//
// A linha de uma equipe que muda de divisao era um caminho unico, e o salto entre
// as faixas virava uma diagonal atravessando a altura inteira do grafico, cortando
// os campeonatos do meio. Cada divisao passa a ter o seu proprio tracado; a
// travessia e outra coisa, com regra propria.
// ---------------------------------------------------------------------------
describe("travessia entre divisoes", () => {
  const vertices = [
    { x: 0, y: 300, point: { year: 2023, band_key: "mazda_rookie" } },
    { x: 100, y: 280, point: { year: 2024, band_key: "mazda_rookie" } },
    { x: 200, y: 40, point: { year: 2025, band_key: "production_mazda" } },
    { x: 300, y: 40, point: { year: 2025, band_key: "production_mazda" }, isClosing: true },
  ];

  it("quebra o tracado por divisao em vez de ligar as duas faixas", () => {
    const runs = splitVerticesByBand(vertices);
    expect(runs.map((run) => run.bandKey)).toEqual(["mazda_rookie", "production_mazda"]);
    expect(runs[0].vertices).toHaveLength(2);
    expect(runs[1].vertices).toHaveLength(2);
  });

  it("liga as duas divisoes por um Z, com a vertical dentro da celula", () => {
    const { segments, crossings } = buildTrackSegments(vertices, null);

    expect(segments).toHaveLength(2);
    expect(crossings).toHaveLength(1);
    const [crossing] = crossings;
    // Subiu: a divisao nova esta mais alta na tela, ou seja, y menor.
    expect(crossing.isPromotion).toBe(true);
    expect(crossing.boundaryYear).toBe(2025);

    // Uma travessia sozinha no ano fica no meio da celula.
    const path = crossingCorridorPath(crossing.from, crossing.to, 0, 1);
    const [, corridor] = path.match(/L ([\d.]+) 280/) ?? [];
    const corridorX = parseFloat(corridor);
    expect(corridorX).toBeGreaterThan(100);
    expect(corridorX).toBeLessThan(200);
    // Quatro vertices: sai reto na altura antiga, sobe na vertical, entra reto na
    // altura nova. Nenhum trecho em diagonal.
    expect(path.split("L")).toHaveLength(4);
    expect(path.startsWith("M 100 280")).toBe(true);
    expect(path.endsWith("L 200 40")).toBe(true);
  });

  it("distribui as verticais do mesmo ano em x diferentes", () => {
    // Cinco equipes trocando de divisao no mesmo ano desenhariam cinco verticais
    // sobrepostas no mesmo pixel — um traco grosso e nenhuma informacao.
    const from = { x: 0, y: 100, point: { year: 2024, band_key: "a" } };
    const to = { x: 100, y: 300, point: { year: 2025, band_key: "b" } };
    const xs = [0, 1, 2].map((lane) => {
      const path = crossingCorridorPath(from, to, lane, 3);
      return parseFloat(path.match(/L ([\d.]+) 100/)[1]);
    });

    expect(new Set(xs).size).toBe(3);
    expect(xs[0]).toBeLessThan(xs[1]);
    expect(xs[1]).toBeLessThan(xs[2]);
    // Nenhuma encosta nas bordas da celula: colada a esquerda sairia do ponto da
    // temporada, colada a direita grudaria na linha da grade.
    expect(Math.min(...xs)).toBeGreaterThan(0);
    expect(Math.max(...xs)).toBeLessThan(100);
  });

  it("a faixa de uma travessia nao depende da ordem em que as linhas chegaram", () => {
    const build = (teamIds) =>
      assignCorridorLanes(
        teamIds.map((teamId) => ({
          track: { team_id: teamId },
          crossings: [
            {
              // Subida: e a que usa o corredor, e portanto a que disputa faixa.
              from: { x: 0, y: 300, point: { year: 2024, band_key: "b" } },
              to: { x: 100, y: 100, point: { year: 2025, band_key: "a" } },
              isPromotion: true,
              boundaryYear: 2025,
            },
          ],
        })),
      );

    const direta = build(["ALFA", "BRAVO"]);
    const invertida = build(["BRAVO", "ALFA"]);
    const pathOf = (lines, teamId) =>
      lines.find((line) => line.track.team_id === teamId).crossings[0].path;

    expect(pathOf(direta, "ALFA")).toBe(pathOf(invertida, "ALFA"));
    expect(pathOf(direta, "BRAVO")).toBe(pathOf(invertida, "BRAVO"));
    expect(pathOf(direta, "ALFA")).not.toBe(pathOf(direta, "BRAVO"));
  });

  it("marca como viva a travessia que desemboca na temporada em curso", () => {
    const promovidaAgora = [
      { x: 0, y: 300, point: { year: 2025, band_key: "mazda_rookie" } },
      { x: 100, y: 40, point: { year: 2026, band_key: "production_mazda" } },
    ];
    const [crossing] = buildTrackSegments(promovidaAgora, 2026).crossings;
    expect(crossing.isLive).toBe(true);
  });

  it("um vertice sozinho nao vira tracado", () => {
    const soUmAno = [{ x: 0, y: 10, point: { year: 2025, band_key: "a" } }];
    expect(buildTrackSegments(soUmAno, null).segments[0].path).toBe("");
  });
});

describe("travessia longa", () => {
  const PLOT_HEIGHT = 600;
  const crossing = (fromY, toY) => ({
    from: { x: 0, y: fromY, point: { band_key: "a" } },
    to: { x: 100, y: toY, point: { band_key: "b" } },
  });

  it("mede distancia percorrida, nao faixas puladas", () => {
    // Numa escada fechada quase toda troca e de um degrau so — contar faixas
    // puladas nunca dispararia. O que incomoda e a altura percorrida: sair do 3o de
    // uma divisao e cair no 10o da seguinte atravessa quase a tela inteira, ainda
    // que as duas sejam vizinhas.
    expect(crossingIsLong(crossing(40, 500), PLOT_HEIGHT)).toBe(true);
    expect(crossingIsLong(crossing(500, 40), PLOT_HEIGHT)).toBe(true);
  });

  it("travessia curta segue inteira", () => {
    // Nao ha miolo a esconder: apagar so tiraria dado da tela.
    expect(crossingIsLong(crossing(300, 340), PLOT_HEIGHT)).toBe(false);
  });

  it("o limiar acompanha a altura do grafico", () => {
    // A mesma travessia e longa numa janela baixa e curta numa alta — o que pesa e
    // a fracao da tela que ela ocupa, nao um numero de pixels fixo.
    const meia = crossing(0, 200);
    expect(crossingIsLong(meia, 400)).toBe(true);
    expect(crossingIsLong(meia, 1000)).toBe(false);
  });

  it("sem altura de grafico nao ha travessia longa", () => {
    expect(crossingIsLong(crossing(40, 500), 0)).toBe(false);
    expect(crossingIsLong(crossing(40, 500), undefined)).toBe(false);
    expect(crossingIsLong(null, PLOT_HEIGHT)).toBe(false);
  });
});

describe("rebaixamento mantem a diagonal", () => {
  const linhas = (crossing) => [{ track: { team_id: "T1" }, crossings: [crossing] }];

  it("a descida vai reto de uma divisao a outra, sem corredor", () => {
    // Sem nada ligando as duas pontas, a equipe brota do nada na divisao de baixo.
    // A diagonal e a continuacao natural: o tempo anda para a direita e a queda
    // anda para baixo.
    const [line] = assignCorridorLanes(
      linhas({
        from: { x: 0, y: 40, point: { year: 2024, band_key: "a" } },
        to: { x: 100, y: 400, point: { year: 2025, band_key: "b" } },
        isPromotion: false,
        boundaryYear: 2025,
      }),
    );

    expect(line.crossings[0].path).toBe("M 0 40 L 100 400");
  });

  it("a subida continua pelo corredor, em Z", () => {
    const [line] = assignCorridorLanes(
      linhas({
        from: { x: 0, y: 400, point: { year: 2024, band_key: "b" } },
        to: { x: 100, y: 40, point: { year: 2025, band_key: "a" } },
        isPromotion: true,
        boundaryYear: 2025,
      }),
    );

    expect(line.crossings[0].path.split("L")).toHaveLength(4);
  });

  it("as diagonais nao gastam faixa do corredor", () => {
    // Contar as descidas na distribuicao deixaria buracos, e as verticais que
    // sobraram ficariam amontoadas num canto da celula.
    const lines = assignCorridorLanes([
      {
        track: { team_id: "DESCE" },
        crossings: [
          {
            from: { x: 0, y: 40, point: { year: 2024, band_key: "a" } },
            to: { x: 100, y: 400, point: { year: 2025, band_key: "b" } },
            isPromotion: false,
            boundaryYear: 2025,
          },
        ],
      },
      {
        track: { team_id: "SOBE" },
        crossings: [
          {
            from: { x: 0, y: 400, point: { year: 2024, band_key: "b" } },
            to: { x: 100, y: 40, point: { year: 2025, band_key: "a" } },
            isPromotion: true,
            boundaryYear: 2025,
          },
        ],
      },
    ]);

    // Sobrou uma unica travessia de corredor: ela fica no meio da celula, como se
    // estivesse sozinha no ano — porque, para efeito de faixa, esta.
    const sozinha = lines.find((line) => line.track.team_id === "SOBE").crossings[0].path;
    const referencia = crossingCorridorPath(
      { x: 0, y: 400 },
      { x: 100, y: 40 },
      0,
      1,
    );
    expect(sozinha).toBe(referencia);
  });
});
