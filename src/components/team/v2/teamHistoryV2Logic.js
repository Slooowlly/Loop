// Lógica pura do dossiê de equipe v2: paleta de colocação, contraste de tinta, se um gráfico
// tem dado para desenhar, e o ranking dos pilotos que vestiram a equipe.
//
// Extraído de `TeamHistoryDrawerV2.jsx` em 11/08/2026. O arquivo tinha 4.525 linhas e 63
// funções internas atrás de um único export — a vistoria de 10/08 marcou como [Alta] e
// apontou o caminho: o próprio v2 já fez isso com `atlasV2Geometry.js` e `gridMetrics.js`,
// que saíram do componente e ganharam teste espelho.
//
// O que veio para cá é o que decide CONTEÚDO: ordem, corte e cor derivada de dado. O que
// ficou lá é o que decide DESENHO: geometria de SVG, JSX e estado de realce.

/// A paleta de medalha, compartilhada por Records, pela faixa de colocação e pelo ranking.
/// Um número só muda de significado entre os blocos se mudar de cor — então não muda.
export const MEDAL_COLORS = {
  first: "#f2c46d",
  second: "#c2ccd8",
  third: "#c07f4a",
  nearMiss: "#46586d",
  dnf: "#ef4444",
};

export const PLACEMENT_COLORS = {
  first: MEDAL_COLORS.first,
  second: MEDAL_COLORS.second,
  third: MEDAL_COLORS.third,
  nearMiss: MEDAL_COLORS.nearMiss,
  topTen: "#22303f",
  outside: "#141f2c",
};

/// A cor do quadrado de uma colocação.
export function placementTone(position) {
  if (position === 1) return PLACEMENT_COLORS.first;
  if (position === 2) return PLACEMENT_COLORS.second;
  if (position === 3) return PLACEMENT_COLORS.third;
  if (position >= 4 && position <= 5) return PLACEMENT_COLORS.nearMiss;
  if (position >= 6 && position <= 10) return PLACEMENT_COLORS.topTen;
  return PLACEMENT_COLORS.outside;
}

/// Texto legível sobre o quadrado: os tons claros (ouro, prata, bronze) precisam de tinta
/// escura, os escuros de tinta clara.
export function placementInk(position) {
  return position >= 1 && position <= 3 ? "#0b1524" : "#8fa3bb";
}

/// Preto sobre ouro e prata, branco sobre os azuis escuros.
///
/// A paleta das faixas vai de #f2c46d a #141f2c, e um texto de cor fixa some em metade delas.
/// A conta é a luminância percebida (ITU-R BT.601): o olho pesa verde muito mais que azul, e
/// uma média aritmética dos três canais erra justamente nos tons de medalha.
export function corDeTextoSobre(hex) {
  const cor = String(hex).replace("#", "");
  if (cor.length !== 6) return "#eaf1f8";
  const r = parseInt(cor.slice(0, 2), 16);
  const g = parseInt(cor.slice(2, 4), 16);
  const b = parseInt(cor.slice(4, 6), 16);
  const luminancia = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  return luminancia > 0.6 ? "#0d1622" : "#eaf1f8";
}

/// Se a campanha do campeonato tem o que desenhar: duas rodadas e ao menos uma linha.
///
/// O seletor de vistas PERGUNTA antes de desenhar. Sem o corte, uma equipe recém-promovida
/// abriria o dossiê num gráfico de um ponto só.
export function campanhaTemDados(run) {
  return (Array.isArray(run?.rounds) ? run.rounds.length : 0) >= 2 && Boolean(run?.lines?.length);
}

/// As temporadas em que a equipe de fato correu. Temporada com zero corridas existe no banco
/// (a equipe foi criada e não entrou no grid) e não é ponto de curva.
export function temporadasDisputadas(seasons) {
  return (Array.isArray(seasons) ? seasons : []).filter((row) => Number(row.races) > 0);
}

/// Se a curva de campeonato tem o que desenhar: duas temporadas disputadas, e ao menos uma
/// com colocação conhecida.
export function curvaTemDados(seasons) {
  const rows = temporadasDisputadas(seasons);
  return rows.length >= 2 && rows.some((row) => /\d/.test(String(row.position ?? "")));
}

