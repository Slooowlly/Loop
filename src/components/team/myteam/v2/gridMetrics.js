// Métricas e geometria da aba Minha Equipe v2.
//
// REGRA DE OURO: este arquivo é PURO — entra payload, sai número. Nada de React,
// nada de i18n, nada de formatação de moeda. Os componentes traduzem e formatam.
// É o que torna os quatro gráficos testáveis sem montar a árvore.
//
// Fronteira de dados: tudo aqui sai do que a aba JÁ busca — `get_teams_standings`,
// `get_team_finance_report` e o `season` do store. Nenhum campo novo foi pedido ao
// backend, e isso tem duas consequências que não dá para contornar com conta:
//
//   1. `TeamStanding` não traz `presenca_publica` nem os salários das outras equipes.
//      Presença e folha salarial NÃO têm média de grid — quem desenhar esses dois
//      medidores desenha sem régua. Inventar uma referência ali seria fabricar dado.
//   2. Os eixos do radar normalizam pelo MÁXIMO DO GRID, não por um teto absoluto.
//      A forma do polígono é posição relativa dentro da categoria, não nota de 0 a 100.

import { clamp } from "../teamMetrics";

// Eixos do radar, na ordem em que são desenhados (topo, sentido horário). São as
// cinco dimensões que TeamStanding entrega para TODAS as equipes — é por isso que
// presença pública não está aqui.
export const RADAR_AXES = [
  { key: "car_level", max: 10 },
  { key: "confiabilidade", max: 100 },
  { key: "pit_crew_quality", max: 100 },
  { key: "cash_balance", max: null },
  { key: "pontos", max: null },
];

// Os três medidores técnicos do painel "O carro". `max` é a escala natural do
// campo — a régua da média do grid é posicionada nessa mesma escala.
export const CAR_METERS = [
  { key: "car_level", max: 10 },
  { key: "confiabilidade", max: 100 },
  { key: "pit_crew_quality", max: 100 },
];

const RADAR_FIELDS = RADAR_AXES.map((axis) => axis.key);

// Leituras agregadas do grid: média, máximo e líder. `count` é o número de equipes
// com payload utilizável — com menos de duas não existe comparação e todos os
// gráficos comparativos se declaram vazios em vez de desenhar uma régua de si mesmo.
export function gridSummary(teams) {
  const rows = Array.isArray(teams) ? teams.filter(Boolean) : [];
  if (rows.length < 2) return { count: rows.length, hasGrid: false, average: {}, max: {}, leader: null };

  const average = {};
  const max = {};
  for (const field of RADAR_FIELDS) {
    const values = rows.map((row) => Number(row?.[field]) || 0);
    average[field] = values.reduce((sum, value) => sum + value, 0) / values.length;
    max[field] = Math.max(...values);
  }

  // O líder do comparativo é quem lidera o CAMPEONATO (posição 1), não quem tem mais
  // caixa — o polígono verde do radar é "a forma de quem está ganhando".
  const leader = [...rows].sort((a, b) => (a.posicao ?? 999) - (b.posicao ?? 999))[0] ?? null;

  return { count: rows.length, hasGrid: true, average, max, leader };
}

// Posição da equipe num campo específico (1 = melhor). Serve para dizer "último do
// grid" sem que o componente refaça a ordenação.
export function rankInGrid(teams, teamId, field) {
  const rows = Array.isArray(teams) ? teams.filter(Boolean) : [];
  if (rows.length === 0 || !teamId) return 0;
  const ordered = [...rows].sort((a, b) => (Number(b?.[field]) || 0) - (Number(a?.[field]) || 0));
  const index = ordered.findIndex((row) => row.id === teamId);
  return index < 0 ? 0 : index + 1;
}

// Os três medidores do painel técnico, cada um com o valor da equipe e a régua da
// média do grid na MESMA escala. Sem grid, `average` vem null e o componente omite
// a régua em vez de desenhar uma marca em zero.
export function carMeterReadings(team, teams) {
  const summary = gridSummary(teams);
  return CAR_METERS.map((meter) => {
    const value = Number(team?.[meter.key]) || 0;
    const average = summary.hasGrid ? summary.average[meter.key] : null;
    return {
      key: meter.key,
      value,
      max: meter.max,
      average,
      percent: clamp((value / meter.max) * 100, 0, 100),
      averagePercent: average === null ? null : clamp((average / meter.max) * 100, 0, 100),
      rank: rankInGrid(teams, team?.id, meter.key),
      gridSize: summary.count,
      // Tom do medidor: comparado à média, não a um patamar absoluto. Um 56 de
      // confiabilidade é bom ou ruim dependendo de contra quem se corre.
      tone: average === null ? "neutral" : value >= average * 1.05 ? "good" : value >= average * 0.9 ? "warn" : "bad",
    };
  });
}

// ---------------------------------------------------------------------------
// Dossiê da dupla
// ---------------------------------------------------------------------------

