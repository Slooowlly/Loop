// Regra de QUEM aparece na torre.
//
// Numa corrida cheia não cabe (nem adianta) mostrar a grade inteira. Então:
//   • Até 15 carros no total  -> mostra todo mundo, sem janela nem separação.
//   • Acima disso, por CLASSE:
//       - na classe do JOGADOR: top 3 + separação + a vizinhança dele (±4)
//       - nas outras classes:   só o top 3 (interessa quem lidera)
//
// A separação só aparece quando ela realmente esconde alguém: se a vizinhança
// encosta no top (ou pula uma linha só), a gente emenda — separar pra ocultar
// um único piloto seria pior que mostrá-lo.

export const MAX_ROWS_WITHOUT_WINDOW = 15;
export const TOP_N = 3;
export const NEIGHBORS = 4;

// Ordem canônica das classes na torre (mais rápida em cima). Cobre os dois tipos
// de evento multiclasse:
//   • endurance:  LMP2 > GT3 > GT4
//   • production: BMW  > Toyota > Mazda
// Uma corrida é só de UM tipo, então a mesma lista serve pros dois. Classe
// desconhecida vai pro fim, mantendo a ordem de entrada (sort estável).
const CLASS_ORDER = ["lmp2", "gt3", "gt4", "bmw", "toyota", "mazda", "prod", "production"];

function classRank(cls) {
  const key = String(cls.id ?? cls.label ?? "").toLowerCase();
  const i = CLASS_ORDER.indexOf(key);
  return i < 0 ? CLASS_ORDER.length : i;
}

export function orderClasses(classes) {
  return [...classes].sort((a, b) => classRank(a) - classRank(b));
}

export function totalCars(data) {
  return data.classes.reduce((n, cls) => n + cls.cars.length, 0);
}

const carRow = (car) => ({ kind: "car", car });
const separatorRow = () => ({ kind: "separator" });

function compressPlayerClass(cars, topN, neighbors) {
  const playerIdx = cars.findIndex((c) => c.player);
  if (playerIdx < 0) {
    // Sem jogador nessa classe (não deveria acontecer aqui): trata como as outras.
    return cars.slice(0, topN).map(carRow);
  }

  const start = Math.max(0, playerIdx - neighbors);
  const end = Math.min(cars.length - 1, playerIdx + neighbors);

  // Emenda quando não há buraco real (0 ou 1 piloto escondido entre os blocos).
  if (start <= topN + 1) {
    return cars.slice(0, end + 1).map(carRow);
  }

  return [
    ...cars.slice(0, topN).map(carRow),
    separatorRow(),
    ...cars.slice(start, end + 1).map(carRow),
  ];
}

/**
 * Monta as seções visíveis da torre.
 * @returns {Array<{cls: object, rows: Array<{kind:'car'|'separator', car?: object}>}>}
 */
export function buildTowerSections(data, opts = {}) {
  const topN = opts.topN ?? TOP_N;
  const neighbors = opts.neighbors ?? NEIGHBORS;
  const maxRows = opts.maxRows ?? MAX_ROWS_WITHOUT_WINDOW;

  const classes = orderClasses(data.classes);

  // Corrida pequena: cabe todo mundo.
  if (totalCars(data) <= maxRows) {
    return classes.map((cls) => ({ cls, rows: cls.cars.map(carRow) }));
  }

  return classes.map((cls) => {
    const hasPlayer = cls.cars.some((c) => c.player);
    const rows = hasPlayer
      ? compressPlayerClass(cls.cars, topN, neighbors)
      : cls.cars.slice(0, topN).map(carRow);
    return { cls, rows };
  });
}

/** Time do jogador (pra marcar o companheiro de equipe). Null se não achar. */
export function playerTeam(data) {
  for (const cls of data.classes) {
    const p = cls.cars.find((c) => c.player);
    if (p) return p.team ?? null;
  }
  return null;
}

/** É companheiro de equipe do jogador? (mesmo time, mas não é ele) */
export function isTeammate(car, team) {
  return Boolean(team) && car.team === team && !car.player;
}
