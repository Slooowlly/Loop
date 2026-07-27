import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";

import BackupsModal from "../components/ui/BackupsModal";
import GlassButton from "../components/ui/GlassButton";
import GlassCard from "../components/ui/GlassCard";
import LoadingOverlay from "../components/ui/LoadingOverlay";
import SaveCard from "../components/ui/SaveCard";
import useCareerStore from "../stores/useCareerStore";

function DeleteSaveConfirmModal({ careerId, onConfirm, onCancel, deleting = false }) {
  const { t } = useTranslation();
  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center px-4">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onCancel} />
      <GlassCard
        hover={false}
        className="glass-strong relative w-full max-w-md rounded-[28px] border-white/12 p-6 sm:p-7"
        role="dialog"
        aria-modal="true"
        aria-labelledby="delete-save-title"
      >
        <p className="text-[11px] uppercase tracking-[0.24em] text-status-red">{t("loadSave.deleteModal.eyebrow")}</p>
        <h2 id="delete-save-title" className="mt-3 text-2xl font-semibold text-text-primary">
          {t("loadSave.deleteModal.title")}
        </h2>
        <p className="mt-3 text-sm leading-7 text-text-secondary">
          {t("loadSave.deleteModal.bodyPre")} <span className="font-semibold text-text-primary">{careerId}</span>{" "}
          {t("loadSave.deleteModal.bodyPost")}
        </p>

        <div
          data-testid="delete-save-actions"
          className="mt-6 flex flex-col items-center justify-center gap-3 sm:flex-row"
        >
          <GlassButton variant="secondary" disabled={deleting} onClick={onCancel}>
            {t("loadSave.deleteModal.cancel")}
          </GlassButton>
          <GlassButton variant="danger" disabled={deleting} onClick={onConfirm}>
            {deleting ? t("loadSave.deleteModal.deleting") : t("loadSave.deleteModal.confirm")}
          </GlassButton>
        </div>
      </GlassCard>
    </div>
  );
}