// Quem são os seus dois pilotos, medidos contra a categoria em que correm.
//
// O v1 (e a primeira versão desta v2) mostrava nome, bandeira e salário — e mais
// nada. A pergunta que a aba de gestão precisa responder é outra: *esse piloto é bom
// para esta categoria, e ele devolve o que custa?* `DriverSummary` já traz skill,
// mídia, idade, pontos, vitórias e pódios de TODOS os pilotos da categoria; a média
// vira a régua, do mesmo jeito que o painel do carro faz com o grid de equipes.
//
// `custoPorPonto` é a leitura de gestão: salário anual dividido pelos pontos da
// temporada. Sem pontos ainda ele é `null` — dividir por zero aqui produziria um
// número infinito que a tela mostraria como se fosse informação.
export function lineupDossier({ drivers, rows }) {
  const pool = Array.isArray(drivers) ? drivers.filter(Boolean) : [];
  const averages = pool.length > 0
    ? {
        skill: mean(pool.map((driver) => Number(driver.skill) || 0)),
        midia: mean(pool.map((driver) => Number(driver.midia) || 0)),
        idade: mean(pool.map((driver) => Number(driver.idade) || 0)),
      }
    : null;

  return {
    hasGrid: pool.length >= 3,
    poolSize: pool.length,
    averages,
    drivers: rows.map((row) => {
      const source = row.driver ?? null;
      const pontos = Math.max(0, Number(source?.pontos) || 0);
      const skill = Number(source?.skill) || 0;
      const midia = Number(source?.midia) || 0;
      return {
        ...row,
        hasDetail: Boolean(source),
        age: Math.max(0, Number(source?.idade) || 0),
        skill,
        midia,
        pontos,
        vitorias: Math.max(0, Number(source?.vitorias) || 0),
        podios: Math.max(0, Number(source?.podios) || 0),
        championshipPosition: Math.max(0, Number(source?.posicao_campeonato) || 0),
        isRookie: Boolean(source?.is_estreante),
        injury: source?.lesao_ativa_tipo ?? null,
        costPerPoint: pontos > 0 && row.salary > 0 ? row.salary / pontos : null,
        skillRank: rankInPool(pool, source?.id, "skill"),
        midiaRank: rankInPool(pool, source?.id, "midia"),
      };
    }),
  };
}

function rankInPool(pool, id, field) {
  if (!id || pool.length === 0) return 0;
  const ordered = [...pool].sort((a, b) => (Number(b?.[field]) || 0) - (Number(a?.[field]) || 0));
  const index = ordered.findIndex((row) => row.id === id);
  return index < 0 ? 0 : index + 1;
}

// ---------------------------------------------------------------------------
// Gráfico 1 — projeção de caixa até o fim da temporada
// ---------------------------------------------------------------------------

// Área de plotagem a partir das dimensões REAIS medidas do card.
//
// O erro da primeira versão foi desenhar com viewBox fixo e `width: 100%`: o SVG
// mantém a proporção, então numa tela de 1920 um viewBox de 600x172 virava um
// gráfico de 470px de altura. Aqui a largura do viewBox É a largura medida (escala
// 1:1) e a altura é escolhida por nós — o gráfico estica na horizontal e não cresce
// na vertical. É a mesma correção que o atlas v2 fez pelo mesmo motivo.
export function chartView(width, height, { left = 46, right = 18, top = 18, bottom = 28 } = {}) {
  const boxWidth = Math.max(320, Math.round(Number(width) || 0) || 600);
  const boxHeight = Math.max(120, Math.round(Number(height) || 0) || 180);
  return {
    width: boxWidth,
    height: boxHeight,
    left,
    right: boxWidth - right,
    top,
    bottom: boxHeight - bottom,
  };
}

export const PROJECTION_VIEW = chartView(600, 176);

// A reserva mínima é o custo de DUAS rodadas de operação — não um número redondo
// inventado. É o piso abaixo do qual a equipe não paga o próximo fim de semana.
export const RESERVE_ROUNDS = 2;

// A tira com a linha do caixa acumulado foi REMOVIDA. Ela era uma segunda escala
// atravessando o topo do card, quase reta (o saldo anda 3% por rodada) e sem eixo
// para ancorar a leitura — decoração cara. O acumulado agora é um rótulo por coluna,
// na base dela, onde a pergunta "e o caixa depois dessa rodada?" realmente aparece.

