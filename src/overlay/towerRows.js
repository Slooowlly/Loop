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
// Quantas linhas o jogador pode chegar da borda da janela antes de ela ceder.
export const WINDOW_MARGIN = 2;

const clamp = (n, lo, hi) => (n < lo ? lo : n > hi ? hi : n);

// A janela GRUDADA no jogador: recentra a cada quadro, sempre com ele no meio.
// É o comportamento histórico e continua sendo o padrão de quem não passa uma janela
// própria (`towerContentHeight`, testes, o preview estático).
const CENTERED_WINDOW = {
  resolve(playerIdx, n, size) {
    return clamp(playerIdx - Math.floor((size - 1) / 2), 0, Math.max(0, n - size));
  },
};

/**
 * Janela SOLTA, com zona morta. Guarda onde ela está e só cede quando o jogador chega
 * a `margin` linhas da borda — e aí anda o mínimo pra restaurar a folga.
 *
 * O motivo é de leitura, não de desempenho: grudada no jogador, toda ultrapassagem
 * arrastava a janela junto, então a linha DELE ficava imóvel e o pelotão inteiro
 * rolava uma casa. Numa torre pequena isso mexia em metade das linhas visíveis por
 * ultrapassagem e ficava impossível de acompanhar. Solta, uma ultrapassagem normal
 * mexe só nos dois carros da disputa — e a linha do jogador volta a subir e descer,
 * que era o ponto da animação.
 *
 * Tem estado, então é UMA POR CANVAS (monitor, VR e preview têm a sua). Duas vistas
 * dividindo a mesma janela brigariam pelo `start`.
 */
export function createTowerWindow() {
  let start = null;
  return {
    resolve(playerIdx, n, size, margin = WINDOW_MARGIN) {
      const max = Math.max(0, n - size);
      // 1ª vez (ou grade nova): entra centrada, como a versão grudada.
      if (start === null) start = CENTERED_WINDOW.resolve(playerIdx, n, size);
      // Zona morta: só anda quando o jogador encosta na borda. `margin` é limitado a
      // metade da janela, senão as duas correções brigariam e a janela oscilaria.
      const m = Math.min(margin, Math.floor((size - 1) / 2));
      if (playerIdx - start < m) start = playerIdx - m;
      if (start + size - 1 - playerIdx < m) start = playerIdx - size + 1 + m;
      start = clamp(start, 0, max);
      return start;
    },
    reset() {
      start = null;
    },
  };
}

// Ordem canônica das classes na torre (mais rápida em cima). Cobre os dois tipos
// de evento multiclasse:
//   • endurance:  LMP2 > GT3 > GT4
//   • production: BMW  > Toyota > Mazda
// Uma corrida é só de UM tipo, então a mesma lista serve pros dois. Classe
// desconhecida vai pro fim, em ordem alfabética.
const CLASS_ORDER = ["lmp2", "gt3", "gt4", "bmw", "toyota", "mazda", "prod", "production"];

function classKey(cls) {
  return String(cls.id ?? cls.label ?? "").toLowerCase();
}

function classRank(cls) {
  const i = CLASS_ORDER.indexOf(classKey(cls));
  return i < 0 ? CLASS_ORDER.length : i;
}

// O desempate alfabético entre as desconhecidas é o que impede a torre de tremer: numa
// prova que não é do Loop nenhuma classe está na CLASS_ORDER, todas empatam no mesmo rank
// e o sort estável devolvia a ordem em que o backend mandou. Como aquela ordem vinha da
// iteração de um HashMap refeito a cada poll, os blocos de classe trocavam de lugar ~1x/s
// e a grade inteira ficava subindo e descendo. O backend já manda ordenado; isto aqui
// garante que a torre não dependa disso.
export function orderClasses(classes) {
  return [...classes].sort(
    (a, b) => classRank(a) - classRank(b) || classKey(a).localeCompare(classKey(b)),
  );
}

export function totalCars(data) {
  return data.classes.reduce((n, cls) => n + cls.cars.length, 0);
}

const carRow = (car) => ({ kind: "car", car });
const separatorRow = () => ({ kind: "separator" });

function compressPlayerClass(cars, topN, neighbors, win) {
  const playerIdx = cars.findIndex((c) => c.player);
  if (playerIdx < 0) {
    // Sem jogador nessa classe (não deveria acontecer aqui): trata como as outras.
    return cars.slice(0, topN).map(carRow);
  }

  const n = cars.length;
  // Total "de sempre" da classe do jogador: topo + vizinhança (ex.: 3 + 9 = 12).
  const target = Math.min(n, topN + 2 * neighbors + 1);

  // Janela de TAMANHO FIXO em torno do jogador. Perto de uma ponta ela DESLIZA pro
  // outro lado em vez de encolher — senão liderar (nada acima) mostraria só ~5 carros.
  // Quem decide ONDE ela fica é a janela recebida: grudada (padrão) ou solta.
  const size = Math.min(n, 2 * neighbors + 1);
  const start = (win ?? CENTERED_WINDOW).resolve(playerIdx, n, size);
  const end = Math.min(n - 1, start + size - 1);

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
      ? compressPlayerClass(cls.cars, topN, neighbors, opts.window)
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