export const BEST_DRIVERS_LIMIT = 10;

/// Cor da posição no ranking: as três primeiras na paleta de medalha, o resto apagado.
export const BEST_RANK_COLORS = [MEDAL_COLORS.first, MEDAL_COLORS.second, MEDAL_COLORS.third];

/// O ranking dos pilotos que vestiram a equipe.
///
/// A galeria de elenco conta a SUCESSÃO — quem veio depois de quem. Ela não responde quem foi
/// o melhor: os números estão lá, espalhados por oito linhas em ordem de ano, e comparar dois
/// deles é trabalho do leitor. Aqui a ordem é a resposta.
///
/// A conta é por PILOTO, e não por passagem: quem saiu e voltou tem dois mandatos na galeria
/// (é assim que a sucessão se lê), mas um só currículo pela casa — somar os dois é o que
/// impede a mesma pessoa de aparecer duas vezes no pódio, cada metade abaixo de quem ela na
/// verdade supera.
export function bestDriversRanking(lineup) {
  const porPiloto = new Map();
  for (const term of lineup ?? []) {
    // Contrato anunciado que nunca virou pista não entra: o titular de hoje sem corrida
    // aparece na galeria (é onde se confere quem está no carro), mas um ranking de quem
    // correu não tem o que fazer com quem não correu.
    if (!(term.races > 0)) continue;
    const acumulado = porPiloto.get(term.driverId);
    if (!acumulado) {
      porPiloto.set(term.driverId, {
        driverId: term.driverId,
        name: term.name,
        nationality: term.nationality,
        isPlayer: term.isPlayer,
        stillHere: term.stillHere,
        races: term.races,
        titles: term.titles,
        wins: term.wins,
        podiums: term.podiums,
        bestPosition: term.bestPosition,
        firstYear: term.firstYear,
        lastYear: term.lastYear,
      });
      continue;
    }
    acumulado.races += term.races;
    acumulado.titles += term.titles;
    acumulado.wins += term.wins;
    acumulado.podiums += term.podiums;
    // Zero é "nunca teve colocação", e não a melhor delas.
    if (term.bestPosition > 0 && (acumulado.bestPosition === 0 || term.bestPosition < acumulado.bestPosition)) {
      acumulado.bestPosition = term.bestPosition;
    }
    acumulado.stillHere = acumulado.stillHere || term.stillHere;
    acumulado.nationality = acumulado.nationality || term.nationality;
    if (term.firstYear && term.firstYear < acumulado.firstYear) acumulado.firstYear = term.firstYear;
    if (term.lastYear && term.lastYear > acumulado.lastYear) acumulado.lastYear = term.lastYear;
  }

  // TÍTULO primeiro, e não vitória. Vencer domingo e vencer o ano não são a mesma moeda em
  // escala diferente: um campeão da casa com seis vitórias vale mais para a história dela que
  // um piloto de quinze vitórias que nunca levou o campeonato — e era exatamente isso que a
  // lista dizia ao contrário. Nenhum peso relativo resolveria: quantas vitórias valem um
  // título é uma pergunta sem resposta, e a ordem lexicográfica não precisa dela.
  return [...porPiloto.values()].sort((a, b) => {
    if (b.titles !== a.titles) return b.titles - a.titles;
    if (b.wins !== a.wins) return b.wins - a.wins;
    if (b.podiums !== a.podiums) return b.podiums - a.podiums;
    // A melhor colocação não é mais coluna, mas continua desempatando: entre dois pilotos sem
    // pódio, quem chegou em quarto fez mais que quem nunca passou de décimo, e sem isso o
    // fundo da tabela sairia em ordem alfabética.
    const melhorA = a.bestPosition > 0 ? a.bestPosition : Number.POSITIVE_INFINITY;
    const melhorB = b.bestPosition > 0 ? b.bestPosition : Number.POSITIVE_INFINITY;
    if (melhorA !== melhorB) return melhorA - melhorB;
    if (b.races !== a.races) return b.races - a.races;
    return a.name.localeCompare(b.name);
  });
}
