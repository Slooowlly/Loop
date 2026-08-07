import { useEffect, useRef, useState } from "react";
import { Navigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import MainLayout from "../components/layout/MainLayout";
import RaceResultViewV2 from "../components/race/RaceResultViewV2";
import ConvocationView from "../components/season/ConvocationView";
import EndOfSeasonView from "../components/season/EndOfSeasonView";
import PreSeasonView from "../components/season/PreSeasonView";
import useCareerStore from "../stores/useCareerStore";
import {
  NEWS_READ_MS,
  recordNewsRead,
  recordNewsSkip,
  resolvePostRaceLanding,
} from "../utils/postRaceLanding";
import CalendarTabRedesign from "./tabs/CalendarTabRedesign";
import MyTeamTab from "./tabs/myteam";
import NewsMagazineTab from "./tabs/NewsMagazineTab";
import NextRaceTab from "./tabs/NextRaceTab";
import StandingsTab from "./tabs/StandingsTab";
import GlobalDriversTab from "./tabs/GlobalDriversTab";
import GlobalTeamsTab from "./tabs/atlas";
import TeamRecordsTab from "./tabs/TeamRecordsTab";

// Ao chegar o dia da corrida: a célula da PISTA pulsa por ~1s no calendário,
// anunciando que vai abrir, e só então entra a sala de estratégia (com fade).
const RACE_ARRIVAL_FEEDBACK_MS = 1000;

function Dashboard() {
  const isLoaded = useCareerStore((state) => state.isLoaded);
  const showResult = useCareerStore((state) => state.showResult);
  const lastRaceResult = useCareerStore((state) => state.lastRaceResult);
  const lastRaceEvaluation = useCareerStore((state) => state.lastRaceEvaluation);
  const lastRaceTelemetry = useCareerStore((state) => state.lastRaceTelemetry);
  const lastRaceMaintenance = useCareerStore((state) => state.lastRaceMaintenance);
  const lastRaceRepercussion = useCareerStore((state) => state.lastRaceRepercussion);
  const dismissResult = useCareerStore((state) => state.dismissResult);
  const lastRaceWasFinale = useCareerStore((state) => state.lastRaceWasFinale);
  const resultIsFresh = useCareerStore((state) => state.resultIsFresh);
  const loadSeasonChampionOverlay = useCareerStore((state) => state.loadSeasonChampionOverlay);
  const season = useCareerStore((state) => state.season);
  const careerId = useCareerStore((state) => state.careerId);
  const pollIracingResult = useCareerStore((state) => state.pollIracingResult);
  const iracingRepair = useCareerStore((state) => state.iracingRepair);
  const dismissIracingRepair = useCareerStore((state) => state.dismissIracingRepair);
  const showEndOfSeason = useCareerStore((state) => state.showEndOfSeason);
  const endOfSeasonResult = useCareerStore((state) => state.endOfSeasonResult);
  const showPreseason = useCareerStore((state) => state.showPreseason);
  const showConvocation = useCareerStore((state) => state.showConvocation);
  const showRaceBriefing = useCareerStore((state) => state.showRaceBriefing);
  const [activeTab, setActiveTab] = useState("standings");
  const [globalDriversSelectedId, setGlobalDriversSelectedId] = useState(null);
  // Métrica de carreira pedida pelos cards de recorde da ficha do piloto — a
  // tela global abre ordenada por ela. Nulo quando a entrada foi pela lista.
  const [globalDriversMetric, setGlobalDriversMetric] = useState(null);
  // A categoria vem junto quando a ficha estava comparando o piloto com o grid
  // atual, e não com o mundo — a lista global abre filtrada por ela.
  const [globalDriversCategory, setGlobalDriversCategory] = useState(null);
  const [globalTeamsSelection, setGlobalTeamsSelection] = useState(null);
  // Pedido em aberto da tela de recordes de equipes (métrica, recorte, equipe de
  // origem) e a aba de onde o clique partiu, para o "Voltar".
  const [teamRecordsRequest, setTeamRecordsRequest] = useState(null);
  const [teamRecordsOrigin, setTeamRecordsOrigin] = useState("standings");
  const [raceArrivalFeedbackActive, setRaceArrivalFeedbackActive] = useState(false);
  // DEBUG (Ctrl+M): mensagem efêmera do atalho "pular corridas → mercado".
  const [debugSkipFlash, setDebugSkipFlash] = useState("");
  const previousShowRaceBriefingRef = useRef(showRaceBriefing);
  const raceArrivalFeedbackTimeoutRef = useRef(null);
  // Avaliação de leitura das Notícias pós-corrida: enquanto `active`, um timer de 15s
  // conta como "leu"; sair da aba antes disso conta como "pulou". `seasonKey` guarda a
  // temporada da corrida para creditar leitura/pulo na temporada certa.
  const newsReadEvalRef = useRef({ timer: null, active: false, seasonKey: null });
  const shouldStartRaceArrivalFeedback =
    activeTab === "calendar" &&
    showRaceBriefing &&
    !previousShowRaceBriefingRef.current;
  const shouldShowRaceArrivalFeedback =
    raceArrivalFeedbackActive || shouldStartRaceArrivalFeedback;

  useEffect(() => {
    const briefingJustOpened = !previousShowRaceBriefingRef.current && showRaceBriefing;

    if (raceArrivalFeedbackTimeoutRef.current) {
      clearTimeout(raceArrivalFeedbackTimeoutRef.current);
      raceArrivalFeedbackTimeoutRef.current = null;
    }

    if (briefingJustOpened && activeTab === "calendar") {
      setRaceArrivalFeedbackActive(true);
      raceArrivalFeedbackTimeoutRef.current = setTimeout(() => {
        setRaceArrivalFeedbackActive(false);
        raceArrivalFeedbackTimeoutRef.current = null;
      }, RACE_ARRIVAL_FEEDBACK_MS);
    } else if (!showRaceBriefing) {
      setRaceArrivalFeedbackActive(false);
    }

    previousShowRaceBriefingRef.current = showRaceBriefing;

    return () => {
      if (raceArrivalFeedbackTimeoutRef.current) {
        clearTimeout(raceArrivalFeedbackTimeoutRef.current);
        raceArrivalFeedbackTimeoutRef.current = null;
      }
    };
  }, [activeTab, showRaceBriefing]);

  // Poller do gatilho automático do iRacing: a cada poucos segundos pergunta ao
  // backend se o resultado da corrida já foi gravado (jogador terminou/saiu). Se
  // sim, a store abre a tela de resultado sozinha. Roda enquanto a carreira está
  // carregada; o backend é barato quando não há nada a importar.
  useEffect(() => {
    if (!careerId) return undefined;
    const resultHandle = setInterval(() => {
      pollIracingResult?.();
    }, 4000);
    // Gatilho inverso: se o iRacing acabou de fechar, traz nossa janela à frente.
    // Checagem leve (atomic no backend), então roda mais rápido que o import.
    const focusHandle = setInterval(() => {
      invoke("iracing_focus_self_if_closed").catch(() => {});
    }, 1500);
    return () => {
      clearInterval(resultHandle);
      clearInterval(focusHandle);
    };
  }, [careerId, pollIracingResult]);

  // Cancela uma avaliação de leitura em andamento (timer + estado).
  function cancelNewsReadEval() {
    if (newsReadEvalRef.current.timer) {
      clearTimeout(newsReadEvalRef.current.timer);
    }
    newsReadEvalRef.current = { timer: null, active: false, seasonKey: null };
  }

  // Sair da aba Notícias antes dos 15s (com uma avaliação ativa) = "pulou".
  useEffect(() => {
    if (newsReadEvalRef.current.active && activeTab !== "news") {
      recordNewsSkip(careerId, newsReadEvalRef.current.seasonKey);
      cancelNewsReadEval();
    }
  }, [activeTab, careerId]);

  // Limpa o timer ao desmontar.
  useEffect(() => cancelNewsReadEval, []);

  // DEBUG hotkey — Ctrl+M (ou Cmd+M): pula TODAS as corridas pendentes da temporada
  // e cai direto no mercado (mesmo caminho de `skipAllPendingRaces`). Só dispara com
  // uma carreira ativa e nenhuma tela de resultado/mercado por cima, e nunca em cima
  // de um avanço já em curso. Lê o estado fresco do store para evitar closure velha.
  useEffect(() => {
    function handleDebugSkip(event) {
      const isMod = event.ctrlKey || event.metaKey;
      if (!isMod || (event.key !== "m" && event.key !== "M")) return;

      const s = useCareerStore.getState();
      if (!s.isLoaded || !s.careerId) return;
      if (
        s.isAdvancing ||
        s.isSimulating ||
        s.isConvocating ||
        s.isEnteringPreseason ||
        s.showResult ||
        s.showEndOfSeason ||
        s.showPreseason ||
        s.showConvocation
      ) {
        return;
      }

      event.preventDefault();
      setDebugSkipFlash("⏭️ DEBUG: pulando corridas → mercado…");
      Promise.resolve(s.skipAllPendingRaces?.())
        .then(() => setDebugSkipFlash(""))
        .catch((err) => {
          console.error("[debug] Ctrl+M (pular temporada) falhou:", err);
          setDebugSkipFlash("");
        });
    }

    window.addEventListener("keydown", handleDebugSkip);
    return () => window.removeEventListener("keydown", handleDebugSkip);
  }, []);

  // DEBUG hotkey — Ctrl+L (ou Cmd+L): simula tudo MENOS a última corrida da categoria
  // do jogador, deixando o save a um "Avançar calendário" da final da temporada — o
  // atalho para ver a tela de Campeão da Temporada sem correr o ano inteiro. Mesmos
  // portões do Ctrl+M, lendo o estado fresco do store.
  useEffect(() => {
    function handleDebugFinale(event) {
      const isMod = event.ctrlKey || event.metaKey;
      if (!isMod || (event.key !== "l" && event.key !== "L")) return;

      const s = useCareerStore.getState();
      if (!s.isLoaded || !s.careerId) return;
      if (
        s.isAdvancing ||
        s.isSimulating ||
        s.isConvocating ||
        s.isEnteringPreseason ||
        s.showResult ||
        s.showEndOfSeason ||
        s.showPreseason ||
        s.showConvocation
      ) {
        return;
      }

      event.preventDefault();
      setDebugSkipFlash("⏭️ DEBUG: pulando até a última corrida…");
      Promise.resolve(s.debugSkipToSeasonFinale?.())
        .then(() => setDebugSkipFlash(""))
        .catch((err) => {
          console.error("[debug] Ctrl+L (pular até a final) falhou:", err);
          setDebugSkipFlash("");
        });
    }

    window.addEventListener("keydown", handleDebugFinale);
    return () => window.removeEventListener("keydown", handleDebugFinale);
  }, []);

  // DEBUG hotkey — Ctrl+K (ou Cmd+K): abre o pop-up de Campeão da Temporada na hora,
  // com o ano ANTERIOR (temporada fechada, calendário e recordes completos). Serve
  // para trabalhar na tela sem correr nada, então é de propósito o atalho mais
  // permissivo dos três: basta a carreira estar carregada. Reabrir por cima do pop-up
  // já aberto é inofensivo — só recarrega o payload.
  useEffect(() => {
    function handleDebugChampion(event) {
      const isMod = event.ctrlKey || event.metaKey;
      if (!isMod || (event.key !== "k" && event.key !== "K")) return;

      const s = useCareerStore.getState();
      if (!s.isLoaded || !s.careerId) return;

      event.preventDefault();
      Promise.resolve(s.debugShowLastSeasonChampion?.())
        .then((payload) => {
          if (payload) return;
          setDebugSkipFlash("🏆 DEBUG: nenhuma temporada com corridas para mostrar");
          setTimeout(() => setDebugSkipFlash(""), 2500);
        })
        .catch((err) => {
          console.error("[debug] Ctrl+K (tela de campeão) falhou:", err);
        });
    }

    window.addEventListener("keydown", handleDebugChampion);
    return () => window.removeEventListener("keydown", handleDebugChampion);
  }, []);

  // "Continuar" no pós-corrida: decide a aba de destino (Notícias por padrão; Home
  // depois de 3 pulos; sempre Notícias no final de campeonato) e, quando cai em
  // Notícias, arma a medição de leitura. Só corridas recém-terminadas contam.
  function handleDismissResult() {
    // Corrida recém-terminada? Lido ANTES do dismiss, que zera `resultIsFresh`.
    const justRaced = resultIsFresh;

    if (resultIsFresh) {
      const seasonKey = season?.numero ?? season?.ano ?? null;
      const { tab, evaluate } = resolvePostRaceLanding(careerId, seasonKey, lastRaceWasFinale);
      cancelNewsReadEval();
      setActiveTab(tab);
      if (evaluate) {
        newsReadEvalRef.current = {
          active: true,
          seasonKey,
          timer: setTimeout(() => {
            recordNewsRead(careerId, seasonKey);
            newsReadEvalRef.current = { timer: null, active: false, seasonKey: null };
          }, NEWS_READ_MS),
        };
      }
    }

    const dismissed = Promise.resolve(dismissResult());
    if (justRaced) {
      // Fim de campeonato → entra o pop-up de Campeão da Temporada, por cima da aba
      // de destino. Só DEPOIS do recarregamento da carreira: aí os resultados da
      // final já estão gravados e o payload fecha com a classificação.
      //
      // O slot narrativo `FinalDaTemporada` é o sinal principal, mas ele depende da
      // geração do calendário ter reservado uma pista forte para a última etapa —
      // por isso "o jogador não tem mais corrida nesta temporada" vale como rede.
      void dismissed.then((reloaded) => {
        const seasonIsOver = reloaded ? !reloaded.next_race : false;
        if (lastRaceWasFinale || seasonIsOver) {
          void loadSeasonChampionOverlay?.();
        }
      });
    }
  }

  if (!isLoaded) {
    return <Navigate to="/menu" replace />;
  }

  const debugFlashOverlay = debugSkipFlash ? (
    <div className="pointer-events-none fixed bottom-6 left-1/2 z-[300] -translate-x-1/2">
      <div className="rounded-full border border-accent-primary/40 bg-black/80 px-5 py-2 text-sm font-semibold text-accent-primary shadow-[0_0_30px_rgba(0,0,0,0.6)] backdrop-blur-sm">
        {debugSkipFlash}
      </div>
    </div>
  ) : null;

  function renderTab() {
    switch (activeTab) {
      case "global-drivers":
        return (
          <GlobalDriversTab
            selectedDriverId={globalDriversSelectedId}
            initialMetric={globalDriversMetric}
            initialCategory={globalDriversCategory}
            onBack={() => setActiveTab("standings")}
          />
        );
      case "global-teams":
        return (
          <GlobalTeamsTab
            selectedTeamId={globalTeamsSelection?.id ?? globalTeamsSelection}
            selectedTeamCategory={globalTeamsSelection?.categoria ?? globalTeamsSelection?.category ?? null}
            selectedTeamClassName={globalTeamsSelection?.classe ?? globalTeamsSelection?.class_name ?? null}
            onOpenTeamRecords={openTeamRecords}
            onBack={() => setActiveTab("standings")}
          />
        );
      case "team-records":
        return (
          <TeamRecordsTab
            category={teamRecordsRequest?.category ?? null}
            teamClass={teamRecordsRequest?.teamClass ?? null}
            metric={teamRecordsRequest?.metric ?? null}
            highlightTeamId={teamRecordsRequest?.teamId ?? null}
            onBack={() => setActiveTab(teamRecordsOrigin)}
          />
        );
      case "news":
        return <NewsMagazineTab />;
      case "my-team":
        return <MyTeamTab onOpenTeamRecords={openTeamRecords} />;
      case "calendar":
        return (
          <CalendarTabRedesign
            activeTab={activeTab}
            raceArrivalFeedbackActive={shouldShowRaceArrivalFeedback}
          />
        );
      case "standings":
      default:
        return (
          <StandingsTab
            onOpenGlobalDrivers={openGlobalDrivers}
            onOpenGlobalTeams={openGlobalTeams}
            onOpenTeamRecords={openTeamRecords}
          />
        );
    }
  }

  // Entrada única da tela global de pilotos. `metric` só vem dos cards de
  // recorde da ficha; entrar pela lista limpa a métrica de propósito, senão a
  // ordem pedida numa visita anterior sobreviveria à visita seguinte.
  function openGlobalDrivers(driverId, { metric = null, category = null } = {}) {
    setGlobalDriversSelectedId(driverId);
    setGlobalDriversMetric(metric);
    setGlobalDriversCategory(category);
    setActiveTab("global-drivers");
  }

  function openGlobalTeams(team) {
    setGlobalTeamsSelection(typeof team === "string" ? { id: team } : team);
    setActiveTab("global-teams");
  }

  // Destino dos cards de record do dossiê. A tela de recordes não está no menu —
  // ela é a resposta a uma pergunta feita no dossiê —, então guarda de onde veio
  // para o "Voltar" devolver o jogador ao lugar de onde ele clicou, e não a um
  // destino fixo que seria certo para uma aba e errado para as outras três.
  function openTeamRecords({ metric, category, teamClass, teamId } = {}) {
    setTeamRecordsRequest({
      metric: metric ?? null,
      category: category ?? null,
      teamClass: teamClass ?? null,
      teamId: teamId ?? null,
    });
    setTeamRecordsOrigin(activeTab);
    setActiveTab("team-records");
  }

  if (showResult && lastRaceResult) {
    return (
      <MainLayout activeTab={activeTab} onTabChange={setActiveTab} hideHeader>
        <RaceResultViewV2
          result={lastRaceResult}
          evaluation={lastRaceEvaluation}
          telemetry={lastRaceTelemetry}
          maintenance={lastRaceMaintenance}
          repercussion={lastRaceRepercussion}
          onDismiss={handleDismissResult}
        />
        {iracingRepair && (
          <RepairPopup repair={iracingRepair} onClose={dismissIracingRepair} />
        )}
      </MainLayout>
    );
  }

  if (showEndOfSeason && endOfSeasonResult) {
    return (
      <EndOfSeasonView />
    );
  }

  if (showPreseason) {
    return (
      <PreSeasonView />
    );
  }

  if (showConvocation) {
    return (
      <ConvocationView />
    );
  }

  if (showRaceBriefing && !shouldShowRaceArrivalFeedback) {
    return (
      <MainLayout activeTab={activeTab} onTabChange={setActiveTab}>
        {/* Fade suave ao abrir a sala de estratégia (depois do pulso da pista). */}
        <div className="tab-pane-fade">
          <NextRaceTab />
        </div>
        {debugFlashOverlay}
      </MainLayout>
    );
  }

  return (
    <MainLayout activeTab={activeTab} onTabChange={setActiveTab}>
      {/* `key` por aba → remonta com o fade suave a cada troca (ex.: ao avançar
          o calendário, a aba muda para Calendário com transição). */}
      <div key={activeTab} className="tab-pane-fade">
        {renderTab()}
      </div>
      {debugFlashOverlay}
    </MainLayout>
  );
}

function RepairPopup({ repair, onClose }) {
  const { t } = useTranslation();
  const valor = `$${Math.round(repair.repair_cost || 0).toLocaleString("en-US")}`;
  return (
    <div
      className="fixed inset-0 z-[200] flex items-center justify-center bg-black/70 backdrop-blur-sm p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-2xl border border-status-red/30 bg-[#161b22] p-6 shadow-[0_0_40px_rgba(0,0,0,0.6)]"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-3">
          <span className="text-3xl">🔧</span>
          <div>
            <h3 className="text-base font-bold text-text-primary">{t("dashboard.repair.title")}</h3>
            <p className="text-[11px] uppercase tracking-wide text-status-red">
              {t("dashboard.repair.crash", { severity: repair.repair_severity })}
            </p>
          </div>
        </div>
        <p className="mt-4 text-sm leading-relaxed text-text-secondary">
          {repair.repair_message}
        </p>
        <div className="mt-4 flex items-baseline justify-between rounded-xl border border-white/10 bg-white/5 px-4 py-3">
          <span className="text-xs text-text-muted">{t("dashboard.repair.cost")}</span>
          <span className="font-mono text-xl font-bold text-status-red">{valor}</span>
        </div>
        <button
          type="button"
          onClick={onClose}
          className="mt-5 w-full rounded-xl bg-white/10 px-4 py-2.5 text-sm font-semibold text-text-primary transition-glass hover:bg-white/15"
        >
          {t("dashboard.repair.ok")}
        </button>
      </div>
    </div>
  );
}

export default Dashboard;
