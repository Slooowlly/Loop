import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import GlassButton from "./GlassButton";
import GlassCard from "./GlassCard";
import { formatDateTime } from "../../utils/formatters";

// Painel de backups de uma carreira. O backend cria um backup por temporada
// (e o novo sobrescreve o antigo), entao as duas acoes destrutivas — criar e
// restaurar — passam por confirmacao explicita antes de chegar no invoke.
function BackupsModal({ save, onClose, onRestored }) {
  const { t } = useTranslation();
  const careerId = save.career_id;
  const currentSeason = save.season;

  const [backups, setBackups] = useState([]);
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  // null | { tipo: "criar" } | { tipo: "restaurar", temporada: number }
  const [pending, setPending] = useState(null);

  useEffect(() => {
    let ativo = true;

    async function carregar() {
      setLoading(true);
      setError("");
      try {
        const lista = await invoke("list_backups", { careerId });
        if (ativo) setBackups(lista ?? []);
      } catch (invokeError) {
        if (ativo) {
          setError(typeof invokeError === "string" ? invokeError : t("loadSave.backups.errors.list"));
        }
      } finally {
        if (ativo) setLoading(false);
      }
    }

    carregar();
    return () => {
      ativo = false;
    };
  }, [careerId, t]);

  async function recarregar() {
    try {
      const lista = await invoke("list_backups", { careerId });
      setBackups(lista ?? []);
    } catch (invokeError) {
      setError(typeof invokeError === "string" ? invokeError : t("loadSave.backups.errors.list"));
    }
  }

  async function confirmarCriacao() {
    setPending(null);
    setWorking(true);
    setError("");

    try {
      await invoke("create_season_backup", { careerId, seasonNumber: currentSeason });
      await recarregar();
    } catch (invokeError) {
      setError(typeof invokeError === "string" ? invokeError : t("loadSave.backups.errors.create"));
    } finally {
      setWorking(false);
    }
  }

  async function confirmarRestauracao(temporada) {
    setPending(null);
    setWorking(true);
    setError("");

    try {
      await invoke("restore_backup", { careerId, seasonNumber: temporada });
      onRestored?.(careerId);
    } catch (invokeError) {
      setError(typeof invokeError === "string" ? invokeError : t("loadSave.backups.errors.restore"));
      setWorking(false);
    }
  }

  const ocupado = loading || working;

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center px-4">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={ocupado ? undefined : onClose} />
      <GlassCard
        hover={false}
        className="glass-strong relative w-full max-w-2xl rounded-[28px] border-white/[0.12] p-6 sm:p-7"
        role="dialog"
        aria-modal="true"
        aria-labelledby="backups-title"
      >
        <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">
          {t("loadSave.backups.eyebrow")}
        </p>
        <h2 id="backups-title" className="mt-3 text-2xl font-semibold text-text-primary">
          {t("loadSave.backups.title")}
        </h2>
        <p className="mt-3 text-sm leading-7 text-text-secondary">{t("loadSave.backups.subtitle")}</p>

        {error ? (
          <div className="mt-5 rounded-2xl border border-status-red/40 bg-status-red/10 px-4 py-3 text-sm text-status-red">
            {error}
          </div>
        ) : null}

        {pending ? (
          <div
            data-testid="backup-confirm"
            className="mt-5 rounded-2xl border border-status-orange/40 bg-status-orange/10 px-4 py-4"
          >
            <p className="text-sm font-semibold text-text-primary">
              {pending.tipo === "criar"
                ? t("loadSave.backups.createConfirm.title", { season: currentSeason })
                : t("loadSave.backups.restoreConfirm.title", { season: pending.temporada })}
            </p>
            <p className="mt-2 text-sm leading-7 text-text-secondary">
              {pending.tipo === "criar"
                ? t("loadSave.backups.createConfirm.body")
                : t("loadSave.backups.restoreConfirm.body")}
            </p>
            <div className="mt-4 flex flex-col gap-3 sm:flex-row">
              <GlassButton variant="secondary" onClick={() => setPending(null)}>
                {t("loadSave.backups.confirmCancel")}
              </GlassButton>
              <GlassButton
                variant={pending.tipo === "criar" ? "primary" : "danger"}
                onClick={() =>
                  pending.tipo === "criar" ? confirmarCriacao() : confirmarRestauracao(pending.temporada)
                }
              >
                {pending.tipo === "criar"
                  ? t("loadSave.backups.createConfirm.confirm")
                  : t("loadSave.backups.restoreConfirm.confirm")}
              </GlassButton>
            </div>
          </div>
        ) : null}

        <div className="mt-6 max-h-[46vh] space-y-3 overflow-y-auto pr-1">
          {loading ? (
            <p className="text-sm text-text-secondary">{t("loadSave.backups.loading")}</p>
          ) : backups.length === 0 ? (
            <p data-testid="backups-empty" className="text-sm text-text-secondary">
              {t("loadSave.backups.empty")}
            </p>
          ) : (
            backups.map((backup) => (
              <div
                key={backup.season_number}
                className="glass-light flex flex-col gap-3 rounded-2xl p-4 sm:flex-row sm:items-center sm:justify-between"
              >
                <div>
                  <p className="text-sm font-semibold text-text-primary">
                    {t("loadSave.backups.seasonLabel", { season: backup.season_number })}
                  </p>
                  <p className="mt-1 text-xs text-text-secondary">
                    {formatDateTime(backup.modified_at)} ·{" "}
                    {t("loadSave.backups.size", { kb: backup.size_kb })}
                  </p>
                </div>
                <GlassButton
                  variant="danger"
                  disabled={ocupado}
                  className="min-w-32"
                  onClick={() => setPending({ tipo: "restaurar", temporada: backup.season_number })}
                >
                  {t("loadSave.backups.restore")}
                </GlassButton>
              </div>
            ))
          )}
        </div>

        <div className="mt-6 flex flex-col gap-3 border-t border-white/10 pt-5 sm:flex-row sm:justify-between">
          <GlassButton variant="secondary" disabled={ocupado} onClick={onClose}>
            {t("loadSave.backups.close")}
          </GlassButton>
          <GlassButton variant="primary" disabled={ocupado} onClick={() => setPending({ tipo: "criar" })}>
            {working ? t("loadSave.backups.working") : t("loadSave.backups.create")}
          </GlassButton>
        </div>
      </GlassCard>
    </div>
  );
}

export default BackupsModal;
