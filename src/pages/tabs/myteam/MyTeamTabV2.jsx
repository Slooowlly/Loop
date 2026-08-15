import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import { TeamHistoryDrawer } from "../../../components/team/history";
import CarPanelV2 from "../../../components/team/myteam/v2/CarPanelV2";
import CommandHeaderV2 from "../../../components/team/myteam/v2/CommandHeaderV2";
import GridComparative from "../../../components/team/myteam/v2/GridComparative";
import HorizonStrip from "../../../components/team/myteam/v2/HorizonStrip";
import DriverCard from "../../../components/team/myteam/v2/DriverCard";
import LineupStrip from "../../../components/team/myteam/v2/LineupStrip";
import RoundMoneyFlow from "../../../components/team/myteam/v2/RoundMoneyFlow";
import RoundLedgerChart from "../../../components/team/myteam/v2/RoundLedgerChart";
import { buildDriverRow, garageClimate, resolveHierarchy } from "../../../components/team/myteam/teamMetrics";
import { lineupDossier } from "../../../components/team/myteam/v2/gridMetrics";
import useCareerStore from "../../../stores/useCareerStore";
import i18n from "../../../i18n/index.js";

// Aba Minha Equipe. A v1 que ela substituiu foi removida em 11/08/2026.
//
// A busca de dados é IDÊNTICA à da v1: os mesmos três comandos, nenhum campo novo
// pedido ao backend. O que mudou é a leitura — a tela desce em horizontes (agora →
// esta rodada → esta temporada → o grid) e cada comparação implícita virou gráfico.

// As três seções abaixo do cabeçalho. Empilhado, isto dava três telas de rolagem
// numa tela de 1080.
//
// Só o CABEÇALHO fica sempre visível — caixa, chips e veredito são a âncora, e
// esconder o veredito atrás de um clique devolveria o problema que a v2 existe para
// resolver. Os cards de rodada e temporada moram na seção Dinheiro: fora dela eram
// dois blocos de finanças pairando sobre uma tela de pilotos e de carro.
//
// A Equipe abre primeiro, e por isso é também a primeira pílula: quem entra em
// "Minha Equipe" quer ver a dupla e o carro; o caixa já está no cabeçalho, que
// fica sempre visível. Deixar a seção padrão fora da primeira posição acenderia
// a segunda pílula na abertura, o que se lê como estado errado.
const SECTIONS = ["team", "money", "grid"];

