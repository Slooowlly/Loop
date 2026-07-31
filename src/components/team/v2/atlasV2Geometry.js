// Geometria do Atlas Histórico v2.
//
// Diferença central para o v1 (../worldTeamChartGeometry.js): lá o gráfico tinha
// largura fixa (CHART_WIDTH = 1000) e reservava 25% da própria área para as tabelas
// de classificação, que ficavam SOBREPOSTAS ao gráfico. Aqui o gráfico recebe a
// largura e a altura reais medidas do card e desenha só dentro delas — os rankings
// vivem numa coluna própria do layout e não têm nada a ver com este arquivo.
//
// REGRA DE OURO: existe UM sistema de coordenadas horizontal, o `timeline` devolvido
// por buildAtlasGeometry. Cabeçalho de anos, grade, hachuras, linhas, pontos e
// etiquetas TÊM que passar por `getBoundaryX`/`getCenterX`. Nenhum componente pode
// calcular X por conta própria — foi assim que o cabeçalho e os pontos passaram a
// usar réguas diferentes e a imagem saiu desalinhada.
//
// Tudo aqui é PURO: entra payload/anos/dimensões, sai número. Nada de React.

import { clamp, getReadableWorldTeamColor } from "../worldTeamChartGeometry";

// Faixa dos anos no topo do card.
export const YEAR_HEADER_HEIGHT = 34;
// Vão entre campeonatos. Vale para os DOIS lados: é o mesmo número que separa as
// faixas no gráfico e os cards na coluna lateral.
export const BAND_GAP = 12;
// Altura da zona de cabeçalho de cada divisão. No card é a barra com o troféu e o
// título; no gráfico é o respiro equivalente acima da primeira posição. Tem de ser
// idêntica dos dois lados, senão as linhas descolam progressivamente card a card.
export const DIVISION_HEADER_HEIGHT = 44;
// Respiro abaixo da última posição de cada divisão.
export const DIVISION_BOTTOM_PADDING = 8;
// A altura da linha é derivada do espaço disponível, mas dentro destes limites.
export const MIN_ROW_HEIGHT = 14;
export const MAX_ROW_HEIGHT = 54;
export const POINT_RADIUS = 3;
// Anos desenhados ANTES da primeira temporada da família. Não são enfeite nem calha
// invisível: são anos reais em que a série ainda não existia, hachurados, e é neles
// que as etiquetas das equipes fundadoras cabem sem empurrar os dados.
export const PRE_SERIES_YEARS = 3;

// Ano de abertura fixado por família. A regra geral é `primeira temporada menos
// PRE_SERIES_YEARS`, mas algumas famílias têm um ano de abertura canônico que não
// sai dessa conta — é o caso da Mazda, que abre em 2014. Título e eixo leem daqui,
// então não têm como divergir.
export const FAMILY_DISPLAY_START_YEAR = {
  mazda: 2014,
};

// Ano em que o eixo abre: o override da família quando existe, senão a regra geral.
// Nunca depois da primeira temporada — um override tardio esconderia dado real.
export function displayStartYear(payload) {
  const first = firstSeriesYear(payload);
  if (!Number.isFinite(first)) return null;
  const configured = FAMILY_DISPLAY_START_YEAR[payload?.selected_family];
  if (Number.isFinite(configured)) return Math.min(configured, first);
  return first - PRE_SERIES_YEARS;
}
// Distância entre a ponta direita da etiqueta e o ponto que ela nomeia.
export const LABEL_GAP = 14;
// Passo do recuo horizontal da anticolisão, e quantas colunas extras ele pode usar.
// O recuo é curto de propósito: a etiqueta tem de continuar perto do próprio grupo.
export const LABEL_COLUMN_SHIFT = 36;
export const MAX_LABEL_COLUMNS = 3;

// Primeiro ano com dado de verdade na família — não o ano de fundação declarado da
// categoria, que pode preceder a primeira temporada realmente disputada.
export function firstSeriesYear(payload) {
  let min = null;
  (payload?.bands ?? []).forEach((band) =>
    (band.rows ?? []).forEach((row) =>
      (row.points ?? []).forEach((point) => {
        if (min === null || point.year < min) min = point.year;
      }),
    ),
  );
  return min;
}

export function lastSeriesYear(payload) {
  let max = null;
  (payload?.bands ?? []).forEach((band) =>
    (band.rows ?? []).forEach((row) =>
      (row.points ?? []).forEach((point) => {
        if (max === null || point.year > max) max = point.year;
      }),
    ),
  );
  return max;
}

// Último ano que vira COLUNA. A temporada em andamento não ganha coluna própria:
// ela não tem um campeonato para preencher a célula, e uma coluna inteira só para
// a linha chegar e ficar parada até a borda era um vão morto do tamanho de um ano.
// A posição de agora é desenhada NA borda direita, que é onde o card lateral está.
//
// A exceção é a carreira que ainda não arquivou temporada alguma: aí a temporada em
// curso é a única coisa que existe, e sem a coluna dela não sobraria eixo nenhum.
export function axisEndYear(payload) {
  const last = lastSeriesYear(payload);
  if (!isLivePayload(payload) || last !== payload.current_year) return last;
  const previous = previousSeriesYear(payload, payload.current_year);
  return Number.isFinite(previous) ? previous : last;
}

// Maior ano com dado ANTES de `year`.
function previousSeriesYear(payload, year) {
  let max = null;
  (payload?.bands ?? []).forEach((band) =>
    (band.rows ?? []).forEach((row) =>
      (row.points ?? []).forEach((point) => {
        if (point.year < year && (max === null || point.year > max)) max = point.year;
      }),
    ),
  );
  return max;
}