// O ledger da temporada: quanto CADA rodada deixou.
//
// A primeira versão desenhava o caixa absoluto ao longo do ano, e ele é sempre
// plano — o saldo anda 3% por rodada, então a linha subia de canto a canto sem
// mostrar nada. O sinal está no resultado POR RODADA, que varia 100% de uma etapa
// para a outra: é ele que diz qual fim de semana pagou e qual doeu.
//
// Três tipos de coluna: as rodadas corridas (verde/vermelho, dado real), as que
// faltam (vazadas, extrapoladas pelo resultado médio) e o prêmio de construtores
// (verde, só no encerramento). A linha fina na tira de cima é o caixa acumulado,
// para quem quiser a leitura de saldo sem trocar de gráfico.
//
// `season` é o SeasonSummary do store (`rodada_atual`, `total_rodadas`); `report` é
// o payload de `get_team_finance_report`. Sem histórico ou sem calendário não há o
// que desenhar — devolve `hasData: false` e a tela mostra o estado vazio.
export function seasonLedger({ report, team, season, view = PROJECTION_VIEW }) {
  const timeline = Array.isArray(report?.cash_timeline) ? report.cash_timeline : [];
  // `cash_timeline` são as ÚLTIMAS 12 rodadas registradas, SEM filtro de temporada
  // (financas.rs corta por `entries.len() - TIMELINE_MAX`). Já `season` do mesmo
  // payload é filtrado por `season_number == current_season`. Misturar os dois pinta
  // rodadas do ano passado como se fossem deste, e a média por rodada — calculada
  // sobre a contagem da temporada — deixa de bater com as colunas desenhadas.
  const currentSeason = Number(report?.season?.season_number);
  const sameSeason = (point) => !Number.isFinite(currentSeason) || point.season_number === currentSeason;
  // A linha de ENCERRAMENTO não é uma rodada: é o lançamento do prêmio. Ela vira a
  // coluna de prêmio, nunca uma etapa do calendário.
  const recorded = timeline.filter((point) => !point.is_season_close && sameSeason(point));
  const totalRounds = Math.max(0, Number(season?.total_rodadas) || 0);
  if (recorded.length === 0 || totalRounds === 0) return { hasData: false };

  // Resultado médio por rodada SEM o prêmio já creditado — senão a extrapolação das
  // rodadas que faltam embutiria o prêmio que a coluna final soma de novo.
  const seasonRounds = Math.max(1, Number(report?.season?.round) || recorded.length);
  const seasonNet = (Number(report?.season?.net) || 0) - (Number(report?.season?.constructor_prize_income) || 0);
  const avgNet = seasonNet / seasonRounds;
  const avgExpenses = (Number(report?.season?.expenses_total) || 0) / seasonRounds;
  const reserve = avgExpenses * RESERVE_ROUNDS;
  const prize = Math.max(0, Number(report?.expected_constructor_prize) || 0);

  // O número da rodada vem do próprio lançamento, não da posição na lista: a janela
  // do backend guarda só as 12 últimas, então numerar por índice renomearia a R14
  // para R1 numa temporada longa.
  const real = recorded.map((point, index) => ({
    key: `real-${index + 1}`,
    round: Math.max(1, Number(point.round) || index + 1),
    kind: "real",
    net: Number(point.net) || 0,
    cash: Number(point.cash_balance) || 0,
  }));

  // Quantas rodadas já foram: a CONTAGEM da temporada (`season.round`) manda, porque
  // a janela do timeline pode ter cortado as primeiras. `rodada_atual` do store entra
  // como piso para o caso de o histórico financeiro estar atrás do calendário.
  const lastRound = Math.max(seasonRounds, real[real.length - 1].round, Number(season?.rodada_atual) || 0);
  const remaining = Math.max(0, totalRounds - lastRound);
  let runningCash = real[real.length - 1].cash;
  const projected = [];
  for (let step = 1; step <= remaining; step += 1) {
    runningCash += avgNet;
    projected.push({
      key: `projected-${step}`,
      round: lastRound + step,
      kind: "projected",
      net: avgNet,
      cash: runningCash,
    });
  }

  const columns = [...real, ...projected];
  if (prize > 0) {
    columns.push({ key: "prize", round: null, kind: "prize", net: prize, cash: runningCash + prize });
  }
  const finalCash = columns[columns.length - 1].cash;

  // Escala das colunas: só os resultados de RODADA. O prêmio costuma ser múltiplo de
  // uma etapa e, se entrasse na escala, achataria justamente a variação que o
  // gráfico existe para mostrar — ele é desenhado clampado e sempre com o valor
  // escrito ao lado.
  const roundNets = columns.filter((column) => column.kind !== "prize").map((column) => column.net);
  const maxUp = Math.max(0, ...roundNets);
  const maxDown = Math.max(0, ...roundNets.map((net) => -net));
  // Temporada inteira no vermelho: `maxUp` é 0, a linha do zero encosta no topo e não
  // sobra altura NENHUMA acima dela — a coluna do prêmio, que é justamente o que pode
  // salvar o ano, virava um traço de 2px. Reservamos para ela, no máximo, a altura da
  // pior rodada: o prêmio fica visível e as rodadas mantêm a escala delas.
  const prizeRoom = prize > 0 ? Math.min(prize, Math.max(maxDown, maxUp)) : 0;
  const upSpan = Math.max(maxUp, prizeRoom);
  const span = Math.max(1, upSpan + maxDown);

  const plotTop = view.top;
  const plotBottom = view.bottom;
  const plotHeight = Math.max(1, plotBottom - plotTop);
  const zeroY = plotTop + plotHeight * (upSpan / span);
  const yFor = (net) => zeroY - (net / span) * plotHeight;

  const slot = (view.right - view.left) / columns.length;
  const barWidth = clamp(slot * 0.62, 4, 44);

  // A reserva não vira faixa nem linha: ela marca a COLUNA da rodada em que o caixa
  // furaria o piso. Desenhar a reserva no eixo custava metade da altura do card para
  // dizer "está tudo bem" na maioria dos saves, onde ela fica a um abismo do saldo.
  const breachColumn = columns.find((column) => column.kind !== "prize" && column.cash < reserve) ?? null;

  const laidOut = columns.map((column, index) => {
    const centerX = view.left + slot * index + slot / 2;
    const rawY = yFor(column.net);
    const clampedY = Math.max(plotTop, rawY);
    const positive = column.net >= 0;
    const y = positive ? clampedY : zeroY;
    const height = Math.max(2, positive ? zeroY - clampedY : Math.min(plotBottom, rawY) - zeroY);
    return {
      ...column,
      x: round2(centerX - barWidth / 2),
      centerX: round2(centerX),
      width: round2(barWidth),
      y: round2(y),
      height: round2(height),
      positive,
      // Duas leituras por coluna, em lados opostos da linha do zero:
      //   `deltaY` — a ponta LIVRE da barra, onde vai quanto a rodada rendeu ou custou;
      //   `cashY`  — o lado vazio da linha do zero, onde vai o caixa DEPOIS da rodada.
      // Ficam sempre separadas porque a barra ocupa só um dos lados: perdeu, a barra
      // desce e o total sobra em cima; ganhou, a barra sobe e o total sobra embaixo.
      deltaY: round2(positive ? y - 6 : y + height + 12),
      cashY: round2(positive ? zeroY + 13 : zeroY - 6),
      // O prêmio estourou o topo da escala das rodadas e foi cortado: o componente
      // marca isso em vez de fingir que a altura é comparável.
      clamped: rawY < plotTop,
      isBreach: breachColumn ? column.key === breachColumn.key : false,
    };
  });

  return {
    hasData: true,
    view,
    columns: laidOut,
    zeroY: round2(zeroY),
    plotTop: round2(plotTop),
    plotBottom: round2(plotBottom),
    position: Math.max(0, Number(report?.current_position) || 0),
    gridSize: Math.max(0, Number(report?.grid_size) || 0),
    // X da divisória entre a última rodada e a coluna do prêmio.
    prizeDividerX: prize > 0 && columns.length > 1 ? round2(view.left + slot * (columns.length - 1)) : null,
    // Rótulo de rodada em toda coluna só cabe até uma dezena delas; acima disso o
    // componente pula de N em N.
    labelEvery: Math.max(1, Math.ceil(columns.length / 12)),
    showValues: barWidth >= 30,
    roundsDone: real.length,
    totalRounds,
    remaining,
    avgNet,
    reserve,
    prize,
    finalCash,
    breach: breachColumn,
  };
}


