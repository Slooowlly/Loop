// Cenários falsos para inspecionar a aba Minha Equipe v2 sem esperar a carreira
// chegar naquele estado.
//
// REGRA: nada aqui toca o banco, o store ou o backend. O gerador devolve um payload
// com a MESMA forma dos comandos reais (`get_teams_standings`,
// `get_team_finance_report`, o `playerTeam` do store e o `SeasonSummary`) e a aba
// simplesmente desenha esse payload no lugar do verdadeiro. Fechar o cenário volta
// tudo ao dado real — não há nada para desfazer.
//
// A segunda regra é a que importa mais: os números têm de ser COERENTES ENTRE SI.
// Um fixture onde `net` não é `entradas - saídas`, ou onde o acumulado da temporada
// não é a soma das rodadas, ensina uma lição errada sobre a tela — o gráfico pode
// parecer certo com dado impossível, ou parecer quebrado com dado que nunca ocorre.
// Por isso tudo aqui é DERIVADO de poucos botões, como o backend faz.

import { CAR_PART_KEYS } from "./gridMetrics";

export const FAKE_SCENARIOS = [
  { id: "vermelho", label: "No vermelho" },
  { id: "divida", label: "Endividada" },
  { id: "dominante", label: "Dominante" },
  { id: "estreante", label: "Primeira rodada" },
  { id: "aleatorio", label: "Aleatório" },
];

const PLAYER_ID = "fake-player";

// Proporção das cinco linhas de despesa. Soma 1: a divisão é de uma despesa total
// única, senão as fatias da cascata não fecham com o total da rodada.
const EXPENSE_SPLIT = [
  ["event_operations_cost", 0.45],
  ["structural_maintenance_cost", 0.27],
  ["salary_expense", 0.13],
  ["technical_investment_cost", 0.11],
  ["debt_service_cost", 0.04],
];

const INCOME_SPLIT = [
  ["sponsorship_income", 0.58],
  ["result_bonus", 0.17],
  ["gate_income", 0.13],
  ["partial_prize_income", 0.12],
];

export function buildFakeScenario(id) {
  const knobs = knobsFor(id);
  return assemble(knobs);
}

function knobsFor(id) {
  if (id === "vermelho") {
    return {
      seed: 7,
      income: 48_000,
      expenseRatio: 1.18,
      cash: 210_000,
      debt: 0,
      totalRounds: 12,
      roundsDone: 7,
      position: 5,
      carLevel: 2,
      reliability: 48,
      pitCrew: 22,
      presence: 38,
      payrollRatio: 0.97,
      tension: 34,
    };
  }
  if (id === "divida") {
    return {
      seed: 11,
      income: 52_000,
      expenseRatio: 1.32,
      cash: 96_000,
      debt: 340_000,
      totalRounds: 12,
      roundsDone: 9,
      position: 6,
      carLevel: 1,
      reliability: 39,
      pitCrew: 15,
      presence: 24,
      payrollRatio: 1.02,
      tension: 71,
    };
  }
  if (id === "dominante") {
    return {
      seed: 3,
      income: 118_000,
      expenseRatio: 0.62,
      cash: 1_420_000,
      debt: 0,
      totalRounds: 12,
      roundsDone: 8,
      position: 1,
      carLevel: 8,
      reliability: 86,
      pitCrew: 74,
      presence: 81,
      payrollRatio: 0.68,
      tension: 8,
    };
  }
  if (id === "estreante") {
    // Uma rodada só: é o estado em que quase todo gráfico tem de decidir se desenha
    // ou se admite que ainda não há série.
    return {
      seed: 5,
      income: 41_000,
      expenseRatio: 1.05,
      cash: 155_000,
      debt: 0,
      totalRounds: 12,
      roundsDone: 1,
      position: 4,
      carLevel: 3,
      reliability: 55,
      pitCrew: 30,
      presence: 44,
      payrollRatio: 0.8,
      tension: 0,
    };
  }
  const random = (min, max) => min + Math.random() * (max - min);
  return {
    seed: Math.floor(random(1, 9999)),
    income: Math.round(random(30_000, 140_000)),
    expenseRatio: random(0.55, 1.45),
    cash: Math.round(random(20_000, 1_600_000)),
    debt: Math.random() < 0.4 ? Math.round(random(50_000, 500_000)) : 0,
    totalRounds: Math.round(random(8, 18)),
    roundsDone: 0,
    position: Math.round(random(1, 6)),
    carLevel: Math.round(random(1, 10)),
    reliability: Math.round(random(25, 95)),
    pitCrew: Math.round(random(0, 90)),
    presence: Math.round(random(10, 95)),
    payrollRatio: random(0.5, 1.05),
    tension: Math.round(random(0, 95)),
  };
}