function LoadSave() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const loadCareer = useCareerStore((state) => state.loadCareer);
  const clearCareer = useCareerStore((state) => state.clearCareer);
  const [saves, setSaves] = useState([]);
  const [backupsSave, setBackupsSave] = useState(null);
  const [loading, setLoading] = useState(false);
  const [loadingMessage, setLoadingMessage] = useState(t("loadSave.status.searching"));
  const [error, setError] = useState("");
  const [pendingDeleteCareerId, setPendingDeleteCareerId] = useState(null);

  useEffect(() => {
    loadSaves();
  }, []);

  async function loadSaves() {
    setLoading(true);
    setLoadingMessage(t("loadSave.status.searching"));
    setError("");

    try {
      const loadedSaves = await invoke("list_saves");
      setSaves(loadedSaves);
    } catch (invokeError) {
      setError(
        typeof invokeError === "string"
          ? invokeError
          : t("loadSave.errors.loadList"),
      );
    } finally {
      setLoading(false);
    }
  }

  async function handleLoad(careerId) {
    setLoading(true);
    setLoadingMessage(t("loadSave.status.loadingCareer"));
    setError("");

    try {
      await loadCareer(careerId);
      navigate("/dashboard");
    } catch (invokeError) {
      setError(
        typeof invokeError === "string"
          ? invokeError
          : t("loadSave.errors.openCareer"),
      );
    } finally {
      setLoading(false);
    }
  }

  function handleDeleteRequest(careerId) {
    setPendingDeleteCareerId(careerId);
  }

  async function handleDeleteConfirm() {
    if (!pendingDeleteCareerId) return;

    const careerId = pendingDeleteCareerId;
    setPendingDeleteCareerId(null);
    setLoading(true);
    setLoadingMessage(t("loadSave.status.deleting"));
    setError("");

    try {
      await invoke("delete_career", { careerId });
      await loadSaves();
    } catch (invokeError) {
      setError(
        typeof invokeError === "string"
          ? invokeError
          : t("loadSave.errors.deleteCareer"),
      );
      setLoading(false);
    }
  }

  // Depois de restaurar, o estado em memoria aponta para o save antigo: limpa a
  // carreira carregada e relista os saves para refletir a viagem no tempo.
  async function handleRestored() {
    clearCareer?.();
    setBackupsSave(null);
    await loadSaves();
  }

  function handleDeleteCancel() {
    setPendingDeleteCareerId(null);
  }

  return (
    <div className="app-shell px-4 py-6 text-text-primary sm:px-6 lg:px-10">
      <div className="app-backdrop" />

      {pendingDeleteCareerId ? (
        <DeleteSaveConfirmModal
          careerId={pendingDeleteCareerId}
          deleting={loading}
          onCancel={handleDeleteCancel}
          onConfirm={handleDeleteConfirm}
        />
      ) : null}

      {backupsSave ? (
        <BackupsModal
          save={backupsSave}
          onClose={() => setBackupsSave(null)}
          onRestored={handleRestored}
        />
      ) : null}

      <div className="relative mx-auto flex min-h-[calc(100vh-3rem)] max-w-7xl items-center justify-center">
        <div className="wizard-panel glass w-full overflow-hidden rounded-[32px] p-5 shadow-[0_30px_80px_rgba(0,0,0,0.42)] sm:p-8 lg:p-10">
          <div className="relative z-10">
            <div className="flex flex-col gap-6 xl:flex-row xl:items-end xl:justify-between">
              <div>
                <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">
                  {t("loadSave.header.eyebrow")}
                </p>
                <h1 className="mt-3 text-4xl font-semibold tracking-[-0.04em] text-text-primary sm:text-5xl">
                  {t("loadSave.header.title")}
                </h1>
                <p className="mt-4 max-w-2xl text-sm leading-7 text-text-secondary sm:text-base">
                  {t("loadSave.header.subtitle")}
                </p>
              </div>

              <GlassCard
                hover={false}
                className="glass-light w-full max-w-xs rounded-3xl px-5 py-4 text-sm text-text-secondary"
              >
                <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                  {t("loadSave.library.eyebrow")}
                </p>
                <p className="mt-2 text-base font-semibold text-text-primary">
                  {saves.length}{" "}
                  {saves.length === 1
                    ? t("loadSave.library.careerSingular")
                    : t("loadSave.library.careerPlural")}
                </p>
                <p className="mt-1">{t("loadSave.library.note")}</p>
              </GlassCard>
            </div>

            {error ? (
              <div className="mt-6 rounded-2xl border border-status-red/40 bg-status-red/10 px-4 py-3 text-sm text-status-red">
                {error}
              </div>
            ) : null}

            <div className="mt-8 space-y-4">
              {saves.length === 0 && !loading ? (
                <GlassCard hover={false} className="glass-light rounded-[28px] p-12 text-center">
                  <div className="mb-4 text-6xl">🏎️</div>
                  <h3 className="text-2xl font-semibold text-text-primary">
                    {t("loadSave.empty.title")}
                  </h3>
                  <p className="mt-3 text-sm text-text-secondary">
                    {t("loadSave.empty.subtitle")}
                  </p>
                  <div className="mt-8">
                    <GlassButton variant="primary" onClick={() => navigate("/new-career")}>
                      {t("loadSave.empty.newCareer")}
                    </GlassButton>
                  </div>
                </GlassCard>
              ) : (
                saves.map((save) => (
                  <SaveCard
                    key={save.career_id}
                    save={save}
                    loading={loading}
                    onLoad={handleLoad}
                    onDelete={handleDeleteRequest}
                    onBackups={setBackupsSave}
                  />
                ))
              )}
            </div>

            <div className="mt-8 flex justify-start border-t border-white/10 pt-6">
              <GlassButton variant="secondary" onClick={() => navigate("/menu")}>
                {t("loadSave.backToMenu")}
              </GlassButton>
            </div>
          </div>
        </div>
      </div>

      <LoadingOverlay open={loading} title={t("loadSave.loadingOverlay.title")} message={loadingMessage} />
    </div>
  );
}

export default LoadSave;
