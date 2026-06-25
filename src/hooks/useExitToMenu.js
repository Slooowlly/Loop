import { useCallback, useState } from "react";
import { useNavigate } from "react-router-dom";

import useCareerStore from "../stores/useCareerStore";

/**
 * Ação de "voltar ao menu principal" compartilhada (menu de pausa + menu da equipe).
 * `exit` sai direto; `saveAndExit` salva antes. Ambos limpam a carreira e navegam.
 */
export default function useExitToMenu() {
  const navigate = useNavigate();
  const clearCareer = useCareerStore((state) => state.clearCareer);
  const flushSave = useCareerStore((state) => state.flushSave);
  const [isSaving, setIsSaving] = useState(false);

  const exit = useCallback(() => {
    clearCareer();
    navigate("/menu");
  }, [clearCareer, navigate]);

  const saveAndExit = useCallback(async () => {
    setIsSaving(true);
    try {
      await flushSave();
    } catch (_) {
      // Falha no flush não impede a saída.
    }
    setIsSaving(false);
    clearCareer();
    navigate("/menu");
  }, [flushSave, clearCareer, navigate]);

  return { isSaving, exit, saveAndExit };
}