function MyTeamTabV2({ onOpenTeamRecords = null }) {
  const { t } = useTranslation();
  const [section, setSection] = useState(SECTIONS[0]);
  const careerId = useCareerStore((state) => state.careerId);
  const player = useCareerStore((state) => state.player);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const season = useCareerStore((state) => state.season);
  const [drivers, setDrivers] = useState([]);
  const [teams, setTeams] = useState([]);
  const [carParts, setCarParts] = useState([]);
  const [financeReport, setFinanceReport] = useState(null);
  const [selectedHistoryTeam, setSelectedHistoryTeam] = useState(null);
  const [activeHistoryTab, setActiveHistoryTab] = useState("records");
  const [error, setError] = useState("");

  useEffect(() => {
    let mounted = true;

    async function load() {
      if (!careerId || !playerTeam?.categoria || !playerTeam?.id) return;
      try {
        setError("");
        const [loadedDrivers, loadedTeams, loadedFinance, loadedCars] = await Promise.all([
          invoke("get_drivers_by_category", { careerId, category: playerTeam.categoria }),
          invoke("get_teams_standings", { careerId, category: playerTeam.categoria }),
          invoke("get_team_finance_report", {
            careerId,
            category: playerTeam.categoria,
            teamId: playerTeam.id,
          }),
          // As 11 peças de cada equipe — o detalhe que `car_level` resume numa média.
          invoke("get_teams_car_parts", { careerId, category: playerTeam.categoria }),
        ]);
        if (mounted) {
          setDrivers(Array.isArray(loadedDrivers) ? loadedDrivers : []);
          setTeams(Array.isArray(loadedTeams) ? loadedTeams : []);
          setFinanceReport(loadedFinance ?? null);
          setCarParts(Array.isArray(loadedCars) ? loadedCars : []);
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

  const team = playerTeam;
  const gridTeams = teams;
  const report = financeReport;
  const seasonInfo = season;
  const categoryDrivers = drivers;
  const gridCars = carParts;

  // A dupla sai da HIERARQUIA da garagem, não da ordem dos assentos: depois de uma
  // inversão no meio da temporada, quem ocupa o slot 1 pode já não ser o N1.
  const hierarchy = resolveHierarchy(team);
  const pilotoN1 = categoryDrivers.find((driver) => driver.id === hierarchy.n1Id);
  const pilotoN2 = categoryDrivers.find((driver) => driver.id === hierarchy.n2Id);
  const standing = gridTeams.find((row) => row.id === team?.id);
  // `buildDriverRow` é do módulo compartilhado e devolve só nome/bandeira/salário. O
  // objeto cru do piloto viaja junto para o dossiê — é dele que saem skill, mídia,
  // idade e a campanha da temporada.
  // Tempo de casa: o payload da classificação conta por ASSENTO (`piloto_1`/`piloto_2`),
  // e a dupla é lida por hierarquia — depois de uma inversão o N1 é o ocupante do slot 2.
  // Sem casar pelos slots da hierarquia, os dois números trocariam de dono.
  const driverRows = [
    {
      ...buildDriverRow("N1", pilotoN1, team, player?.id, hierarchy.n1Slot),
      driver: pilotoN1 ?? null,
      tenureSeasons: standing?.[`piloto_${hierarchy.n1Slot}_tenure_seasons`] ?? null,
    },
    {
      ...buildDriverRow("N2", pilotoN2, team, player?.id, hierarchy.n2Slot),
      driver: pilotoN2 ?? null,
      tenureSeasons: standing?.[`piloto_${hierarchy.n2Slot}_tenure_seasons`] ?? null,
    },
  ];
  const dossier = lineupDossier({ drivers: categoryDrivers, rows: driverRows });
  const climate = garageClimate(team);
  const payroll = driverRows.reduce((sum, driver) => sum + driver.salary, 0);

  const roundNet = team?.last_round_net ?? 0;
  // Net da temporada SEM o prêmio já pago: a linha de encerramento entra em
  // `season.net`, e somá-la à expectativa contaria o prêmio duas vezes.
  const seasonNetToDate = (report?.season?.net ?? 0) - (report?.season?.constructor_prize_income ?? 0);
  const expectedPrize = report?.expected_constructor_prize ?? 0;
  const currentPosition = report?.current_position ?? 0;
  const hasProjection = currentPosition > 0 && expectedPrize > 0;
  const projectedAnnual = seasonNetToDate + expectedPrize;
  // A cor da equipe é o realce da aba ativa. Sem equipe carregada ainda, cai no azul
  // do tema em vez de sumir — `inset 0 -2px 0 undefined` não desenha nada.
  const sectionAccent = team?.cor_primaria || "#58a6ff";

  return (
    <div className="space-y-3">
      <CommandHeaderV2
        team={team}
        teams={gridTeams}
        standing={standing}
        gridSize={report?.grid_size ?? gridTeams.length}
        roundNet={roundNet}
        projectedAnnual={projectedAnnual}
        hasProjection={hasProjection}
        payroll={payroll}
        salaryCeiling={team?.salary_ceiling ?? 0}
      />

      {error ? (
        <div className="rounded-lg border border-status-red/30 bg-status-red/10 px-4 py-3 text-sm text-status-red">
          {error}
        </div>
      ) : null}

      {/* As seções são abas de filete, não pílulas soltas. A pílula flutuava sobre o
          fundo e não dizia a que ela pertence; a aba encosta na folha seguinte e a
          linha de base costura as três ao conteúdo. O realce da ativa é a cor da
          equipe, o mesmo traço da borda esquerda da faixa de comando. */}
      <div
        className="flex flex-wrap items-stretch border-b border-white/[0.08]"
        role="tablist"
        data-testid="my-team-v2-sections"
      >
        {SECTIONS.map((id) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={section === id}
            onClick={() => setSection(id)}
            style={section === id ? { boxShadow: `inset 0 -2px 0 ${sectionAccent}` } : undefined}
            className={`border-r border-white/[0.08] px-5 py-2 text-[10px] font-semibold uppercase tracking-[0.2em] transition-glass ${
              section === id ? "bg-white/[0.04] text-text-primary" : "text-text-muted hover:text-text-secondary"
            }`}
          >
            {t(`myTeamTabV2.sections.${id}`)}
          </button>
        ))}
      </div>

      {section === "money" ? (
        <>
          <HorizonStrip
            roundIncome={team?.last_round_income ?? 0}
            roundExpenses={team?.last_round_expenses ?? 0}
            roundNet={roundNet}
            seasonNetToDate={seasonNetToDate}
            expectedPrize={expectedPrize}
            projectedAnnual={projectedAnnual}
            hasProjection={hasProjection}
          />
          <RoundMoneyFlow latest={report?.latest} teamColor={team?.cor_primaria} />
          <RoundLedgerChart report={report} team={team} season={seasonInfo} />
        </>
      ) : null}

      {section === "team" ? (
        <>
          <div className="grid gap-3 lg:grid-cols-2">
            {dossier.drivers.map((driver, index) => (
              <DriverCard
                key={driver.role}
                driver={driver}
                averages={dossier.averages}
                hasGrid={dossier.hasGrid}
                poolSize={dossier.poolSize}
                payroll={payroll}
                // A mídia do companheiro é o que diz quem é o rosto da equipe: a
                // presença pública pesa 70% no mais midiático e 30% no outro.
                teammateMedia={dossier.drivers[index === 0 ? 1 : 0]?.hasDetail
                  ? dossier.drivers[index === 0 ? 1 : 0].midia
                  : null}
              />
            ))}
          </div>
          {/* A faixa da dupla fica ENTRE os pilotos e o carro porque é sobre eles dois
              juntos — presença é a média das duas mídias, clima é a relação entre os
              dois. Era um cartão de "garagem" que juntava isso com folha salarial. */}
          <LineupStrip
            presence={team?.presenca_publica ?? 0}
            climate={climate}
            sponsorshipIncome={report?.latest?.sponsorship_income ?? 0}
            gateIncome={report?.latest?.gate_income ?? 0}
          />
          <CarPanelV2 team={team} teams={gridTeams} />
        </>
      ) : null}

      {section === "grid" ? (
        <GridComparative
          teams={gridTeams}
          cars={gridCars}
          playerTeam={team}
          historyTeamId={selectedHistoryTeam?.id}
          onTeamHistoryOpen={(team) => {
            setSelectedHistoryTeam(team);
            setActiveHistoryTab("records");
          }}
        />
      ) : null}

      {selectedHistoryTeam ? (
        <TeamHistoryDrawer
          careerId={careerId}
          team={selectedHistoryTeam}
          teams={gridTeams}
          playerTeam={team}
          activeCategory={playerTeam?.categoria}
          activeTab={activeHistoryTab}
          onTabChange={setActiveHistoryTab}
          onSelectTeam={setSelectedHistoryTeam}
          onOpenRecordsRanking={onOpenTeamRecords}
          onClose={() => setSelectedHistoryTeam(null)}
        />
      ) : null}
    </div>
  );
}

export default MyTeamTabV2;
