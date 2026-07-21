// Contexto de briefing da próxima corrida: lógica PURA (sem React, sem store) que
// monta fatos/tese/editorial da prévia pré-corrida. Extraído de NextRaceTab.jsx para
// que o store (useCareerStore) reuse `buildBriefingContext` sem importar um componente
// — quebrando o ciclo store↔componente. Segue o padrão dos irmãos nextRaceBriefing /
// nextRaceEditorial / nextRaceThesis.
import { buildFavoriteExpectationSelection, recentResults } from "./nextRaceBriefing";
import { buildEditorialCopy, classifyChampionshipState } from "./nextRaceEditorial";
import { selectThesis } from "./nextRaceThesis";
export function getFavoriteMedalTone(index) {
  if (index === 0) return "text-[#f5c76d]";
  if (index === 1) return "text-[#d8dfef]";
  if (index === 2) return "text-[#cf8d63]";
  return "text-gray-500";
}


// Cor/rótulo do nível de risco de quebra (card da Sala de Estratégia).
export function riskColor(level) {
  if (level === "alto") return "#f87171";
  if (level === "médio") return "#f0b37a";
  return "#34d399";
}
export function riskLabel(level) {
  if (level === "alto") return "Alto";
  if (level === "médio") return "Médio";
  return "Baixo";
}

