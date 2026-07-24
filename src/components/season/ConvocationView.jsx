import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import LoadingOverlay from "../ui/LoadingOverlay";
import useCareerStore from "../../stores/useCareerStore";
import ConvocacaoHeader from "./convocacao/ConvocacaoHeader";
import PainelCandidatos from "./convocacao/PainelCandidatos";
import MapaEquipes from "./convocacao/MapaEquipes";
import PainelDecisao from "./convocacao/PainelDecisao";
import { CATEGORY_COLORS, CATEGORY_LABELS } from "./convocacao/constantes.js";
import {
  buildCandidateGroups,
  buildDailyLogGroups,
  categorySortValue,
  filterEligibleCandidates,
} from "./convocacao/agrupamentos.js";

export default function ConvocationView() {
  const careerId = useCareerStore((state) => state.careerId);
  const season = useCareerStore((state) => state.season);
  const specialWindowState = useCareerStore((state) => state.specialWindowState);
  const playerSpecialOffers = useCareerStore((state) => state.playerSpecialOffers);
  const acceptedSpecialOffer = useCareerStore((state) => state.acceptedSpecialOffer);
  const isConvocating = useCareerStore((state) => state.isConvocating);
  const error = useCareerStore((state) => state.error);
  const loadSpecialWindowState = useCareerStore((state) => state.loadSpecialWindowState);
  const acceptSpecialOfferForDay = useCareerStore((state) => state.acceptSpecialOfferForDay);
  const advanceSpecialWindowDay = useCareerStore((state) => state.advanceSpecialWindowDay);
  const confirmSpecialBlock = useCareerStore((state) => state.confirmSpecialBlock);

  const { t } = useTranslation();

  const [selectedCategory, setSelectedCategory] = useState("all");

  useEffect(() => {
    if (!careerId || specialWindowState) {
      return;
    }
    void loadSpecialWindowState();
  }, [careerId, loadSpecialWindowState, specialWindowState]);

  const groupedOffers = useMemo(() => {
    const grouped = new Map();

    for (const offer of playerSpecialOffers) {
      const category = offer.special_category ?? "especial";
      if (!grouped.has(category)) {
        grouped.set(category, []);
      }
      grouped.get(category).push(offer);
    }

    return [...grouped.entries()].sort(
      ([left], [right]) => categorySortValue(left) - categorySortValue(right),
    );
  }, [playerSpecialOffers]);

  const filteredSections = useMemo(() => {
    const teamSections = (specialWindowState?.team_sections ?? []).map((section) => ({
      ...section,
      label: section.label ?? CATEGORY_LABELS[section.category] ?? section.category,
      color: CATEGORY_COLORS[section.category] ?? "#58a6ff",
    }));
    if (selectedCategory === "all") {
      return teamSections;
    }
    return teamSections.filter((section) => section.category === selectedCategory);
  }, [selectedCategory, specialWindowState]);

  const candidateGroups = useMemo(() => {
    const candidates = filterEligibleCandidates(
      specialWindowState?.eligible_candidates ?? [],
      selectedCategory,
    );
    return buildCandidateGroups(candidates);
  }, [selectedCategory, specialWindowState]);

  const dailyLogGroups = useMemo(
    () => buildDailyLogGroups(specialWindowState?.last_day_log ?? []),
    [specialWindowState],
  );

  const totalVisibleTeams = filteredSections.reduce((sum, section) => sum + section.teams.length, 0);
  const currentDay = specialWindowState?.current_day ?? 1;
  const totalDays = specialWindowState?.total_days ?? 7;
  const primaryCtaLabel = specialWindowState?.is_finished
    ? acceptedSpecialOffer
      ? t("convocation.cta.enterSpecialBlock")
      : t("convocation.cta.proceedWithout")
    : t("convocation.cta.advanceDay");

  return (
    <div
      data-testid="convocation-page"
      className="app-shell relative h-screen w-full overflow-hidden text-[color:var(--text-primary)]"
    >
      <div className="app-backdrop pointer-events-none absolute inset-0" />

      <LoadingOverlay
        open={isConvocating}
        title={t("convocation.loading.title")}
        message={t("convocation.loading.message")}
      />

      <div className="relative z-10 mx-auto flex h-full max-w-[1680px] flex-col px-3 pb-3 pt-3 sm:px-4 lg:px-5 xl:px-6">
        <ConvocacaoHeader
          selectedCategory={selectedCategory}
          onSelectCategory={setSelectedCategory}
          acceptedSpecialOffer={acceptedSpecialOffer}
          specialWindowState={specialWindowState}
          season={season}
          currentDay={currentDay}
          totalDays={totalDays}
          isConvocating={isConvocating}
          primaryCtaLabel={primaryCtaLabel}
          onPrimaryCta={() =>
            void (specialWindowState?.is_finished
              ? confirmSpecialBlock()
              : advanceSpecialWindowDay())
          }
          error={error}
        />

        <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 xl:grid-cols-[20%_62%_18%]">
          <PainelCandidatos
            candidateGroups={candidateGroups}
            groupedOffers={groupedOffers}
            playerSpecialOffers={playerSpecialOffers}
            isConvocating={isConvocating}
            specialWindowState={specialWindowState}
            onAcceptOffer={(offerId) => void acceptSpecialOfferForDay(offerId)}
          />

          <MapaEquipes
            filteredSections={filteredSections}
            totalVisibleTeams={totalVisibleTeams}
            currentDay={currentDay}
          />

          <PainelDecisao
            acceptedSpecialOffer={acceptedSpecialOffer}
            specialWindowState={specialWindowState}
            dailyLogGroups={dailyLogGroups}
          />
        </div>
      </div>
    </div>
  );
}
