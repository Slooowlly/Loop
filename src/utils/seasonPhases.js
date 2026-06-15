export const NEW_SEASON_PHASES = new Set(["PreTemporada", "Temporada", "Encerramento"]);

export const LEGACY_SEASON_PHASES = new Set([
  "BlocoRegular",
  "JanelaConvocacao",
  "BlocoEspecial",
  "PosEspecial",
]);

export function isLegacySeasonPhase(phase) {
  return LEGACY_SEASON_PHASES.has(phase);
}

export function isNewSeasonPhase(phase) {
  return NEW_SEASON_PHASES.has(phase);
}
