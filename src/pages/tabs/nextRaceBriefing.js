function phrase(id, text) {
  return { id, text };
}

const FAVORITE_EXPECTATION_GROUPS = {
  p1: {
    controlled: [
      phrase(
        "p1-controlled-1",
        "Chega como a principal referência da etapa, embalado pela sequência mais solida entre os nomes da frente.",
      ),
      phrase(
        "p1-controlled-2",
        "Abre o fim de semana como parametro do grid, sustentado por uma fase que vem entregando controle e pontos grandes.",
      ),
    ],
    charging: [
      phrase(
        "p1-charging-1",
        "Puxa a fila dos favoritos com moral alta, trazendo ritmo recente de quem sabe controlar a ponta do fim de semana.",
      ),
      phrase(
        "p1-charging-2",
        "Assume o papel de homem a ser batido nesta etapa, apoiado por uma tendência forte de crescimento nas ultimas corridas.",
      ),
    ],
    volatile: [
      phrase(
        "p1-volatile-1",
        "Segue como a referência natural da etapa, mas o paddock quer ver essa velocidade virar uma corrida limpa do inicio ao fim.",
      ),
      phrase(
        "p1-volatile-2",
        "Ainda chega como nome central da frente, embora carregue um recado claro: transformar ritmo em execução sem erros.",
      ),
    ],
    stable: [
      phrase(
        "p1-stable-1",
        "Entra na etapa como eixo da disputa, com campanha estável o bastante para ditar o tom dos carros da frente.",
      ),
      phrase(
        "p1-stable-2",
        "Continua sendo a referência mais confiavel do grid, mesmo sem abrir uma margem absoluta sobre os perseguidores imediatos.",
      ),
    ],
    baseline: [
      phrase(
        "p1-baseline-1",
        "Chega como o nome de maior peso esportivo da etapa, com leitura de campeonato suficiente para comandar a frente.",
      ),
      phrase(
        "p1-baseline-2",
        "Parte como referência natural do fim de semana, apoiado pelo conjunto mais forte entre pontuacao, ritmo e presenca na frente.",
      ),
    ],
  },
  p2: {
    controlled: [
      phrase(
        "p2-controlled-1",
        "Entra no fim de semana com leitura clara de primeira fila, sustentado por resultados fortes e pouca margem para erro.",
      ),
      phrase(
        "p2-controlled-2",
        "Chega como perseguidor direto mais consistente da etapa, com pacote real para empurrar o favorito desde a largada.",
      ),
    ],
    charging: [
      phrase(
        "p2-charging-1",
        "Aparece como o nome mais pronto para atacar a frente, embalado por uma curva recente de crescimento bem visivel.",
      ),
      phrase(
        "p2-charging-2",
        "Traz argumento real para pressionar pela ponta, com forma em alta e ritmo de quem pode mexer na hierarquia da etapa.",
      ),
    ],
    volatile: [
      phrase(
        "p2-volatile-1",
        "Tem velocidade para andar na primeira fila, mas ainda precisa converter esse teto de performance em um domingo limpo.",
      ),
      phrase(
        "p2-volatile-2",
        "Chega com leitura de ataque real a frente, desde que consiga estabilizar uma campanha que ainda alterna picos e escapes.",
      ),
    ],
    stable: [
      phrase(
        "p2-stable-1",
        "Sustenta uma candidatura forte a primeira fila, apoiado por regularidade recente e presenca constante no bloco da frente.",
      ),
      phrase(
        "p2-stable-2",
        "Entra como perseguidor credenciado da etapa, com consistência suficiente para pressionar o topo se encaixar a classificação.",
      ),
    ],
    baseline: [
      phrase(
        "p2-baseline-1",
        "Comeca o fim de semana como nome firme de primeira fila, carregando material para incomodar os carros da frente.",
      ),
      phrase(
        "p2-baseline-2",
        "Abre a etapa com credencial real de ataque, apoiado por um pacote que o mantem perto dos lideres do campeonato.",
      ),
    ],
  },
  p3: {
    controlled: [
      phrase(
        "p3-controlled-1",
      "Chega no bloco de pódio com base técnica forte, apoiado por resultados recentes que o mantém perto da frente.",
      ),
      phrase(
        "p3-controlled-2",
        "Entra como nome serio para o pódio, sustentado por uma campanha recente limpa e competitiva.",
      ),
    ],
    charging: [
      phrase(
        "p3-charging-1",
        "Vem ganhando corpo na disputa da frente e aparece como candidato real a entrar no pódio desta etapa.",
      ),
      phrase(
        "p3-charging-2",
        "A tendência recente o coloca dentro do bloco mais quente do pódio, com margem clara para crescer no fim de semana.",
      ),
    ],
    volatile: [
      phrase(
        "p3-volatile-1",
        "Tem teto para se meter no pódio, mas a leitura do paddock ainda passa por transformar velocidade em constancia.",
      ),
      phrase(
        "p3-volatile-2",
        "Surge com potencial de top 3, embora ainda carregue uma dose de instabilidade que pede corrida bem executada.",
      ),
    ],
    stable: [
      phrase(
        "p3-stable-1",
        "A consistência recente o mantem no radar do pódio, mesmo sem o mesmo teto bruto dos dois nomes mais fortes.",
      ),
      phrase(
        "p3-stable-2",
        "Chega como presenca firme no bloco da frente, com repertorio suficiente para sustentar uma corrida de pódio.",
      ),
    ],
    baseline: [
      phrase(
        "p3-baseline-1",
        "Parte como candidato legitimo ao pódio, com campanha forte o bastante para andar na mesma conversa dos favoritos.",
      ),
      phrase(
        "p3-baseline-2",
        "Abre a etapa dentro do recorte de pódio, apoiado por um conjunto solido de pontos, ritmo e presenca na frente.",
      ),
    ],
  },
  p4: {
    controlled: [
      phrase(
        "p4-controlled-1",
        "Aparece como ameaca real ao top 5, com base recente suficientemente limpa para punir qualquer erro a frente.",
      ),
      phrase(
        "p4-controlled-2",
        "Chega um degrau abaixo do bloco de pódio, mas com consistencia para roubar pontos pesados se a corrida abrir.",
      ),
    ],
    charging: [
      phrase(
        "p4-charging-1",
        "Vem em ascensao e entra como nome perigoso para atravessar o top 5 caso a prova saia do roteiro esperado.",
      ),
      phrase(
        "p4-charging-2",
        "A curva recente de crescimento o coloca como uma ameaca concreta aos nomes mais estabelecidos da frente.",
      ),
    ],
    volatile: [
      phrase(
        "p4-volatile-1",
        "Tem velocidade para se infiltrar no bloco alto da etapa, mas ainda carrega uma leitura de risco maior que os rivais diretos.",
      ),
      phrase(
        "p4-volatile-2",
        "Surge como carta agressiva do grid: teto interessante, embora a execução ainda oscile demais para confiar cegamente.",
      ),
    ],
    stable: [
      phrase(
        "p4-stable-1",
        "Ocupa o segundo pelotao da frente com consistencia, pronto para capitalizar qualquer corrida quebrada entre os favoritos.",
      ),
      phrase(
        "p4-stable-2",
        "Mantem perfil forte de ameaca secundaria, sustentado por regularidade suficiente para somar muito se a etapa embaralhar.",
      ),
    ],
    baseline: [
      phrase(
        "p4-baseline-1",
        "Entra como nome perigoso para esticar a disputa do top 5, especialmente se o fim de semana abrir espaço na frente.",
      ),
      phrase(
        "p4-baseline-2",
        "Abre a etapa como ameaca secundaria bem posicionada, com pacote para transformar caos em pontos grandes.",
      ),
    ],
  },
  p5: {
    controlled: [
      phrase(
        "p5-controlled-1",
        "Corre por fora, mas chega com uma base limpa o bastante para entrar cedo no radar dos pontos grandes.",
      ),
      phrase(
        "p5-controlled-2",
        "Aparece como outsider bem arrumado da etapa, com consistência recente para incomodar o top 5.",
      ),
    ],
    charging: [
      phrase(
        "p5-charging-1",
        "Vem subindo no momento certo e surge como o outsider mais vivo para surpreender no bloco principal da corrida.",
      ),
      phrase(
        "p5-charging-2",
        "A tendência recente o mantem por fora apenas no papel, porque o ritmo já o coloca no radar da etapa.",
      ),
    ],
    volatile: [
      phrase(
        "p5-volatile-1",
        "Chega como nome de surpresa, ainda tentando trocar oscilacao por uma corrida que realmente o coloque na conversa da frente.",
      ),
      phrase(
        "p5-volatile-2",
        "Tem margem para aparecer bem acima do previsto, mas a campanha recente ainda mistura lampejos fortes com perdas evitaveis.",
      ),
    ],
    stable: [
      phrase(
        "p5-stable-1",
        "Segue por fora da hierarquia principal, mas com regularidade suficiente para aproveitar qualquer oportunidade de top 5.",
      ),
      phrase(
        "p5-stable-2",
        "Entra no radar como outsider competitivo, apoiado por uma sequência que o mantem perto do bloco forte da etapa.",
      ),
    ],
    baseline: [
      phrase(
        "p5-baseline-1",
        "Corre por fora, mas pode se transformar em surpresa real se repetir o nivel recente quando a prova apertar.",
      ),
      phrase(
        "p5-baseline-2",
        "Abre a etapa no grupo de vigilancia do paddock, com margem para crescer se os favoritos abrirem brecha.",
      ),
    ],
  },
};

