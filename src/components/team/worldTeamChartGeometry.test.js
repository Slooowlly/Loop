import { describe, expect, it } from "vitest";

import {
  BAND_LABEL_HEIGHT,
  CHART_HEADER_HEIGHT,
  CHART_WIDTH,
  DEFAULT_FAMILY,
  DEFAULT_WINDOW_SIZE,
  MAX_VISIBLE_SPAN,
  MIN_BAND_HEIGHT,
  MIN_CHART_HEIGHT,
  ROW_HEIGHT,
  ROW_TOP_OFFSET,
  axisEdgeZoneStyle,
  axisEndYear,
  bandPreStartStyle,
  bandReferenceYearRight,
  bandRowOffsetY,
  bandStartDividerStyle,
  buildGeometry,
  buildPath,
  buildTeamTracks,
  buildYears,
  clamp,
  clampVisibleStart,
  familyFromTeamContext,
  familyMaxYear,
  familyMinYear,
  flattenTeams,
  formatPercent,
  getReadableWorldTeamColor,
  latestWindowStart,
  normalizePayload,
  round,
  roundedDisplayStartYear,
  rowPositionAtYear,
  scrollMinYear,
  teamEntryLabels,
  teamHighlight,
  teamMovementMarkers,
  teamRowToTeam,
  teamToTeamRow,
  teamTrackToTeamRow,
  trackLineGroups,
  visibleBandRows,
  visibleWindowEndYear,
  visibleWindowSize,
  windowRailStyle,
  yearFromClientX,
} from "./worldTeamChartGeometry";

// Teste espelho da geometria do atlas histórico de equipes (WorldTeamHistoryGrid /
// GlobalTeamsTab v1). São 669 linhas de cálculo determinístico sem uma linha de teste —
// a vistoria de 10/08/2026 apontou o desencontro com os pares do mesmo diretório
// (`atlasV2Geometry`, `gridMetrics`), que têm teste do tamanho do fonte.
//
// O que se guarda aqui é o que quebra na TELA e não no console: uma faixa de altura
// errada empurra todas as linhas de baixo, uma âncora de X fora do lugar solta a seta de
// promoção da linha, e um clamp de janela deixa o gráfico rolar para um ano vazio.

/// Um ponto de temporada como o payload de `get_global_team_history` entrega.
function ponto(year, position, extra = {}) {
  return { year, position, slot: "regular", points: 0, wins: 0, ...extra };
}

/// Uma linha de equipe dentro de uma faixa.
function linha(teamId, nome, points, extra = {}) {
  return { team_id: teamId, nome, cor_primaria: "#3080ff", points, ...extra };
}

/// Uma faixa (categoria) do atlas.
function faixa(key, rows, extra = {}) {
  return { key, category: key, starts_year: 2000, rows, ...extra };
}

describe("normalizePayload", () => {
  it("devolve null para payload ausente ou não-objeto", () => {
    expect(normalizePayload(null)).toBeNull();
    expect(normalizePayload(undefined)).toBeNull();
    expect(normalizePayload("gt3")).toBeNull();
  });

  it("garante array em families, bands, rows e points", () => {
    // O gráfico varre os quatro níveis com forEach/flatMap. Um `null` vindo do
    // backend em qualquer um deles derruba a tela inteira, e não só a faixa.
    const normalizado = normalizePayload({ families: null, bands: [{ key: "gt3", rows: null }] });
    expect(normalizado.families).toEqual([]);
    expect(normalizado.bands[0].rows).toEqual([]);

    const comLinha = normalizePayload({ bands: [{ key: "gt3", rows: [{ team_id: "T1", points: null }] }] });
    expect(comLinha.bands[0].rows[0].points).toEqual([]);
  });

  it("preserva os demais campos do payload", () => {
    const normalizado = normalizePayload({ min_year: 1994, window_start: 2010, bands: [] });
    expect(normalizado.min_year).toBe(1994);
    expect(normalizado.window_start).toBe(2010);
  });
});

describe("clamp, round e formatPercent", () => {
  it("clamp prende nos dois extremos", () => {
    expect(clamp(5, 0, 10)).toBe(5);
    expect(clamp(-3, 0, 10)).toBe(0);
    expect(clamp(42, 0, 10)).toBe(10);
  });

  it("round guarda uma casa decimal", () => {
    expect(round(12.34)).toBe(12.3);
    expect(round(12.35)).toBe(12.4);
    expect(round(12)).toBe(12);
  });

  it("formatPercent guarda quatro casas e sai como string", () => {
    // O valor vai direto para um `width: "x%"` de CSS: quatro casas é o que separa
    // duas colunas de um eixo de trinta e poucos anos.
    expect(formatPercent(33.333333)).toBe("33.3333");
    expect(formatPercent(50)).toBe("50");
  });
});

