import { useTranslation } from "react-i18next";
import AppPlaceholder from "../../components/ui/AppPlaceholder";

function MarketTab() {
  const { t } = useTranslation();
  return <AppPlaceholder title={t("marketTab.title")} embedded />;
}

export default MarketTab;
