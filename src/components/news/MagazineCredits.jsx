import { useTranslation } from "react-i18next";

// Cabeçalho de créditos da página direita ("reportagem de ... · temporada de ...").
function MagazineCredits({ catLabel }) {
  const { t } = useTranslation();
  return (
    <div className="credits">
      {t("newsMagazine.credits.reportedBy")} <b>{t("newsMagazine.credits.pressOffice")}</b>
      <br />
      {catLabel ? (
        <>
          {t("newsMagazine.credits.seasonOf")} <b>{catLabel}</b>
        </>
      ) : null}
    </div>
  );
}

export default MagazineCredits;