describe("buildGeometry: altura das faixas", () => {
  it("dimensiona a faixa pela MAIOR posição que ela já teve, não pelo número de equipes", () => {
    // Vinte equipes passaram pela categoria ao longo dos anos, mas o grid tem 3 vagas.
    // Dimensionar por equipe distinta daria uma faixa sete vezes mais alta que o
    // desenho — as linhas são posicionadas por POSIÇÃO, não por índice de equipe.
    const rows = Array.from({ length: 20 }, (_, i) =>
      linha(`T${i}`, `Equipe ${i}`, [ponto(2000 + i, (i % 3) + 1)]),
    );
    const { bands } = buildGeometry({ bands: [faixa("gt3", rows)] }, [2000]);
    const esperado = ROW_TOP_OFFSET + BAND_LABEL_HEIGHT + 3 * ROW_HEIGHT + 16;
    expect(bands.gt3.height).toBe(Math.max(MIN_BAND_HEIGHT, esperado));
  });

  it("respeita a altura mínima quando a faixa é rasa", () => {
    const { bands } = buildGeometry({ bands: [faixa("gt3", [linha("T1", "A", [ponto(2000, 1)])])] }, [2000]);
    expect(bands.gt3.height).toBe(MIN_BAND_HEIGHT);
  });

  it("empilha as faixas na ordem do payload, sem sobreposição nem folga", () => {
    const geometry = buildGeometry(
      {
        bands: [
          faixa("gt3", [linha("T1", "A", [ponto(2000, 1)])]),
          faixa("gt4", [linha("T2", "B", [ponto(2000, 1)])]),
        ],
      },
      [2000],
    );
    expect(geometry.bands.gt3.top).toBe(0);
    expect(geometry.bands.gt4.top).toBe(geometry.bands.gt3.height);
  });

  it("faixa sem ponto nenhum ainda ocupa uma linha", () => {
    // `Math.max(1, ...[])` é -Infinity sem o piso; a faixa sairia com altura negativa.
    const { bands } = buildGeometry({ bands: [faixa("gt3", [linha("T1", "A", [])])] }, [2000]);
    expect(bands.gt3.height).toBe(MIN_BAND_HEIGHT);
  });

  it("o gráfico tem piso de altura e soma o cabeçalho no total", () => {
    const geometry = buildGeometry({ bands: [] }, [2000, 2001]);
    expect(geometry.chartHeight).toBe(MIN_CHART_HEIGHT);
    expect(geometry.totalHeight).toBe(CHART_HEADER_HEIGHT + MIN_CHART_HEIGHT);
    expect(geometry.yearCount).toBe(2);
  });
});

describe("bandRowOffsetY", () => {
  it("a primeira posição encosta no topo útil da faixa", () => {
    expect(bandRowOffsetY(0, 1)).toBe(ROW_TOP_OFFSET + BAND_LABEL_HEIGHT);
  });

  it("cada posição desce uma linha", () => {
    expect(bandRowOffsetY(0, 3) - bandRowOffsetY(0, 2)).toBe(ROW_HEIGHT);
  });

  it("posição ausente ou zerada cai na primeira linha", () => {
    // P0 é o sentinela de "sem colocação" do payload — desenhá-lo acima da P1
    // colocaria a linha fora da faixa.
    expect(bandRowOffsetY(0, null)).toBe(bandRowOffsetY(0, 1));
    expect(bandRowOffsetY(0, 0)).toBe(bandRowOffsetY(0, 1));
  });
});