export function buildBriefingContext({
  player,
  playerTeam,
  season,
  nextRace,
  nextRaceBriefing,
  driverStandings,
  teamStandings,
  briefingPhraseHistory,
  playerInterests = null,
  breakdownForecast = null,
}) {
  const orderedDrivers = [...driverStandings].sort(
    (left, right) => (left.posicao_campeonato ?? 999) - (right.posicao_campeonato ?? 999),
  );
  const orderedTeams = [...teamStandings].sort(
    (left, right) => (left.posicao ?? 999) - (right.posicao ?? 999),
  );
  const playerStanding =
    orderedDrivers.find((driver) => driver.is_jogador) ??
    orderedDrivers.find((driver) => driver.id === player?.id) ??
    null;
  const standingsTopFive = orderedDrivers.slice(0, 5);
  const leader = standingsTopFive[0] ?? null;
  const trackHistory = nextRaceBriefing?.track_history ?? null;
  const briefingRival = nextRaceBriefing?.primary_rival ?? null;
  const weekendStories = normalizeWeekendStories(nextRaceBriefing?.weekend_stories);
  const teammate =
    playerStanding && playerStanding.equipe_id
      ? orderedDrivers.find(
          (driver) => driver.equipe_id === playerStanding.equipe_id && driver.id !== playerStanding.id,
        ) ?? null
      : null;
  const teamStanding =
    orderedTeams.find((team) => team.id === playerTeam?.id) ?? orderedTeams[0] ?? null;
  const gapToLeader = Math.max(0, (leader?.pontos ?? 0) - (playerStanding?.pontos ?? 0));
  const behindDriver =
    playerStanding && playerStanding.posicao_campeonato > 0
      ? orderedDrivers[playerStanding.posicao_campeonato] ?? null
      : null;
  const gapBehind =
    playerStanding && behindDriver
      ? Math.max(0, (playerStanding.pontos ?? 0) - (behindDriver.pontos ?? 0))
      : null;
  const remainingRounds = Math.max(0, (season?.total_rodadas ?? 0) - (nextRace?.rodada ?? 0));
  const ratedDrivers = orderedDrivers
    .map((driver) => ({
      ...driver,
      rating: buildFavoriteRating(driver),
      formLabel: buildFormLabel(driver),
      formChips: buildFormChips(driver),
    }))
    .sort((left, right) => right.rating - left.rating || left.posicao_campeonato - right.posicao_campeonato);
  const favorites = ratedDrivers
    .slice()
    .sort((left, right) => right.rating - left.rating || left.posicao_campeonato - right.posicao_campeonato)
    .slice(0, 6)
    .map((driver, index) => {
      const selection = buildFavoriteExpectationSelection(driver, index, {
        seasonNumber: season?.numero,
        roundNumber: nextRace?.rodada,
        historyEntries: briefingPhraseHistory?.entries ?? [],
      });

      return {
        ...driver,
        expectation: selection.text,
        expectationPhraseId: selection.phraseId,
        expectationBucketKey: selection.bucketKey,
      };
    });
  const audienceEstimate = nextRace?.event_interest?.display_value ?? estimateAudience(nextRace?.event_interest?.tier_label);
  const totalRounds = Math.max(1, season?.total_rodadas ?? 1);
  const currentRound = Math.max(1, nextRace?.rodada ?? 1);
  const playerCompetitive = ratedDrivers.find((driver) => driver.id === playerStanding?.id) ?? null;
  const leaderCompetitive = ratedDrivers.find((driver) => driver.id === leader?.id) ?? null;
  const outlook = buildCompetitiveOutlook({
    playerStanding,
    leader,
    remainingRounds,
    playerRating: playerCompetitive?.rating ?? 0,
    leaderRating: leaderCompetitive?.rating ?? 0,
  });
  // Estrelato (Fase 3): fração do público/bilheteria que a equipe do jogador puxa
  // (piso + prêmio de fama do lineup). Só entra na narrativa quando há valor real.
  const publicFameShare = nextRace?.public_fame_share ?? null;
  const fameSharePct =
    publicFameShare != null && publicFameShare > 0 ? Math.round(publicFameShare * 100) : null;
  const fameClause =
    fameSharePct != null && fameSharePct >= 1
      ? ` Sua equipe responde por cerca de ${fameSharePct}% do público esperado.`
      : "";
  const attendanceNarrative =
    (audienceEstimate > 0
      ? `A expectativa do paddock aponta para ${formatAudience(audienceEstimate)} de público estimado ao longo do fim de semana.`
      : "O paddock espera bom movimento de público nesta etapa.") + fameClause;
  // Abertura de temporada: enquanto ninguém pontuou, a "tabela" é só ordem de largada
  // (todos com 0 pontos). Tratar gaps/líder/posição como reais produz texto absurdo
  // ("12º, 0 pontos atrás da liderança"). Detectamos isso e usamos o estado "opener".
  const championshipUnderway = orderedDrivers.some((driver) => (driver.pontos ?? 0) > 0);
  const championshipState = classifyChampionshipState({
    playerStanding,
    leader,
    remainingRounds,
    outlook,
    gapBehind,
    championshipUnderway,
  });

  // Fatos curados (PT) da PRÉVIA pré-corrida → enviados ao servidor de IA. Curtos e
  // factuais; o servidor escreve a narrativa + voz da equipe (no idioma do app) só
  // em cima disto. Reaproveita o que já computamos aqui (estado, gap, rival, forma).
  const stateLabel = {
    opener: "na largada da temporada, com tudo ainda por definir",
    leader: "defendendo a liderança do campeonato",
    chase: "perseguindo o líder, com chance real de encurtar a tabela",
    pressure: "sob pressão para proteger a posição na tabela",
    outsider: "longe da briga pelo título, jogando por orgulho e pontos",
    survival: "precisando reagir e recolocar a campanha nos trilhos",
  }[championshipState] ?? "disputando a etapa";
  const recentForm = recentResults(playerStanding)
    .map((r) => (r ? (r.is_dnf ? "DNF" : `P${r.position ?? "?"}`) : null))
    .filter(Boolean)
    .join(", ");
  const targetLabel =
    {
      podium: "brigar pelo pódio",
      top5: "buscar o top 5",
      top8: "somar pontos sólidos no top 8",
    }[outlook?.targetResult] ?? "fazer um fim de semana limpo e sem perdas";
  const playerIsLeader = !!(playerStanding && leader && playerStanding.id === leader.id);
  const topFavorite = favorites?.[0] ?? null;
  const topFavoriteIsPlayer = !!(topFavorite && playerStanding && topFavorite.id === playerStanding.id);
  const leadStory = weekendStories[0] ?? null;
  const climaWet = ["Damp", "Wet", "HeavyRain"].includes(nextRace?.clima);
  const weatherFact = nextRace?.clima
    ? climaWet
      ? `FATOR CLIMA (alto peso): previsão de ${buildWeatherSummary(nextRace.clima).toLowerCase()} — ${buildWeatherNarrative(nextRace.clima)} Pode embaralhar o grid e decidir a corrida.`
      : `Previsão de clima: ${buildWeatherSummary(nextRace.clima)}, sem grandes surpresas no horizonte.`
    : null;
  const audienceRankLabel = buildAudienceRankLabel(nextRace, season);
  const bigEvent = audienceEstimate >= 60000 || /maior|maiores/i.test(audienceRankLabel);
  // IMPORTANTE: descrever o porte pela OCASIÃO, não com superlativo absoluto. O
  // rótulo "maior público da temporada" é só um heurístico de UI (rodada 1 e final)
  // e NÃO é uma comparação real do calendário — uma etapa principal ou a final podem
  // atrair mais. Mandar isso como fato faria a IA cravar algo que não dá pra checar.
  const isFinaleRound = totalRounds > 1 && currentRound === totalRounds;
  const isOpenerRound = currentRound === 1;
  const eventOccasion = isFinaleRound
    ? "grande final da temporada"
    : isOpenerRound
      ? "abertura da temporada"
      : "etapa de destaque do calendário";
  const importanceFact =
    audienceEstimate > 0
      ? bigEvent
        ? `ETAPA DE GRANDE IMPORTÂNCIA: ${eventOccasion} com casa cheia, cerca de ${formatAudience(audienceEstimate)} pessoas esperadas — vitrine e pressão extra pesam aqui.`
        : `Público estimado: cerca de ${formatAudience(audienceEstimate)} pessoas ao longo do fim de semana.`
      : null;
  // Risco de quebra (aviso pré-corrida): só entra no briefing quando é NOTÁVEL — carro
  // confiável não vira assunto. É RISCO, não certeza (o engenheiro pode sugerir poupar).
  const forecastParts = breakdownForecast?.parts ?? [];
  const forecastNotable =
    breakdownForecast?.available &&
    (breakdownForecast.overall_level !== "baixo" || forecastParts.some((p) => p.level !== "baixo"));
  const breakdownRiskFact = forecastNotable
    ? (() => {
        const risky = forecastParts
          .filter((p) => p.level !== "baixo")
          .slice(0, 3)
          .map((p) => `${p.part_name} (risco ${p.level})`)
          .join(", ");
        const geral = breakdownForecast.overall_level === "alto" ? "ALTO" : breakdownForecast.overall_level;
        return `RISCO DE QUEBRA DE PEÇA: risco geral de falha ${geral} nesta corrida${risky ? ` — atenção a ${risky}` : ""}. É risco, não certeza; se fizer sentido, sugira poupar o carro.`;
      })()
    : null;
  // --- TESE DOMINANTE ---------------------------------------------------------
  // Antes: ~23 fatos numa lista plana; a IA se agarrava no único bloco com carga
  // (o DNF) e ignorava o resto. Agora elegemos UM eixo por corrida e organizamos
  // tudo em camadas (EIXO → APOIO → PANO DE FUNDO), dando hierarquia real.
  const lastResult = recentResults(playerStanding)[0] ?? null;
  const climaLabel = climaWet && nextRace?.clima ? buildWeatherSummary(nextRace.clima).toLowerCase() : null;
  const breakdownLevelLabel =
    breakdownForecast?.overall_level === "alto" ? "ALTO" : breakdownForecast?.overall_level ?? null;
  const breakdownPartsLabel = forecastNotable
    ? forecastParts
        .filter((p) => p.level !== "baixo")
        .slice(0, 3)
        .map((p) => `${p.part_name} (risco ${p.level})`)
        .join(", ")
    : null;
  const nemesisRaw = playerInterests?.nemesis ?? null;
  const nemesisSignal = nemesisRaw
    ? { ...nemesisRaw, in_grid: orderedDrivers.some((d) => d.id === nemesisRaw.driver_id) }
    : null;

  const thesis = selectThesis({
    trackName: nextRace?.track_name,
    championshipUnderway,
    playerIsLeader,
    championshipState,
    gapToLeader,
    gapBehind,
    remainingRounds,
    leaderName: !playerIsLeader ? leader?.nome ?? null : null,
    lastResult,
    averageFinish: outlook?.averageFinish ?? null,
    nemesis: nemesisSignal,
    trackHistory,
    climaWet,
    climaLabel,
    breakdownNotable: forecastNotable,
    breakdownLevel: breakdownLevelLabel,
    breakdownParts: breakdownPartsLabel,
    grandStage: isFinaleRound || bigEvent,
    eventOccasion: eventOccasion.charAt(0).toUpperCase() + eventOccasion.slice(1),
    audienceLabel: audienceEstimate > 0 ? formatAudience(audienceEstimate) : null,
  });

  // Template determinístico (fallback quando a IA não responde) dirigido pela MESMA
  // tese. Uma fonte só de verdade para o eixo do fim de semana.
  const editorialCopy = buildEditorialCopy({
    thesis,
    playerStanding,
    leader,
    briefingRival,
    playerTeam,
    nextRace,
    trackHistory,
    gapToLeader,
    remainingRounds,
    nemesisName: nemesisSignal?.in_grid ? nemesisSignal.driver_name : null,
    climaLabel,
    eventOccasion: eventOccasion.charAt(0).toUpperCase() + eventOccasion.slice(1),
  });

  // Cada fato é gerado uma vez, indexado por id. A tese decide quem sobe pro APOIO;
  // o resto vira PANO DE FUNDO. `null` = fato não se aplica a esta corrida.
  const factText = {
    championship_situation: !championshipUnderway
      ? "Abertura da temporada: ninguém pontuou ainda, todo o grid larga do zero e a tabela só começa a se formar nesta etapa."
      : playerStanding
        ? `Situação no campeonato: ${playerStanding.posicao_campeonato}º lugar, ${stateLabel}${gapToLeader > 0 ? `, a ${gapToLeader} pontos da liderança` : ""}.`
        : `Leitura do momento: ${stateLabel}.`,
    objective: championshipUnderway
      ? `Objetivo realista pela forma atual: ${targetLabel}.`
      : "Objetivo da estreia: começar a temporada construindo, sem correr atrás de prejuízo logo de cara.",
    recent_form: recentForm ? `Últimos resultados do piloto: ${recentForm}.` : null,
    avg_finish:
      outlook?.averageFinish != null
        ? `Média de chegada recente: ${outlook.averageFinish.toFixed(1)}º${outlook.winCount > 0 ? `, ${outlook.winCount} vitória(s)` : ""}${outlook.podiumCount > 0 ? `, ${outlook.podiumCount} pódio(s)` : ""} nas últimas corridas.`
        : null,
    leader:
      championshipUnderway && !playerIsLeader && leader?.nome
        ? `Líder do campeonato: ${leader.nome}, ${leader.pontos ?? 0} pontos.`
        : null,
    chaser:
      championshipUnderway && gapBehind != null
        ? `Perseguidor direto na tabela a ${gapBehind} pontos atrás.`
        : null,
    rival_direct:
      championshipUnderway && briefingRival?.driver_name
        ? `Rival direto: ${briefingRival.driver_name} (${briefingRival.championship_position}º), ${briefingRival.is_ahead ? "à frente" : "atrás"} por ${briefingRival.gap_points} ponto(s).`
        : null,
    rivalry_label: briefingRival?.rivalry_label
      ? championshipUnderway
        ? `Essa rivalidade é conhecida como "${briefingRival.rivalry_label}".`
        : `Rivalidade que vem de temporadas anteriores: "${briefingRival.rivalry_label}" (${briefingRival.driver_name}).`
      : null,
    // O nemesis só vira fato solto quando NÃO é o próprio eixo (senão duplica).
    nemesis:
      nemesisSignal?.in_grid && thesis.key !== "nemesis"
        ? `Seu nemesis está no grid: ${nemesisSignal.driver_name}${nemesisSignal.label ? ` ("${nemesisSignal.label}")` : ""}${nemesisSignal.chapters > 0 ? ` — confronto direto ${nemesisSignal.h2h_player_wins}-${nemesisSignal.h2h_rival_wins}` : ""}.`
        : null,
    track_rivals:
      (playerInterests?.rivais ?? [])
        .filter((r) => orderedDrivers.some((d) => d.id === r.driver_id))
        .map(
          (r) =>
            `Rival de pista no grid: ${r.driver_name}${r.label ? ` ("${r.label}")` : ""}${r.chapters > 0 ? ` — ${r.h2h_player_wins}-${r.h2h_rival_wins}` : ""}.`,
        )
        .join(" ") || null,
    track_history: trackHistory?.has_data
      ? `Histórico nesta pista: ${trackHistory.starts} largada(s)${trackHistory.best_finish != null ? `, melhor resultado ${trackHistory.best_finish}º` : ""}${trackHistory.dnfs > 0 ? `, ${trackHistory.dnfs} abandono(s)` : ""}.`
      : null,
    track_last:
      trackHistory?.has_data && trackHistory.last_finish != null
        ? `Última passagem por aqui terminou em ${trackHistory.last_finish}º${trackHistory.last_visit_season != null ? ` (temporada ${trackHistory.last_visit_season})` : ""}.`
        : null,
    constructors:
      championshipUnderway && teamStanding
        ? `Equipe ${playerTeam?.nome ?? ""} está em ${teamStanding.posicao}º entre os construtores${teamStanding.pontos != null ? ` (${teamStanding.pontos} pts)` : ""}.`.replace("  ", " ")
        : null,
    teammate: teammate?.nome
      ? championshipUnderway
        ? `Companheiro de equipe: ${teammate.nome} (${teammate.posicao_campeonato}º no campeonato) — referência interna do box.`
        : `Companheiro de equipe: ${teammate.nome} — referência interna do box já na estreia.`
      : null,
    favorite: topFavorite?.nome
      ? topFavoriteIsPlayer
        ? `A imprensa coloca o próprio ${topFavorite.nome} como favorito da etapa.`
        : `Favorito da etapa pela imprensa: ${topFavorite.nome}.`
      : null,
    weather: weatherFact,
    importance: importanceFact,
    breakdown: breakdownRiskFact,
    story_lead: leadStory
      ? `Pauta do fim de semana: ${leadStory.title}${leadStory.summary ? ` — ${leadStory.summary}` : ""}.`
      : null,
    story_others: weekendStories.slice(1, 3).map((s) => `Outra pauta: ${s.title}.`).join(" ") || null,
  };

  // Ordem estável em que os fatos aparecem dentro de cada camada.
  const FACT_ORDER = [
    "championship_situation",
    "objective",
    "recent_form",
    "avg_finish",
    "leader",
    "chaser",
    "rival_direct",
    "rivalry_label",
    "nemesis",
    "track_rivals",
    "track_history",
    "track_last",
    "constructors",
    "teammate",
    "favorite",
    "weather",
    "importance",
    "breakdown",
    "story_lead",
    "story_others",
  ];

  const cenario = [
    `Corrida: ${nextRace?.track_name ?? "a etapa"} — temporada ${season?.ano ?? "atual"}, etapa ${currentRound} de ${totalRounds}.`,
    player?.nome ? `Piloto acompanhado pelo leitor: ${player.nome} (equipe ${playerTeam?.nome ?? "sem equipe"}).` : null,
  ]
    .filter(Boolean)
    .join(" ");

  const apoio = [];
  const fundo = [];
  for (const id of FACT_ORDER) {
    const text = factText[id];
    if (!text) continue;
    if (thesis.support.has(id)) apoio.push(text);
    else fundo.push(text);
  }

  const aiFacts = [
    `CENÁRIO: ${cenario}`,
    "",
    "EIXO DA CORRIDA — o gancho que esta prévia deve desenvolver (é o coração do texto, não uma linha solta; construa a narrativa a partir dele):",
    thesis.statement,
    ...(apoio.length
      ? ["", "APOIO — fatos que sustentam o eixo (use os que reforçarem a história):", ...apoio.map((t) => `- ${t}`)]
      : []),
    ...(fundo.length
      ? [
          "",
          "PANO DE FUNDO — contexto secundário (use com parcimônia, só se couber; NÃO liste como estatística nem force todos):",
          ...fundo.map((t) => `- ${t}`),
        ]
      : []),
  ].join("\n");

  return {
    aiFacts,
    thesisKey: thesis.key,
    thesisTitle: thesis.title,
    audienceEstimate,
    audienceRankLabel: buildAudienceRankLabel(nextRace, season),
    eventDateShort: formatEventSummaryDate(nextRace?.display_date),
    interestLabel: nextRace?.event_interest?.tier_label ?? "Padrão da temporada",
    broadcastLabel: isLiveCoverageEvent(nextRace, season) ? "Cobertura" : "Expectativa",
    broadcastValue: isLiveCoverageEvent(nextRace, season)
      ? "Ao vivo"
      : buildTeamExpectationValue({ playerStanding, teamStanding, gapToLeader, outlook }),
    headline: editorialCopy.headline,
    paragraphs: editorialCopy.paragraphs,
    goals: buildGoals({
      playerStanding,
      teammate,
      teamStanding,
      gapToLeader,
      remainingRounds,
      outlook,
      driverAbove: playerStanding?.posicao_campeonato > 1
        ? orderedDrivers[playerStanding.posicao_campeonato - 2] ?? null
        : null,
    }),
    favorites,
    championshipTable: orderedDrivers,
    constructorsTable: orderedTeams,
    playerTeamId: playerStanding?.equipe_id ?? playerTeam?.id ?? null,
    standingsTopFive,
    gapToLeaderLabel: gapToLeader === 0 ? "Liderança" : `${gapToLeader} pts`,
    gapBehindLabel: gapBehind == null ? "Sem perseguidor direto" : `${gapBehind} pts`,
    progressPercent: Math.max(5, Math.min(100, Math.round((currentRound / totalRounds) * 100))),
    progressLabel: `${currentRound}/${totalRounds}`,
    quote: editorialCopy.quote,
    teamVoiceLabel: playerTeam?.nome ?? "Equipe do jogador",
    teamColor: playerTeam?.cor_primaria ?? null,
    attendanceNarrative,
    weatherIcon: buildWeatherIcon(nextRace?.clima),
    weatherSummary: buildWeatherSummary(nextRace?.clima),
    weatherNarrative: buildWeatherNarrative(nextRace?.clima),
    trackTemperatureLabel:
      nextRace?.temperatura == null ? "-" : `${Math.round(nextRace.temperatura)}°C`,
    temperatureNarrative: buildTemperatureNarrative(nextRace?.temperatura),
    trackConditionLabel: buildTrackConditionLabel(nextRace?.clima),
    boxNarrative: buildBoxNarrative(nextRace?.clima),
    timePeriodPrefix: buildTimePeriodPrefix(nextRace?.horario),
    timePeriodHighlight: buildTimePeriodHighlight(nextRace?.horario),
    actionHint: editorialCopy.actionHint,
    weekendStories,
  };
}

