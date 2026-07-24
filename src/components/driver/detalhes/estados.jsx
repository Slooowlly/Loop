import { useTranslation } from "react-i18next";

export function InactivePreviousSeasonState({ context }) {
  const { t } = useTranslation();
  const isWithoutTeam = context === "sem_time_temporada_passada";
  const title = isWithoutTeam
    ? t("driverDetail.summary.noTeamLastSeason")
    : t("driverDetail.summary.noRacesLastSeason");
  const body = isWithoutTeam
    ? t("driverDetail.summary.noTeamLastSeasonBody")
    : t("driverDetail.summary.noRacesLastSeasonBody");

  return (
    <div className="relative overflow-hidden rounded-xl border border-[#d29922]/24 bg-[#d29922]/9 p-4">
      <div className="absolute inset-x-4 top-4 h-px bg-[#d29922]/30" />
      <div className="relative flex min-h-[156px] flex-col items-center justify-center text-center">
        <div className="text-[10px] font-bold uppercase tracking-[0.22em] text-[#d29922]">
          {t("driverDetail.summary.offGrid")}
        </div>
        <div className="mt-3 text-2xl font-bold text-[#e6edf3]">{title}</div>
        <div className="mt-2 max-w-md text-sm text-[#8b949e]">{body}</div>
      </div>
    </div>
  );
}