describe("buildYears", () => {
  const payload = {
    min_year: 2000,
    bands: [faixa("gt3", [linha("T1", "A", [ponto(2000, 1), ponto(2009, 1)])], { starts_year: 2000 })],
  };

  it("sem payload não há eixo", () => {
    expect(buildYears(null)).toEqual([]);
  });

  it("abre uma margem passada antes do primeiro ano de dado", () => {
    // É nessa margem que moram as etiquetas das equipes fundadoras.
    const years = buildYears(payload);
    expect(years[0]).toBeLessThan(2000);
    expect(years[0]).toBe(scrollMinYear(payload));
  });

  it("estende o eixo além do último ano navegável", () => {
    // As colunas extras são contexto do cabeçalho sob a tabela da direita: o eixo
    // renderizado passa do ano navegável de propósito.
    const years = buildYears(payload);
    expect(years[years.length - 1]).toBeGreaterThan(axisEndYear(payload));
  });

  it("o zoom recorta os últimos N anos", () => {
    const longo = {
      min_year: 1990,
      bands: [faixa("gt3", [linha("T1", "A", [ponto(1990, 1), ponto(2020, 1)])], { starts_year: 1990 })],
    };
    const semZoom = buildYears(longo);
    const comZoom = buildYears(longo, 5);
    expect(comZoom.length).toBeLessThan(semZoom.length);
    expect(comZoom[0]).toBe(2016);
  });

  it("é contínuo, sem buraco de ano", () => {
    const years = buildYears(payload);
    years.forEach((year, index) => {
      if (index > 0) expect(year).toBe(years[index - 1] + 1);
    });
  });
});

describe("janela visível e rolagem", () => {
  function payloadDeSpan(primeiro, ultimo) {
    return {
      min_year: primeiro,
      bands: [
        faixa("gt3", [linha("T1", "A", [ponto(primeiro, 1), ponto(ultimo, 1)])], { starts_year: primeiro }),
      ],
    };
  }

  it("uma família curta cabe inteira na janela", () => {
    const payload = payloadDeSpan(2010, 2019);
    expect(visibleWindowSize(payload)).toBeLessThan(MAX_VISIBLE_SPAN);
  });

  it("uma família longa para de crescer e passa a rolar", () => {
    // Este é o corte que existe para o gráfico não espremer quarenta colunas.
    const payload = payloadDeSpan(1960, 2026);
    expect(visibleWindowSize(payload)).toBe(MAX_VISIBLE_SPAN);
  });

  it("o início da janela nunca passa do último início possível", () => {
    const payload = payloadDeSpan(1960, 2026);
    const limite = latestWindowStart(payload, DEFAULT_WINDOW_SIZE);
    expect(clampVisibleStart(payload, 9999, DEFAULT_WINDOW_SIZE)).toBe(limite);
    expect(clampVisibleStart(payload, 0, DEFAULT_WINDOW_SIZE)).toBe(scrollMinYear(payload));
  });

  it("o arredondamento do início respeita os mesmos limites", () => {
    const payload = payloadDeSpan(1960, 2026);
    expect(roundedDisplayStartYear(payload, 1990.4, DEFAULT_WINDOW_SIZE)).toBe(1990);
    expect(roundedDisplayStartYear(payload, 9999, DEFAULT_WINDOW_SIZE)).toBe(
      latestWindowStart(payload, DEFAULT_WINDOW_SIZE),
    );
    expect(roundedDisplayStartYear(null, 1990)).toBe(1990);
  });

  it("o fim da janela nunca passa do último ano navegável", () => {
    const payload = payloadDeSpan(2010, 2019);
    expect(visibleWindowEndYear(payload, 2015, 20)).toBe(axisEndYear(payload));
    expect(visibleWindowEndYear(payload, 2010, 3)).toBe(2012);
    expect(visibleWindowEndYear(null, 2010)).toBeNull();
  });

  it("o ano navegável termina no último com dado, não na margem futura", () => {
    const payload = payloadDeSpan(2010, 2019);
    expect(axisEndYear(payload)).toBe(2019);
    expect(familyMaxYear(payload)).toBe(2019);
  });

  it("sem ponto nenhum, o máximo cai no window_end/max_year do payload", () => {
    expect(familyMaxYear({ bands: [], max_year: 2024 })).toBe(2024);
    expect(familyMaxYear({ bands: [], window_end: 2030, max_year: 2024 })).toBe(2030);
  });

  it("o mínimo da família é o início mais antigo entre as faixas, com piso no dado", () => {
    const payload = {
      min_year: 1994,
      bands: [faixa("gt3", [], { starts_year: 2005 }), faixa("gt4", [], { starts_year: 1999 })],
    };
    expect(familyMinYear(payload)).toBe(1999);
    // Uma faixa que diz ter começado antes do dado não puxa o eixo para o vazio.
    expect(familyMinYear({ min_year: 2000, bands: [faixa("gt3", [], { starts_year: 1900 })] })).toBe(2000);
  });
});