// Gerador determinístico por semente: o mesmo cenário sempre desenha o mesmo
// gráfico, senão comparar "antes e depois" de um ajuste vira adivinhação.
function seededRandom(seed) {
  let state = seed % 2147483647;
  if (state <= 0) state += 2147483646;
  return () => {
    state = (state * 16807) % 2147483647;
    return (state - 1) / 2147483646;
  };
}

function assemble(knobs) {
  const random = seededRandom(knobs.seed);
  const roundsDone = knobs.roundsDone > 0 ? knobs.roundsDone : Math.max(1, Math.round(knobs.totalRounds * random()));
  const seasonNumber = 3;

  // Cada rodada varia em torno dos botões — uma série lisa demais esconde justamente
  // o que o gráfico de colunas existe para mostrar.
  const rounds = [];
  for (let index = 0; index < roundsDone; index += 1) {
    const income = Math.round(knobs.income * (0.78 + random() * 0.45));
    const expenses = Math.round(income * knobs.expenseRatio * (0.85 + random() * 0.32));
    rounds.push({ round: index + 1, income, expenses, net: income - expenses });
  }

  // O caixa ANDA com os resultados: o saldo de hoje menos os nets recentes dá o saldo
  // de cada rodada anterior. Sem isso a linha do acumulado contradiz as colunas.
  const cashNow = knobs.cash;
  let walking = cashNow;
  const cashByRound = new Array(rounds.length);
  for (let index = rounds.length - 1; index >= 0; index -= 1) {
    cashByRound[index] = walking;
    walking -= rounds[index].net;
  }

  const latest = rounds[rounds.length - 1];
  const seasonTotals = rounds.reduce(
    (acc, round) => ({
      income_total: acc.income_total + round.income,
      expenses_total: acc.expenses_total + round.expenses,
      net: acc.net + round.net,
    }),
    { income_total: 0, expenses_total: 0, net: 0 },
  );

  const gridSize = 6;
  const prizeByPosition = [0, 260_000, 190_000, 140_000, 96_000, 64_000, 38_000];
  const expectedPrize = prizeByPosition[Math.min(knobs.position, gridSize)] ?? 40_000;
  const salaryCeiling = Math.round(knobs.income * 0.42);
  const payroll = Math.round(salaryCeiling * knobs.payrollRatio);

  const team = {
    id: PLAYER_ID,
    nome: "Equipe de Teste",
    nome_curto: "TST",
    cor_primaria: "#8b5cf6",
    categoria: "gt3",
    cash_balance: cashNow,
    debt_balance: knobs.debt,
    salary_ceiling: salaryCeiling,
    spending_power: Math.round(seasonTotals.net + expectedPrize),
    last_round_income: latest.income,
    last_round_expenses: latest.expenses,
    last_round_net: latest.net,
    presenca_publica: knobs.presence,
    financial_state: financialStateFor(knobs),
    season_strategy: knobs.expenseRatio > 1 ? "expansion" : "balanced",
    car_level: knobs.carLevel,
    confiabilidade: knobs.reliability,
    pit_crew_quality: knobs.pitCrew,
    pit_strategy_risk: Math.round(30 + random() * 45),
    hierarquia_n1_id: "fake-n1",
    hierarquia_n2_id: "fake-n2",
    hierarquia_status: knobs.tension > 50 ? "tensao" : knobs.tension >= 20 ? "competitivo" : "estavel",
    hierarquia_tensao: knobs.tension,
    hierarquia_inversoes_temporada: knobs.tension > 60 ? 1 : 0,
    piloto_1_id: "fake-n1",
    piloto_1_nome: "Piloto Um",
    piloto_1_salario_anual: Math.round(payroll * 0.56),
    piloto_2_id: "fake-n2",
    piloto_2_nome: "Piloto Dois",
    piloto_2_salario_anual: Math.round(payroll * 0.44),
  };

  const teams = buildGrid(team, knobs, gridSize, random);

  return {
    team,
    drivers: buildDrivers(team, knobs, random),
    teams,
    cars: buildCars(teams, random),
    report: {
      rounds_recorded: rounds.length,
      latest: buildRound(latest, seasonNumber),
      season: {
        ...buildRound({ round: rounds.length, income: seasonTotals.income_total, expenses: seasonTotals.expenses_total, net: seasonTotals.net }, seasonNumber),
        constructor_prize_income: 0,
      },
      cash_timeline: rounds.map((round, index) => ({
        season_number: seasonNumber,
        round: round.round,
        cash_balance: cashByRound[index],
        net: round.net,
        is_season_close: false,
      })),
      expected_constructor_prize: expectedPrize,
      current_position: knobs.position,
      grid_size: gridSize,
    },
    season: { numero: seasonNumber, ano: 2026, rodada_atual: rounds.length, total_rodadas: knobs.totalRounds },
  };
}

