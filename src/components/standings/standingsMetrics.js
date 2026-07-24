// Métricas derivadas da tabela: pontuação por resultado e a variação de posição em
// relação à rodada anterior (as setas ▲▼). Lógica pura extraída de
// `pages/tabs/StandingsTab.jsx`.
import { currentLang } from "../../i18n/format.js";

const STANDARD_POINTS = {
  1: 25,
  2: 18,
  3: 15,
  4: 12,
  5: 10,
  6: 8,
  7: 6,
  8: 4,
  9: 2,
  10: 1,
};

export function pointsForResult(result) {
  if (!result || result.is_dnf) {
    return 0;
  }

  return STANDARD_POINTS[result.position] ?? 0;
}

export function calculatePointsThroughRound(results, roundCount) {
  if (!Array.isArray(results) || roundCount <= 0) {
    return 0;
  }

  return results.slice(0, roundCount).reduce((total, result) => total + pointsForResult(result), 0);
}

export function calculateBestFinish(results, roundCount) {
  if (!Array.isArray(results) || roundCount <= 0) {
    return Number.MAX_SAFE_INTEGER;
  }

  return results.slice(0, roundCount).reduce((best, result) => {
    if (!result || result.is_dnf) {
      return best;
    }
    return Math.min(best, result.position ?? Number.MAX_SAFE_INTEGER);
  }, Number.MAX_SAFE_INTEGER);
}

// Recalcula a tabela como ela estava na rodada anterior e devolve, por piloto,
// quantas posições ele ganhou (positivo) ou perdeu (negativo) desde então.
export function buildPositionDeltaMap(drivers, completedRounds) {
  const deltaMap = new Map();

  if (!Array.isArray(drivers) || completedRounds <= 1) {
    return deltaMap;
  }

  const previousRoundCount = completedRounds - 1;
  const previousStandings = [...drivers]
    .map((driver) => ({
      id: driver.id,
      nome: driver.nome,
      currentPosition: driver.posicao_campeonato ?? Number.MAX_SAFE_INTEGER,
      previousPoints: calculatePointsThroughRound(driver.results, previousRoundCount),
      previousBestFinish: calculateBestFinish(driver.results, previousRoundCount),
    }))
    .sort((left, right) => {
      if (right.previousPoints !== left.previousPoints) {
        return right.previousPoints - left.previousPoints;
      }
      if (left.previousBestFinish !== right.previousBestFinish) {
        return left.previousBestFinish - right.previousBestFinish;
      }
      if (left.currentPosition !== right.currentPosition) {
        return left.currentPosition - right.currentPosition;
      }
      return left.nome.localeCompare(right.nome, currentLang());
    });

  previousStandings.forEach((driver, index) => {
    deltaMap.set(driver.id, index + 1);
  });

  return new Map(
    drivers.map((driver) => {
      const previousPosition = deltaMap.get(driver.id);
      const currentPosition = driver.posicao_campeonato ?? 0;
      return [driver.id, previousPosition ? previousPosition - currentPosition : 0];
    }),
  );
}