describe("windowRailStyle e yearFromClientX", () => {
  const payload = {
    min_year: 2000,
    window_start: 2005,
    window_size: 10,
    bands: [faixa("gt3", [linha("T1", "A", [ponto(2000, 1), ponto(2029, 1)])], { starts_year: 2000 })],
  };

  it("sem payload o polegar tem tamanho de partida", () => {
    expect(windowRailStyle(null)).toEqual({ left: "0%", width: "20%" });
  });

  it("no começo da trilha o polegar encosta na borda esquerda", () => {
    const estilo = windowRailStyle(payload, scrollMinYear(payload), 10);
    expect(estilo.left).toBe("0%");
  });

  it("o polegar tem largura mínima visível", () => {
    const estilo = windowRailStyle(payload, scrollMinYear(payload), 1);
    expect(parseFloat(estilo.width)).toBeGreaterThanOrEqual(3);
  });

  it("sem trilho medido, o arrasto devolve o começo em vez de NaN", () => {
    expect(yearFromClientX(payload, null, 100, 10)).toBe(scrollMinYear(payload));
    const semLargura = { getBoundingClientRect: () => ({ left: 0, width: 0 }) };
    expect(yearFromClientX(payload, semLargura, 100, 10)).toBe(scrollMinYear(payload));
  });

  it("o arrasto mapeia a trilha inteira entre o primeiro e o último início", () => {
    const trilho = { getBoundingClientRect: () => ({ left: 0, width: 200 }) };
    expect(yearFromClientX(payload, trilho, 0, 10)).toBe(scrollMinYear(payload));
    expect(yearFromClientX(payload, trilho, 200, 10)).toBe(latestWindowStart(payload, 10));
    // Fora do trilho, prende nas pontas em vez de extrapolar.
    expect(yearFromClientX(payload, trilho, -50, 10)).toBe(scrollMinYear(payload));
    expect(yearFromClientX(payload, trilho, 999, 10)).toBe(latestWindowStart(payload, 10));
  });
});

describe("bandPreStartStyle e bandStartDividerStyle", () => {
  const bandBox = { top: 40, height: 120 };
  const years = [2000, 2001, 2002, 2003, 2004];

  it("sem caixa, sem eixo ou sem ano de início não há zona", () => {
    expect(bandPreStartStyle({ starts_year: 2002 }, null, years, 2000)).toBeNull();
    expect(bandPreStartStyle({ starts_year: 2002 }, bandBox, [], 2000)).toBeNull();
    expect(bandPreStartStyle({ starts_year: null }, bandBox, years, 2000)).toBeNull();
  });

  it("a faixa que já existia no primeiro ano não ganha zona cinza", () => {
    expect(bandPreStartStyle({ starts_year: 2000 }, bandBox, years, 2000)).toBeNull();
    expect(bandPreStartStyle({ starts_year: 1998 }, bandBox, years, 2000)).toBeNull();
  });

  it("a zona começa no primeiro ano de dado e termina no início da faixa", () => {
    // Sem o piso no primeiro ano de dado, a zona cinza empilhava sobre a moldura
    // amarela global e as duas se liam como uma faixa dupla.
    const estilo = bandPreStartStyle({ starts_year: 2003 }, bandBox, years, 2001);
    expect(estilo.left).toBe(formatPercent((1 / 5) * 100) + "%");
    expect(estilo.width).toBe(formatPercent((2 / 5) * 100) + "%");
    expect(estilo.top).toBe(CHART_HEADER_HEIGHT + bandBox.top);
    expect(estilo.height).toBe(bandBox.height);
  });

  it("o divisor fica na borda ESQUERDA da coluna do ano de início", () => {
    // Mesma âncora da moldura e das linhas de fundação: meio de coluna deixaria o
    // divisor meia coluna à direita da moldura.
    const estilo = bandStartDividerStyle({ starts_year: 2002 }, bandBox, years);
    expect(estilo.left).toBe(formatPercent((2 / 5) * 100) + "%");
  });

  it("não desenha divisor fora do recorte visível", () => {
    expect(bandStartDividerStyle({ starts_year: 2000 }, bandBox, years)).toBeNull();
    expect(bandStartDividerStyle({ starts_year: 2010 }, bandBox, years)).toBeNull();
    expect(bandStartDividerStyle({ starts_year: null }, bandBox, years)).toBeNull();
  });
});

