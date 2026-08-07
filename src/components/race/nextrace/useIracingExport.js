import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import { exportSuccess } from "../../../utils/sfx";
import { IRACING_TUTORIAL_KEY, carKeyForCategory, getDisplayError } from "./nextRaceHelpers";

// Exportação para o iRacing (roster + temporada), toasts de ação, tutorial da 1ª ida
// e o vínculo da cor do carro do jogador.
export function useIracingExport({ careerId, player, playerTeam, setError }) {
  const { t } = useTranslation();
  const [exportNotice, setExportNotice] = useState("");
  const [isExporting, setIsExporting] = useState(false);
  const [exported, setExported] = useState(false);
  const [showGoToast, setShowGoToast] = useState(false);
  const [iracingFocusMsg, setIracingFocusMsg] = useState("");
  const [showTutorial, setShowTutorial] = useState(false);
  const [tutorialMsg, setTutorialMsg] = useState("");
  const toastTimers = useRef([]);
  // "Pegar a cor do carro": botão ao lado de Simular Corrida que abre o modal.
  // Só aparece quando o jogador já conectou ao iRacing (temos o custid) e ainda
  // não vinculou a cor a este save.
  const [canPickPaint, setCanPickPaint] = useState(false);
  const [showPaintPrompt, setShowPaintPrompt] = useState(false);
  const [paintBusy, setPaintBusy] = useState(false);
  const [paintError, setPaintError] = useState("");
  const [paintToast, setPaintToast] = useState("");
  // Modo janela do iRacing: o overlay não aparece por cima de tela cheia exclusiva.
  // O pedido nasce aqui, na exportação, porque é o único momento em que o jogador
  // tem o iRacing no lobby (o simulador fechado) e está indo correr.
  const [showWindowPrompt, setShowWindowPrompt] = useState(false);
  const [windowBusy, setWindowBusy] = useState(false);
  const [windowError, setWindowError] = useState("");
  const [windowToast, setWindowToast] = useState("");

  // Decide se a opção "pegar a cor do carro" aparece na Sala de Estratégia: só
  // quando o jogador JÁ conectou ao iRacing (temos o custid — ele correu de verdade,
  // não simulou) e ainda NÃO vinculou a cor a este save. Checa o vínculo uma vez e
  // então faz um poll leve da conexão (o custid pode surgir enquanto ele está aqui).
  useEffect(() => {
    let active = true;
    if (!careerId) {
      setCanPickPaint(false);
      return undefined;
    }
    let handle;
    invoke("iracing_linked_custid", { careerId })
      .then((linked) => {
        if (!active) return;
        if (linked !== null && linked !== undefined) {
          setCanPickPaint(false); // já vinculado → não aparece mais aqui (vai pro mercado)
          return;
        }
        const check = () => {
          invoke("iracing_has_player_id")
            .then((hasId) => {
              if (active) setCanPickPaint(Boolean(hasId));
            })
            .catch(() => {});
        };
        check();
        handle = setInterval(check, 5000);
      })
      .catch(() => {});
    return () => {
      active = false;
      if (handle) clearInterval(handle);
    };
  }, [careerId]);

  async function handleGrabPaint() {
    if (!careerId) return;
    const carKey = carKeyForCategory(playerTeam?.categoria);
    setPaintBusy(true);
    setPaintError("");
    try {
      const res = await invoke("iracing_link_player_paint", { careerId, carKey });
      setShowPaintPrompt(false);
      setCanPickPaint(false); // vinculado → não aparece mais na Sala de Estratégia
      setPaintToast(
        t("nextRaceTab.paint.toastPainted", {
          color: res?.color ?? t("nextRaceTab.paint.teamColorFallback"),
        }),
      );
      const timer = setTimeout(() => setPaintToast(""), 6000);
      toastTimers.current.push(timer);
    } catch (e) {
      setPaintError(getDisplayError(e, t("nextRaceTab.errors.grabPaint")));
    } finally {
      setPaintBusy(false);
    }
  }

  async function handleExport() {
    setError("");
    const categoria = playerTeam?.categoria;
    if (!careerId || !categoria) {
      setError(t("nextRaceTab.errors.noCareerCategory"));
      return;
    }
    const rosterName = `Carreira ${player?.nome ?? "Loop"}`.trim();
    const carKey = carKeyForCategory(categoria);
    setExportNotice("");
    setIracingFocusMsg("");
    setIsExporting(true);
    try {
      await invoke("iracing_generate_roster", { careerId, categoria, rosterName, carKey });
      await invoke("iracing_generate_season", { careerId, categoria, rosterName, carKey });
      // Garante a macro de bandeira instalada agora — o iRacing está fechado neste
      // momento, então a escrita no app.ini "cola" (o sim só reescreve ao fechar).
      // Não-fatal: se falhar (ex.: app.ini não encontrado), não bloqueia a exportação.
      await invoke("iracing_install_yellow_macro").catch(() => {});
      dismissToasts();
      setExported(true);
      exportSuccess();
      // O pedido do modo janela entra ANTES do convite de ir ao iRacing: ajustar
      // depois de o jogador já ter entrado no simulador não adiantaria nada — o
      // sim reescreve esses arquivos ao fechar. Sem nada a ajustar, segue direto.
      const modo = await invoke("iracing_modo_janela_status").catch(() => null);
      if (modo?.deve_perguntar) {
        setWindowError("");
        setShowWindowPrompt(true);
      } else {
        agendarToastsDeIda();
      }
    } catch (invokeError) {
      setError(getDisplayError(invokeError, t("nextRaceTab.errors.export")));
    } finally {
      setIsExporting(false);
    }
  }

  // Stack de toasts: "Dados exportados" agora; "Entrar no iRacing" surge logo
  // abaixo (empurrando o primeiro pra cima). Ambos somem em 15s.
  function agendarToastsDeIda() {
    toastTimers.current.push(setTimeout(() => setShowGoToast(true), 550));
    toastTimers.current.push(setTimeout(() => dismissToasts(), 15000));
  }

  // "Sim, pode ajustar": escreve nos rendererDX11*.ini e só então convida a ir.
  async function handleWindowModeConfirm() {
    setWindowBusy(true);
    setWindowError("");
    try {
      await invoke("iracing_modo_janela_aplicar");
      setShowWindowPrompt(false);
      setWindowToast(t("nextRaceTab.windowMode.toastDone"));
      toastTimers.current.push(setTimeout(() => setWindowToast(""), 6000));
      agendarToastsDeIda();
    } catch (e) {
      // Fica no popup com o motivo (simulador aberto, arquivo ausente): é um erro
      // que o jogador consegue resolver e tentar de novo ali mesmo.
      setWindowError(getDisplayError(e, t("nextRaceTab.errors.windowMode")));
    } finally {
      setWindowBusy(false);
    }
  }

  // "Agora não": a exportação continua valendo, só o overlay é que não vai aparecer.
  function handleWindowModeSkip() {
    setShowWindowPrompt(false);
    setWindowError("");
    agendarToastsDeIda();
  }

  // Limpa os toasts pós-exportação e seus timers.
  function dismissToasts() {
    toastTimers.current.forEach(clearTimeout);
    toastTimers.current = [];
    setExported(false);
    setShowGoToast(false);
    setIracingFocusMsg("");
  }

  // Vai pro iRacing de fato: foca a janela se aberta; senão tenta abrir o
  // iRacingUI. Devolve { ok, message } para quem chamou decidir onde mostrar.
  async function goToIracingNow() {
    try {
      const focused = await invoke("iracing_focus_window");
      if (focused) return { ok: true };
      const launched = await invoke("iracing_launch_ui");
      if (launched) return { ok: true };
      return {
        ok: false,
        message: t("nextRaceTab.iracing.notOpen"),
      };
    } catch {
      return { ok: false, message: t("nextRaceTab.iracing.cantOpen") };
    }
  }

  // Clique no toast de ação: na 1ª vez abre o tutorial; depois vai direto.
  async function handleGoToIracing() {
    setIracingFocusMsg("");
    const seen = (() => {
      try {
        return localStorage.getItem(IRACING_TUTORIAL_KEY) === "1";
      } catch {
        return false;
      }
    })();
    if (!seen) {
      setTutorialMsg("");
      setShowTutorial(true);
      return;
    }
    const res = await goToIracingNow();
    if (res.ok) dismissToasts();
    else setIracingFocusMsg(res.message);
  }

  function markTutorialSeen() {
    try {
      localStorage.setItem(IRACING_TUTORIAL_KEY, "1");
    } catch {
      /* ignore */
    }
  }

  // "Done" no último passo: marca como visto e leva pro iRacing.
  async function handleTutorialDone() {
    markTutorialSeen();
    const res = await goToIracingNow();
    if (res.ok) {
      setShowTutorial(false);
      dismissToasts();
    } else {
      setTutorialMsg(res.message);
    }
  }

  // Fechar (X/fora): também marca como visto — o tutorial é só na 1ª vez.
  function handleTutorialClose() {
    markTutorialSeen();
    setShowTutorial(false);
  }

  return {
    exportNotice,
    setExportNotice,
    isExporting,
    exported,
    showGoToast,
    iracingFocusMsg,
    showTutorial,
    tutorialMsg,
    canPickPaint,
    showPaintPrompt,
    setShowPaintPrompt,
    paintBusy,
    paintError,
    setPaintError,
    paintToast,
    showWindowPrompt,
    windowBusy,
    windowError,
    windowToast,
    handleWindowModeConfirm,
    handleWindowModeSkip,
    handleGrabPaint,
    handleExport,
    handleGoToIracing,
    handleTutorialDone,
    handleTutorialClose,
  };
}