const POSITION_KEYS = ["p1", "p2", "p3", "p4", "p5"];
const PROFILE_OFFSETS = {
  controlled: 0,
  charging: 2,
  volatile: 4,
  stable: 6,
  baseline: 8,
};

export const FAVORITE_EXPECTATION_POOLS = Object.fromEntries(
  Object.entries(FAVORITE_EXPECTATION_GROUPS).map(([key, groups]) => [key, Object.values(groups).flat()]),
);

export function buildFavoriteExpectation(driver, index, options = {}) {
  return buildFavoriteExpectationSelection(driver, index, options).text;
}

export function buildFavoriteExpectationSelection(driver, index, options = {}) {
  const bucketKey = POSITION_KEYS[Math.max(0, Math.min(index, POSITION_KEYS.length - 1))] ?? "p5";
  const roundNumber = options.roundNumber ?? null;
  const seasonNumber = options.seasonNumber ?? null;
  const historyEntries = Array.isArray(options.historyEntries) ? options.historyEntries : [];
  const form = buildFavoriteFormContext(driver);
  const profile = resolveFormProfile(driver, form);
  const allBucketPhrases = FAVORITE_EXPECTATION_POOLS[bucketKey];
  const roundPinned = resolvePinnedSelection({
    bucketKey,
    roundNumber,
    seasonNumber,
    driverId: driver?.id,
    historyEntries,
    candidates: allBucketPhrases,
  });

  if (roundPinned) {
    return roundPinned;
  }

  const recentPhraseIds = resolveRecentPhraseIds({
    bucketKey,
    roundNumber,
    seasonNumber,
    driverId: driver?.id,
    historyEntries,
  });
  const preferredPool = FAVORITE_EXPECTATION_GROUPS[bucketKey][profile];
  let candidates = preferredPool.filter((entry) => !recentPhraseIds.has(entry.id));

  if (candidates.length === 0) {
    candidates = allBucketPhrases.filter((entry) => !recentPhraseIds.has(entry.id));
  }

  if (candidates.length === 0) {
    candidates = allBucketPhrases;
  }

  const variantIndex = resolveVariantIndex(driver, form, profile, candidates.length);
  const choice = candidates[variantIndex] ?? candidates[0] ?? allBucketPhrases[0];

  return {
    phraseId: choice.id,
    text: choice.text,
    bucketKey,
    profile,
  };
}

