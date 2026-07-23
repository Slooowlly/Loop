/**
 * Modal do auto-update. Lê tudo do UpdaterProvider e reage ao `status`:
 *   available  → oferece baixar (+ "novidades" se o manifesto trouxe notas)
 *   downloading→ barra de progresso
 *   installing → "Instalando…"
 *   error      → mensagem de falha
 *   uptodate   → confirmação "já na versão mais recente" (só no fluxo manual)
 * Nos demais status (idle/checking) não renderiza nada.
 */
import { useTranslation } from "react-i18next";
import { useUpdater } from "./UpdaterProvider";

export default function UpdateGate() {
  const { t } = useTranslation();
  const {
    update,
    channel,
    status,
    progress,
    install,
    dismiss,
    openChangelog,
  } = useUpdater();

  const active = ["available", "downloading", "installing", "error", "uptodate"];
  if (!active.includes(status)) return null;

  const busy = status === "downloading" || status === "installing";
  const isBeta = channel === "beta";

  return (
    <div className="fixed inset-0 z-[130] flex items-center justify-center">
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={busy ? undefined : dismiss}
      />
      <div className="glass-strong relative w-[340px] rounded-2xl border border-white/12 p-6 shadow-2xl">
        {status === "uptodate" ? (
          <>
            <h3 className="mb-1 text-[15px] font-semibold text-text-primary">
              {t("updater.upToDateTitle")}
            </h3>
            <p className="mb-5 text-[13px] text-text-secondary">
              {t("updater.upToDate")}
            </p>
            <button
              type="button"
              onClick={dismiss}
              className="flex h-9 w-full items-center justify-center rounded-xl bg-accent-primary text-[13px] font-medium text-white transition-opacity hover:opacity-90"
            >
              {t("updater.close")}
            </button>
          </>
        ) : (
          <>
            <div className="mb-1 flex items-center gap-2">
              <h3 className="text-[15px] font-semibold text-text-primary">
                {t("updater.available")}
              </h3>
              {isBeta ? (
                <span className="rounded-md bg-accent-primary/20 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-accent-primary">
                  {t("updater.betaLabel")}
                </span>
              ) : null}
            </div>

            <p className="mb-4 text-[13px] text-text-secondary">
              {status === "error"
                ? t("updater.failed")
                : t("updater.availableBody", { version: update?.version })}
            </p>

            {busy ? (
              <div className="mb-4">
                <div className="mb-2 text-[12px] text-text-secondary">
                  {status === "installing"
                    ? t("updater.installing")
                    : progress == null
                      ? t("updater.downloading")
                      : `${t("updater.downloading")} ${Math.round(progress * 100)}%`}
                </div>
                <div className="h-1.5 w-full overflow-hidden rounded-full bg-white/10">
                  <div
                    className="h-full rounded-full bg-accent-primary transition-[width] duration-200"
                    style={{
                      width:
                        progress == null ? "40%" : `${Math.round(progress * 100)}%`,
                    }}
                  />
                </div>
              </div>
            ) : null}

            {update?.body && !busy ? (
              <button
                type="button"
                onClick={openChangelog}
                className="mb-4 text-[12px] font-medium text-accent-primary transition-opacity hover:opacity-80"
              >
                {t("updater.whatsNew")}
              </button>
            ) : null}

            <div className="flex flex-col gap-2">
              <button
                type="button"
                disabled={busy}
                onClick={install}
                className="flex h-9 w-full items-center justify-center rounded-xl bg-accent-primary text-[13px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
              >
                {status === "installing"
                  ? t("updater.installing")
                  : busy
                    ? t("updater.downloading")
                    : t("updater.install")}
              </button>
              {!busy ? (
                <button
                  type="button"
                  onClick={dismiss}
                  className="flex h-9 w-full items-center justify-center rounded-xl text-[13px] text-text-secondary transition-colors hover:text-text-primary"
                >
                  {t("updater.later")}
                </button>
              ) : null}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
