import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { TeamHistoryDrawer } from "../../components/team/history";
import CommandHeader from "../../components/team/myteam/CommandHeader";
import CostChart from "../../components/team/myteam/CostChart";
import DriverPanel from "../../components/team/myteam/DriverPanel";
import FinanceDossier from "../../components/team/myteam/FinanceDossier";
import GaragePanel from "../../components/team/myteam/GaragePanel";
import RankingTable from "../../components/team/myteam/RankingTable";
import TechPanel from "../../components/team/myteam/TechPanel";
import { buildDriverRow, garageClimate, resolveHierarchy } from "../../components/team/myteam/teamMetrics";
import useCareerStore from "../../stores/useCareerStore";
import i18n from "../../i18n/index.js";

function MyTeamTab() {
  const careerId = useCareerStore((state) => state.careerId);
  const player = useCareerStore((state) => state.player);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const [drivers, setDrivers] = useState([]);
  const [teams, setTeams] = useState([]);
  const [financeReport, setFinanceReport] = useState(null);
  const [activeAxis, setActiveAxis] = useState("development");
  const [selectedHistoryTeam, setSelectedHistoryTeam] = useState(null);
  const [activeHistoryTab, setActiveHistoryTab] = useState("records");
  const [error, setError] = useState("");

  useEffect(() => {
    let mounted = true;

    async function load() {
      if (!careerId || !playerTeam?.categoria || !playerTeam?.id) return;
      try {
        setError("");
        const [loadedDrivers, loadedTeams, loadedFinance] = await Promise.all([
          invoke("get_drivers_by_category", { careerId, category: playerTeam.categoria }),
          invoke("get_teams_standings", { careerId, category: playerTeam.categoria }),
          invoke("get_team_finance_report", {
            careerId,
            category: playerTeam.categoria,
            teamId: playerTeam.id,
          }),
        ]);
        if (mounted) {
          setDrivers(Array.isArray(loadedDrivers) ? loadedDrivers : []);
          setTeams(Array.isArray(loadedTeams) ? loadedTeams : []);
          setFinanceReport(loadedFinance ?? null);
        }
      } catch (invokeError) {
        if (mounted) {
          setError(typeof invokeError === "string" ? invokeError : i18n.t("myTeamTab.errors.load"));
        }
      }
    }

    load();
    return () => {
      mounted = false;
    };
  }, [careerId, playerTeam?.categoria, playerTeam?.id]);

  // A dupla sai da HIERARQUIA da garagem, não da ordem dos assentos: depois de uma
  // inversão no meio da temporada, quem ocupa o slot 1 pode já não ser o N1.
  const hierarchy = resolveHierarchy(playerTeam);
  const pilotoN1 = drivers.find((driver) => driver.id === hierarchy.n1Id);
  const pilotoN2 = drivers.find((driver) => driver.id === hierarchy.n2Id);
  const standing = teams.find((team) => team.id === playerTeam?.id);
  const driverRows = [
    buildDriverRow("N1", pilotoN1, playerTeam, player?.id, hierarchy.n1Slot),
    buildDriverRow("N2", pilotoN2, playerTeam, player?.id, hierarchy.n2Slot),
  ];
  const climate = garageClimate(playerTeam);

  return (
    <div className="space-y-5">
      <CommandHeader team={playerTeam} standing={standing} />

      {error ? (
        <div className="rounded-2xl border border-status-red/30 bg-status-red/10 px-4 py-3 text-sm text-status-red">
          {error}
        </div>
      ) : null}

      <div className="grid gap-5 xl:grid-cols-[0.72fr_1.28fr]">
        <div className="space-y-5" data-testid="my-team-side-rail">
          <DriverPanel drivers={driverRows} salaryCeiling={playerTeam?.salary_ceiling ?? 0} />
          {hierarchy.hasData ? (
            <GaragePanel
              climate={climate}
              n1Name={driverRows[0].name}
              n2Name={driverRows[1].name}
              inverted={hierarchy.inverted}
            />
          ) : null}
          <TechPanel team={playerTeam} activeAxis={activeAxis} setActiveAxis={setActiveAxis} />
          <CostChart report={financeReport} />
        </div>
        <FinanceDossier team={playerTeam} drivers={driverRows} report={financeReport} />
      </div>

      <RankingTable
        teams={teams}
        playerTeam={playerTeam}
        historyTeamId={selectedHistoryTeam?.id}
        onTeamHistoryOpen={(team) => {
          setSelectedHistoryTeam(team);
          setActiveHistoryTab("records");
        }}
      />

      {selectedHistoryTeam ? (
        <TeamHistoryDrawer
          careerId={careerId}
          team={selectedHistoryTeam}
          teams={teams}
          playerTeam={playerTeam}
          activeCategory={playerTeam?.categoria}
          activeTab={activeHistoryTab}
          onTabChange={setActiveHistoryTab}
          onSelectTeam={setSelectedHistoryTeam}
          onClose={() => setSelectedHistoryTeam(null)}
        />
      ) : null}
    </div>
  );
}

export default MyTeamTab;