describe("axisEdgeZoneStyle", () => {
  const payload = {
    min_year: 2000,
    bands: [faixa("gt3", [linha("T1", "A", [ponto(2002, 1), ponto(2006, 1)])], { starts_year: 2002 })],
  };
  const years = [2000, 2001, 2002, 2003, 2004, 2005, 2006, 2007];

  it("a moldura da esquerda cobre só os anos antes do primeiro dado", () => {
    const estilo = axisEdgeZoneStyle("left", payload, years);
    expect(estilo.width).toBe(formatPercent((2 / years.length) * 100) + "%");
    expect(estilo.left).toBe(0);
  });

  it("sem anos antes do dado não há moldura à esquerda", () => {
    expect(axisEdgeZoneStyle("left", payload, [2002, 2003])).toBeNull();
  });

  it("a moldura da direita começa DEPOIS da última temporada fechada", () => {
    // No meio da coluna, o último ano se leria como um ano pela metade.
    const estilo = axisEdgeZoneStyle("right", payload, years);
    expect(estilo.left).toBe(formatPercent((7 / years.length) * 100) + "%");
    expect(estilo.right).toBe(0);
  });

  it("sem payload ou sem eixo não há moldura", () => {
    expect(axisEdgeZoneStyle("left", null, years)).toBeNull();
    expect(axisEdgeZoneStyle("right", payload, [])).toBeNull();
  });
});

describe("visibleBandRows e rowPositionAtYear", () => {
  it("só entram as linhas com ponto no ano de referência", () => {
    const rows = [
      linha("T1", "Alfa", [ponto(2000, 3)]),
      linha("T2", "Beta", [ponto(2001, 1)]),
      linha("T3", "Gama", [ponto(2000, 1)]),
    ];
    expect(visibleBandRows(rows, 2000).map((r) => r.nome)).toEqual(["Gama", "Alfa"]);
  });

  it("empate de posição desempata pelo nome", () => {
    const rows = [linha("T1", "Zeta", [ponto(2000, 1)]), linha("T2", "Alfa", [ponto(2000, 1)])];
    expect(visibleBandRows(rows, 2000).map((r) => r.nome)).toEqual(["Alfa", "Zeta"]);
  });

  it("rowPositionAtYear devolve null fora do ano e trata P0 como P1", () => {
    const row = linha("T1", "Alfa", [ponto(2000, 0), ponto(2001, 4)]);
    expect(rowPositionAtYear(row, 2000)).toBe(1);
    expect(rowPositionAtYear(row, 2001)).toBe(4);
    expect(rowPositionAtYear(row, 1999)).toBeNull();
    expect(rowPositionAtYear({}, 2000)).toBeNull();
  });
});

describe("bandReferenceYearRight", () => {
  const band = faixa("gt3", [
    linha("T1", "Alfa", [ponto(2000, 1), ponto(2004, 1)]),
    linha("T2", "Beta", [ponto(2002, 2)]),
  ]);

  it("mostra a última temporada com dado até o ano visível da direita", () => {
    expect(bandReferenceYearRight(band, 2003)).toBe(2002);
    expect(bandReferenceYearRight(band, 2004)).toBe(2004);
  });

  it("devolve null quando a faixa ainda não correu no recorte", () => {
    expect(bandReferenceYearRight(band, 1999)).toBeNull();
    expect(bandReferenceYearRight(null, 2004)).toBeNull();
  });
});

describe("buildTeamTracks e trackLineGroups", () => {
  const years = [2000, 2001, 2002];
  const payload = {
    bands: [
      faixa("gt3", [linha("T1", "Alfa", [ponto(2001, 1), ponto(2002, 2)])]),
      faixa("gt4", [linha("T1", "Alfa", [ponto(2000, 3)]), linha("T2", "Beta", [ponto(2000, 1)])]),
    ],
  };
  const geometry = buildGeometry(payload, years);

  it("junta as passagens da MESMA equipe por faixas diferentes numa trilha só", () => {
    // É o que permite desenhar a linha atravessando a promoção: duas linhas de
    // payload, uma trajetória na tela.
    const tracks = buildTeamTracks(payload, geometry, years);
    const alfa = tracks.find((t) => t.team_id === "T1");
    expect(alfa.points.map((p) => [p.year, p.band_key])).toEqual([
      [2000, "gt4"],
      [2001, "gt3"],
      [2002, "gt3"],
    ]);
  });

  it("descarta o ponto de um ano fora do eixo", () => {
    const tracks = buildTeamTracks(payload, geometry, [2001, 2002]);
    const alfa = tracks.find((t) => t.team_id === "T1");
    expect(alfa.points.every((p) => p.year >= 2001)).toBe(true);
  });

  it("a cor da equipe já sai legível sobre o fundo escuro", () => {
    const escuro = {
      bands: [faixa("gt3", [linha("T1", "Alfa", [ponto(2000, 1)], { cor_primaria: "#0a0a0a" })])],
    };
    const [track] = buildTeamTracks(escuro, buildGeometry(escuro, [2000]), [2000]);
    expect(track.cor_primaria).toBe(getReadableWorldTeamColor("#0a0a0a"));
    expect(track.cor_primaria).not.toBe("#0a0a0a");
  });

  it("trackLineGroups separa a linha regular da especial", () => {
    const track = {
      team_id: "T1",
      nome: "Alfa",
      points: [ponto(2000, 1), ponto(2000, 2, { slot: "special" })],
    };
    const grupos = trackLineGroups(track);
    expect(grupos.map((g) => g.line_key)).toEqual(["regular", "special"]);
    expect(grupos[0].points).toHaveLength(1);
  });

  it("grupo sem ponto não vira linha vazia", () => {
    const track = { team_id: "T1", nome: "Alfa", points: [ponto(2000, 1)] };
    expect(trackLineGroups(track).map((g) => g.line_key)).toEqual(["regular"]);
  });
});

