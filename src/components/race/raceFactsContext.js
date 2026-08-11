// Os FATOS da prévia pré-corrida: risco de quebra, o pacote curado que vai para a IA
// (cenário → eixo → apoio → pano de fundo), a tese dominante do fim de semana e o
// texto determinístico de fallback. Lógica pura extraída de
// `pages/tabs/nextRaceContext.js`.
import { recentResults } from "../../pages/tabs/nextRaceBriefing";
import { buildEditorialCopy } from "../../pages/tabs/nextRaceEditorial";
import { selectThesis } from "../../pages/tabs/nextRaceThesis";
import { ordinal } from "../../i18n/format.js";
import i18n from "../../i18n/index.js";
import { buildWeatherNarrative, buildWeatherSummary, isWetWeather } from "./raceTrackContext";
import { formatAudience } from "./raceEventContext";

// Gravidades de lesão que o backend manda (`lesao_ativa_tipo`, o enum `InjuryType`). Aqui
// era um mapa de "Leve"/"Moderada"/"Grave"/"Critica" — a grafia do BANCO — para chave i18n,
// com o "Critica" sem acento incluído: corrigir a acentuação lá apagava a gravidade aqui, em
// silêncio. Hoje o backend já manda a chave, e esta lista existe só para separar gravidade
// conhecida de valor estranho (save antigo, cache velho).
const INJURY_SEVERITIES = new Set(["light", "moderate", "severe", "critical"]);

// Cor/rótulo do nível de risco de quebra (card da Sala de Estratégia).
export function riskColor(level) {
  if (level === "alto") return "#f87171";
  if (level === "médio") return "#f0b37a";
  return "#34d399";
}

export function riskLabel(level) {
  if (level === "alto") return i18n.t("raceContext.display.risk.high");
  if (level === "médio") return i18n.t("raceContext.display.risk.medium");
  return i18n.t("raceContext.display.risk.low");
}

// Palavra do nível de risco DENTRO dos fatos de IA (minúscula, ex.: "risco alto").
// Separada de `riskLabel` (rótulo capitalizado do card de display).
function riskLevelWord(level) {
  if (level === "alto") return i18n.t("raceContext.breakdownRisk.wordHigh");
  if (level === "médio") return i18n.t("raceContext.breakdownRisk.wordMedium");
  return i18n.t("raceContext.breakdownRisk.wordLow");
}

// Fato do rival direto para a IA. A direção da comparação é SEMPRE explícita e nomeada
// ("Fulano está à frente de você por N pontos"): frase com sujeito solto ("à frente por
// N") já fez o modelo inverter quem liderava. Sempre que a tabela oficial estiver
// disponível, a direção e o gap são recomputados dos pontos dos DOIS lados (nada de
// confiar num flag pré-digerido) e o empate em pontos vira frase própria, em vez de
// "à frente por 0 ponto(s)".
function buildRivalDirectFact({ briefingRival, orderedDrivers, playerStanding }) {
  if (!briefingRival?.driver_name) {
    return null;
  }
  const rivalStanding = orderedDrivers.find((d) => d.id === briefingRival.driver_id) ?? null;
  const rivalPointsRaw = rivalStanding?.pontos;
  const playerPointsRaw = playerStanding?.pontos;
  if (rivalPointsRaw != null && playerPointsRaw != null) {
    // Compara os valores JÁ arredondados (os mesmos inteiros que a tabela exibe), para o
    // gap do texto nunca divergir da tabela que o jogador está vendo.
    const rivalPts = Math.round(rivalPointsRaw);
    const playerPts = Math.round(playerPointsRaw);
    const gap = Math.abs(rivalPts - playerPts);
    const side =
      gap === 0
        ? i18n.t("raceContext.facts.rivalDirectTied", { name: briefingRival.driver_name })
        : rivalPts > playerPts
          ? i18n.t("raceContext.facts.rivalDirectAhead", { name: briefingRival.driver_name, gap })
          : i18n.t("raceContext.facts.rivalDirectBehind", { name: briefingRival.driver_name, gap });
    return i18n.t("raceContext.facts.rivalDirect", {
      name: briefingRival.driver_name,
      pos: ordinal(rivalStanding.posicao_campeonato ?? briefingRival.championship_position),
      rivalPts,
      playerPos: ordinal(playerStanding.posicao_campeonato),
      playerPts,
      side,
    });
  }
  // Sem a tabela em mãos: usa o resumo do backend (`is_ahead` = o RIVAL está à frente
  // do jogador), mantendo a mesma frase nomeada e o caso de empate.
  const gap = briefingRival.gap_points ?? 0;
  const side =
    gap === 0
      ? i18n.t("raceContext.facts.rivalDirectTied", { name: briefingRival.driver_name })
      : briefingRival.is_ahead
        ? i18n.t("raceContext.facts.rivalDirectAhead", { name: briefingRival.driver_name, gap })
        : i18n.t("raceContext.facts.rivalDirectBehind", { name: briefingRival.driver_name, gap });
  return i18n.t("raceContext.facts.rivalDirectNoTable", {
    name: briefingRival.driver_name,
    pos: ordinal(briefingRival.championship_position),
    side,
  });
}