function financialStateFor(knobs) {
  if (knobs.debt > knobs.cash) return "crisis";
  if (knobs.expenseRatio > 1.1) return "pressured";
  if (knobs.cash > 1_000_000) return "elite";
  return "healthy";
}

// Uma rodada no formato de `TeamFinanceRound`: as linhas individuais são fatias do
// total, para que as somas fechem com `income_total`/`expenses_total`.
function buildRound(round, seasonNumber) {
  const lines = {};
  for (const [key, share] of INCOME_SPLIT) lines[key] = Math.round(round.income * share);
  for (const [key, share] of EXPENSE_SPLIT) lines[key] = Math.round(round.expenses * share);
  return {
    season_number: seasonNumber,
    round: round.round,
    aid_income: 0,
    constructor_prize_income: 0,
    ...lines,
    income_total: round.income,
    expenses_total: round.expenses,
    net: round.net,
  };
}

// A dupla do jogador MAIS um pelotão de rivais, no formato de `DriverSummary`. Sem
// esse pelotão o dossiê não teria média de categoria para usar como régua, e os
// cenários falsos não exercitariam justamente a parte nova do painel.
//
// A força dos dois pilotos acompanha o cenário: uma equipe dominante não tem como ter
// a pior dupla do grid, e isso precisa aparecer no fixture, senão a tela mostra uma
// combinação que o jogo nunca produz.
function buildDrivers(team, knobs, random) {
  const strength = clamp((knobs.carLevel / 10) * 0.6 + (knobs.presence / 100) * 0.4, 0, 1);
  const dupla = [
    {
      id: "fake-n1",
      nome: "Piloto Um",
      nacionalidade: "Brasileiro",
      idade: 27,
      skill: Math.round(clamp(48 + strength * 45 + random() * 6, 1, 100)),
      midia: Math.round(clamp(knobs.presence * 1.15, 1, 100)),
      equipe_id: team.id,
      is_jogador: false,
      is_estreante: false,
      lesao_ativa_tipo: null,
      pontos: Math.round(strength * 70 + random() * 12),
      vitorias: Math.round(strength * 5),
      podios: Math.round(strength * 11),
      posicao_campeonato: Math.max(1, Math.round(14 - strength * 12)),
    },
    {
      id: "fake-n2",
      nome: "Piloto Dois",
      nacionalidade: "Britanico",
      idade: 21,
      skill: Math.round(clamp(40 + strength * 40 + random() * 6, 1, 100)),
      midia: Math.round(clamp(knobs.presence * 0.7, 1, 100)),
      equipe_id: team.id,
      is_jogador: false,
      is_estreante: true,
      lesao_ativa_tipo: knobs.tension > 60 ? "contusao" : null,
      pontos: Math.round(strength * 34 + random() * 8),
      vitorias: 0,
      podios: Math.round(strength * 3),
      posicao_campeonato: Math.max(1, Math.round(20 - strength * 12)),
    },
  ];

  const rivais = [];
  for (let index = 0; index < 18; index += 1) {
    const rivalStrength = random();
    rivais.push({
      id: `fake-rival-driver-${index}`,
      nome: `Rival ${index + 1}`,
      nacionalidade: "Britanico",
      idade: Math.round(19 + random() * 18),
      skill: Math.round(clamp(35 + rivalStrength * 60, 1, 100)),
      midia: Math.round(clamp(10 + rivalStrength * 75, 1, 100)),
      equipe_id: `fake-rival-${(index % 5) + 1}`,
      is_jogador: false,
      is_estreante: false,
      lesao_ativa_tipo: null,
      pontos: Math.round(rivalStrength * 80),
      vitorias: Math.round(rivalStrength * 4),
      podios: Math.round(rivalStrength * 9),
      posicao_campeonato: index + 3,
    });
  }

  return [...dupla, ...rivais];
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

// O grid tem de conter a equipe do jogador com o MESMO id e a posição pedida, senão
// o radar não acha o jogador e a tabela destaca a linha errada.
function buildGrid(team, knobs, gridSize, random) {
  // Nomes do CATÁLOGO de logos (`TeamLogoMark`), não inventados. A tabela e a
  // dispersão desenham a arte da equipe e caem num círculo de cor quando o nome não
  // está lá — um fixture com nomes fora do catálogo exercitaria só o fallback e
  // esconderia justamente o caminho que o jogo usa.
  const names = [
    "Apex Academy Racing",
    "Weekend Warriors Racing",
    "Amateur Hour Racing",
    "Track Day Heroes",
    "Late Apex Contenders",
    "Club Racer Motorsport",
  ];
  const colors = ["#22d3a7", "#8b949e", "#e3c15a", "#58a6ff", "#f0883e", "#bc8cff"];
  const rows = [];
  for (let position = 1; position <= gridSize; position += 1) {
    const isPlayer = position === knobs.position;
    const strength = (gridSize - position + 1) / gridSize;
    // A CONVERSÃO de caixa em resultado é o assunto do gráfico de gestão. Enquanto o
    // caixa do rival era função só da posição — como os pontos também são —, caixa e
    // pontos ficavam perfeitamente correlacionados e a dispersão desenhava uma reta:
    // seis pontos em cima da linha, e trocar de cenário não mudava o desenho. Cada
    // rival ganha um fator de eficiência PRÓPRIO, então o grid passa a ter quem queima
    // dinheiro e quem faz mais com menos — que é o que o gráfico existe para separar.
    const efficiency = 0.6 + random() * 0.9;
    rows.push(
      isPlayer
        ? {
            id: team.id,
            nome: team.nome,
            nome_curto: team.nome_curto,
            cor_primaria: team.cor_primaria,
            posicao: position,
            pontos: Math.round(12 + strength * 46),
            cash_balance: team.cash_balance,
            car_level: team.car_level,
            confiabilidade: team.confiabilidade,
            pit_crew_quality: team.pit_crew_quality,
          }
        : {
            id: `fake-rival-${position}`,
            nome: names[position - 1],
            nome_curto: names[position - 1].slice(0, 3).toUpperCase(),
            cor_primaria: colors[position - 1],
            posicao: position,
            pontos: Math.round(12 + strength * 46 + random() * 6),
            cash_balance: Math.round((team.cash_balance * (0.45 + strength * 0.95)) / efficiency),
            car_level: Math.max(1, Math.min(10, Math.round(strength * 9 + random() * 1.5))),
            confiabilidade: Math.round(35 + strength * 50 + random() * 8),
            pit_crew_quality: Math.round(strength * 60 + random() * 12),
          },
    );
  }
  return rows;
}

// As 11 peças de cada equipe, no formato de `get_teams_car_parts`.
//
// O nível de cada peça ORBITA o `car_level` da equipe em vez de repeti-lo: é
// justamente o desequilíbrio entre peças que o radar existe para mostrar, e um
// fixture com onze níveis idênticos desenharia onze polígonos regulares e ensinaria
// a corrigir um problema que não existe.
function buildCars(teams, random) {
  return teams.map((team) => {
    const base = Math.max(1, Math.min(10, Number(team.car_level) || 1));
    return {
      team_id: team.id,
      nome: team.nome,
      nome_curto: team.nome_curto,
      cor_primaria: team.cor_primaria,
      parts: CAR_PART_KEYS.map((key) => ({
        key,
        level: Math.max(1, Math.min(10, Math.round(base + (random() * 4 - 2)))),
      })),
    };
  });
}
