// Regra de QUEM aparece na torre.
//
// Numa corrida cheia não cabe (nem adianta) mostrar a grade inteira. Então:
//   • Até 15 carros no total  -> mostra todo mundo, sem janela nem separação.
//   • Acima disso, por CLASSE:
//       - na classe do JOGADOR: top 3 + separação + a vizinhança dele (±4)
//       - nas outras classes:   só o top 3 (interessa quem lidera)
//
// A vizinhança tem TAMANHO FIXO (2·NEIGHBORS+1). Perto de uma ponta ela DESLIZA
// pro outro lado em vez de encolher: liderando (nada acima) ela estica pra baixo,
// na lanterna estica pra cima. Sem isso, o líder via só 5 carros. O total visível
// da classe do jogador fica constante (topo + vizinhança ≈ 12), esteja ele onde estiver.
//
// A separação só aparece quando ela realmente esconde alguém: se a vizinhança
// encosta no top, a gente emenda num bloco contínuo do 1º — separar pra ocultar
// um punhado de pilotos seria pior que mostrá-los.

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

  const n = cars.length;
  // Total "de sempre" da classe do jogador: topo + vizinhança (ex.: 3 + 9 = 12).
  const target = Math.min(n, topN + 2 * neighbors + 1);

  // Janela de TAMANHO FIXO em torno do jogador. Perto de uma ponta, DESLIZA pro outro
  // lado em vez de encolher — senão liderar (nada acima) mostraria só ~5 carros.
  let start = playerIdx - neighbors;
  let end = playerIdx + neighbors;
  if (start < 0) {
    end -= start; // empurra a janela pra baixo (líder)
    start = 0;
  }
  if (end > n - 1) {
    start -= end - (n - 1); // empurra pra cima (lanterna)
    end = n - 1;
  }
  start = Math.max(0, start);

  // Janela encosta no topo → bloco contínuo do 1º, com pelo menos o total de sempre e
  // sem cortar a vizinhança do jogador (o `end` deslizado).
  if (start <= topN + 1) {
    return cars.slice(0, Math.max(target, end + 1)).map(carRow);
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