export function buildFavoriteFormContext(driver) {
  const recent = recentResults(driver).filter(Boolean);
  const cleanResults = recent.filter((result) => !result.is_dnf);
  const positions = cleanResults.map((result) => result.position ?? 99);
  const latestPosition = cleanResults[0]?.position ?? null;
  const oldestPosition = cleanResults[cleanResults.length - 1]?.position ?? null;
  let trend = "stable";

  if (latestPosition != null && oldestPosition != null) {
    if (latestPosition <= oldestPosition - 2) {
      trend = "up";
    } else if (latestPosition >= oldestPosition + 2) {
      trend = "down";
    }
  }

  return {
    averageFinish: positions.length
      ? positions.reduce((total, position) => total + position, 0) / positions.length
      : null,
    topFiveCount: positions.filter((position) => position <= 5).length,
    podiumCount: positions.filter((position) => position <= 3).length,
    bestFinish: positions.length ? Math.min(...positions) : null,
    dnfCount: recent.filter((result) => result?.is_dnf).length,
    trend,
  };
}

export function recentResults(driver) {
  return [...(driver?.results ?? [])]
    .filter((result) => result != null)
    .slice(-3)
    .reverse();
}

function resolvePinnedSelection({ bucketKey, roundNumber, seasonNumber, driverId, historyEntries, candidates }) {
  if (roundNumber == null || seasonNumber == null || !driverId) {
    return null;
  }

  const pinnedEntry = historyEntries.find(
    (entry) =>
      entry.season_number === seasonNumber &&
      entry.round_number === roundNumber &&
      entry.driver_id === driverId &&
      entry.bucket_key === bucketKey,
  );

  if (!pinnedEntry) {
    return null;
  }

  const match = candidates.find((candidate) => candidate.id === pinnedEntry.phrase_id);
  if (!match) {
    return null;
  }

  return {
    phraseId: match.id,
    text: match.text,
    bucketKey,
    profile: "persisted",
  };
}