function normalizeWeekendStories(stories) {
  if (!Array.isArray(stories)) {
    return [];
  }

  return stories.map((story) => ({
    id: story.id,
    icon: story.icon,
    title: story.title,
    summary: story.summary,
    importanceLabel: story.importance ?? "Contexto",
  }));
}

function buildCompetitiveOutlook({ playerStanding, leader, remainingRounds, playerRating, leaderRating }) {
  if (!playerStanding || !leader) {
    return {
      titleFight: "neutral",
      targetResult: "clean",
    };
  }

  const recentKnown = recentResults(playerStanding).filter(Boolean);
  const averageFinish = recentKnown.length
    ? recentKnown.reduce((total, result) => total + (result.position ?? 12), 0) / recentKnown.length
    : null;
  const topFiveCount = recentKnown.filter((result) => !result.is_dnf && (result.position ?? 99) <= 5).length;
  const podiumCount = recentKnown.filter((result) => !result.is_dnf && (result.position ?? 99) <= 3).length;
  const winCount = recentKnown.filter((result) => !result.is_dnf && result.position === 1).length;
  const racesLeftIncludingCurrent = Math.max(1, remainingRounds + 1);
  const gapToLeader = Math.max(0, (leader.pontos ?? 0) - (playerStanding.pontos ?? 0));
  const ratingGap = Math.max(0, leaderRating - playerRating);
  const weakRecentForm = averageFinish != null && averageFinish >= 7;
  const strongRecentForm = averageFinish != null && averageFinish <= 4.5;
  const titleLongshot =
    playerStanding.posicao_campeonato >= 6 ||
    gapToLeader > racesLeftIncludingCurrent * 12 ||
    (racesLeftIncludingCurrent <= 2 && (weakRecentForm || topFiveCount === 0 || ratingGap >= 10));
  const titleContender =
    gapToLeader <= racesLeftIncludingCurrent * 6 &&
    (strongRecentForm || topFiveCount >= 2 || podiumCount >= 1 || ratingGap <= 4);

  let titleFight = "outsider";
  if (playerStanding.posicao_campeonato === 1) {
    titleFight = "leader";
  } else if (titleContender) {
    titleFight = "contender";
  } else if (titleLongshot) {
    titleFight = "longshot";
  }

  let targetResult = "top8";
  if (winCount >= 1 || podiumCount >= 2 || playerRating >= 80) {
    targetResult = "podium";
  } else if (topFiveCount >= 1 || (averageFinish != null && averageFinish <= 6)) {
    targetResult = "top5";
  }

  return {
    titleFight,
    targetResult,
    averageFinish,
    topFiveCount,
    podiumCount,
    winCount,
    racesLeftIncludingCurrent,
    gapToLeader,
  };
}