// O eixo do v2: PRE_SERIES_YEARS anos antes da primeira temporada até a última
// temporada disputada. Sem a cauda de anos futuros do v1 — lá ela existia para
// acomodar a janela deslizante e a tabela no vão da direita, que aqui não existem.
export function buildAtlasYears(payload, zoomYears = null) {
  if (!payload) return [];
  const start = displayStartYear(payload);
  const last = axisEndYear(payload);
  if (!Number.isFinite(start) || !Number.isFinite(last) || start > last) return [];

  const displayStart = Number.isFinite(zoomYears) && zoomYears != null
    ? Math.max(start, last - zoomYears + 1)
    : start;

  const years = [];
  for (let year = displayStart; year <= last; year += 1) years.push(year);
  return years;
}

// Quantas posições simultâneas o campeonato chega a ter — é quantas linhas ele
// precisa, e não quantas equipes passaram por ele ao longo dos anos.
// Cor para ELEMENTOS PEQUENOS (nome, brasão, borda, marcador de estreia).
//
// A cor bruta da equipe funciona numa linha de 2px, mas some num texto de 11px sobre
// fundo escuro. Aqui ela é clareada até um piso de luminosidade — a identidade da
// equipe continua a mesma, só a versão usada em miudezas fica legível. A linha do
// gráfico segue com a cor de sempre.
export function ensureMinimumLuminance(color, minLuminance = 0.52) {
  const rgb = parseColor(color);
  if (!rgb) return "#9fb2c4";
  const [r, g, b] = rgb;
  const luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
  if (luminance >= minLuminance) return color;

  // Mistura com branco na medida exata para alcançar o piso.
  const mix = clamp((minLuminance - luminance) / Math.max(1 - luminance, 0.001), 0, 0.85);
  const lift = (channel) => Math.round(channel + (255 - channel) * mix);
  return `rgb(${lift(r)}, ${lift(g)}, ${lift(b)})`;
}