// ---------------------------------------------------------------------------
// Gráfico 2 — radar contra a média do grid e o líder
// ---------------------------------------------------------------------------

// O radar é quadrado e pequeno de propósito — quem o esticava para a largura do card
// o transformava num pentágono de 600px. A caixa é mais LARGA que alta porque os
// rótulos laterais precisam de espaço fora do círculo: com `size` quadrado eles eram
// cortados pela borda do viewBox ("ontos" no lugar de "Pontos").
export const RADAR_VIEW = { width: 240, height: 186, cx: 120, cy: 100, radius: 66, labelGap: 15 };

// Caixa do radar a partir da largura MEDIDA do card, como os gráficos da aba
// Dinheiro. O teto fixo de 260px que existia aqui deixava um pentágono de 130px
// boiando numa coluna de 640 — a forma até aparecia, mas "onde estou torto" não se
// lia nesse tamanho, e agora cada eixo ainda carrega o número ao lado do rótulo.
//
// A caixa é mais LARGA que alta porque os rótulos vivem FORA do círculo: a reserva
// lateral (`labelRoom`) é o que impede "Pontos" de virar "ontos" cortado na borda.
export function radarView(width, { max = 460, min = 240 } = {}) {
  const boxWidth = clamp(Math.round(Number(width) || 0) || min, min, max);
  const labelRoom = Math.round(boxWidth * 0.19);
  const radius = Math.round((boxWidth / 2 - labelRoom) * 0.94);
  const height = Math.round(radius * 2 + labelRoom * 2.1);
  return {
    width: boxWidth,
    height,
    cx: Math.round(boxWidth / 2),
    cy: Math.round(height / 2),
    radius,
    labelGap: Math.round(radius * 0.19),
  };
}

// Vetor unitário de cada eixo: o primeiro aponta para cima e os demais seguem em
// sentido horário, em passos iguais.
function axisUnit(index, total) {
  const angle = -Math.PI / 2 + (index * 2 * Math.PI) / total;
  return { x: Math.cos(angle), y: Math.sin(angle) };
}

function radarPoint(index, total, ratio, radius, view) {
  const unit = axisUnit(index, total);
  return {
    x: view.cx + unit.x * radius * ratio,
    y: view.cy + unit.y * radius * ratio,
  };
}

function toPolygon(points) {
  return points.map((point) => `${round2(point.x)},${round2(point.y)}`).join(" ");
}

function round2(value) {
  return Math.round(value * 100) / 100;
}


// ---------------------------------------------------------------------------
// Radar das 11 peças do carro — todas as equipes no mesmo desenho
// ---------------------------------------------------------------------------

// As 11 peças, na ordem estável de `PartType::ALL` no Rust. O eixo N precisa ser a
// mesma peça em todas as equipes, então a ordem é contrato, não preferência.
export const CAR_PART_KEYS = [
  "chassis",
  "engine",
  "front_wing",
  "rear_wing",
  "underbody",
  "sidepods",
  "cooling",
  "gearbox",
  "brakes",
  "suspension",
  "electronics",
];

// Nível de peça vai de 1 a 10 — escala ABSOLUTA do jogo, não o máximo do grid. É a
// diferença que importa aqui: um grid inteiro no nível 2 deve desenhar polígonos
// pequenos, e não polígonos cheios que sugerem carros no teto.
export const PART_LEVEL_MAX = 10;

// As 11 peças agrupadas em 5 ÁREAS por função. Onze eixos com quatro anéis viravam
// teia: o desenho tinha mais linha de grade do que informação, e ninguém compara
// onze direções de uma vez. Cinco áreas é o que o olho separa num relance — e as
// peças de cada uma sobem juntas na prática, então o agrupamento não esconde
// escolha nenhuma que o jogador realmente faça peça a peça.
export const CAR_PART_GROUPS = [
  { key: "structure", parts: ["chassis", "underbody"] },
  { key: "aero", parts: ["front_wing", "rear_wing", "sidepods"] },
  { key: "powertrain", parts: ["engine", "gearbox"] },
  { key: "running_gear", parts: ["brakes", "suspension"] },
  { key: "systems", parts: ["cooling", "electronics"] },
];

