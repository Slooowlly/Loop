import { useTranslation } from "react-i18next";
import GlassCard from "./GlassCard";

/// `children` é o painel opcional que acompanha a espera, DEBAIXO do card da mensagem
/// e fora dele: o card é estreito por decisão (`max-w-sm` centraliza a frase), e um
/// conteúdo largo dentro dele espremeria a frase junto. Quem passa filho decide a
/// própria largura; o overlay só empilha e centraliza.
function LoadingOverlay({ open = false, title, message, children }) {
  const { t } = useTranslation();
  if (!open) return null;

  return (
    <div className="absolute inset-0 z-50 flex flex-col items-center justify-center gap-6 overflow-hidden bg-app-bg/55 backdrop-blur-[20px]">
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
      {children}
    </div>
  );
}

export default LoadingOverlay;