function parseColor(color) {
  if (typeof color !== "string") return null;
  const hex = color.trim().match(/^#([0-9a-f]{6})$/i);
  if (hex) {
    return [0, 2, 4].map((offset) => parseInt(hex[1].slice(offset, offset + 2), 16));
  }
  const rgb = color.trim().match(/^rgba?\(\s*(\d+)\D+(\d+)\D+(\d+)/i);
  return rgb ? [Number(rgb[1]), Number(rgb[2]), Number(rgb[3])] : null;
}

export function bandSlotCount(band) {
  const positions = (band?.rows ?? []).flatMap((row) =>
    (row.points ?? []).map((point) => Math.max(point.position ?? 1, 1)),
  );
  return positions.length ? Math.max(...positions) : 1;
}

// RÉGUA VERTICAL ÚNICA — a contraparte da régua horizontal.
//
// Gráfico e cards laterais têm de derivar TODA posição vertical daqui. Enquanto cada
// lado calculava a sua (o gráfico a partir do topo da faixa, o card a partir do topo
// do card mais a altura do seu cabeçalho), a diferença de cabeçalho se acumulava e o
// desalinhamento crescia card a card. Aqui a zona de cabeçalho é uma medida só,
// `DIVISION_HEADER_HEIGHT`, e vale para os dois.
//
// `rankY(divisionId, rank)` é o contrato: o mesmo número serve ao ponto do gráfico,
// à linha histórica, ao vértice terminal e ao centro da linha no card.
export function buildAtlasVerticalGeometry({
  totalHeight,
  divisions,
  headerHeight = DIVISION_HEADER_HEIGHT,
  gap = BAND_GAP,
  bottomPadding = DIVISION_BOTTOM_PADDING,
}) {
  const list = divisions ?? [];
  const height = Math.max(totalHeight ?? 0, 0);
  const totalRows = list.reduce((sum, division) => sum + Math.max(division.rowCount ?? 1, 1), 0);
  const chrome = list.length * (headerHeight + bottomPadding) + Math.max(list.length - 1, 0) * gap;
  const rowHeight = totalRows > 0
    ? clamp((height - chrome) / totalRows, MIN_ROW_HEIGHT, MAX_ROW_HEIGHT)
    : MIN_ROW_HEIGHT;

  // Se a altura de linha bateu no teto e sobrou espaço, a sobra vira respiro no pé
  // de cada divisão — em partes iguais. Como os dois lados leem `division.height`,
  // o card cresce exatamente o mesmo tanto que a faixa do gráfico.
  const used = totalRows * rowHeight + chrome;
  const slack = list.length > 0 ? Math.max(height - used, 0) / list.length : 0;
  const footer = bottomPadding + slack;

  const boxes = {};
  let cursor = 0;
  list.forEach((division) => {
    const rowCount = Math.max(division.rowCount ?? 1, 1);
    const divisionHeight = headerHeight + rowCount * rowHeight + footer;
    boxes[division.id] = {
      id: division.id,
      top: cursor,
      bottom: cursor + divisionHeight,
      height: divisionHeight,
      headerTop: cursor,
      headerBottom: cursor + headerHeight,
      headerHeight,
      rowsTop: cursor + headerHeight,
      rowHeight,
      rowCount,
    };
    cursor += divisionHeight + gap;
  });

  const rankY = (divisionId, rank) => {
    const box = boxes[divisionId];
    if (!box) return NaN;
    return box.rowsTop + (Math.max(rank ?? 1, 1) - 0.5) * rowHeight;
  };

  return {
    divisions: boxes,
    order: list.map((division) => division.id),
    rowHeight,
    headerHeight,
    gap,
    bottomPadding,
    contentHeight: Math.max(cursor - gap, 0),
    rankY,
  };
}

// Divisões na ordem do payload, com quantas posições cada uma precisa.
export function atlasDivisions(payload) {
  return (payload?.bands ?? []).map((band) => ({ id: band.key, rowCount: bandSlotCount(band) }));
}

// A geometria completa do gráfico: régua horizontal + a régua vertical recebida
// pronta, para os cards laterais consumirem exatamente a mesma.
//
// A timeline ocupa a largura INTEIRA da área de plotagem. Cada ano é um intervalo
// entre dois limites verticais — para N anos há N células e N+1 limites. O ponto de
// uma temporada mora no LIMITE de abertura dela; o rótulo do ano, no centro da
// célula. É o que faz o primeiro ponto cair exatamente na divisa 2016|2017.
export function buildAtlasGeometry(payload, years, size, vertical) {
  // `size` é a área de plotagem já medida (sem a faixa dos anos, que é uma linha
  // separada do card, porém da mesma largura).
  const plotWidth = Math.max(size?.width ?? 0, 0);
  const yearCount = Math.max(years?.length ?? 0, 0);
  const verticalGeometry = vertical
    ?? buildAtlasVerticalGeometry({ totalHeight: size?.height ?? 0, divisions: atlasDivisions(payload) });

  const timelineLeft = 0;
  const timelineRight = plotWidth;
  const timelineWidth = timelineRight - timelineLeft;
  const yearWidth = yearCount > 0 ? timelineWidth / yearCount : 0;

  const getBoundaryX = (boundaryIndex) => timelineLeft + clamp(boundaryIndex, 0, yearCount) * yearWidth;
  const getCenterX = (yearIndex) => getBoundaryX(yearIndex) + yearWidth / 2;
  // Conveniências por ANO (e não por índice), que é como o resto do código pensa.
  const boundaryOfYear = (year) => getBoundaryX((year ?? 0) - (years?.[0] ?? 0));
  const centerOfYear = (year) => getCenterX((year ?? 0) - (years?.[0] ?? 0));

  return {
    // `bands` é a MESMA caixa que os cards usam — nenhuma altura é recalculada aqui.
    bands: verticalGeometry.divisions,
    vertical: verticalGeometry,
    rowHeight: verticalGeometry.rowHeight,
    bandGap: verticalGeometry.gap,
    plotWidth,
    bodyHeight: verticalGeometry.contentHeight,
    contentHeight: verticalGeometry.contentHeight,
    // --- régua horizontal única ---
    years: years ?? [],
    yearCount,
    yearWidth,
    timelineLeft,
    timelineRight,
    timelineWidth,
    getBoundaryX,
    getCenterX,
    boundaryOfYear,
    centerOfYear,
  };
}

// Y de uma classificação. Delega para a régua vertical compartilhada — é o mesmo
// número que posiciona a linha correspondente no card lateral.
export function slotY(geometry, bandKey, position) {
  return geometry.vertical ? geometry.vertical.rankY(bandKey, position) : NaN;
}

// ---------------------------------------------------------------------------
// Linhas
// ---------------------------------------------------------------------------

// Vértices de uma linha, em pixels. Cada temporada abre no seu limite esquerdo, e
// depois do último ponto entra um vértice terminal no FIM daquela temporada — sem
// ele a linha pararia na abertura do último ano, antes da borda da célula.
//
// O terminal fecha a célula da última temporada DA EQUIPE, não a do gráfico: quem
// parou de correr em 2020 termina no fim de 2020. Para quem chega até a temporada
// atual isso é exatamente a borda direita do gráfico.
export function trackVertices(line, geometry, years) {
  const vertices = (line.points ?? [])
    .map((point) => {
      const x = geometry.boundaryOfYear(point.year);
      const y = slotY(geometry, point.band_key, point.position);
      return Number.isFinite(x) && Number.isFinite(y) ? { x, y, point } : null;
    })
    .filter(Boolean);

  if (vertices.length > 0) {
    const last = vertices[vertices.length - 1];
    const closingX = geometry.boundaryOfYear(last.point.year + 1);
    if (closingX > last.x) {
      vertices.push({ x: closingX, y: last.y, point: last.point, isClosing: true });
    }
  }

  return vertices;
}

// Parte a linha em duas: o que já está decidido e o trecho da temporada em curso.
//
// O corte inclui o ÚLTIMO vértice decidido nas duas metades — é ele que ancora a
// diagonal — então a subida ou queda rumo à coluna viva sai inteira tracejada. Uma
// equipe que só existe na temporada em curso devolve `settled` vazio: não há
// passado dela para desenhar cheio.
export function splitVerticesAtLiveYear(vertices, liveYear) {
  if (!Number.isFinite(liveYear)) return { settled: vertices, live: [] };
  const index = vertices.findIndex((vertex) => vertex.point?.year === liveYear);
  if (index < 0) return { settled: vertices, live: [] };
  return {
    settled: vertices.slice(0, index),
    live: vertices.slice(Math.max(index - 1, 0)),
  };
}

// Um trecho por DIVISÃO: vértices consecutivos que pertencem à mesma faixa.
//
// Sem isso a linha de uma equipe promovida ou rebaixada é um caminho só, e o salto
// de uma divisão para a outra vira uma diagonal que atravessa a altura inteira do
// gráfico, cortando em cheio os campeonatos que estão no meio do caminho. Cada
// trecho é desenhado dentro da sua faixa; a travessia entre eles é outra coisa,
// com regra própria (ver `crossingStub`).
export function splitVerticesByBand(vertices) {
  const runs = [];
  (vertices ?? []).forEach((vertex) => {
    const bandKey = vertex.point?.band_key ?? null;
    const current = runs[runs.length - 1];
    if (current && current.bandKey === bandKey) {
      current.vertices.push(vertex);
      return;
    }
    runs.push({ bandKey, vertices: [vertex] });
  });
  return runs;
}

// Onde a vertical do corredor pode cair dentro da célula do ano, em fração da
// largura dela. Nunca encostada nas bordas: colada à esquerda ela sairia do ponto
// da temporada, colada à direita ela grudaria na linha da grade.
export const CORRIDOR_MIN_RATIO = 0.3;
export const CORRIDOR_MAX_RATIO = 0.82;

// A travessia entre divisões, em Z: reta na altura da divisão antiga até o
// corredor, vertical dentro dele, e reta de novo já na altura da divisão nova.
//
// A diagonal direta era o problema: ela cruza os campeonatos do meio em ângulo,
// no mesmo ângulo das linhas de dados, e vira mais um fio no meio do novelo. A
// vertical não se parece com dado nenhum — nenhuma equipe muda de posição dentro
// de uma temporada — então ela se lê como passagem, e não como trajetória.
//
// `lane` distribui as verticais do MESMO ano em x diferentes: sem isso, cinco
// equipes trocando de divisão no mesmo ano desenhariam cinco verticais sobrepostas
// no mesmo pixel, que é um traço grosso e nenhuma informação.
export function crossingCorridorPath(from, to, lane = 0, laneCount = 1) {
  if (!from || !to) return "";
  const span = to.x - from.x;
  if (!Number.isFinite(span) || span <= 0) return verticesToPath([from, to]);
  const slot = laneCount > 1 ? lane / (laneCount - 1) : 0.5;
  const x = from.x + span * (CORRIDOR_MIN_RATIO + (CORRIDOR_MAX_RATIO - CORRIDOR_MIN_RATIO) * slot);
  return verticesToPath([from, { x, y: from.y }, { x, y: to.y }, to]);
}

// Tudo o que o desenho de uma linha precisa: um traçado por divisão (cada um já
// partido entre temporada decidida e temporada em curso) e uma travessia por
// mudança de divisão.
export function buildTrackSegments(vertices, liveYear = null) {
  const runs = splitVerticesByBand(vertices);
  const segments = runs.map((run) => {
    const { settled, live } = splitVerticesAtLiveYear(run.vertices, liveYear);
    // Um vértice sozinho não é traçado — é só o ponto daquela temporada. Emitir
    // um path com um `M` solitário desenharia nada e ainda apareceria como linha
    // para quem lê o SVG.
    return {
      bandKey: run.bandKey,
      path: settled.length > 1 ? verticesToPath(settled) : "",
      livePath: live.length > 1 ? verticesToPath(live) : "",
    };
  });

  const crossings = [];
  for (let index = 1; index < runs.length; index += 1) {
    const previous = runs[index - 1].vertices;
    const from = previous[previous.length - 1];
    const to = runs[index].vertices[0];
    crossings.push({
      from,
      to,
      // Subiu se foi para uma posição mais ALTA na tela, ou seja, y menor.
      isPromotion: to.y < from.y,
      isLive: Number.isFinite(liveYear) && to.point?.year === liveYear,
      // O ano em que a equipe apareceu na divisão nova. É por ele que as verticais
      // são distribuídas em faixas, e a conta só fecha olhando TODAS as equipes de
      // uma vez — por isso o caminho é fechado fora daqui (ver `assignCorridorLanes`).
      boundaryYear: to.point?.year ?? null,
    });
  }

  return { segments, crossings };
}

// Quanto da altura do gráfico uma travessia precisa percorrer para ser considerada
// longa. É por DISTÂNCIA, não por quantas divisões ela pula: numa escada fechada
// quase toda troca é de um degrau só, e mesmo assim sair do 3º de uma divisão para
// o 10º da seguinte atravessa quase a tela inteira. Contar faixas puladas mediria
// a coisa errada.
export const CORRIDOR_LONG_RATIO = 0.3;

export function crossingIsLong(crossing, plotHeight, ratio = CORRIDOR_LONG_RATIO) {
  const from = crossing?.from;
  const to = crossing?.to;
  if (!from || !to || !Number.isFinite(plotHeight) || plotHeight <= 0) return false;
  const travel = Math.abs(to.y - from.y);
  return Number.isFinite(travel) && travel > plotHeight * ratio;
}

// Fecha o caminho de cada travessia distribuindo as verticais do mesmo ano em
// faixas. Precisa enxergar TODAS as linhas de uma vez — daí não morar dentro de
// `buildTrackSegments`, que só conhece uma equipe. Escreve `path` nas travessias
// que recebeu e devolve a mesma lista.
// Rebaixamento continua com a DIAGONAL direta, sem passar pelo corredor.
//
// A linha que desce reaparece na divisão de baixo, e sem nada ligando as duas
// pontas ela lê como dado faltando — a equipe brota do nada no meio do gráfico. A
// diagonal é a continuação natural: o tempo anda para a direita e a queda anda para
// baixo, então o traço acompanha as duas coisas de uma vez.
//
// PARA VOLTAR ATRÁS: `false` manda a descida pelo corredor também, como a subida.
// Para dar o mesmo tratamento à subida, é a mesma linha com o sinal trocado.
export const DIAGONAL_ON_DESCENT = true;

function usesDiagonal(crossing) {
  return DIAGONAL_ON_DESCENT && !crossing?.isPromotion;
}

export function assignCorridorLanes(lines) {
  const byYear = new Map();
  (lines ?? []).forEach((line) => {
    (line?.crossings ?? []).forEach((crossing) => {
      const year = crossing.boundaryYear;
      if (!byYear.has(year)) byYear.set(year, []);
      byYear.get(year).push({ teamId: line.track?.team_id ?? "", crossing });
    });
  });

  byYear.forEach((entries) => {
    // Ordem por equipe: a faixa de uma travessia não pode depender da ordem em que
    // as linhas foram montadas, senão ela dança a cada renderização.
    entries.sort((left, right) => String(left.teamId).localeCompare(String(right.teamId)));

    // As faixas são repartidas só entre quem de fato usa o corredor. Contar as
    // diagonais aqui deixaria buracos na distribuição, e as verticais restantes
    // ficariam amontoadas num canto da célula.
    const corridors = entries.filter((entry) => !usesDiagonal(entry.crossing));
    corridors.forEach((entry, index) => {
      entry.crossing.path = crossingCorridorPath(
        entry.crossing.from,
        entry.crossing.to,
        index,
        corridors.length,
      );
    });

    entries
      .filter((entry) => usesDiagonal(entry.crossing))
      .forEach((entry) => {
        entry.crossing.path = verticesToPath([entry.crossing.from, entry.crossing.to]);
      });
  });

  return lines ?? [];
}

// Retângulo da coluna da temporada em curso, quando ela está no eixo. É a faixa
// que separa visualmente "campeonato decidido" de "placar de agora".
export function liveColumnBox(payload, geometry, years) {
  if (!isLivePayload(payload) || !years.length) return null;
  const index = years.indexOf(payload.current_year);
  if (index < 0) return null;
  const left = geometry.getBoundaryX(index);
  if (!Number.isFinite(left) || !Number.isFinite(geometry.yearWidth)) return null;
  return { left, width: geometry.yearWidth, year: payload.current_year };
}

export function verticesToPath(vertices) {
  if (!vertices.length) return "";
  return vertices
    .map(({ x, y }, index) => `${index === 0 ? "M" : "L"} ${round(x)} ${round(y)}`)
    .join(" ");
}

function round(value) {
  return Math.round(value * 10) / 10;
}

// ---------------------------------------------------------------------------
// Hachuras — três tipos, com escopos diferentes
// ---------------------------------------------------------------------------

// (A) Antes de a FAMÍLIA existir: altura inteira do gráfico. São os anos pré-série
// que o eixo passou a mostrar de propósito.
export function familyPreSeriesBox(geometry, years, firstYear) {
  if (!years.length || !Number.isFinite(firstYear)) return null;
  const left = geometry.getBoundaryX(0);
  const right = geometry.boundaryOfYear(firstYear);
  return right > left ? { left, width: right - left } : null;
}

// (B) Categoria que surgiu depois da família: continua hachurada só na altura dela,
// do fim da hachura da família até a sua primeira temporada.
export function bandUnavailableBox(band, geometry, years, familyFirstYear) {
  const box = geometry.bands?.[band?.key];
  if (!box || !years.length) return null;
  const bandFirst = bandFirstSeason(band);
  if (!Number.isFinite(bandFirst) || !Number.isFinite(familyFirstYear)) return null;
  const left = geometry.boundaryOfYear(familyFirstYear);
  const right = geometry.boundaryOfYear(bandFirst);
  return right > left ? { left, top: box.top, width: right - left, height: box.height } : null;
}

export function bandFirstSeason(band) {
  let min = null;
  (band?.rows ?? []).forEach((row) =>
    (row.points ?? []).forEach((point) => {
      if (min === null || point.year < min) min = point.year;
    }),
  );
  return min;
}

export function bandStartDividerX(band, geometry, years) {
  if (!years.length) return null;
  const bandFirst = bandFirstSeason(band);
  if (!Number.isFinite(bandFirst) || bandFirst <= years[0] || bandFirst > years[years.length - 1]) {
    return null;
  }
  return geometry.boundaryOfYear(bandFirst);
}

// (C) Equipe que ainda não participava não ganha fundo próprio — seria ruído. Ela
// simplesmente não tem linha antes da estreia, e ganha etiqueta e marcador no
// primeiro ponto (ver buildEntryLabels e o círculo de estreia no AtlasChart).

// ---------------------------------------------------------------------------
// Cor de identidade da faixa
//
// O que a cor codifica é o DEGRAU da escada, não a marca: dentro de uma família
// todas as faixas seriam iguais se a cor viesse do carro. Production no topo,
// championship no meio, rookie na base — a mesma leitura em qualquer família.
// ---------------------------------------------------------------------------

export const BAND_ACCENT_PRODUCTION = "#a78bfa";
export const BAND_ACCENT_CHAMPIONSHIP = "#ff6b66";
export const BAND_ACCENT_ROOKIE = "#f2c46d";
// Endurance é o formato, não um degrau: as faixas de prova longa dividem o mesmo
// verde porque o que elas têm em comum é justamente correr longo.
export const BAND_ACCENT_ENDURANCE = "#4ade80";
// GT3 e GT4 têm cor própria na versão SPRINT. Elas são o degrau de championship das
// suas famílias, mas usar o vermelho do degrau deixava as duas famílias idênticas
// uma à outra — a única diferença entre elas seria o rótulo.
export const BAND_ACCENT_GT3 = "#38bdf8";
export const BAND_ACCENT_GT4 = "#2dd4bf";
export const BAND_ACCENT_DEFAULT = "#f5b877";

// Chave manda sobre categoria: `gt3`/`gt4` cairiam no vermelho do degrau se
// dependessem da regra por categoria. A exceção precisa ser consultada antes.
//
// A LMP2 já teve cor própria (rosa) por ser a única faixa da família dela. Não
// tem mais: ela é endurance, e endurance é uma cor só — o rosa a separava
// visualmente das outras faixas de resistência, que é justamente o oposto do que
// a paleta deveria dizer.
const BAND_ACCENT_BY_KEY = {
  gt3: BAND_ACCENT_GT3,
  gt4: BAND_ACCENT_GT4,
};

export function bandAccent(band) {
  const key = band?.key ?? "";
  const category = band?.category ?? "";
  if (BAND_ACCENT_BY_KEY[key]) return BAND_ACCENT_BY_KEY[key];
  if (category === "production_challenger" || key.startsWith("production")) return BAND_ACCENT_PRODUCTION;
  if (key.endsWith("_rookie")) return BAND_ACCENT_ROOKIE;
  if (category === "endurance") return BAND_ACCENT_ENDURANCE;
  return BAND_ACCENT_CHAMPIONSHIP;
}

// ---------------------------------------------------------------------------
// Etiquetas
// ---------------------------------------------------------------------------

export const LABEL_HEIGHT = 27;
export const LABEL_MIN_WIDTH = 96;
export const LABEL_MAX_WIDTH = 210;

// Tamanho base da fonte do chip e o que ele gasta fora do texto (padding + brasão
// + respiro entre os dois). Espelham o CSS do EntryLabel: se um mudar lá, muda aqui.
export const LABEL_FONT_SIZE = 11;
export const LABEL_CHROME_WIDTH = 54;
const CHAR_WIDTH_PER_FONT_PX = 0.6;
// Reduções de fonte tentadas quando o nome inteiro não cabe no espaço disponível.
// Encolher a letra é preferível a cortar o nome: "Mercedes-AMG" pequeno ainda é
// "Mercedes-AMG"; "Mercedes-A…" não é equipe nenhuma.
export const LABEL_FONT_SCALES = [1, 0.92, 0.84, 0.76, 0.7];

// Largura estimada do chip: nome + brasão + respiro. Não dá para medir texto sem
// renderizar, então a conta erra para MAIS — assim a caixa usada na anticolisão
// nunca é menor que o chip desenhado.
export function labelWidthFor(nome, fontScale = 1) {
  const textWidth = String(nome ?? "").length * LABEL_FONT_SIZE * fontScale * CHAR_WIDTH_PER_FONT_PX;
  return clamp(Math.ceil(textWidth + LABEL_CHROME_WIDTH), LABEL_MIN_WIDTH, LABEL_MAX_WIDTH);
}

// Uma etiqueta por linha, no ponto de estreia, encostada à esquerda dele. Para as
// equipes fundadoras isso cai dentro da faixa hachurada pré-série — que é justamente
// para isso que ela existe. A etiqueta é camada visual: não altera timelineLeft,
// yearWidth, nem a posição de ponto algum.
// Uma etiqueta por linha, no ponto de estreia, encostada à esquerda dele. Para as
// equipes fundadoras isso cai dentro da faixa hachurada pré-série — que é justamente
// para isso que ela existe. A etiqueta é camada visual: não altera timelineLeft,
// yearWidth, nem a posição de ponto algum.
export function buildEntryLabels(tracks, geometry, years, payload) {
  if (!years.length) return [];
  const bandByKey = new Map((payload?.bands ?? []).map((band) => [band.key, band]));
  // Fundadora = estreou na primeira temporada do PRÓPRIO campeonato — não da
  // família. Cada categoria nasce no seu ano, e a formação inaugural dela é um
  // marco que merece ficar na tela. Quem chegou com a categoria já em andamento
  // aparece no hover da própria linha. Ver `placeEntryLabels`.
  const bandFirstSeasons = new Map(
    (payload?.bands ?? []).map((band) => [band.key, bandFirstSeason(band)]),
  );
  const labels = [];

  tracks.forEach((track) => {
    ["regular", "special"].forEach((lineKey) => {
      const points = (track.points ?? []).filter((point) => point.slot === lineKey);
      const first = points[0];
      if (!first) return;
      if (bandByKey.get(first.band_key)?.is_special) return;
      const pointX = geometry.boundaryOfYear(first.year);
      const targetY = slotY(geometry, first.band_key, first.position);
      if (!Number.isFinite(pointX) || !Number.isFinite(targetY)) return;
      labels.push({
        key: `${track.team_id}-${lineKey}-${first.year}`,
        team_id: track.team_id,
        nome: track.nome,
        cor: track.cor_display ?? track.cor_primaria,
        band_key: first.band_key,
        year: first.year,
        isFounder: first.year === bandFirstSeasons.get(first.band_key),
        pointX,
        // Borda direita ideal: um respiro antes do ponto que a etiqueta nomeia.
        idealRight: pointX - LABEL_GAP,
        width: labelWidthFor(track.nome),
        height: LABEL_HEIGHT,
        targetY,
      });
    });
  });

  return placeEntryLabels(labels, geometry);
}

// ---------------------------------------------------------------------------
// Colocação das etiquetas
//
// A unidade de colocação é o GRUPO: todas as etiquetas de um mesmo campeonato que
// estreiam no mesmo ano dividem uma única borda direita e formam uma coluna. Dentro
// do grupo só o Y varia; se o grupo não couber, o GRUPO INTEIRO muda de coluna —
// nunca uma etiqueta isolada, que era o que tirava a Backmesa da fila dos outros
// cinco fundadores da Production.
//
// A busca é determinística (coluna × layout vertical) e o contrato é explícito:
// ou a etiqueta tem um retângulo livre, ou fica `unresolved` e não é desenhada.
// Não existe caminho que aceite sobreposição.
// ---------------------------------------------------------------------------

// Respiro entre a etiqueta e as bordas da faixa do campeonato.
export const LABEL_BAND_PADDING = 4;

export function rectFromRightCenter({ right, centerY, width, height }) {
  return {
    left: right - width,
    right,
    top: centerY - height / 2,
    bottom: centerY + height / 2,
    centerY,
    width,
    height,
  };
}

export function intersects(a, b) {
  return a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top;
}

// Layout vertical de um grupo numa dada coluna. Preserva a ORDEM dos pontos de
// entrada (a etiqueta de quem está acima continua acima) e devolve null quando o
// grupo não cabe na faixa ou cruza algo já colocado.
function layoutGroup(group, right, band, gap, placedRects, width) {
  const height = group[0].height;
  // Y é o CENTRO da etiqueta, então os limites já descontam metade da altura — é o
  // que impedia a última etiqueta de sair cortada pela base da faixa.
  const minCenter = band.top + LABEL_BAND_PADDING + height / 2;
  const maxCenter = band.bottom - LABEL_BAND_PADDING - height / 2;
  const step = height + gap;
  if (maxCenter < minCenter) return null;
  if ((group.length - 1) * step > maxCenter - minCenter) return null;

  const ordered = [...group].sort((left, rightLabel) => left.targetY - rightLabel.targetY);

  // Desce respeitando o passo mínimo...
  const centers = [];
  let previous = -Infinity;
  ordered.forEach((label) => {
    centers.push(Math.max(clamp(label.targetY, minCenter, maxCenter), previous + step));
    previous = centers[centers.length - 1];
  });
  // ...e sobe o que estourou a base, mantendo a ordem intacta.
  let ceiling = maxCenter;
  for (let index = centers.length - 1; index >= 0; index -= 1) {
    centers[index] = Math.min(centers[index], ceiling);
    ceiling = centers[index] - step;
  }
  if (centers[0] < minCenter) return null;

  const rects = centers.map((centerY) => rectFromRightCenter({ right, centerY, width, height }));
  if (rects.some((rect) => placedRects.some((other) => intersects(rect, other)))) return null;

  return ordered.map((label, index) => ({ label, rect: rects[index] }));
}

export function placeEntryLabels(labels, geometry, gap = 6) {
  const groups = new Map();
  labels.forEach((label) => {
    const groupKey = `${label.band_key}#${label.year}`;
    if (!groups.has(groupKey)) groups.set(groupKey, []);
    groups.get(groupKey).push(label);
  });

  // Ordem determinística: campeonato de cima para baixo, e dentro dele da estreia
  // mais antiga para a mais nova — assim os fundadores reivindicam a coluna ideal.
  const ordered = [...groups.values()].sort((left, right) => {
    const leftBox = geometry.bands?.[left[0].band_key];
    const rightBox = geometry.bands?.[right[0].band_key];
    return (leftBox?.top ?? 0) - (rightBox?.top ?? 0) || left[0].year - right[0].year;
  });

  const founderRects = [];
  const result = [];

  function place(group, obstacles, collect) {
    const band = geometry.bands?.[group[0].band_key];
    if (!band) {
      group.forEach((label) => result.push({ ...label, unresolved: true }));
      return;
    }

    const idealRight = group[0].idealRight;
    // Beirada esquerda do gráfico. A etiqueta cresce PARA A ESQUERDA, então numa
    // categoria com muitos anos — janela larga, célula estreita — a faixa pré-série
    // não tem os ~200px do chip e ele saía cortado pela borda do card. Aqui o
    // limite é explícito, e o que cede é o TAMANHO DA LETRA: o nome inteiro é
    // inegociável, tanto que a última escala é aceita mesmo sem caber, encostando
    // na beirada em vez de abreviar a equipe.
    const leftLimit = (geometry.timelineLeft ?? 0) + LABEL_BAND_PADDING;
    const available = Math.max(idealRight - leftLimit, 0);

    const tried = new Set();
    for (const [index, fontScale] of LABEL_FONT_SCALES.entries()) {
      // Largura do grupo: a maior do conjunto naquela escala, para a coluna ficar
      // alinhada e nenhum nome do grupo sobrar para fora.
      const width = Math.max(...group.map((label) => labelWidthFor(label.nome, fontScale)));
      const isLastScale = index === LABEL_FONT_SCALES.length - 1;
      if (width > available && !isLastScale) continue;

      for (let column = 0; column < MAX_LABEL_COLUMNS; column += 1) {
        // O recuo de coluna nunca ultrapassa a beirada: se não há espaço à
        // esquerda, o grupo fica onde está e tenta a próxima escala.
        const right = Math.max(idealRight - column * LABEL_COLUMN_SHIFT, leftLimit + width);
        const attempt = `${width}@${right}`;
        if (tried.has(attempt)) continue;
        tried.add(attempt);

        const layout = layoutGroup(group, right, band, gap, obstacles, width);
        if (!layout) continue;

        layout.forEach(({ label, rect }) => {
          if (collect) obstacles.push(rect);
          result.push({
            ...label,
            width,
            fontScale,
            right: rect.right,
            renderY: rect.centerY,
            column,
            unresolved: false,
          });
        });
        return;
      }
    }

    // Nenhuma coluna, em nenhuma largura, ofereceu retângulo livre. A etiqueta fica
    // sem lugar em vez de ser desenhada por cima de outra — a equipe continua
    // identificada pelo marcador de estreia e pelo card lateral.
    group.forEach((label) => result.push({ ...label, unresolved: true }));
  }

  // As fundadoras são colocadas primeiro e disputam espaço entre si: são elas que
  // ficam permanentemente na tela, formando a coluna da esquerda.
  ordered.filter((group) => group[0].isFounder).forEach((group) => place(group, founderRects, true));

  // As demais só aparecem no hover da própria linha, uma de cada vez. Por isso
  // desviam apenas das fundadoras — entre si não podem colidir, já que nunca estão
  // visíveis ao mesmo tempo. Assim quase todas caem no lugar ideal, coladas ao
  // próprio ponto de estreia.
  ordered.filter((group) => !group[0].isFounder).forEach((group) => place(group, [...founderRects], false));

  return result;
}

// A etiqueta precisa de conector quando não está encostada no seu próprio ponto —
// seja porque desceu/subiu, seja porque o grupo recuou de coluna.
export function labelIsDisplaced(label) {
  if (label.unresolved) return false;
  return Math.abs(label.renderY - label.targetY) > 2 || Math.abs(label.right - label.idealRight) > 1;
}

// Conectores só depois de TODAS as posições resolvidas: o traço vai da borda direita
// do chip até a beirada do marcador de estreia, nunca até o centro dele.
export const ENTRY_MARKER_RADIUS = 4;

export function labelConnector(label) {
  if (!labelIsDisplaced(label)) return null;
  // Etiqueta encostada na beirada esquerda pode terminar DEPOIS do próprio ponto;
  // aí o conector viraria um traço para trás, e não há o que ligar.
  if (label.right >= label.pointX - ENTRY_MARKER_RADIUS) return null;
  return {
    x1: label.right,
    y1: label.renderY,
    x2: label.pointX - ENTRY_MARKER_RADIUS,
    y2: label.targetY,
  };
}
// ---------------------------------------------------------------------------
// Trilhas
// ---------------------------------------------------------------------------

// Uma trilha por equipe, com todos os pontos que caem dentro do eixo visível.
// Igual ao buildTeamTracks do v1, menos o filtro por geometria: aqui a geometria
// depende do tamanho medido do card e ainda não existe na hora de montar as trilhas.
export function buildAtlasTracks(payload, years) {
  const yearSet = new Set(years ?? []);
  // A temporada em andamento não é coluna (ver `axisEndYear`) e mesmo assim é
  // desenhada, na borda direita. Sem admiti-la aqui a linha viva sumiria junto
  // com a coluna — o ponto existe no payload e seria descartado no filtro.
  if (isLivePayload(payload) && yearSet.size) yearSet.add(payload.current_year);
  const tracks = new Map();

  (payload?.bands ?? []).forEach((band) => {
    (band.rows ?? []).forEach((row) => {
      if (!tracks.has(row.team_id)) {
        tracks.set(row.team_id, {
          team_id: row.team_id,
          nome: row.nome,
          nome_curto: row.nome_curto,
          // `cor_primaria` desenha a LINHA; `cor_display` desenha nome, brasão,
          // borda e marcador, com piso de luminosidade mais alto.
          cor_primaria: getReadableWorldTeamColor(row.cor_primaria),
          cor_display: ensureMinimumLuminance(getReadableWorldTeamColor(row.cor_primaria)),
          cor_secundaria: row.cor_secundaria,
          base_position: row.base_position,
          points: [],
        });
      }
      const track = tracks.get(row.team_id);
      (row.points ?? []).forEach((point) => {
        if (!yearSet.has(point.year)) return;
        track.points.push({ ...point, band_key: band.key, team_id: row.team_id });
      });
    });
  });

  return Array.from(tracks.values())
    .filter((track) => track.points.length > 0)
    .map((track) => ({
      ...track,
      points: track.points.sort(
        (left, right) => left.year - right.year || slotOrder(left.slot) - slotOrder(right.slot),
      ),
    }));
}

function slotOrder(slot) {
  return slot === "special" ? 2 : 1;
}

// ---------------------------------------------------------------------------
// Rankings laterais
// ---------------------------------------------------------------------------

export function positionAtYear(row, year) {
  const point = (row.points ?? []).find((item) => item.year === year);
  return point ? Math.max(point.position ?? 1, 1) : null;
}

export function pointAtYear(row, year) {
  return (row.points ?? []).find((item) => item.year === year) ?? null;
}

// A coluna do ano corrente só é "viva" quando o backend diz que a temporada
// começou e não terminou. Fora disso `current_year` é apenas o último ano
// arquivado, e a tabela lateral fala de um campeonato já decidido.
export function isLivePayload(payload) {
  return Boolean(payload?.in_progress) && Number.isFinite(payload?.current_year);
}

// Ano mais recente com dado, até o último ano visível — é a temporada que a tabela
// lateral mostra.
export function bandReferenceYear(band, lastYear) {
  let latest = null;
  (band?.rows ?? []).forEach((row) =>
    (row.points ?? []).forEach((point) => {
      if (point.year <= lastYear && (latest === null || point.year > latest)) latest = point.year;
    }),
  );
  return latest;
}

export function totalTitles(row) {
  return (row.titles ?? []).reduce((sum, title) => sum + (title.count ?? 0), 0);
}

// Títulos DA CATEGORIA em que a equipe está agora — não os da carreira dela na
// família inteira. Um tetracampeão da Rookie que subiu para a Championship chega
// lá sem troféu nenhum ao lado do nome, e recupera os quatro se for rebaixado de
// volta. O troféu ali diz "mandou nesta divisão", não "já ganhou alguma coisa".
export function bandTitles(row, bandKey) {
  const entry = (row?.titles ?? []).find((title) => title.band_key === bandKey);
  return Math.max(entry?.count ?? 0, 0);
}

// Um card de ranking por campeonato (blocos especiais ficam de fora, como no v1).
//
// NÃO recebe `years` de propósito: os cards mostram a temporada atual, e isso não
// pode depender do que está visível no gráfico. O ano de referência sai do próprio
// payload (última temporada disputada), nunca de índice de coluna nem do primeiro
// ano exibido.
export function buildRankingCards(payload) {
  if (!payload) return [];
  const lastYear = lastSeriesYear(payload);
  if (!Number.isFinite(lastYear)) return [];

  const live = isLivePayload(payload);
  // Ano contra o qual a variação é medida: a última temporada DECIDIDA. Comparar
  // com `referenceYear - 1` daria bobagem quando a equipe pulou um ano.
  const baselineYear = live ? payload.last_completed_year : null;

  return (payload.bands ?? [])
    .filter((band) => !band.is_special)
    .map((band) => {
      const referenceYear = bandReferenceYear(band, lastYear);
      // Um card só é "ao vivo" se a temporada em curso for justamente a que ele
      // está mostrando. Uma faixa que parou de existir há três anos continua
      // exibindo o último ano dela, decidido, sem selo nenhum.
      const isLive = live && referenceYear === payload.current_year;
      const rows = Number.isFinite(referenceYear)
        ? (band.rows ?? [])
            .filter((row) => Number.isFinite(positionAtYear(row, referenceYear)))
            .map((row) => {
              const point = pointAtYear(row, referenceYear);
              const position = Math.max(point?.position ?? 1, 1);
              // Posição na mesma faixa na última temporada decidida. `null` quando
              // a equipe não estava aqui — estreante ou recém-chegada de outra
              // divisão —, e aí não há variação a mostrar, só a estreia.
              const previous = isLive && Number.isFinite(baselineYear)
                ? positionAtYear(row, baselineYear)
                : null;
              return {
                team_id: row.team_id,
                nome: row.nome,
                // Linha de ranking é texto miúdo sobre fundo escuro: usa a cor de
                // exibição, não a bruta.
                cor: ensureMinimumLuminance(getReadableWorldTeamColor(row.cor_primaria)),
                position,
                titles: bandTitles(row, band.key),
                // Pontos e vitórias só aparecem na tabela ao vivo: num ano fechado
                // quem conta a história é a posição final, não o placar.
                points: isLive ? Math.max(point?.points ?? 0, 0) : null,
                wins: isLive ? Math.max(point?.wins ?? 0, 0) : null,
                // Positivo = subiu. É variação de POSIÇÃO, então o sinal é o
                // inverso da diferença dos números.
                delta: Number.isFinite(previous) ? previous - position : null,
                isNewInBand: isLive && !Number.isFinite(previous),
                row,
              };
            })
            .sort((left, right) => left.position - right.position)
        : [];
      return {
        key: band.key,
        band,
        label: band.label,
        referenceYear,
        isLive,
        baselineYear: isLive ? baselineYear : null,
        rows,
      };
    });
}