function buildFavoriteRating(driver) {
  const recentScore = recentResults(driver).reduce((total, result) => {
    if (!result) return total;
    if (result.is_dnf) return total - 10;
    return total + Math.max(0, 14 - (result.position ?? 12));
  }, 0);

  const rawScore =
    (driver.skill ?? 70) * 0.74 +
    (driver.pontos ?? 0) * 0.24 +
    (driver.vitorias ?? 0) * 6 +
    (driver.podios ?? 0) * 1.4 +
    recentScore;

  return Math.max(52, Math.min(98, Math.round(rawScore / 2.1)));
}

function buildFormLabel(driver) {
  const snapshot = recentResults(driver)
    .map((result) => {
      if (!result) return "P--";
      if (result.is_dnf) return "DNF";
      return `P${result.position ?? "--"}`;
    })
    .join(" - ");

  return snapshot ? `Forma recente: ${snapshot}` : "Sem histórico recente.";
}

function buildFormChips(driver) {
  const chips = recentResults(driver).map((result) => {
    if (!result) {
      return {
        label: "Sem dado",
        tone: "border-white/10 bg-white/[0.04] text-text-secondary",
      };
    }

    if (result.is_dnf) {
      return {
        label: "DNF",
        tone: "border-status-red/30 bg-status-red/12 text-status-red",
      };
    }

    const position = result.position ?? 99;
    if (position === 1) {
      return {
        label: "P1",
        tone: "border-podium-gold/30 bg-podium-gold/10 text-podium-gold",
      };
    }
    if (position === 2) {
      return {
        label: "P2",
        tone: "border-podium-silver/30 bg-podium-silver/10 text-podium-silver",
      };
    }
    if (position === 3) {
      return {
        label: "P3",
        tone: "border-podium-bronze/30 bg-podium-bronze/10 text-podium-bronze",
      };
    }

    if (position <= 6) {
      return {
        label: `P${position}`,
        tone: "border-accent-primary/25 bg-accent-primary/10 text-accent-primary",
      };
    }

    return {
      label: `P${position}`,
      tone: "border-white/10 bg-white/[0.04] text-text-secondary",
    };
  });

  return chips.length > 0
    ? chips
    : [{ label: "Sem histórico", tone: "border-white/10 bg-white/[0.04] text-text-secondary" }];
}