// Ordem estável em que os fatos aparecem dentro de cada camada.
const FACT_ORDER = [
  "championship_situation",
  "objective",
  "recent_form",
  "injury",
  "avg_finish",
  "leader",
  "chaser",
  "pressure",
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
  "fame",
  "breakdown",
  "story_lead",
  "story_others",
];

// Monta a tese dominante, o texto determinístico e o bundle de fatos da IA a partir do
// contexto já resolvido pelo orquestrador (`buildBriefingContext`).
export function buildRaceFactsBundle({
  player,
  playerTeam,
  season,
  nextRace,
  playerInterests,
  breakdownForecast,
  orderedDrivers,
  playerStanding,
  leader,
  behindDriver,
  teammate,
  teamStanding,
  briefingRival,
  trackHistory,
  weekendStories,
  favorites,
  outlook,
  gapToLeader,
  gapBehind,
  remainingRounds,
  championshipUnderway,
  championshipState,
  audienceEstimate,
  audienceRankLabel,
  eventOccasion,
  isFinaleRound,
  currentRound,
  totalRounds,
  fameSharePct,
  playerIsLeader,
}) {
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
  const topFavorite = favorites?.[0] ?? null;
  const topFavoriteIsPlayer = !!(topFavorite && playerStanding && topFavorite.id === playerStanding.id);
  const leadStory = weekendStories[0] ?? null;
  const climaWet = isWetWeather(nextRace?.clima);
  const weatherFact = nextRace?.clima
    ? climaWet
      ? i18n.t("raceContext.weatherFact.wet", {
          summary: buildWeatherSummary(nextRace.clima).toLowerCase(),
          narrative: buildWeatherNarrative(nextRace.clima),
        })
      : i18n.t("raceContext.weatherFact.dry", { summary: buildWeatherSummary(nextRace.clima) })
    : null;
  const bigEvent = audienceEstimate >= 60000 || /maior|maiores/i.test(audienceRankLabel);
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

  // Lesão ATIVA do jogador (carrega da classificação/summary — `lesao_ativa_tipo`, a chave de
  // gravidade: "light"/"moderate"/"severe"/"critical"). Corre machucado → o briefing avisa (o
  // debrief no backend fecha o loop se levar dano na corrida). Traduzimos a gravidade pro
  // idioma ativo; chave desconhecida cai no valor cru em vez de sumir com o fato.
  const injuryType = playerStanding?.lesao_ativa_tipo ?? player?.lesao_ativa_tipo ?? null;
  const injurySeverityLabel = injuryType
    ? INJURY_SEVERITIES.has(injuryType)
      ? i18n.t(`raceContext.facts.injurySeverity.${injuryType}`)
      : injuryType
    : null;

  // Pressão de TÍTULO defensiva: está no topo, com um caçador colado e poucas etapas. É o
  // beat de "segurar sob pressão" (o gêmeo pré-corrida do clutch/choke da pressure.rs, que
  // só existe no sim). Gate apertado pra não virar ruído em toda corrida.
  const underTitlePressure =
    championshipUnderway &&
    !!playerStanding &&
    !!behindDriver &&
    gapBehind != null &&
    gapBehind <= 12 &&
    remainingRounds <= 6 &&
    (playerStanding.posicao_campeonato ?? 99) <= 5;

  // Estrelato: fama REAL do jogador (`midia`, 0–100). Vira FACT quando é estrela (>70) ou
  // ídolo (>87). A fração de público (`fameSharePct`, do `public_fame_share`) entra como
  // detalhe da manchete quando é um chamariz de verdade.
  const playerFame = playerStanding?.midia ?? player?.midia ?? null;
  const fameLevelKey =
    playerFame == null ? null : playerFame > 87 ? "idol" : playerFame > 70 ? "star" : null;

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
    injury: injuryType ? i18n.t("raceContext.facts.injury", { severity: injurySeverityLabel }) : null,
    pressure: underTitlePressure
      ? i18n.t("raceContext.facts.pressure", {
          name: behindDriver.nome,
          gap: gapBehind,
          rounds: remainingRounds,
        })
      : null,
    fame: fameLevelKey
      ? i18n.t("raceContext.facts.fame", {
          level: i18n.t(`raceContext.facts.fameLevel.${fameLevelKey}`),
          value: playerFame,
          crowd:
            fameSharePct != null && fameSharePct >= 10
              ? i18n.t("raceContext.facts.fameCrowd", { pct: fameSharePct })
              : "",
        })
      : null,
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
    rival_direct: championshipUnderway
      ? buildRivalDirectFact({ briefingRival, orderedDrivers, playerStanding })
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

  return { aiFacts, thesis, editorialCopy };
}