// Geometria do radar de peças: um polígono por equipe, na cor dela.
//
// O radar anterior tinha cinco eixos que misturavam grandezas — carro, confiança,
// pit crew, caixa e pontos — e só três polígonos (você, média, líder). Este responde
// outra pergunta, mais específica: em QUAL peça a minha equipe está atrás, e de quem.
// `car_level` é a média das onze, e média esconde exatamente isso.
export function carPartsRadar({ cars, playerTeamId, view = RADAR_VIEW }) {
  const rows = (Array.isArray(cars) ? cars.filter(Boolean) : []).filter(
    (car) => Array.isArray(car.parts) && car.parts.length > 0,
  );
  if (rows.length === 0) return { hasData: false };

  const total = CAR_PART_GROUPS.length;
  const at = (index, ratio, radius = view.radius) => radarPoint(index, total, ratio, radius, view);
  const levelOf = (car, key) => {
    const part = car.parts.find((entry) => entry.key === key);
    return clamp(Number(part?.level) || 0, 0, PART_LEVEL_MAX);
  };

  const teams = rows.map((car) => {
    // O nível da ÁREA é a média das peças dela. Média e não máximo: uma asa dianteira
    // no teto com a traseira no chão não é um pacote aerodinâmico de topo.
    const levels = CAR_PART_GROUPS.map((group) => mean(group.parts.map((key) => levelOf(car, key))));
    return {
      id: car.team_id ?? car.teamId,
      name: car.nome_curto || car.nome || "",
      fullName: car.nome || car.nome_curto || "",
      color: car.cor_primaria ?? "#8b949e",
      isPlayer: (car.team_id ?? car.teamId) === playerTeamId,
      levels,
      // Nível de cada peça solta, para quem quiser abrir a área.
      partLevels: Object.fromEntries(CAR_PART_KEYS.map((key) => [key, levelOf(car, key)])),
      average: mean(levels),
      polygon: toPolygon(levels.map((level, index) => at(index, level / PART_LEVEL_MAX))),
    };
  });

  // Média do grid por área: é contra ela que a leitura escrita mede o buraco.
  const gridAverage = CAR_PART_GROUPS.map((_, index) => mean(teams.map((team) => team.levels[index])));
  const gridMax = CAR_PART_GROUPS.map((_, index) => Math.max(...teams.map((team) => team.levels[index])));
  const player = teams.find((team) => team.isPlayer) ?? null;

  const axes = CAR_PART_GROUPS.map((group, index) => {
    const unit = axisUnit(index, total);
    return {
      key: group.key,
      parts: group.parts,
      spoke: at(index, 1),
      label: at(index, 1, view.radius + view.labelGap),
      anchor: Math.abs(unit.x) < 0.2 ? "middle" : unit.x > 0 ? "start" : "end",
      above: unit.y < -0.2,
      playerLevel: player ? player.levels[index] : null,
      averageLevel: gridAverage[index],
      maxLevel: gridMax[index],
      // Buraco contra o melhor do grid nessa área: é o que diz onde investir.
      gapToBest: player ? gridMax[index] - player.levels[index] : null,
    };
  });

  const ranked = player
    ? [...axes].filter((axis) => axis.gapToBest !== null).sort((a, b) => b.gapToBest - a.gapToBest)
    : [];

  return {
    hasData: true,
    view,
    teams,
    player,
    axes,
    // Dois anéis, não quatro: com cinco eixos a grade densa era o que mais aparecia
    // no desenho, e o meio-caminho basta para situar o polígono.
    rings: [0.5, 1].map((ratio) => toPolygon(CAR_PART_GROUPS.map((_, index) => at(index, ratio)))),
    // A área mais atrasada e a mais adiantada — a leitura acionável do desenho.
    weakest: ranked[0] ?? null,
    strongest: ranked[ranked.length - 1] ?? null,
  };
}

// ---------------------------------------------------------------------------
// Gráfico 4 — dispersão caixa × pontos
// ---------------------------------------------------------------------------

export const SCATTER_VIEW = chartView(600, 184, { left: 52 });

