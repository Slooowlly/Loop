import { useTranslation } from "react-i18next";
import GlassCard from "./GlassCard";

function LoadingOverlay({ open = false, title, message }) {
  const { t } = useTranslation();
  if (!open) return null;

  return (
    <div className="absolute inset-0 z-50 flex items-center justify-center bg-app-bg/55 backdrop-blur-[20px]">
      <GlassCard hover={false} className="glass-strong w-full max-w-sm text-center">
        <div className="mx-auto mb-5 h-14 w-14 animate-spin rounded-full border-4 border-white/10 border-t-accent-primary" />
        <p className="text-xs font-semibold uppercase tracking-[0.22em] text-accent-primary">
          {title ?? t("loadingOverlay.defaultTitle")}
        </p>
        <h3 className="mt-3 text-2xl font-semibold text-text-primary">
          {t("loadingOverlay.subtitle")}
        </h3>
        <p className="mt-3 text-sm text-text-secondary">{message ?? t("loadingOverlay.defaultMessage")}</p>
      </GlassCard>
    </div>
  );
}

export default LoadingOverlay;
