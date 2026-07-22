// Contexto de briefing da próxima corrida: lógica PURA (sem React, sem store) que
// monta fatos/tese/editorial da prévia pré-corrida. Extraído de NextRaceTab.jsx para
// que o store (useCareerStore) reuse `buildBriefingContext` sem importar um componente
// — quebrando o ciclo store↔componente. Segue o padrão dos irmãos nextRaceBriefing /
// nextRaceEditorial / nextRaceThesis.
import { buildFavoriteExpectationSelection, recentResults } from "./nextRaceBriefing";
import { currentLang, ordinal } from "../../i18n/format.js";
import i18n from "../../i18n/index.js";
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

// Palavra do nível de risco DENTRO dos fatos de IA (minúscula, ex.: "risco alto").
// Separada de `riskLabel` (rótulo capitalizado do card de display).
function riskLevelWord(level) {
  if (level === "alto") return i18n.t("raceContext.breakdownRisk.wordHigh");
  if (level === "médio") return i18n.t("raceContext.breakdownRisk.wordMedium");
  return i18n.t("raceContext.breakdownRisk.wordLow");
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
    opener: i18n.t("raceContext.state.opener"),
    leader: i18n.t("raceContext.state.leader"),
    chase: i18n.t("raceContext.state.chase"),
    pressure: i18n.t("raceContext.state.pressure"),
    outsider: i18n.t("raceContext.state.outsider"),
    survival: i18n.t("raceContext.state.survival"),
  }[championshipState] ?? i18n.t("raceContext.state.default");
  const recentForm = recentResults(playerStanding)
    .map((r) => (r ? (r.is_dnf ? "DNF" : `P${r.position ?? "?"}`) : null))
    .filter(Boolean)
    .join(", ");
  const targetLabel =
    {
      podium: i18n.t("raceContext.target.podium"),
      top5: i18n.t("raceContext.target.top5"),
      top8: i18n.t("raceContext.target.top8"),
    }[outlook?.targetResult] ?? i18n.t("raceContext.target.default");
  const playerIsLeader = !!(playerStanding && leader && playerStanding.id === leader.id);
  const topFavorite = favorites?.[0] ?? null;
  const topFavoriteIsPlayer = !!(topFavorite && playerStanding && topFavorite.id === playerStanding.id);
  const leadStory = weekendStories[0] ?? null;
  const climaWet = ["Damp", "Wet", "HeavyRain"].includes(nextRace?.clima);
  const weatherFact = nextRace?.clima
    ? climaWet
      ? i18n.t("raceContext.weatherFact.wet", {
          summary: buildWeatherSummary(nextRace.clima).toLowerCase(),
          narrative: buildWeatherNarrative(nextRace.clima),
        })
      : i18n.t("raceContext.weatherFact.dry", { summary: buildWeatherSummary(nextRace.clima) })
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
    ? i18n.t("raceContext.occasion.finale")
    : isOpenerRound
      ? i18n.t("raceContext.occasion.opener")
      : i18n.t("raceContext.occasion.highlight");
  const importanceFact =
    audienceEstimate > 0
      ? bigEvent
        ? i18n.t("raceContext.importance.big", {
            occasion: eventOccasion,
            audience: formatAudience(audienceEstimate),
          })
        : i18n.t("raceContext.importance.normal", { audience: formatAudience(audienceEstimate) })
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
          .map((p) =>
            i18n.t("raceContext.breakdownRisk.part", { name: p.part_name, level: riskLevelWord(p.level) }),
          )
          .join(", ");
        const geral =
          breakdownForecast.overall_level === "alto"
            ? i18n.t("raceContext.breakdownRisk.levelHigh")
            : riskLevelWord(breakdownForecast.overall_level);
        const parts = risky ? i18n.t("raceContext.breakdownRisk.partsSuffix", { parts: risky }) : "";
        return i18n.t("raceContext.breakdownRisk.main", { level: geral, parts });
      })()
    : null;
  // --- TESE DOMINANTE ---------------------------------------------------------
  // Antes: ~23 fatos numa lista plana; a IA se agarrava no único bloco com carga
  // (o DNF) e ignorava o resto. Agora elegemos UM eixo por corrida e organizamos
  // tudo em camadas (EIXO → APOIO → PANO DE FUNDO), dando hierarquia real.
  const lastResult = recentResults(playerStanding)[0] ?? null;
  const climaLabel = climaWet && nextRace?.clima ? buildWeatherSummary(nextRace.clima).toLowerCase() : null;
  const breakdownLevelLabel =
    breakdownForecast?.overall_level === "alto"
      ? i18n.t("raceContext.breakdownRisk.levelHigh")
      : breakdownForecast?.overall_level
        ? riskLevelWord(breakdownForecast.overall_level)
        : null;
  const breakdownPartsLabel = forecastNotable
    ? forecastParts
        .filter((p) => p.level !== "baixo")
        .slice(0, 3)
        .map((p) =>
          i18n.t("raceContext.breakdownRisk.part", { name: p.part_name, level: riskLevelWord(p.level) }),
        )
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
      ? i18n.t("raceContext.facts.championshipOpener")
      : playerStanding
        ? i18n.t("raceContext.facts.championshipSituation", {
            pos: ordinal(playerStanding.posicao_campeonato),
            state: stateLabel,
            gap:
              gapToLeader > 0
                ? i18n.t("raceContext.facts.championshipSituationGap", { gap: gapToLeader })
                : "",
          })
        : i18n.t("raceContext.facts.championshipReading", { state: stateLabel }),
    objective: championshipUnderway
      ? i18n.t("raceContext.facts.objectiveForm", { target: targetLabel })
      : i18n.t("raceContext.facts.objectiveDebut"),
    recent_form: recentForm ? i18n.t("raceContext.facts.recentForm", { form: recentForm }) : null,
    avg_finish:
      outlook?.averageFinish != null
        ? i18n.t("raceContext.facts.avgFinish", {
            avg: outlook.averageFinish.toFixed(1),
            wins:
              outlook.winCount > 0
                ? i18n.t("raceContext.facts.avgFinishWins", { n: outlook.winCount })
                : "",
            podiums:
              outlook.podiumCount > 0
                ? i18n.t("raceContext.facts.avgFinishPodiums", { n: outlook.podiumCount })
                : "",
          })
        : null,
    leader:
      championshipUnderway && !playerIsLeader && leader?.nome
        ? i18n.t("raceContext.facts.leader", { name: leader.nome, points: leader.pontos ?? 0 })
        : null,
    chaser:
      championshipUnderway && gapBehind != null
        ? i18n.t("raceContext.facts.chaser", { gap: gapBehind })
        : null,
    rival_direct:
      championshipUnderway && briefingRival?.driver_name
        ? i18n.t("raceContext.facts.rivalDirect", {
            name: briefingRival.driver_name,
            pos: ordinal(briefingRival.championship_position),
            side: briefingRival.is_ahead
              ? i18n.t("raceContext.facts.rivalDirectAhead")
              : i18n.t("raceContext.facts.rivalDirectBehind"),
            gap: briefingRival.gap_points,
          })
        : null,
    rivalry_label: briefingRival?.rivalry_label
      ? championshipUnderway
        ? i18n.t("raceContext.facts.rivalryLabel", { label: briefingRival.rivalry_label })
        : i18n.t("raceContext.facts.rivalryLabelPast", {
            label: briefingRival.rivalry_label,
            name: briefingRival.driver_name,
          })
      : null,
    // O nemesis só vira fato solto quando NÃO é o próprio eixo (senão duplica).
    nemesis:
      nemesisSignal?.in_grid && thesis.key !== "nemesis"
        ? i18n.t("raceContext.facts.nemesis", {
            name: nemesisSignal.driver_name,
            label: nemesisSignal.label
              ? i18n.t("raceContext.facts.nemesisLabel", { label: nemesisSignal.label })
              : "",
            h2h:
              nemesisSignal.chapters > 0
                ? i18n.t("raceContext.facts.nemesisH2h", {
                    wins: nemesisSignal.h2h_player_wins,
                    losses: nemesisSignal.h2h_rival_wins,
                  })
                : "",
          })
        : null,
    track_rivals:
      (playerInterests?.rivais ?? [])
        .filter((r) => orderedDrivers.some((d) => d.id === r.driver_id))
        .map((r) =>
          i18n.t("raceContext.facts.trackRival", {
            name: r.driver_name,
            label: r.label ? i18n.t("raceContext.facts.nemesisLabel", { label: r.label }) : "",
            h2h:
              r.chapters > 0
                ? i18n.t("raceContext.facts.trackRivalH2h", {
                    wins: r.h2h_player_wins,
                    losses: r.h2h_rival_wins,
                  })
                : "",
          }),
        )
        .join(" ") || null,
    track_history: trackHistory?.has_data
      ? i18n.t("raceContext.facts.trackHistory", {
          starts: trackHistory.starts,
          best:
            trackHistory.best_finish != null
              ? i18n.t("raceContext.facts.trackHistoryBest", { pos: ordinal(trackHistory.best_finish) })
              : "",
          dnfs:
            trackHistory.dnfs > 0
              ? i18n.t("raceContext.facts.trackHistoryDnfs", { n: trackHistory.dnfs })
              : "",
        })
      : null,
    track_last:
      trackHistory?.has_data && trackHistory.last_finish != null
        ? i18n.t("raceContext.facts.trackLast", {
            pos: ordinal(trackHistory.last_finish),
            season:
              trackHistory.last_visit_season != null
                ? i18n.t("raceContext.facts.trackLastSeason", {
                    season: trackHistory.last_visit_season,
                  })
                : "",
          })
        : null,
    constructors:
      championshipUnderway && teamStanding
        ? i18n
            .t("raceContext.facts.constructors", {
              team: playerTeam?.nome ?? "",
              pos: ordinal(teamStanding.posicao),
              points:
                teamStanding.pontos != null
                  ? i18n.t("raceContext.facts.constructorsPoints", { points: teamStanding.pontos })
                  : "",
            })
            .replace("  ", " ")
        : null,
    teammate: teammate?.nome
      ? championshipUnderway
        ? i18n.t("raceContext.facts.teammate", {
            name: teammate.nome,
            pos: ordinal(teammate.posicao_campeonato),
          })
        : i18n.t("raceContext.facts.teammateDebut", { name: teammate.nome })
      : null,
    favorite: topFavorite?.nome
      ? topFavoriteIsPlayer
        ? i18n.t("raceContext.facts.favoriteSelf", { name: topFavorite.nome })
        : i18n.t("raceContext.facts.favoriteOther", { name: topFavorite.nome })
      : null,
    weather: weatherFact,
    importance: importanceFact,
    breakdown: breakdownRiskFact,
    story_lead: leadStory
      ? i18n.t("raceContext.facts.storyLead", {
          title: leadStory.title,
          summary: leadStory.summary
            ? i18n.t("raceContext.facts.storyLeadSummary", { summary: leadStory.summary })
            : "",
        })
      : null,
    story_others:
      weekendStories
        .slice(1, 3)
        .map((s) => i18n.t("raceContext.facts.storyOther", { title: s.title }))
        .join(" ") || null,
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
    i18n.t("raceContext.bundle.scenario", {
      track: nextRace?.track_name ?? i18n.t("raceContext.bundle.scenarioTrackFallback"),
      year: season?.ano ?? i18n.t("raceContext.bundle.scenarioYearFallback"),
      round: currentRound,
      total: totalRounds,
    }),
    player?.nome
      ? i18n.t("raceContext.bundle.reader", {
          name: player.nome,
          team: playerTeam?.nome ?? i18n.t("raceContext.bundle.readerTeamFallback"),
        })
      : null,
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
    i18n.t("raceContext.bundle.scenarioLine", { scenario: cenario }),
    "",
    i18n.t("raceContext.bundle.axisHead"),
    thesis.statement,
    ...(apoio.length
      ? ["", i18n.t("raceContext.bundle.supportHead"), ...apoio.map((t) => `- ${t}`)]
      : []),
    ...(fundo.length
      ? ["", i18n.t("raceContext.bundle.backgroundHead"), ...fundo.map((t) => `- ${t}`)]
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
  if (clima === "HeavyRain") return i18n.t("raceContext.weather.summary.heavyRain");
  if (clima === "Wet") return i18n.t("raceContext.weather.summary.wet");
  if (clima === "Damp") return i18n.t("raceContext.weather.summary.damp");
  return i18n.t("raceContext.weather.summary.dry");
}

function buildWeatherIcon(clima) {
  if (clima === "HeavyRain") return "⛈";
  if (clima === "Wet") return "🌧";
  if (clima === "Damp") return "🌦";
  return "☀";
}

function buildWeatherNarrative(clima) {
  if (clima === "HeavyRain") return i18n.t("raceContext.weather.narrative.heavyRain");
  if (clima === "Wet") return i18n.t("raceContext.weather.narrative.wet");
  if (clima === "Damp") return i18n.t("raceContext.weather.narrative.damp");
  return i18n.t("raceContext.weather.narrative.dry");
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
  return value ? value.toLocaleString(currentLang()) : "-";
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