// Cada equipe é um ponto; a reta é a conversão MÉDIA de caixa em pontos no grid
// (mínimos quadrados). Quem está acima da reta pontua mais do que o dinheiro
// explica — é essa a leitura de gestão que a tabela ordenada nunca dá.
export function efficiencyScatter(teams, playerTeamId, view = SCATTER_VIEW) {
  const rows = (Array.isArray(teams) ? teams.filter(Boolean) : []).filter((row) => Number.isFinite(Number(row?.cash_balance)));
  if (rows.length < 3) return { hasData: false, reason: "grid" };

  const cashValues = rows.map((row) => Number(row.cash_balance) || 0);
  const pointValues = rows.map((row) => Number(row.pontos) || 0);
  const cashMin = Math.min(...cashValues);
  const cashMax = Math.max(...cashValues);
  const pointsMax = Math.max(...pointValues);
  // Antes da primeira pontuação da temporada o grid inteiro está no zero: a reta de
  // conversão é horizontal, todo resíduo é zero e as seis logos empilham em cima do
  // eixo. Não é um gráfico ruim, é um gráfico sem pergunta — e desenhá-lo assim
  // ainda inventava escala, com "1" repetido nas duas marcas de cima.
  if (pointsMax <= 0) return { hasData: false, reason: "noPoints" };
  const cashPad = Math.max(1, (cashMax - cashMin) * 0.12);
  const xMin = cashMin - cashPad;
  const xMax = cashMax + cashPad;
  const yMax = Math.max(1, pointsMax * 1.12);

  const xFor = (cash) => view.left + ((view.right - view.left) * (cash - xMin)) / Math.max(1, xMax - xMin);
  const yFor = (points) => view.bottom - ((view.bottom - view.top) * points) / yMax;

  const slope = leastSquaresSlope(cashValues, pointValues);
  const meanCash = mean(cashValues);
  const meanPoints = mean(pointValues);
  const predict = (cash) => meanPoints + slope * (cash - meanCash);

  const points = rows.map((row) => {
    const cash = Number(row.cash_balance) || 0;
    const pontos = Number(row.pontos) || 0;
    const expected = predict(cash);
    return {
      id: row.id,
      name: row.nome_curto || row.nome,
      // O nome COMPLETO é a chave do catálogo de logos — a sigla não acha nada lá.
      fullName: row.nome || row.nome_curto,
      color: row.cor_primaria,
      isPlayer: row.id === playerTeamId,
      cash,
      pontos,
      x: round2(xFor(cash)),
      y: round2(yFor(pontos)),
      // Onde a reta coloca esta equipe: é o pé do traço vertical que mede o desvio.
      expected,
      expectedY: round2(yFor(clamp(expected, 0, yMax))),
      // Resíduo: positivo = pontua acima do que o caixa prevê.
      residual: pontos - expected,
    };
  });

  // Tolerância: o desvio MÉDIO do grid. Sem ela, "acima da reta" dispara com meio
  // ponto de diferença e o gráfico promete um veredito que o dado não sustenta —
  // com seis equipes o ajuste tem ruído da ordem de alguns pontos. O piso de 2% da
  // escala evita que um grid perfeitamente alinhado declare todo mundo fora da faixa.
  const tolerance = Math.max(mean(points.map((point) => Math.abs(point.residual))), yMax * 0.02);
  for (const point of points) {
    point.verdict = point.residual > tolerance ? "above" : point.residual < -tolerance ? "below" : "onPar";
  }

  // A faixa é a reta engordada pela tolerância — o "esperado" deixa de ser uma linha
  // sem espessura e vira território, que é como o jogador lê o gráfico.
  const bandY = ((view.bottom - view.top) * tolerance) / yMax;

  const player = points.find((point) => point.isPlayer) ?? null;
  // O resíduo ordenado é o veredito de gestão do grid inteiro: quem tira mais ponto
  // por dólar está no topo. O gráfico sozinho mostrava a posição relativa à reta e
  // deixava o ranking implícito — que é a parte acionável.
  const byResidual = [...points].sort((a, b) => b.residual - a.residual);
  const playerRank = player ? byResidual.findIndex((point) => point.id === player.id) + 1 : 0;

  const trend = {
    x1: round2(xFor(xMin)),
    y1: round2(yFor(clamp(predict(xMin), 0, yMax))),
    x2: round2(xFor(xMax)),
    y2: round2(yFor(clamp(predict(xMax), 0, yMax))),
  };

  return {
    hasData: true,
    view,
    points,
    player,
    ranking: byResidual,
    best: byResidual[0] ?? null,
    worst: byResidual[byResidual.length - 1] ?? null,
    playerRank,
    xMin,
    xMax,
    yMax,
    tolerance,
    trend,
    band: [
      `${trend.x1},${round2(trend.y1 - bandY)}`,
      `${trend.x2},${round2(trend.y2 - bandY)}`,
      `${trend.x2},${round2(trend.y2 + bandY)}`,
      `${trend.x1},${round2(trend.y1 + bandY)}`,
    ].join(" "),
    // Marcas de escala: sem elas o gráfico tem dois números soltos nos cantos e o
    // olho não tem régua para estimar a distância entre dois pontos.
    // Marcas com valor REPETIDO não são marcas: numa categoria com poucos pontos o
    // meio e o topo arredondavam para o mesmo número e a escala mostrava "1" duas
    // vezes. Uma marca por valor distinto.
    yTicks: dedupeByValue(
      [0, 0.5, 1].map((fraction) => ({
        value: Math.round(yMax * fraction),
        y: round2(yFor(yMax * fraction)),
      })),
    ),
    xTicks: [0, 0.5, 1].map((fraction) => ({
      value: xMin + (xMax - xMin) * fraction,
      x: round2(xFor(xMin + (xMax - xMin) * fraction)),
      anchor: fraction === 0 ? "start" : fraction === 1 ? "end" : "middle",
    })),
    // Onde o jogador DEVERIA estar segundo a reta — a linha pontilhada que mede o
    // desvio no gráfico.
    playerExpectedY: player ? round2(yFor(clamp(predict(player.cash), 0, yMax))) : null,
  };
}