describe("buildPath", () => {
  const years = [2000, 2001, 2002];
  const payload = { bands: [faixa("gt3", [linha("T1", "Alfa", [ponto(2000, 1), ponto(2001, 2)])])] };
  const geometry = buildGeometry(payload, years);

  it("sem ponto ou sem eixo o caminho é vazio", () => {
    expect(buildPath({ points: [] }, geometry, years)).toBe("");
    expect(buildPath({ points: [ponto(2000, 1)] }, geometry, [])).toBe("");
  });

  it("liga início de ano a início de ano e segura a última coluna até a borda", () => {
    const [track] = buildTeamTracks(payload, geometry, years);
    const comandos = buildPath(track, geometry, years).split(" L ");
    expect(comandos).toHaveLength(3);
    expect(comandos[0].startsWith("M ")).toBe(true);
    // O último vértice repete o Y do penúltimo e avança só em X: é a temporada
    // segurando até o fim da própria coluna.
    const [ultimoX, ultimoY] = comandos[2].split(" ").map(Number);
    const [penultimoX, penultimoY] = comandos[1].split(" ").map(Number);
    expect(ultimoY).toBe(penultimoY);
    expect(ultimoX).toBeGreaterThan(penultimoX);
    expect(ultimoX).toBe(round((2 / 3) * CHART_WIDTH));
  });

  it("ignora ponto de faixa que não existe na geometria", () => {
    const track = { points: [ponto(2000, 1), { ...ponto(2001, 1), band_key: "inexistente" }] };
    track.points[0].band_key = "gt3";
    expect(buildPath(track, geometry, years).split(" L ")).toHaveLength(2);
  });
});

describe("teamMovementMarkers", () => {
  const years = [2000, 2001];
  const payload = {
    bands: [
      faixa("gt3", [linha("T1", "Alfa", [ponto(2001, 1)])]),
      faixa("gt4", [linha("T1", "Alfa", [ponto(2000, 1)])]),
    ],
  };
  const geometry = buildGeometry(payload, years);
  const [track] = buildTeamTracks(payload, geometry, years);

  it("marca a promoção no ano em que a equipe sobe de faixa", () => {
    const [marcador] = teamMovementMarkers(track, geometry, years);
    expect(marcador.type).toBe("promotion");
    expect(marcador.year).toBe(2000);
    expect(marcador.band_key).toBe("gt4");
  });

  it("a seta usa a mesma âncora da linha: borda esquerda da coluna", () => {
    // Meio de coluna soltaria a seta meia coluna à frente do vértice.
    const [marcador] = teamMovementMarkers(track, geometry, years);
    expect(marcador.x).toBe((0 / years.length) * CHART_WIDTH);
  });

  it("marca o rebaixamento quando a faixa de destino é mais baixa", () => {
    const descida = {
      bands: [
        faixa("gt3", [linha("T1", "Alfa", [ponto(2000, 1)])]),
        faixa("gt4", [linha("T1", "Alfa", [ponto(2001, 1)])]),
      ],
    };
    const geo = buildGeometry(descida, years);
    const [t] = buildTeamTracks(descida, geo, years);
    expect(teamMovementMarkers(t, geo, years)[0].type).toBe("demotion");
  });

  it("não marca nada quando há um ano de intervalo entre as faixas", () => {
    // Uma equipe que sumiu do grid e voltou dois anos depois não foi promovida.
    const comBuraco = {
      bands: [
        faixa("gt3", [linha("T1", "Alfa", [ponto(2002, 1)])]),
        faixa("gt4", [linha("T1", "Alfa", [ponto(2000, 1)])]),
      ],
    };
    const anos = [2000, 2001, 2002];
    const geo = buildGeometry(comBuraco, anos);
    const [t] = buildTeamTracks(comBuraco, geo, anos);
    expect(teamMovementMarkers(t, geo, anos)).toEqual([]);
  });

  it("não marca nada quando a equipe fica na mesma faixa", () => {
    const parada = { bands: [faixa("gt3", [linha("T1", "Alfa", [ponto(2000, 1), ponto(2001, 3)])])] };
    const geo = buildGeometry(parada, years);
    const [t] = buildTeamTracks(parada, geo, years);
    expect(teamMovementMarkers(t, geo, years)).toEqual([]);
  });
});

