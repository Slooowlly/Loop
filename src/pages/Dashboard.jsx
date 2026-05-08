import { useEffect, useRef, useState } from "react";
import { Navigate } from "react-router-dom";

import MainLayout from "../components/layout/MainLayout";
import RaceResultView from "../components/race/RaceResultView";
import ConvocationView from "../components/season/ConvocationView";
import EndOfSeasonView from "../components/season/EndOfSeasonView";
import PreSeasonView from "../components/season/PreSeasonView";
import useCareerStore from "../stores/useCareerStore";
import CalendarTab from "./tabs/CalendarTab";
import MyTeamTab from "./tabs/MyTeamTab";
import NewsTab from "./tabs/NewsTab";
import NextRaceTab from "./tabs/NextRaceTab";
import StandingsTab from "./tabs/StandingsTab";
import GlobalDriversTab from "./tabs/GlobalDriversTab";
import GlobalTeamsTab from "./tabs/GlobalTeamsTab";

const RACE_ARRIVAL_FEEDBACK_MS = 280;

function Dashboard() {
  const isLoaded = useCareerStore((state) => state.isLoaded);
  const showResult = useCareerStore((state) => state.showResult);
  const lastRaceResult = useCareerStore((state) => state.lastRaceResult);
  const dismissResult = useCareerStore((state) => state.dismissResult);
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
        return <NewsTab />;
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
        <RaceResultView result={lastRaceResult} onDismiss={dismissResult} />
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
        <NextRaceTab />
      </MainLayout>
    );
  }

  return (
    <MainLayout activeTab={activeTab} onTabChange={setActiveTab}>
      {renderTab()}
    </MainLayout>
  );
}

export default Dashboard;