function getFavoritePositionTone(index) {
  if (index === 0) return "text-[#f5c76d]";
  if (index === 1) return "text-[#d8dfef]";
  if (index === 2) return "text-[#cf8d63]";
  return "text-text-primary";
}

function buildGoals({ playerStanding, teammate, teamStanding, gapToLeader, remainingRounds, outlook, driverAbove }) {
  const teamGoal =
    teamStanding?.posicao === 1
      ? "Manter a liderança do campeonato de equipes."
      : teamStanding
        ? `Levar a equipe ao top ${Math.min(3, teamStanding.posicao)} entre os construtores.`
        : "Sair da etapa com pontos fortes para a equipe.";

  const playerPos = playerStanding?.posicao_campeonato ?? 0;
  const teammatePos = teammate?.posicao_campeonato ?? 0;
  const teammateIsClose = teammate && Math.abs(playerPos - teammatePos) <= 2;

  const personalGoal = teammateIsClose
    ? `Terminar a frente de ${teammate.nome} na leitura interna do box.`
    : driverAbove
      ? `Superar ${driverAbove.nome} e subir para o ${playerPos - 1}º no campeonato.`
      : "Executar um fim de semana limpo, sem perdas na largada.";

  let championshipGoal = "Pontuar forte para manter o campeonato vivo.";
  if (playerStanding?.posicao_campeonato === 1) {
    championshipGoal = "Controlar os danos e sair da etapa ainda no topo.";
  } else if (outlook?.titleFight === "longshot") {
    championshipGoal = "Somar o máximo de pontos possível e manter o campeonato respeitável até o fim.";
  } else if (gapToLeader <= 7) {
    championshipGoal = "Atacar a liderança agora que a distância é curta.";
  } else if (remainingRounds <= 3) {
    championshipGoal = "Maximizar pontos agora para não deixar a temporada escapar.";
  }

  return [
    { label: "Meta da equipe", value: teamGoal },
    { label: "Meta pessoal", value: personalGoal },
    { label: "Meta do campeonato", value: championshipGoal },
  ];
}

