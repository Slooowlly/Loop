import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useLocation, useNavigate } from "react-router-dom";

import useCareerStore from "../../stores/useCareerStore";

const widgetItems = [{ emoji: "🏠", route: "/menu", title: "Home", clearsCareer: true }];

const buttonClass =
  "flex h-9 w-9 items-center justify-center rounded-xl text-text-secondary transition-glass hover:bg-white/8 hover:text-text-primary";

function SaveConfirmModal({ onSave, onDiscard, onCancel, isSaving }) {
  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onCancel} />
      <div className="glass-strong relative w-[340px] rounded-2xl border border-white/12 p-6 shadow-2xl">
        <h3 className="mb-1 text-[15px] font-semibold text-text-primary">
          Deseja sair da carreira agora?
        </h3>
        <p className="mb-5 text-[13px] text-text-secondary">
          Você pode salvar antes de voltar ao menu principal ou fechar o jogo.
        </p>
        <div className="flex flex-col gap-2">
          <button
            type="button"
            disabled={isSaving}
            onClick={onSave}
            className="flex h-9 w-full items-center justify-center rounded-xl bg-accent-primary text-[13px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
          >
            {isSaving ? "Salvando..." : "Salvar e sair"}
          </button>
          <button
            type="button"
            disabled={isSaving}
            onClick={onDiscard}
            className="flex h-9 w-full items-center justify-center rounded-xl border border-white/10 bg-white/6 text-[13px] text-text-secondary transition-colors hover:bg-white/10 hover:text-text-primary disabled:opacity-50"
          >
            Sair sem salvar
          </button>
          <button
            type="button"
            disabled={isSaving}
            onClick={onCancel}
            className="flex h-9 w-full items-center justify-center rounded-xl text-[13px] text-text-secondary transition-colors hover:text-text-primary disabled:opacity-50"
          >
            Cancelar
          </button>
        </div>
      </div>
    </div>
  );
}

function WindowControlsDrawer() {
  const navigate = useNavigate();
  const location = useLocation();
  const shouldHideDrawer = location.pathname === "/" || location.pathname === "/splash";
  const clearCareer = useCareerStore((state) => state.clearCareer);
  const flushSave = useCareerStore((state) => state.flushSave);
  const [isFullscreen, setIsFullscreen] = useState(true);
  const [isOpen, setIsOpen] = useState(false);
  const [showWidgets, setShowWidgets] = useState(false);
  const widgetsTimerRef = useRef(null);
  const [savePrompt, setSavePrompt] = useState(null);
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

  useEffect(() => {
    return () => {
      if (widgetsTimerRef.current) {
        clearTimeout(widgetsTimerRef.current);
      }
    };
  }, []);

  function handleDrawerEnter() {
    setIsOpen(true);

    if (widgetsTimerRef.current) {
      clearTimeout(widgetsTimerRef.current);
    }

    widgetsTimerRef.current = setTimeout(() => {
      setShowWidgets(true);
    }, 500);
  }

  function handleDrawerLeave() {
    if (widgetsTimerRef.current) {
      clearTimeout(widgetsTimerRef.current);
    }

    setShowWidgets(false);
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
    setSavePrompt({ mode: "close" });
  }

  function handleWidgetClick(event, widget) {
    event.stopPropagation();
    setSavePrompt({ mode: "navigate", widget });
  }

  async function handleSaveAndProceed() {
    setIsSaving(true);
    try {
      await flushSave();
    } catch (_) {
      // Falha no flush não impede a saída.
    }
    setIsSaving(false);
    await proceedAfterPrompt(savePrompt);
  }

  async function handleDiscardAndProceed() {
    await proceedAfterPrompt(savePrompt);
  }

  async function proceedAfterPrompt(prompt) {
    setSavePrompt(null);
    if (!prompt) return;

    if (prompt.mode === "close") {
      await doClose();
      return;
    }

    if (prompt.mode === "navigate") {
      const { widget } = prompt;
      if (widget.clearsCareer) {
        clearCareer();
      }
      if (location.pathname !== widget.route) {
        navigate(widget.route);
      }
    }
  }

  function handleCancelPrompt() {
    setSavePrompt(null);
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
        <div className="relative h-[390px] w-[148px]" onMouseLeave={handleDrawerLeave}>
          <div className="relative z-10 ml-auto w-[132px]" onMouseEnter={handleDrawerEnter}>
            <div
              className={[
                "absolute right-0 top-0 flex w-[132px] flex-col items-center transition-all duration-300 ease-out",
                isOpen ? "translate-x-0 opacity-100" : "translate-x-4 opacity-0",
              ].join(" ")}
              style={{ pointerEvents: isOpen ? "auto" : "none" }}
            >
              <div className="glass-strong flex items-center gap-0.5 rounded-2xl border border-white/12 px-1.5 py-1.5">
                <button type="button" className={buttonClass} onClick={handleMinimize}>
                  <span className="block -translate-y-[1px] text-[11px]">&minus;</span>
                </button>
                <button type="button" className={buttonClass} onClick={handleToggleFullscreen}>
                  <span className="block text-[12px]">{isFullscreen ? "❐" : "□"}</span>
                </button>
                <button
                  type="button"
                  aria-label="Fechar app"
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
                  v0.10
                </p>
              </div>

              <div
                className={[
                  "glass-strong absolute left-1/2 top-[84px] flex w-[58px] -translate-x-1/2 flex-col items-center gap-2 rounded-[28px] border border-white/10 px-2 py-3 transition-all duration-500 ease-out",
                  showWidgets ? "translate-y-0 opacity-100" : "-translate-x-1/2 -translate-y-4 opacity-0",
                ].join(" ")}
                style={{ pointerEvents: showWidgets ? "auto" : "none" }}
              >
                {widgetItems.map((widget) => {
                  const isActive = location.pathname === widget.route;

                  return (
                    <button
                      key={widget.route}
                      type="button"
                      title={widget.title}
                      aria-label={widget.title}
                      onClick={(event) => handleWidgetClick(event, widget)}
                      className={[
                        "flex h-10 w-10 items-center justify-center rounded-2xl border text-[18px] shadow-[inset_0_1px_0_rgba(255,255,255,0.14)] transition-all duration-300 hover:-translate-y-[1px] hover:bg-white/12",
                        isActive
                          ? "border-white/25 bg-white/14 text-text-primary"
                          : "border-white/10 bg-white/6",
                      ].join(" ")}
                    >
                      <span className="drop-shadow-[0_1px_6px_rgba(255,255,255,0.18)]">
                        {widget.emoji}
                      </span>
                    </button>
                  );
                })}
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
