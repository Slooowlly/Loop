import { useTranslation } from "react-i18next";

import GlassButton from "../ui/GlassButton";
import GlassCard from "../ui/GlassCard";
import LoadingOverlay from "../ui/LoadingOverlay";

// Sala de Estratégia SEM próxima corrida: agente livre, pré-temporada, fim de
// temporada e os estados legados (bloco especial/regular). Só desenha — a decisão
// de qual ação rodar fica com o NextRaceTab (`onAdvance`).
function NextRaceEmptyState({
  phase,
  isLegacyPhase,
  isFreeAgent,
  hasPendingRegularRaces,
  hasExistingPreseason,
  busy,
  error,
  onAdvance,
  onSkipSeason,
}) {
  const { t } = useTranslation();

  const heading = isFreeAgent
    ? t("nextRaceTab.status.noTeamHeading")
    : phase === "PreTemporada"
    ? t("nextRaceTab.status.preseasonOpenHeading")
    : phase === "Encerramento"
    ? t("nextRaceTab.status.seasonEndHeading")
    : isLegacyPhase && phase === "BlocoEspecial"
    ? t("nextRaceTab.status.specialBlockHeading")
    : isLegacyPhase && phase === "PosEspecial"
    ? t("nextRaceTab.status.specialDoneHeading")
    : t("nextRaceTab.status.seasonFinishedHeading");
  const description = isFreeAgent
    ? t("nextRaceTab.status.noTeamDesc")
    : phase === "PreTemporada"
    ? t("nextRaceTab.status.preseasonOpenDesc")
    : phase === "Encerramento"
    ? t("nextRaceTab.status.seasonEndDesc")
    : isLegacyPhase && phase === "BlocoEspecial"
    ? t("nextRaceTab.status.specialBlockDesc")
    : isLegacyPhase && phase === "BlocoRegular"
    ? hasPendingRegularRaces
      ? t("nextRaceTab.status.regularPendingDesc")
      : t("nextRaceTab.status.regularDoneDesc")
    : isLegacyPhase && phase === "PosEspecial"
    ? t("nextRaceTab.status.posEspecialDesc")
    : hasExistingPreseason
    ? t("nextRaceTab.status.preseasonStartedDesc")
    : t("nextRaceTab.status.allRacesDoneDesc");
  const buttonLabel = isFreeAgent
    ? t("nextRaceTab.actions.skipSeason")
    : isLegacyPhase && phase === "BlocoEspecial"
    ? t("nextRaceTab.actions.skipSpecialBlock")
    : hasPendingRegularRaces
    ? t("nextRaceTab.actions.advanceCalendar")
    : isLegacyPhase && phase === "BlocoRegular"
    ? t("nextRaceTab.actions.advanceToCallup")
    : isLegacyPhase && phase === "PosEspecial"
    ? t("nextRaceTab.actions.endSeason")
    : phase === "PreTemporada" || hasExistingPreseason
    ? t("nextRaceTab.actions.continuePreseason")
    : t("nextRaceTab.actions.advanceToPreseason");

  return (
    <div className="relative">
      <LoadingOverlay
        open={busy.open}
        title={
          busy.isEnteringPreseason
            ? t("nextRaceTab.loading.openingMarketTitle")
            : isFreeAgent
            ? t("nextRaceTab.loading.skippingSeasonTitle")
            : isLegacyPhase && phase === "BlocoEspecial"
            ? t("nextRaceTab.loading.simulatingSpecialTitle")
            : isLegacyPhase && phase === "BlocoRegular"
            ? t("nextRaceTab.loading.openingCallupTitle")
            : t("nextRaceTab.loading.turningSeasonTitle")
        }
        message={
          busy.isEnteringPreseason
            ? t("nextRaceTab.loading.openingMarketMsg")
            : isFreeAgent
            ? t("nextRaceTab.loading.skippingSeasonMsg")
            : isLegacyPhase && phase === "BlocoEspecial"
            ? t("nextRaceTab.loading.simulatingSpecialMsg")
            : isLegacyPhase && phase === "BlocoRegular"
            ? t("nextRaceTab.loading.openingCallupMsg")
            : t("nextRaceTab.loading.turningSeasonMsg")
        }
      />

      <GlassCard hover={false} className="rounded-[28px] p-10">
        <div className="py-6 text-center">
          <div className="text-6xl">{isFreeAgent ? "🏳️" : "PQ"}</div>
          <p className="mt-4 text-sm uppercase tracking-[0.22em] text-accent-primary">
            {isFreeAgent ? t("nextRaceTab.labels.freeAgent") : t("nextRaceTab.labels.nextRace")}
          </p>
          <h2 className="mt-3 text-3xl font-semibold text-text-primary">{heading}</h2>
          <p className="mt-3 text-sm text-text-secondary">{description}</p>
          <div className="mt-6">
            <GlassButton
              variant="primary"
              disabled={busy.open}
              onClick={() => {
                if (isFreeAgent) {
                  onSkipSeason();
                } else {
                  onAdvance();
                }
              }}
            >
              {busy.open ? t("nextRaceTab.actions.processing") : buttonLabel}
            </GlassButton>
          </div>
          {error ? <p className="mt-4 text-sm text-status-red">{error}</p> : null}
        </div>
      </GlassCard>
    </div>
  );
}

export default NextRaceEmptyState;
