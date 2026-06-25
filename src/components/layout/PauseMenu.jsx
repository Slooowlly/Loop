import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

import useCareerStore from "../../stores/useCareerStore";
import useExitToMenu from "../../hooks/useExitToMenu";
import LeaveToMenuModal from "./LeaveToMenuModal";

const btnBase =
  "flex h-10 w-full items-center justify-center rounded-xl text-[13px] font-medium transition-colors";

/**
 * Menu de pausa estilo jogo: ESC abre/fecha um overlay central com Continuar /
 * Salvar / Configurações / Sair para o menu principal. Montado no MainLayout, então
 * vale em todas as telas da carreira.
 */
export default function PauseMenu() {
  const navigate = useNavigate();
  const flushSave = useCareerStore((state) => state.flushSave);
  const { isSaving, exit, saveAndExit } = useExitToMenu();

  const [open, setOpen] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [savedFlash, setSavedFlash] = useState(false);
  const confirmRef = useRef(false);
  confirmRef.current = confirm;

  // ESC alterna o pause; se o confirm de saída estiver aberto, ESC fecha só ele.
  useEffect(() => {
    function onKey(e) {
      if (e.key !== "Escape") return;
      e.preventDefault();
      if (confirmRef.current) {
        setConfirm(false);
        return;
      }
      setOpen((v) => !v);
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  async function handleSave() {
    try {
      await flushSave();
      setSavedFlash(true);
      setTimeout(() => setSavedFlash(false), 1500);
    } catch (_) {
      // silencioso
    }
  }

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center">
      <div className="absolute inset-0 bg-black/55 backdrop-blur-md" onClick={() => setOpen(false)} />

      <div className="glass-strong relative w-[320px] rounded-2xl border border-white/12 p-6 shadow-2xl">
        <div className="mb-4 flex items-center justify-between">
          <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-accent-primary">
            Pausa
          </p>
          <span className="text-[10px] text-text-secondary/70">ESC</span>
        </div>

        <div className="flex flex-col gap-2">
          <button
            type="button"
            onClick={() => setOpen(false)}
            className={`${btnBase} bg-accent-primary text-white hover:opacity-90`}
          >
            Continuar
          </button>
          <button
            type="button"
            onClick={handleSave}
            className={`${btnBase} border border-white/10 bg-white/6 text-text-secondary hover:bg-white/10 hover:text-text-primary`}
          >
            {savedFlash ? "Salvo ✓" : "Salvar"}
          </button>
          <button
            type="button"
            onClick={() => {
              setOpen(false);
              navigate("/settings", { state: { from: "/dashboard" } });
            }}
            className={`${btnBase} border border-white/10 bg-white/6 text-text-secondary hover:bg-white/10 hover:text-text-primary`}
          >
            Configurações
          </button>
          <button
            type="button"
            onClick={() => setConfirm(true)}
            className={`${btnBase} border border-status-red/25 bg-status-red/10 text-status-red hover:bg-status-red/20`}
          >
            Sair para o menu principal
          </button>
        </div>
      </div>

      <LeaveToMenuModal
        open={confirm}
        isSaving={isSaving}
        onSaveAndExit={saveAndExit}
        onExitWithoutSave={exit}
        onCancel={() => setConfirm(false)}
      />
    </div>
  );
}