function buildWeatherSummary(clima) {
  if (clima === "HeavyRain") return "Chuva forte";
  if (clima === "Wet") return "Chuva";
  if (clima === "Damp") return "Úmido";
  return "Seco";
}

function buildWeatherIcon(clima) {
  if (clima === "HeavyRain") return "⛈";
  if (clima === "Wet") return "🌧";
  if (clima === "Damp") return "🌦";
  return "☀";
}

function buildWeatherNarrative(clima) {
  if (clima === "HeavyRain") return "Corrida reativa, spray alto e erro caro.";
  if (clima === "Wet") return "Pista pedindo paciência na entrada e tração limpa.";
  if (clima === "Damp") return "Linha mudando rápido volta a volta.";
  return "Janela previsível para empurrar mais cedo.";
}

function buildTemperatureNarrative(temperatura) {
  if (temperatura == null) return "Leitura térmica ainda indefinida para o fim de semana.";
  if (temperatura <= 16) return "Ar frio ajudando a segurar desgaste.";
  if (temperatura <= 28) return "Temperatura equilibrada para stints consistentes.";
  return "Calor cobrando mais do conjunto de pneus.";
}

function buildTrackConditionLabel(clima) {
  if (clima === "HeavyRain") return "Visibilidade apertada";
  if (clima === "Wet") return "Trajetória molhada";
  if (clima === "Damp") return "Janela instável";
  return "Alta aderência";
}

