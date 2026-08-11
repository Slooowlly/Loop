import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// As ferramentas de bancada da tela de Configurações: gravador de corrida, demo do overlay
// de rádio e o armador de quebra ao vivo.
//
// Todas existem para calibrar o app contra pista de verdade e nenhuma delas faz parte do
// fluxo do jogo. Ficam juntas aqui porque falham do mesmo jeito (mensagem na tela, nada
// quebra) e porque tirá-las do componente deixa visível que são um bloco só, com destino
// próprio no build comercial.
export function useFerramentasDeDebug(t) {
  // Gravador: salva telemetria crua + YAML + histórico num arquivo.
  const [capture, setCapture] = useState({ active: false, frames: 0, dir: "" });
  const [captureMsg, setCaptureMsg] = useState("");

  // Demo do overlay de rádio: card de exemplo ciclando, para achar e posicionar o overlay
  // sem esperar uma quebra real. O estado mora no backend, porque vale para a janela do rádio.
  const [radioDemo, setRadioDemo] = useState(false);

  // Armador: falha garantida no carro do jogador para a próxima volta cruzada. Valida o
  // pipeline ponta a ponta (detecção de volta + disparo do !black/!dq).
  const [armBusy, setArmBusy] = useState(false);
  const [armMsg, setArmMsg] = useState("");

  const refreshCapture = useCallback(async () => {
    try {
      setCapture(await invoke("race_capture_status"));
    } catch {
      /* ignora */
    }
  }, []);

  useEffect(() => {
    refreshCapture();
    const id = setInterval(refreshCapture, 2000);
    return () => clearInterval(id);
  }, [refreshCapture]);

  useEffect(() => {
    invoke("overlay_demo_enabled")
      .then((v) => setRadioDemo(Boolean(v)))
      .catch(() => {});
  }, []);

  const toggleCapture = useCallback(async () => {
    try {
      if (capture.active) {
        const path = await invoke("race_capture_stop");
        setCaptureMsg(
          path ? t("settings.debug.captureSaved", { path }) : t("settings.debug.captureStopped"),
        );
      } else {
        const path = await invoke("race_capture_start");
        setCaptureMsg(t("settings.debug.capturing", { path }));
      }
      await refreshCapture();
    } catch (err) {
      setCaptureMsg(String(err));
    }
  }, [capture.active, refreshCapture, t]);

  const toggleRadioDemo = useCallback(async () => {
    const next = !radioDemo;
    setRadioDemo(next);
    try {
      await invoke("overlay_set_demo", { on: next });
    } catch {
      /* ignora */
    }
  }, [radioDemo]);

  const armBreakdown = useCallback(async () => {
    if (armBusy) return;
    setArmBusy(true);
    setArmMsg("");
    try {
      const ok = await invoke("iracing_arm_test_breakdown");
      setArmMsg(ok ? t("settings.debug.armedMyCar") : t("settings.debug.armFailed"));
    } catch (err) {
      setArmMsg(String(err));
    } finally {
      setArmBusy(false);
    }
  }, [armBusy, t]);

  const armBreakdownGrid = useCallback(async () => {
    if (armBusy) return;
    setArmBusy(true);
    setArmMsg("");
    try {
      await invoke("iracing_arm_test_breakdown_grid");
      setArmMsg(t("settings.debug.armedGrid"));
    } catch (err) {
      setArmMsg(String(err));
    } finally {
      setArmBusy(false);
    }
  }, [armBusy, t]);

  return {
    capture,
    captureMsg,
    toggleCapture,
    radioDemo,
    toggleRadioDemo,
    armBusy,
    armMsg,
    armBreakdown,
    armBreakdownGrid,
  };
}

export default useFerramentasDeDebug;
