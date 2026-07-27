// O GRID e as previsões do briefing pré-corrida: rating dos favoritos, leitura de
// forma recente, perspectiva competitiva do jogador e metas do fim de semana.
// Lógica pura extraída de `pages/tabs/nextRaceContext.js`.
import { buildFavoriteExpectationSelection, recentResults } from "../../pages/tabs/nextRaceBriefing";
import { getReadableTeamColor as readableTeamColor } from "../../utils/teamColors";
import { ordinal } from "../../i18n/format.js";
import i18n from "../../i18n/index.js";

export function getFavoriteMedalTone(index) {
  if (index === 0) return "text-[#f5c76d]";
  if (index === 1) return "text-[#d8dfef]";
  if (index === 2) return "text-[#cf8d63]";
  return "text-gray-500";
}

// Cor de equipe legível sobre o fundo escuro dos painéis de corrida.
// O fallback `#58a6ff` é o accent primário do app (`utils/colors.js`), provavelmente
// copiado por engano do default de `getCategoryColor` — um piloto sem equipe acaba
// pintado com a cor de destaque da UI. Trocar é mudança visual: commit próprio.
export const getReadableTeamColor = (color) => readableTeamColor(color, { fallback: "#58a6ff" });

export function buildFavoriteRating(driver) {
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

export function buildFormLabel(driver) {
  const snapshot = recentResults(driver)
    .map((result) => {
      if (!result) return "P--";
      if (result.is_dnf) return "DNF";
      return `P${result.position ?? "--"}`;
    })
    .join(" - ");

  return snapshot
    ? i18n.t("raceContext.display.formLabel", { snapshot })
    : i18n.t("raceContext.display.formLabelEmpty");
}

export function buildFormChips(driver) {
  const chips = recentResults(driver).map((result) => {
    if (!result) {
      return {
        label: i18n.t("raceContext.display.formChip.noData"),
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
    : [
        {
          label: i18n.t("raceContext.display.formChip.noHistory"),
          tone: "border-white/10 bg-white/[0.04] text-text-secondary",
        },
      ];
}

// Grid ordenado por rating (o "quem está voando" da prévia), já com os rótulos de forma.
export function buildRatedDrivers(orderedDrivers) {
  return orderedDrivers
    .map((driver) => ({
      ...driver,
      rating: buildFavoriteRating(driver),
      formLabel: buildFormLabel(driver),
      formChips: buildFormChips(driver),
    }))
    .sort((left, right) => right.rating - left.rating || left.posicao_campeonato - right.posicao_campeonato);
}

// Os 6 favoritos ao pódio, cada um com a frase de expectativa sorteada (o histórico
// de frases evita repetir a mesma linha para o mesmo piloto em rodadas seguidas).
export function buildFavorites(ratedDrivers, { season, nextRace, briefingPhraseHistory }) {
  return ratedDrivers
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
}

export function buildCompetitiveOutlook({ playerStanding, leader, remainingRounds, playerRating, leaderRating }) {
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

export function buildGoals({ playerStanding, teammate, teamStanding, gapToLeader, remainingRounds, outlook, driverAbove }) {
  const teamGoal =
    teamStanding?.posicao === 1
      ? i18n.t("raceContext.display.goals.teamLead")
      : teamStanding
        ? i18n.t("raceContext.display.goals.teamTop", { n: Math.min(3, teamStanding.posicao) })
        : i18n.t("raceContext.display.goals.teamDefault");

  const playerPos = playerStanding?.posicao_campeonato ?? 0;
  const teammatePos = teammate?.posicao_campeonato ?? 0;
  const teammateIsClose = teammate && Math.abs(playerPos - teammatePos) <= 2;

  const personalGoal = teammateIsClose
    ? i18n.t("raceContext.display.goals.personalBeatTeammate", { name: teammate.nome })
    : driverAbove
      ? i18n.t("raceContext.display.goals.personalBeatDriver", {
          name: driverAbove.nome,
          pos: ordinal(playerPos - 1),
        })
      : i18n.t("raceContext.display.goals.personalDefault");

  let championshipGoal = i18n.t("raceContext.display.goals.championshipDefault");
  if (playerStanding?.posicao_campeonato === 1) {
    championshipGoal = i18n.t("raceContext.display.goals.championshipLeader");
  } else if (outlook?.titleFight === "longshot") {
    championshipGoal = i18n.t("raceContext.display.goals.championshipLongshot");
  } else if (gapToLeader <= 7) {
    championshipGoal = i18n.t("raceContext.display.goals.championshipClose");
  } else if (remainingRounds <= 3) {
    championshipGoal = i18n.t("raceContext.display.goals.championshipFinal");
  }

  return [
    { label: i18n.t("raceContext.display.goals.teamLabel"), value: teamGoal },
    { label: i18n.t("raceContext.display.goals.personalLabel"), value: personalGoal },
    { label: i18n.t("raceContext.display.goals.championshipLabel"), value: championshipGoal },
  ];
}