function buildBoxNarrative(clima) {
  if (clima === "HeavyRain") return "Linha ideal curta e comunicação constante.";
  if (clima === "Wet") return "Trajetória molhada e janela sensível.";
  if (clima === "Damp") return "Aderencia oscilando fora do trilho seco.";
  return "Alta aderência para atacar mais cedo.";
}

function formatEventSummaryDate(displayDate) {
  if (!displayDate) return "--/--";

  const [year, month, day] = displayDate.split("-");
  if (!year || !month || !day) return displayDate;
  return `${day}/${month}`;
}

function buildTimePeriodPrefix(horario) {
  const hour = parseHour(horario);
  if (hour == null) return "Horário ";
  if (hour < 6) return "Madrugada de ";
  if (hour < 12) return "Início da ";
  if (hour < 18) return "Início da ";
  return "Início da ";
}

function buildTimePeriodHighlight(horario) {
  const hour = parseHour(horario);
  if (hour == null) return "pista";
  if (hour < 6) return "madrugada";
  if (hour < 12) return "manhã";
  if (hour < 18) return "tarde";
  return "noite";
}

function parseHour(horario) {
  if (typeof horario !== "string") return null;
  const [rawHour] = horario.split(":");
  const parsed = Number.parseInt(rawHour, 10);
  return Number.isNaN(parsed) ? null : parsed;
}