describe("teamEntryLabels", () => {
  const years = [2000, 2001, 2002];
  const payload = { bands: [faixa("gt3", [linha("T1", "Alfa", [ponto(2001, 1), ponto(2002, 1)])])] };
  const geometry = buildGeometry(payload, years);
  const [track] = buildTeamTracks(payload, geometry, years);

  it("rotula a equipe que estreia dentro da janela", () => {
    const [rotulo] = teamEntryLabels(track, geometry, years, payload, 2000);
    expect(rotulo.year).toBe(2001);
    expect(rotulo.band_key).toBe("gt3");
  });

  it("não rotula quem já estava lá antes do recorte", () => {
    expect(teamEntryLabels(track, geometry, years, payload, 2001)).toEqual([]);
  });

  it("faixa especial não ganha etiqueta de estreia", () => {
    const especial = {
      bands: [faixa("endurance", [linha("T1", "Alfa", [ponto(2001, 1)])], { is_special: true })],
    };
    const geo = buildGeometry(especial, years);
    const [t] = buildTeamTracks(especial, geo, years);
    expect(teamEntryLabels(t, geo, years, especial, 2000)).toEqual([]);
  });

  it("a largura da etiqueta acompanha o nome, presa entre 124 e 236", () => {
    const curto = teamEntryLabels(track, geometry, years, payload, 2000)[0];
    expect(curto.width).toBe(clamp("Alfa".length * 7.2 + 70, 124, 236));

    const nomao = "Equipe De Nome Absurdamente Longo Para Caber";
    const payloadLongo = { bands: [faixa("gt3", [linha("T9", nomao, [ponto(2001, 1)])])] };
    const geo = buildGeometry(payloadLongo, years);
    const [t] = buildTeamTracks(payloadLongo, geo, years);
    expect(teamEntryLabels(t, geo, years, payloadLongo, 2000)[0].width).toBe(236);
  });

  it("a âncora nunca encosta nas bordas do gráfico", () => {
    const [rotulo] = teamEntryLabels(track, geometry, years, payload, 2000);
    expect(rotulo.anchorX).toBeGreaterThanOrEqual(16);
    expect(rotulo.anchorX).toBeLessThanOrEqual(CHART_WIDTH - 6);
  });
});

