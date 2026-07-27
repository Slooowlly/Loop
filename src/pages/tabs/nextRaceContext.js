// Contexto de briefing da próxima corrida: lógica PURA (sem React, sem store) que
// monta fatos/tese/editorial da prévia pré-corrida. Extraído de NextRaceTab.jsx para
// que o store (useCareerStore) reuse `buildBriefingContext` sem importar um componente
// — quebrando o ciclo store↔componente. Segue o padrão dos irmãos nextRaceBriefing /
// nextRaceEditorial / nextRaceThesis.
//
// Este arquivo é só o ORQUESTRADOR: resolve o contexto comum (classificação, gaps,
// rivais) e delega cada assunto para os módulos de `components/race/`:
//   - raceTrackContext  → clima, temperatura, condição de pista, horário
//   - raceEventContext  → público, transmissão, ocasião, histórias do fim de semana
//   - raceGridContext   → favoritos, forma, perspectiva competitiva, metas
//   - raceFactsContext  → tese dominante, texto editorial e o bundle de fatos da IA
import { classifyChampionshipState } from "./nextRaceEditorial";
import i18n from "../../i18n/index.js";
import {
  buildBoxNarrative,
  buildTemperatureNarrative,
  buildTimePeriodHighlight,
  buildTimePeriodPrefix,
  buildTrackConditionLabel,
  buildTrackTemperatureLabel,
  buildWeatherIcon,
  buildWeatherNarrative,
  buildWeatherSummary,
} from "../../components/race/raceTrackContext";
import {
  buildAttendanceNarrative,
  buildAudienceRankLabel,
  buildEventOccasion,
  buildTeamExpectationValue,
  estimateAudience,
  formatAudience,
  formatEventSummaryDate,
  isLiveCoverageEvent,
  normalizeWeekendStories,
} from "../../components/race/raceEventContext";
import {
  buildCompetitiveOutlook,
  buildFavorites,
  buildGoals,
  buildRatedDrivers,
  getFavoriteMedalTone,
  getReadableTeamColor,
} from "../../components/race/raceGridContext";
import { buildRaceFactsBundle, riskColor, riskLabel } from "../../components/race/raceFactsContext";

// Reexportados para quem já consumia estes helpers por aqui (NextRaceTab e afins).
export { formatAudience, getFavoriteMedalTone, getReadableTeamColor, riskColor, riskLabel };

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
  const ratedDrivers = buildRatedDrivers(orderedDrivers);
  const favorites = buildFavorites(ratedDrivers, { season, nextRace, briefingPhraseHistory });
  const audienceEstimate = nextRace?.event_interest?.display_value ?? estimateAudience(nextRace?.event_interest?.tier);
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
  const attendanceNarrative = buildAttendanceNarrative({ audienceEstimate, fameSharePct });
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
  const playerIsLeader = !!(playerStanding && leader && playerStanding.id === leader.id);
  const audienceRankLabel = buildAudienceRankLabel(nextRace, season);
  // IMPORTANTE: descrever o porte pela OCASIÃO, não com superlativo absoluto. O
  // rótulo "maior público da temporada" é só um heurístico de UI (rodada 1 e final)
  // e NÃO é uma comparação real do calendário — uma etapa principal ou a final podem
  // atrair mais. Mandar isso como fato faria a IA cravar algo que não dá pra checar.
  const occasion = buildEventOccasion({ currentRound, totalRounds });

  const { aiFacts, thesis, editorialCopy } = buildRaceFactsBundle({
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
    eventOccasion: occasion.label,
    isFinaleRound: occasion.isFinaleRound,
    currentRound,
    totalRounds,
    fameSharePct,
    playerIsLeader,
  });

  return {
    aiFacts,
    thesisKey: thesis.key,
    thesisTitle: thesis.title,
    audienceEstimate,
    audienceRankLabel,
    eventDateShort: formatEventSummaryDate(nextRace?.display_date),
    interestLabel:
      nextRace?.event_interest?.tier_label ?? i18n.t("raceContext.display.interestLabelFallback"),
    broadcastLabel: isLiveCoverageEvent(nextRace, season)
      ? i18n.t("raceContext.display.broadcast.coverage")
      : i18n.t("raceContext.display.broadcast.expectation"),
    broadcastValue: isLiveCoverageEvent(nextRace, season)
      ? i18n.t("raceContext.display.broadcast.live")
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
    gapToLeaderLabel:
      gapToLeader === 0
        ? i18n.t("raceContext.display.gapToLeaderLead")
        : i18n.t("raceContext.display.gapPts", { pts: gapToLeader }),
    gapBehindLabel:
      gapBehind == null
        ? i18n.t("raceContext.display.gapBehindNone")
        : i18n.t("raceContext.display.gapPts", { pts: gapBehind }),
    progressPercent: Math.max(5, Math.min(100, Math.round((currentRound / totalRounds) * 100))),
    progressLabel: `${currentRound}/${totalRounds}`,
    quote: editorialCopy.quote,
    teamVoiceLabel: playerTeam?.nome ?? i18n.t("raceContext.display.teamVoiceFallback"),
    teamColor: playerTeam?.cor_primaria ?? null,
    attendanceNarrative,
    weatherIcon: buildWeatherIcon(nextRace?.clima),
    weatherSummary: buildWeatherSummary(nextRace?.clima),
    weatherNarrative: buildWeatherNarrative(nextRace?.clima),
    trackTemperatureLabel: buildTrackTemperatureLabel(nextRace?.temperatura),
    temperatureNarrative: buildTemperatureNarrative(nextRace?.temperatura),
    trackConditionLabel: buildTrackConditionLabel(nextRace?.clima),
    boxNarrative: buildBoxNarrative(nextRace?.clima),
    timePeriodPrefix: buildTimePeriodPrefix(nextRace?.horario),
    timePeriodHighlight: buildTimePeriodHighlight(nextRace?.horario),
    actionHint: editorialCopy.actionHint,
    weekendStories,
  };
}