function buildAudienceRankLabel(nextRace, season) {
  const totalRounds = Math.max(1, season?.total_rodadas ?? 1);
  const round = nextRace?.rodada ?? 1;
  const interestTier = nextRace?.event_interest?.tier_label?.toLowerCase() ?? "";

  if (round === 1 || round === totalRounds) {
    return "Maior público da temporada";
  }

  if (interestTier.includes("principal")) {
    return "3º Maior público da temporada";
  }

  if (interestTier.includes("alto")) {
    return "Entre os maiores públicos da temporada";
  }

  return "Movimento forte dentro da temporada";
}

function isLiveCoverageEvent(nextRace, season) {
  const totalRounds = Math.max(1, season?.total_rodadas ?? 1);
  const round = nextRace?.rodada ?? 1;
  const interestTier = nextRace?.event_interest?.tier_label?.toLowerCase() ?? "";

  return round === 1 || round === totalRounds || interestTier.includes("principal");
}

function buildTeamExpectationValue({ playerStanding, teamStanding, gapToLeader, outlook }) {
  if (playerStanding?.posicao_campeonato === 1) {
    return "Controlar a ponta";
  }

  if (outlook?.titleFight === "longshot") {
    return "Pontuar forte";
  }

  if (gapToLeader <= 10) {
    return "Pressionar a frente";
  }

  if ((teamStanding?.posicao ?? 99) <= 3) {
    return "Top 5 no radar";
  }

  return "Fim de semana limpo";
}

function estimateAudience(tierLabel) {
  if (tierLabel?.toLowerCase().includes("principal")) return 84000;
  if (tierLabel?.toLowerCase().includes("alto")) return 62000;
  if (tierLabel?.toLowerCase().includes("moderado")) return 41000;
  return 28000;
}

export function formatAudience(value) {
  return value ? value.toLocaleString("pt-BR") : "-";
}



export function getReadableTeamColor(color) {
  if (!color || !/^#([0-9a-f]{6})$/i.test(color)) {
    return "#58a6ff";
  }

  const hex = color.slice(1);
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  const luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;

  if (luminance < 0.32) {
    const mixWithWhite = 0.58;
    const boost = (channel) => Math.round(channel + (255 - channel) * mixWithWhite);
    return `rgb(${boost(r)}, ${boost(g)}, ${boost(b)})`;
  }

  return color;
}