// Preto sobre amarelo, branco sobre azul-escuro: a cor da equipe é arbitrária e a
// sigla escrita em cima dela precisa sobreviver a qualquer uma. Mora aqui, no módulo
// puro, porque tanto a tabela quanto a leitura da dispersão desenham essa etiqueta —
// e importar uma da outra fecharia um ciclo (o comparativo já importa a dispersão).
export function readableOn(hex) {
  const value = String(hex).replace("#", "");
  if (value.length !== 6) return "#0d1117";
  const [r, g, b] = [0, 2, 4].map((offset) => parseInt(value.slice(offset, offset + 2), 16) / 255);
  const luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return luminance > 0.55 ? "#0d1117" : "#f0f6fc";
}

function dedupeByValue(ticks) {
  const vistos = new Set();
  return ticks.filter((tick) => {
    if (vistos.has(tick.value)) return false;
    vistos.add(tick.value);
    return true;
  });
}

function mean(values) {
  return values.reduce((sum, value) => sum + value, 0) / Math.max(1, values.length);
}

function leastSquaresSlope(xs, ys) {
  const meanX = mean(xs);
  const meanY = mean(ys);
  let numerator = 0;
  let denominator = 0;
  for (let index = 0; index < xs.length; index += 1) {
    numerator += (xs[index] - meanX) * (ys[index] - meanY);
    denominator += (xs[index] - meanX) ** 2;
  }
  return denominator === 0 ? 0 : numerator / denominator;
}

// ---------------------------------------------------------------------------
// Cascata da rodada
// ---------------------------------------------------------------------------

// Composição da rodada em proporção: quanto da entrada cada saída consumiu e o que
// sobrou. Substitui as duas listas paralelas do v1, que davam os mesmos valores sem
// mostrar peso relativo.
export function roundFlow(latest, expenseLines) {
  const income = Math.max(0, Number(latest?.income_total) || 0);
  if (income <= 0) return { hasData: false };

  const expenses = expenseLines
    .map((line) => ({ key: line.key, value: Math.max(0, Number(latest?.[line.key]) || 0) }))
    .filter((row) => row.value >= 1)
    .sort((a, b) => b.value - a.value);

  const spent = expenses.reduce((sum, row) => sum + row.value, 0);
  const net = income - spent;

  return {
    hasData: true,
    income,
    spent,
    net,
    expenses: expenses.map((row) => ({ ...row, share: (row.value / income) * 100 })),
    netShare: (Math.abs(net) / income) * 100,
  };
}

// ---------------------------------------------------------------------------
// Sankey da rodada
// ---------------------------------------------------------------------------

export const FLOW = {
  pillWidth: 9,
  gap: 8,
  minBand: 4,
  padY: 30,
  trunkWidth: 13,
  labelGap: 9,
  // Espaço vertical MÍNIMO entre o começo de um nó e o do seguinte. Sem esse piso,
  // quatro linhas pequenas viram bandas de 4px encostadas e os rótulos se empilham
  // por cima uns dos outros — foi o que aconteceu com Manutenção/Investimento/
  // Salários/Sobra do lado direito.
  minSlot: 26,
  // Passo do escalonamento horizontal. Quanto MENOR o nó, mais perto do tronco ele
  // nasce: além de dar ar ao desenho, espalha os rótulos no eixo X, e aí dois nós
  // vizinhos deixam de disputar a mesma faixa de altura.
  stagger: 0.1,
  // Largura mínima que sobra para a fita depois de todo o escalonamento.
  minRibbon: 96,
  // Pílula grossa dos dois nós de VEREDITO (sobra e cobertura).
  pillStrong: 18,
  // Respiro extra antes do nó de veredito: descola do bloco de custos E abre espaço
  // para as duas linhas do rótulo grande, que ficam acima da banda.
  verdictGap: 42,
};

// "Sobra" e "Tirado do caixa" não são mais uma linha do livro-caixa: são a RESPOSTA
// da rodada — sobrou ou faltou. Por isso escapam das duas regras que valem para as
// outras: não entram no escalonamento (nascem na borda, no ponto mais visível, e não
// espremidos contra o tronco por serem pequenos) e ganham respiro extra acima.
const VERDICT_KEYS = new Set(["coverage", "balance"]);

