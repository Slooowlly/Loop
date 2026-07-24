// O EVENTO e o estado do fim de semana: público estimado, porte da etapa, transmissão,
// data e as histórias do fim de semana. Lógica pura extraída de
// `pages/tabs/nextRaceContext.js`.
import { currentLang } from "../../i18n/format.js";
import i18n from "../../i18n/index.js";

// Público estimado quando o backend não manda o valor real: só o tier do evento.
export function estimateAudience(tierLabel) {
  if (tierLabel?.toLowerCase().includes("principal")) return 84000;
  if (tierLabel?.toLowerCase().includes("alto")) return 62000;
  if (tierLabel?.toLowerCase().includes("moderado")) return 41000;
  return 28000;
}

export function formatAudience(value) {
  return value ? value.toLocaleString(currentLang()) : "-";
}

export function buildAudienceRankLabel(nextRace, season) {
  const totalRounds = Math.max(1, season?.total_rodadas ?? 1);
  const round = nextRace?.rodada ?? 1;
  const interestTier = nextRace?.event_interest?.tier_label?.toLowerCase() ?? "";

  if (round === 1 || round === totalRounds) {
    return i18n.t("raceContext.display.audienceRank.biggest");
  }

  if (interestTier.includes("principal")) {
    return i18n.t("raceContext.display.audienceRank.third");
  }

  if (interestTier.includes("alto")) {
    return i18n.t("raceContext.display.audienceRank.amongBiggest");
  }

  return i18n.t("raceContext.display.audienceRank.strong");
}

export function isLiveCoverageEvent(nextRace, season) {
  const totalRounds = Math.max(1, season?.total_rodadas ?? 1);
  const round = nextRace?.rodada ?? 1;
  const interestTier = nextRace?.event_interest?.tier_label?.toLowerCase() ?? "";

  return round === 1 || round === totalRounds || interestTier.includes("principal");
}

export function buildTeamExpectationValue({ playerStanding, teamStanding, gapToLeader, outlook }) {
  if (playerStanding?.posicao_campeonato === 1) {
    return i18n.t("raceContext.display.teamExpectation.controlLead");
  }

  if (outlook?.titleFight === "longshot") {
    return i18n.t("raceContext.display.teamExpectation.scoreStrong");
  }

  if (gapToLeader <= 10) {
    return i18n.t("raceContext.display.teamExpectation.pressureFront");
  }

  if ((teamStanding?.posicao ?? 99) <= 3) {
    return i18n.t("raceContext.display.teamExpectation.top5");
  }

  return i18n.t("raceContext.display.teamExpectation.cleanWeekend");
}

export function formatEventSummaryDate(displayDate) {
  if (!displayDate) return "--/--";

  const [year, month, day] = displayDate.split("-");
  if (!year || !month || !day) return displayDate;
  return `${day}/${month}`;
}

// A OCASIÃO da etapa (abertura / final / etapa comum). Descrevemos o porte pela
// ocasião, não por superlativo absoluto — ver a nota em `buildBriefingContext`.
export function buildEventOccasion({ currentRound, totalRounds }) {
  const isFinaleRound = totalRounds > 1 && currentRound === totalRounds;
  const isOpenerRound = currentRound === 1;
  const label = isFinaleRound
    ? i18n.t("raceContext.occasion.finale")
    : isOpenerRound
      ? i18n.t("raceContext.occasion.opener")
      : i18n.t("raceContext.occasion.highlight");

  return { label, isFinaleRound, isOpenerRound };
}

// Narrativa de público: estimativa + a fatia que a fama do lineup do jogador puxa
// (Estrelato, Fase 3). Só cita a fama quando há valor real.
export function buildAttendanceNarrative({ audienceEstimate, fameSharePct }) {
  const fameClause =
    fameSharePct != null && fameSharePct >= 1
      ? i18n.t("raceContext.display.fameClause", { pct: fameSharePct })
      : "";

  return (
    (audienceEstimate > 0
      ? i18n.t("raceContext.display.attendance.withEstimate", {
          audience: formatAudience(audienceEstimate),
        })
      : i18n.t("raceContext.display.attendance.generic")) + fameClause
  );
}

export function normalizeWeekendStories(stories) {
  if (!Array.isArray(stories)) {
    return [];
  }

  return stories.map((story) => ({
    id: story.id,
    icon: story.icon,
    title: story.title,
    summary: story.summary,
    importanceLabel: story.importance ?? i18n.t("raceContext.display.storyImportanceFallback"),
  }));
}
