import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useLocation } from "react-router-dom";
import { useTranslation } from "react-i18next";

import useCareerStore from "../../stores/useCareerStore";
import Tooltip from "../ui/Tooltip";

const buttonClass =
  "flex h-9 w-9 items-center justify-center rounded-xl text-text-secondary transition-glass hover:bg-white/8 hover:text-text-primary";

function SaveConfirmModal({ onSave, onDiscard, onCancel, isSaving }) {
  const { t } = useTranslation();
  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onCancel} />
      <div className="glass-strong relative w-[340px] rounded-2xl border border-white/12 p-6 shadow-2xl">
        <h3 className="mb-1 text-[15px] font-semibold text-text-primary">{t("closeApp.title")}</h3>
        <p className="mb-5 text-[13px] text-text-secondary">
          {t("closeApp.desc")}
        </p>
        <div className="flex flex-col gap-2">
          <button
            type="button"
            disabled={isSaving}
            onClick={onSave}
            className="flex h-9 w-full items-center justify-center rounded-xl bg-accent-primary text-[13px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
          >
            {isSaving ? t("leaveModal.saving") : t("leaveModal.saveAndExit")}
          </button>
          <button
            type="button"
            disabled={isSaving}
            onClick={onDiscard}
            className="flex h-9 w-full items-center justify-center rounded-xl border border-white/10 bg-white/6 text-[13px] text-text-secondary transition-colors hover:bg-white/10 hover:text-text-primary disabled:opacity-50"
          >
            {t("leaveModal.exitNoSave")}
          </button>
          <button
            type="button"
            disabled={isSaving}
            onClick={onCancel}
            className="flex h-9 w-full items-center justify-center rounded-xl text-[13px] text-text-secondary transition-colors hover:text-text-primary disabled:opacity-50"
          >
            {t("leaveModal.cancel")}
          </button>
        </div>
      </div>
    </div>
  );
}