// O fluxo do dinheiro de UMA rodada: as linhas de receita convergem num tronco e
// saem repartidas em custos e sobra.
//
// Mesma ideia do Sankey da carreira que já existe no dossiê de equipe
// (`TeamHistoryDrawerV2`), aplicada à rodada. A geometria é escrita aqui em vez de
// importada de lá porque aquele componente não é exportado e vive no meio de um
// arquivo de 4 mil linhas do redesenho do Atlas — a regra destas pastas v2 é copiar
// em vez de mexer no vizinho.
//
// A conta fecha dos dois lados por construção. Quando a rodada gasta mais do que
// arrecada, a diferença entra como um nó PRÓPRIO à esquerda: o dinheiro veio de
// algum lugar (o caixa da equipe), e o desenho não pode fingir que apareceu.
export function roundMoneyFlow({ latest, incomeLines, expenseLines, width }) {
  const value = (key) => Math.max(0, Number(latest?.[key]) || 0);
  const income = incomeLines.map((line) => ({ key: line.key, value: value(line.key) })).filter((row) => row.value >= 1);
  const expenses = expenseLines.map((line) => ({ key: line.key, value: value(line.key) })).filter((row) => row.value >= 1);
  if (income.length === 0 && expenses.length === 0) return { hasData: false };

  const incomeTotal = income.reduce((sum, row) => sum + row.value, 0);
  const expensesTotal = expenses.reduce((sum, row) => sum + row.value, 0);
  const balance = incomeTotal - expensesTotal;
  const coverage = Math.max(0, -balance);
  const trunk = incomeTotal + coverage;
  if (trunk <= 0) return { hasData: false };

  const left = [...income].sort((a, b) => b.value - a.value).map((row) => ({ ...row, tone: "income", verdict: false }));
  if (coverage > 0) left.push({ key: "coverage", value: coverage, tone: "coverage", verdict: true });
  const right = [...expenses].sort((a, b) => b.value - a.value).map((row) => ({ ...row, tone: "expense", verdict: false }));
  if (balance > 0) right.push({ key: "balance", value: balance, tone: "balance", verdict: true });

  const boxWidth = Math.max(360, Math.round(Number(width) || 0) || 600);
  const leftX = 0;
  const rightX = boxWidth - FLOW.pillWidth;
  const trunkX = (boxWidth - FLOW.trunkWidth) / 2;
  const halfSpan = trunkX - leftX;
  // Escalonamento horizontal: o maior nó nasce na borda e cada nó menor nasce um
  // passo mais para dentro. O teto garante que a fita mais curta ainda tenha corpo.
  const step = clamp(halfSpan * FLOW.stagger, 14, 56);
  const maxStagger = Math.max(0, halfSpan - FLOW.minRibbon);
  const inset = (node, index) => (VERDICT_KEYS.has(node.key) ? 0 : Math.min(index * step, maxStagger));

  // O tronco é a altura de referência das bandas; a altura do CARD pode passar dela
  // quando os nós pequenos precisam de espaço para os rótulos.
  const body = Math.max(150, 26 * Math.max(left.length, right.length));
  const band = (amount) => Math.max(FLOW.minBand, (amount / trunk) * body);

  // Empilhamento com piso de espaçamento: cada nó começa pelo menos `minSlot` abaixo
  // do anterior, mesmo que a banda dele seja de 4px. É o que garante uma linha de
  // rótulo por nó sem sobreposição.
  const stack = (nodes) => {
    const bands = nodes.map((node) => band(node.value));
    const tops = [];
    let cursor = 0;
    bands.forEach((size, index) => {
      if (index > 0) {
        const gap = VERDICT_KEYS.has(nodes[index].key) ? FLOW.verdictGap : FLOW.gap;
        cursor = Math.max(cursor + gap - FLOW.gap, tops[index - 1] + FLOW.minSlot);
      }
      tops.push(cursor);
      cursor = tops[index] + size + FLOW.gap;
    });
    return { bands, tops, span: tops[tops.length - 1] + bands[bands.length - 1] };
  };

  const leftStack = stack(left);
  const rightStack = stack(right);
  const content = Math.max(body, leftStack.span, rightStack.span);
  const height = FLOW.padY * 2 + content;
  const trunkTop = FLOW.padY + (content - body) / 2;

  // As fitas encostam no tronco na MESMA ordem em que saem dos nós e sem folga entre
  // elas: o tronco é contínuo porque é o total. As âncoras são reescaladas para somar
  // exatamente o tronco — sem isso, o piso de `minBand` faz as pontas vazarem por
  // baixo dele nas rodadas com uma linha de valor irrisório.
  const place = (nodes, layout, side) => {
    const offset = FLOW.padY + (content - layout.span) / 2;
    const bandTotal = layout.bands.reduce((sum, size) => sum + size, 0);
    const scale = bandTotal > 0 ? body / bandTotal : 1;
    let anchorCursor = trunkTop;
    return nodes.map((node, index) => {
      const size = layout.bands[index];
      const top = offset + layout.tops[index];
      const anchorSize = size * scale;
      const anchorTop = anchorCursor;
      anchorCursor += anchorSize;
      const pill = node.verdict ? FLOW.pillStrong : FLOW.pillWidth;
      const offsetX = inset(node, index);
      return {
        ...node,
        pill,
        x: round2(side === "left" ? leftX + offsetX : boxWidth - pill - offsetX),
        top: round2(top),
        bottom: round2(top + size),
        anchorTop: round2(anchorTop),
        anchorBottom: round2(anchorTop + anchorSize),
        share: (node.value / trunk) * 100,
      };
    });
  };

  return {
    hasData: true,
    width: boxWidth,
    height: round2(height),
    trunk,
    trunkX: round2(trunkX),
    trunkTop: round2(trunkTop),
    trunkWidth: FLOW.trunkWidth,
    body: round2(body),
    leftX,
    rightX,
    incomeTotal,
    expensesTotal,
    balance,
    coverage,
    left: place(left, leftStack, "left"),
    right: place(right, rightStack, "right"),
  };
}

// Fita do Sankey: duas cúbicas espelhadas com os controles no meio do vão, que é o
// que dá o "S" contínuo em vez de dois arcos emendados.
export function ribbonPath(x1, top1, bottom1, x2, top2, bottom2) {
  const middle = (x1 + x2) / 2;
  return [
    `M ${round2(x1)},${round2(top1)}`,
    `C ${round2(middle)},${round2(top1)} ${round2(middle)},${round2(top2)} ${round2(x2)},${round2(top2)}`,
    `L ${round2(x2)},${round2(bottom2)}`,
    `C ${round2(middle)},${round2(bottom2)} ${round2(middle)},${round2(bottom1)} ${round2(x1)},${round2(bottom1)}`,
    "Z",
  ].join(" ");
}
