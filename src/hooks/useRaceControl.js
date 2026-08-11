import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Race Control: a macro de bandeira amarela no `app.ini` do iRacing, o disparo automático
// e o chat de texto livre que valida o caminho sem macro.
//
// A macro é instalada SOZINHA ao abrir as Configurações: quando o jogador for correr ela
// já está pronta, sem ele precisar saber o que é "macro". O iRacing reescreve o `app.ini`
// ao fechar, então o momento certo é com o sim fechado — daí a instalação ser na abertura
// da tela, e não na hora da corrida.
export function useRaceControl(t) {
  const [yellowStatus, setYellowStatus] = useState(null);
  const [yellowMsg, setYellowMsg] = useState("");
  const [autoYellow, setAutoYellow] = useState(false);
  const [yellowBusy, setYellowBusy] = useState(false);

  const [chatText, setChatText] = useState("!black #1 20");
  const [chatMsg, setChatMsg] = useState("");
  const [chatBusy, setChatBusy] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        let status = await invoke("iracing_yellow_macro_status");
        if (status?.app_ini_found && !status.installed) {
          status = await invoke("iracing_install_yellow_macro");
        }
        setYellowStatus(status);
      } catch (err) {
        console.error("Falha ao preparar a macro de bandeira:", err);
      }
    })();
    invoke("iracing_auto_yellow_enabled")
      .then((v) => setAutoYellow(Boolean(v)))
      .catch(() => {});
  }, []);

  const raceControlOn = Boolean(yellowStatus?.installed && autoYellow);

  const toggleRaceControl = useCallback(async () => {
    if (yellowBusy || !yellowStatus?.installed) return;
    const next = !autoYellow;
    setYellowBusy(true);
    setAutoYellow(next);
    try {
      await invoke("iracing_set_auto_yellow", { enabled: next });
    } catch (err) {
      setAutoYellow(!next);
      setYellowMsg(String(err));
    } finally {
      setYellowBusy(false);
    }
  }, [autoYellow, yellowBusy, yellowStatus]);

  const sendChatTest = useCallback(async () => {
    const text = chatText.trim();
    if (chatBusy || !text) return;
    setChatBusy(true);
    setChatMsg("");
    try {
      await invoke("iracing_send_chat_text", { text });
      setChatMsg(t("settings.debug.chatSent", { text }));
    } catch (err) {
      setChatMsg(String(err));
    } finally {
      setChatBusy(false);
    }
  }, [chatBusy, chatText, t]);

  return {
    yellowStatus,
    yellowMsg,
    autoYellow,
    yellowBusy,
    raceControlOn,
    toggleRaceControl,
    chatText,
    setChatText,
    chatMsg,
    chatBusy,
    sendChatTest,
  };
}

export default useRaceControl;