function resolveRecentPhraseIds({ bucketKey, roundNumber, seasonNumber, driverId, historyEntries }) {
  if (seasonNumber == null || !driverId) {
    return new Set();
  }

  return new Set(
    historyEntries
      .filter(
        (entry) =>
          entry.season_number === seasonNumber &&
          entry.driver_id === driverId &&
          entry.bucket_key === bucketKey &&
          entry.round_number !== roundNumber,
      )
      .sort((left, right) => right.round_number - left.round_number)
      .slice(0, 5)
      .map((entry) => entry.phrase_id),
  );
}

function resolveFormProfile(driver, form) {
  if (form.dnfCount >= 1) {
    return "volatile";
  }

  if (
    form.trend === "up" &&
    (form.topFiveCount >= 2 || form.podiumCount >= 1 || (driver.rating ?? 0) >= 80)
  ) {
    return "charging";
  }

  if (form.averageFinish != null && form.averageFinish <= 3 && form.topFiveCount >= 2) {
    return "controlled";
  }

  if (form.topFiveCount >= 2 || ((driver.rating ?? 0) >= 76 && (form.bestFinish ?? 99) <= 5)) {
    return "stable";
  }

  return "baseline";
}

function resolveVariantIndex(driver, form, profile, candidateCount) {
  if (candidateCount <= 1) {
    return 0;
  }

  const profileOffset = PROFILE_OFFSETS[profile] ?? PROFILE_OFFSETS.baseline;
  const score =
    ((driver.posição_campeonato ?? 0) * 7) +
    ((driver.rating ?? 0) * 3) +
    (form.topFiveCount * 11) +
    (form.podiumCount * 13) +
    ((form.bestFinish ?? 0) * 5) +
    (form.dnfCount * 17);

  return (score + profileOffset) % candidateCount;
}