describe("getReadableWorldTeamColor", () => {
  it("cor inválida cai no cinza neutro", () => {
    expect(getReadableWorldTeamColor(null)).toBe("#7d8590");
    expect(getReadableWorldTeamColor("azul")).toBe("#7d8590");
    expect(getReadableWorldTeamColor("#abc")).toBe("#7d8590");
  });

  it("cor já legível passa intacta", () => {
    expect(getReadableWorldTeamColor("#ffffff")).toBe("#ffffff");
    expect(getReadableWorldTeamColor("#3080FF")).toBe("#3080FF");
  });

  it("cor escura demais é clareada em vez de sumir no fundo", () => {
    // A luminância é a percebida (ITU-R BT.709): um azul-marinho e um verde-escuro
    // com o mesmo valor de canal não somem no mesmo ponto.
    expect(getReadableWorldTeamColor("#000000")).toBe("rgb(163, 163, 163)");
    expect(getReadableWorldTeamColor("#0a1428")).toMatch(/^rgb\(/);
  });
});

describe("familyFromTeamContext", () => {
  it("reconhece a família pela categoria", () => {
    expect(familyFromTeamContext("toyota_rookie", null)).toBe("toyota");
    expect(familyFromTeamContext("bmw_m2", null)).toBe("bmw");
    expect(familyFromTeamContext("gt4", null)).toBe("gt4");
    expect(familyFromTeamContext("gt3", null)).toBe("gt3");
    expect(familyFromTeamContext("lmp2", null)).toBe("lmp2");
    expect(familyFromTeamContext("mazda_amador", null)).toBe("mazda");
  });

  it("reconhece pela classe quando a categoria é multiclasse", () => {
    expect(familyFromTeamContext("production_challenger", "BMW")).toBe("bmw");
    expect(familyFromTeamContext("endurance", "GT4")).toBe("gt4");
  });

  it("multiclasse sem classe conhecida cai na família de referência", () => {
    expect(familyFromTeamContext("production_challenger", null)).toBe("mazda");
    expect(familyFromTeamContext("endurance", "lmp3")).toBe("gt3");
  });

  it("não é sensível a caixa nem a espaço", () => {
    expect(familyFromTeamContext("  GT3  ", null)).toBe("gt3");
  });

  it("contexto desconhecido cai na família padrão", () => {
    expect(familyFromTeamContext(null, null)).toBe(DEFAULT_FAMILY);
    expect(familyFromTeamContext("formula_e", "x")).toBe(DEFAULT_FAMILY);
  });
});

describe("conversões de linha e equipe", () => {
  it("flattenTeams carrega a categoria da faixa para dentro da linha", () => {
    const payload = {
      bands: [faixa("gt3", [linha("T1", "Alfa", [])]), faixa("gt4", [linha("T2", "Beta", [])])],
    };
    expect(flattenTeams(payload).map((r) => [r.nome, r.band_category])).toEqual([
      ["Alfa", "gt3"],
      ["Beta", "gt4"],
    ]);
    expect(flattenTeams(null)).toEqual([]);
  });

  it("teamRowToTeam usa os campos da linha e cai no fallback quando falta", () => {
    const row = linha("T1", "Alfa", [ponto(2000, 1, { points: 120, wins: 3 })], {
      band_category: "gt3",
      base_position: 2,
    });
    expect(teamRowToTeam(row)).toMatchObject({
      id: "T1",
      nome: "Alfa",
      nome_curto: "Alfa",
      categoria: "gt3",
      posicao: 2,
      pontos: 120,
      vitorias: 3,
    });
    expect(teamRowToTeam({ id: "X", nome: "Beta" })).toMatchObject({ id: "X", pontos: 0, vitorias: 0 });
  });

  it("teamTrackToTeamRow herda categoria e classe da faixa", () => {
    const bandas = new Map([["gt3", { category: "gt3", class_name: "GT3" }]]);
    expect(teamTrackToTeamRow({ team_id: "T1", nome: "Alfa" }, "gt3", bandas)).toMatchObject({
      band_key: "gt3",
      band_category: "gt3",
      class_name: "GT3",
    });
  });

  it("teamTrackToTeamRow sobrevive a faixa desconhecida", () => {
    expect(teamTrackToTeamRow({ team_id: "T1", nome: "Alfa" }, "gtX", new Map())).toMatchObject({
      band_category: "",
      class_name: null,
    });
  });

  it("teamToTeamRow preserva o fallback e sobrescreve a identidade", () => {
    const fallback = { base_position: 5, band_key: "gt3" };
    expect(teamToTeamRow({ id: "T1", nome: "Alfa", posicao: 2 }, fallback)).toMatchObject({
      team_id: "T1",
      nome: "Alfa",
      band_key: "gt3",
      base_position: 2,
    });
    expect(teamToTeamRow({ id: "T1", nome: "Alfa" }, fallback).base_position).toBe(5);
  });
});

describe("teamHighlight", () => {
  it("sem nada aceso, ninguém apaga", () => {
    expect(teamHighlight("T1", null, null)).toEqual({ isFocused: false, isDimmed: false });
  });

  it("a equipe sob o mouse acende e as outras apagam", () => {
    expect(teamHighlight("T1", "T1", null)).toEqual({ isFocused: true, isDimmed: false });
    expect(teamHighlight("T2", "T1", null)).toEqual({ isFocused: false, isDimmed: true });
  });

  it("a equipe fixada continua acesa ao passar o mouse por outra", () => {
    // Era o ponto do desenho: sem isto, comparar duas linhas apagava a que
    // interessava no instante em que se olhava a outra.
    expect(teamHighlight("T1", "T2", "T1")).toEqual({ isFocused: true, isDimmed: false });
    expect(teamHighlight("T2", "T2", "T1")).toEqual({ isFocused: true, isDimmed: false });
  });
});