function WindowControlsDrawer() {
  const { t } = useTranslation();
  const location = useLocation();
  // Mostrar em todas as telas (inclusive o menu inicial "/") — é o controle de janela
  // (sair do fullscreen, minimizar, fechar). Só fica oculto numa splash dedicada.
  const shouldHideDrawer = location.pathname === "/splash";
  const flushSave = useCareerStore((state) => state.flushSave);
  const [isFullscreen, setIsFullscreen] = useState(true);
  const [isOpen, setIsOpen] = useState(false);
  const [savePrompt, setSavePrompt] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    let mounted = true;

    async function syncWindowState() {
      try {
        const fullscreen = await invoke("get_window_fullscreen");
        if (mounted) {
          setIsFullscreen(Boolean(fullscreen));
        }
      } catch (error) {
        console.error("Falha ao ler estado da janela:", error);
      }
    }

    syncWindowState();

    return () => {
      mounted = false;
    };
  }, []);

  function handleDrawerEnter() {
    setIsOpen(true);
  }

  function handleDrawerLeave() {
    setIsOpen(false);
  }

  async function handleMinimize(event) {
    event.stopPropagation();
    try {
      await invoke("minimize_window");
    } catch (error) {
      console.error("Falha ao minimizar janela:", error);
    }
  }

  async function handleStartDrag(event) {
    event.stopPropagation();
    if (event.button !== 0) return; // só botão esquerdo
    // O arrasto a nível de SO rouba os eventos do mouse, então o onMouseLeave
    // que fecharia o drawer nunca dispara. Fecha manualmente ANTES, senão o
    // overlay de escurecimento fica preso ligado.
    handleDrawerLeave();
    try {
      await invoke("start_window_drag");
    } catch (error) {
      console.error("Falha ao arrastar janela:", error);
    }
  }

  async function handleToggleFullscreen(event) {
    event.stopPropagation();
    try {
      const fullscreen = await invoke("toggle_fullscreen_window");
      setIsFullscreen(Boolean(fullscreen));
    } catch (error) {
      console.error("Falha ao alternar modo tela cheia:", error);
    }
  }

  async function doClose() {
    try {
      await invoke("close_window");
    } catch (error) {
      console.error("Falha ao fechar janela:", error);
    }
  }

  function handleClose(event) {
    event.stopPropagation();
    setSavePrompt(true);
  }

  async function handleSaveAndProceed() {
    setIsSaving(true);
    try {
      await flushSave();
    } catch (_) {
      // Falha no flush não impede o fechamento.
    }
    setIsSaving(false);
    setSavePrompt(false);
    await doClose();
  }

  async function handleDiscardAndProceed() {
    setSavePrompt(false);
    await doClose();
  }

  function handleCancelPrompt() {
    setSavePrompt(false);
  }

  if (shouldHideDrawer) {
    return null;
  }

  return (
    <>
      {savePrompt && (
        <SaveConfirmModal
          isSaving={isSaving}
          onSave={handleSaveAndProceed}
          onDiscard={handleDiscardAndProceed}
          onCancel={handleCancelPrompt}
        />
      )}

      <div
        className={[
          "pointer-events-none fixed inset-0 z-40 bg-black/0 backdrop-blur-[0px] transition-all duration-300 ease-out",
          isOpen ? "bg-black/18 backdrop-blur-[5px]" : "",
        ].join(" ")}
      />

      <div className="fixed right-5 top-[36px] z-50">
        <div className="relative h-[120px] w-[148px]" onMouseLeave={handleDrawerLeave}>
          <div className="relative z-10 ml-auto w-[132px]" onMouseEnter={handleDrawerEnter}>
            <div
              className={[
                "absolute right-0 top-0 flex w-[132px] flex-col items-center transition-all duration-300 ease-out",
                isOpen ? "translate-x-0 opacity-100" : "translate-x-4 opacity-0",
              ].join(" ")}
              style={{ pointerEvents: isOpen ? "auto" : "none" }}
            >
              <div className="glass-strong flex items-center gap-0.5 rounded-2xl border border-white/12 px-1.5 py-1.5">
                <Tooltip texto={t("windowControls.dragHint")}>
                  <div
                    role="button"
                    aria-label={t("windowControls.drag")}
                    onMouseDown={handleStartDrag}
                    className={`${buttonClass} cursor-grab active:cursor-grabbing`}
                  >
                    <span className="block text-[13px] leading-none">⠿</span>
                  </div>
                </Tooltip>
                <button type="button" className={buttonClass} onClick={handleMinimize}>
                  <span className="block -translate-y-[1px] text-[11px]">&minus;</span>
                </button>
                <button type="button" className={buttonClass} onClick={handleToggleFullscreen}>
                  <span className="block text-[12px]">{isFullscreen ? "❐" : "□"}</span>
                </button>
                <button
                  type="button"
                  aria-label={t("windowControls.close")}
                  className={`${buttonClass} hover:bg-status-red/20 hover:text-status-red`}
                  onClick={handleClose}
                >
                  <span className="block text-[14px]">&times;</span>
                </button>
              </div>

              <div className="pointer-events-none mt-0 w-full text-center">
                <p className="text-[13px] font-semibold tracking-[0.12em] text-text-primary/86">
                  Loop
                </p>
                <p className="mt-0 text-[8px] font-medium tracking-[0.12em] text-text-secondary/75">
                  v{__APP_VERSION__.replace(/\.0$/, "")}
                </p>
              </div>
            </div>
          </div>

          <div
            data-testid="window-controls-hover-target"
            className={[
              "absolute right-0 top-[8px] z-20 flex h-10 w-10 items-center justify-center rounded-xl transition-all duration-300 ease-out",
              isOpen ? "pointer-events-none" : "",
            ].join(" ")}
            onMouseEnter={handleDrawerEnter}
          >
            <span
              className={[
                "select-none text-[22px] leading-none text-accent-primary/75 transition-all duration-300 ease-out",
                isOpen ? "-translate-x-0.5 opacity-35" : "translate-x-0 opacity-90",
              ].join(" ")}
            >
              &#x2039;
            </span>
          </div>
        </div>
      </div>
    </>
  );
}

export default WindowControlsDrawer;
