import i18n from "../../i18n/index.js";

const POSITION_KEYS = ["p1", "p2", "p3", "p4", "p5"];
const FORM_PROFILES = ["controlled", "charging", "volatile", "stable", "baseline"];

// Banco de frases de expectativa dos favoritos (5 posições × 5 perfis × 2 variantes).
// O `text` resolve i18n no ACESSO (getter) → não congela no idioma do boot; o `id`
// (estável) é a chave i18n e o pivô do pinning/dedup/histórico. Os textos vivem em
// `briefing.expectation.<id>` (locales).
function phrase(id) {
  return {
    id,
    get text() {
      return i18n.t(`briefing.expectation.${id}`);
    },
  };
}

const FAVORITE_EXPECTATION_GROUPS = Object.fromEntries(
  POSITION_KEYS.map((position) => [
    position,
    Object.fromEntries(
      FORM_PROFILES.map((profile) => [
        profile,
        [1, 2].map((variant) => phrase(`${position}-${profile}-${variant}`)),
      ]),
    ),
  ]),
);

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
