// O EVENTO e o estado do fim de semana: público estimado, porte da etapa, transmissão,
// data e as histórias do fim de semana. Lógica pura extraída de
// `pages/tabs/nextRaceContext.js`.
import { currentLang } from "../../i18n/format.js";
import i18n from "../../i18n/index.js";

// DÍVIDA CONHECIDA: número de público inventado no FRONT. Só entra quando o backend
// não mandou `display_value` (save antigo). Não amplie o padrão — todo valor novo de
// interesse deve nascer no `event_interest` do Rust. Ver docs/briefings/F07.
//
// Chaveado pelo ENUM `InterestTier` (serializado como "Baixo"…"EventoPrincipal"), não
// pelo `tier_label`: o label é traduzido pelo locale do backend e sniffar o texto
// quebrava em inglês — e já falhava em português para o tier `Alto` ("Grande público",
// que não contém "alto").
const AUDIENCE_BY_TIER = {
  EventoPrincipal: 84000,
  MuitoAlto: 72000,
  Alto: 62000,
  Moderado: 41000,
  Baixo: 28000,
};

export function estimateAudience(tier) {
  return AUDIENCE_BY_TIER[tier] ?? AUDIENCE_BY_TIER.Baixo;
}

export function formatAudience(value) {
  return value ? value.toLocaleString(currentLang()) : "-";
}

export function buildAudienceRankLabel(nextRace, season) {
  const totalRounds = Math.max(1, season?.total_rodadas ?? 1);
  const round = nextRace?.rodada ?? 1;
  const interestTier = nextRace?.event_interest?.tier;

  if (round === 1 || round === totalRounds) {
    return i18n.t("raceContext.display.audienceRank.biggest");
  }

  if (interestTier === "EventoPrincipal") {
    return i18n.t("raceContext.display.audienceRank.third");
  }

  if (interestTier === "MuitoAlto" || interestTier === "Alto") {
    return i18n.t("raceContext.display.audienceRank.amongBiggest");
  }

  return i18n.t("raceContext.display.audienceRank.strong");
}

export function isLiveCoverageEvent(nextRace, season) {
  const totalRounds = Math.max(1, season?.total_rodadas ?? 1);
  const round = nextRace?.rodada ?? 1;
  const interestTier = nextRace?.event_interest?.tier;

  return round === 1 || round === totalRounds || interestTier === "EventoPrincipal";
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
