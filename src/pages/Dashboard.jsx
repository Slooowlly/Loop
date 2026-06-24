import { useEffect, useRef, useState } from "react";
import { Navigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";

import MainLayout from "../components/layout/MainLayout";
import RaceResultView from "../components/race/RaceResultView";
import ConvocationView from "../components/season/ConvocationView";
import EndOfSeasonView from "../components/season/EndOfSeasonView";
import PreSeasonView from "../components/season/PreSeasonView";
import useCareerStore from "../stores/useCareerStore";
import CalendarTab from "./tabs/CalendarTab";
import MyTeamTab from "./tabs/MyTeamTab";
import NewsMagazineTab from "./tabs/NewsMagazineTab";
import NextRaceTab from "./tabs/NextRaceTab";
import StandingsTab from "./tabs/StandingsTab";
import GlobalDriversTab from "./tabs/GlobalDriversTab";
import GlobalTeamsTab from "./tabs/GlobalTeamsTab";

// Ao chegar o dia da corrida: a célula da PISTA pulsa por ~1s no calendário,
// anunciando que vai abrir, e só então entra a sala de estratégia (com fade).
const RACE_ARRIVAL_FEEDBACK_MS = 1000;

function Dashboard() {
  const isLoaded = useCareerStore((state) => state.isLoaded);
  const showResult = useCareerStore((state) => state.showResult);
  const lastRaceResult = useCareerStore((state) => state.lastRaceResult);
  const lastRaceEvaluation = useCareerStore((state) => state.lastRaceEvaluation);
  const lastRaceTelemetry = useCareerStore((state) => state.lastRaceTelemetry);
  const dismissResult = useCareerStore((state) => state.dismissResult);
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
  const [globalTeamsSelection, setGlobalTeamsSelection] = useState(null);
  const [raceArrivalFeedbackActive, setRaceArrivalFeedbackActive] = useState(false);
  const previousShowRaceBriefingRef = useRef(showRaceBriefing);
  const raceArrivalFeedbackTimeoutRef = useRef(null);
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

  if (!isLoaded) {
    return <Navigate to="/menu" replace />;
  }

  function renderTab() {
    switch (activeTab) {
      case "global-drivers":
        return (
          <GlobalDriversTab
            selectedDriverId={globalDriversSelectedId}
            onBack={() => setActiveTab("standings")}
          />
        );
      case "global-teams":
        return (
          <GlobalTeamsTab
            selectedTeamId={globalTeamsSelection?.id ?? globalTeamsSelection}
            selectedTeamCategory={globalTeamsSelection?.categoria ?? globalTeamsSelection?.category ?? null}
            selectedTeamClassName={globalTeamsSelection?.classe ?? globalTeamsSelection?.class_name ?? null}
            onBack={() => setActiveTab("standings")}
          />
        );
      case "news":
        return <NewsMagazineTab />;
      case "my-team":
        return <MyTeamTab />;
      case "calendar":
        return (
          <CalendarTab
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
          />
        );
    }
  }

  function openGlobalDrivers(driverId) {
    setGlobalDriversSelectedId(driverId);
    setActiveTab("global-drivers");
  }

  function openGlobalTeams(team) {
    setGlobalTeamsSelection(typeof team === "string" ? { id: team } : team);
    setActiveTab("global-teams");
  }

  if (showResult && lastRaceResult) {
    return (
      <MainLayout activeTab={activeTab} onTabChange={setActiveTab} hideHeader>
        <RaceResultView
          result={lastRaceResult}
          evaluation={lastRaceEvaluation}
          telemetry={lastRaceTelemetry}
          onDismiss={dismissResult}
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
    </MainLayout>
  );
}

function RepairPopup({ repair, onClose }) {
  const valor = `R$ ${Math.round(repair.repair_cost || 0).toLocaleString("pt-BR")}`;
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
            <h3 className="text-base font-bold text-text-primary">Conserto do carro</h3>
            <p className="text-[11px] uppercase tracking-wide text-status-red">
              Batida {repair.repair_severity}
            </p>
          </div>
        </div>
        <p className="mt-4 text-sm leading-relaxed text-text-secondary">
          {repair.repair_message}
        </p>
        <div className="mt-4 flex items-baseline justify-between rounded-xl border border-white/10 bg-white/5 px-4 py-3">
          <span className="text-xs text-text-muted">Custo do reparo</span>
          <span className="font-mono text-xl font-bold text-status-red">{valor}</span>
        </div>
        <button
          type="button"
          onClick={onClose}
          className="mt-5 w-full rounded-xl bg-white/10 px-4 py-2.5 text-sm font-semibold text-text-primary transition-glass hover:bg-white/15"
        >
          OK, entendi
        </button>
      </div>
    </div>
  );
}

export default Dashboard;
