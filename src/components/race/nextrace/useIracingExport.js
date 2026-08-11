import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import { exportSuccess } from "../../../utils/sfx";
import { IRACING_TUTORIAL_KEY, carKeyForCategory, getDisplayError } from "./nextRaceHelpers";

// Chave (por save) do aviso único da pintura. A cor do carro é aplicada em toda
// exportação sem perguntar nada, e avisar sempre viraria ruído. Avisamos uma vez,
// para o jogador saber de onde veio a cor e onde desligar.
const PAINT_NOTICE_KEY = "loop.paintNoticed.";

// Exportação para o iRacing (roster + temporada), toasts de ação, tutorial da 1ª ida
// e a pintura automática do carro do jogador.
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
  // Timer próprio: o aviso da pintura NÃO entra em `toastTimers`, que é limpo pelo
  // `dismissToasts` da exportação. Limpo ali, o toast ficaria preso na tela.
  const paintTimer = useRef(null);
  const [paintToast, setPaintToast] = useState("");

  // Desmontagem: os dois conjuntos de timers morrem juntos. Separá-los é o que faz o
  // aviso da pintura sobreviver ao `dismissToasts`, mas ninguém sobrevive à saída da
  // aba — um `setTimeout` pendente aqui dispara um `setState` num hook que não existe
  // mais, e a aba da próxima corrida é montada de novo a cada visita à Sala.
  useEffect(
    () => () => {
      toastTimers.current.forEach(clearTimeout);
      toastTimers.current = [];
      clearTimeout(paintTimer.current);
    },
    [],
  );

  // Pinta o carro do jogador na cor da equipe junto com a exportação da etapa. Sem
  // pergunta: o arquivo é local (só ele vê essa cor), a cor é a da carreira, e o
  // .tga que já existia fica preservado ao lado, em .tga.loop-bak. O backend devolve
  // null quando a pintura está desligada nas Configurações ou quando ainda não temos
  // o ID do iRacing — nos dois casos não há nada a dizer.
  async function pintarCarro(careerIdAtual, carKey) {
    let res;
    try {
      res = await invoke("iracing_auto_paint_player", { careerId: careerIdAtual, carKey });
    } catch {
      return; // pintura é acessório da exportação, nunca a derruba
    }
    if (!res) return;
    const chave = PAINT_NOTICE_KEY + careerIdAtual;
    try {
      if (localStorage.getItem(chave) === "1") return;
      localStorage.setItem(chave, "1");
    } catch {
      return; // sem localStorage, prefira o silêncio a avisar toda exportação
    }
    setPaintToast(
      t("nextRaceTab.paint.toastPainted", {
        color: res.color ?? t("nextRaceTab.paint.teamColorFallback"),
      }),
    );
    clearTimeout(paintTimer.current);
    paintTimer.current = setTimeout(() => setPaintToast(""), 8000);
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
      // Mesmo tratamento para o modo janela (pré-requisito do overlay): o boot do
      // Loop já tentou, e esta é a segunda chance para quem abriu o Loop com o
      // simulador rodando. Ajustar depois que ele entrar no sim não adiantaria — o
      // iRacing reescreve esses arquivos ao fechar. Não-fatal e sem pergunta.
      await invoke("iracing_modo_janela_aplicar").catch(() => {});
      dismissToasts();
      setExported(true);
      exportSuccess();
      agendarToastsDeIda();
      // Depois do `dismissToasts` acima, senão o aviso da pintura nasceria e morreria
      // no mesmo instante.
      await pintarCarro(careerId, carKey);
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
    paintToast,
    handleExport,
    handleGoToIracing,
    handleTutorialDone,
    handleTutorialClose,
  };
}
