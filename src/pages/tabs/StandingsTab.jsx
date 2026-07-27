import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import i18n from "../../i18n/index.js";
import DriverDetailModal from "../../components/driver/DriverDetailModal";
import DriverStandingsTable from "../../components/standings/DriverStandingsTable";
import SeriesNavigator from "../../components/standings/SeriesNavigator";
import TeamStandingsPanel from "../../components/standings/TeamStandingsPanel";
import { SpecialPendingNotice } from "../../components/standings/SpecialStandingNotices";
import { buildPositionDeltaMap } from "../../components/standings/standingsMetrics";
import {
  CATEGORY_SERIES,
  SPECIAL_STANDING_GROUPS,
  buildSpecialStandingSections,
  getForcedSpecialStandingCategory,
  hasSpecialStandingResults,
  orderSpecialGroupsForClass,
  resolveInitialNav,
} from "../../components/standings/standingsLadder";
import GlassCard from "../../components/ui/GlassCard";
import useDeferredLoading from "../../hooks/useLoading";
import useCareerStore from "../../stores/useCareerStore";
import { isLegacySeasonPhase } from "../../utils/seasonPhases";
import { TeamHistoryDrawer } from "../../components/team/TeamHistoryDrawer";

const DRIVER_CLICK_DELAY_MS = 220;
const TEAM_CLICK_DELAY_MS = 220;

function StandingsTab({ onOpenGlobalDrivers = null, onOpenGlobalTeams = null }) {
  const { t } = useTranslation();
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const season = useCareerStore((state) => state.season);
  const acceptedSpecialOffer = useCareerStore((state) => state.acceptedSpecialOffer);
  const setHomeCategory = useCareerStore((state) => state.setHomeCategory);
  const openSavedRaceScreen = useCareerStore((state) => state.openSavedRaceScreen);
  const forcedSpecialCategory = getForcedSpecialStandingCategory(
    season?.fase,
    playerTeam?.categoria,
    acceptedSpecialOffer,
  );
  const initialNav = resolveInitialNav(
    season?.fase,
    playerTeam?.categoria,
    playerTeam?.classe,
    acceptedSpecialOffer,
  );
  const [seriesId, setSeriesId] = useState(initialNav.seriesId);
  const [viewCategory, setViewCategory] = useState(initialNav.viewCategory);
  const [driverStandings, setDriverStandings] = useState([]);
  const [teamStandings, setTeamStandings] = useState([]);
  const [previousChampionId, setPreviousChampionId] = useState(null);
  const [selectedDriverId, setSelectedDriverId] = useState(null);
  const [hoveredDriverId, setHoveredDriverId] = useState(null);
  const [selectedHistoryTeam, setSelectedHistoryTeam] = useState(null);
  const [activeHistoryTab, setActiveHistoryTab] = useState("records");
  const driverClickTimeoutRef = useRef(null);
  const teamClickTimeoutRef = useRef(null);

  const currentSeries =
    CATEGORY_SERIES.find((serie) => serie.id === seriesId) ?? CATEGORY_SERIES[0];
  const tierIndex = Math.max(0, currentSeries.categories.indexOf(viewCategory));
  const navLocked = Boolean(forcedSpecialCategory);
  const hasTierAbove = !navLocked && tierIndex < currentSeries.categories.length - 1;
  const hasTierBelow = !navLocked && tierIndex > 0;

  function setNav(nextSeriesId, nextCategory) {
    setSeriesId(nextSeriesId);
    setViewCategory(nextCategory);
  }
  // Trocar de série preserva a "distância do topo": as multiclasse (production/
  // endurance) alinham no topo, então trocar de série estando nelas mantém você
  // na multiclasse — só muda qual classe aparece primeiro. Séries mais curtas
  // clampam (ex.: Mazda Rookie → BMW cai no BMW M2, o mais baixo que a BMW tem).
  function switchSeries(nextSeriesIndex) {
    if (navLocked) return;
    const next = CATEGORY_SERIES[nextSeriesIndex];
    const depthFromTop = currentSeries.categories.length - 1 - tierIndex;
    const clampedDepth = Math.min(depthFromTop, next.categories.length - 1);
    const nextTier = next.categories.length - 1 - clampedDepth;
    setNav(next.id, next.categories[nextTier]);
  }
  function goUpTier() {
    if (hasTierAbove) setViewCategory(currentSeries.categories[tierIndex + 1]);
  }
  function goDownTier() {
    if (hasTierBelow) setViewCategory(currentSeries.categories[tierIndex - 1]);
  }

  const activeDriverId = hoveredDriverId ?? selectedDriverId;
  const activeDriver = driverStandings.find((d) => d.id === activeDriverId) ?? null;
  const selectedTeamId = activeDriver?.equipe_id ?? null;
  const selectedTeamColor = activeDriver?.equipe_cor ?? null;
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  // Só mostra o placeholder de carregamento se o fetch demorar de verdade —
  // evita o "flash" de "Carregando..." ao trocar de aba (fetch local é rápido).
  const showLoadingUI = useDeferredLoading(loading);

  useEffect(() => {
    if (!forcedSpecialCategory) return;
    const nav = resolveInitialNav(
      season?.fase,
      playerTeam?.categoria,
      playerTeam?.classe,
      acceptedSpecialOffer,
    );
    if (viewCategory !== nav.viewCategory || seriesId !== nav.seriesId) {
      setNav(nav.seriesId, nav.viewCategory);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [forcedSpecialCategory, viewCategory, seriesId]);

  // Espelha a categoria em exibição para o store, para que o banner cinematográfico
  // do topo (Header) acompanhe a troca de série/tier e mostre a próxima corrida da
  // categoria certa. useLayoutEffect roda antes do paint → sem "flash" de banner na
  // categoria errada ao montar/trocar. Ao desmontar (trocar de aba, pós-corrida etc.)
  // zera para o banner voltar à categoria do jogador.
  useLayoutEffect(() => {
    setHomeCategory(viewCategory);
  }, [viewCategory, setHomeCategory]);
  useEffect(() => () => setHomeCategory(null), [setHomeCategory]);

  useEffect(() => () => {
    clearDriverClickTimeout();
    clearTeamClickTimeout();
  }, []);

  function clearDriverClickTimeout() {
    if (driverClickTimeoutRef.current) {
      clearTimeout(driverClickTimeoutRef.current);
      driverClickTimeoutRef.current = null;
    }
  }

  function openDriverDetail(driverId) {
    setSelectedDriverId((prev) => (prev === driverId ? null : driverId));
  }

  function handleDriverClick(driverId) {
    clearDriverClickTimeout();
    driverClickTimeoutRef.current = setTimeout(() => {
      openDriverDetail(driverId);
      driverClickTimeoutRef.current = null;
    }, DRIVER_CLICK_DELAY_MS);
  }

  function handleDriverDoubleClick(driverId) {
    clearDriverClickTimeout();
    setSelectedDriverId(null);
    onOpenGlobalDrivers?.(driverId);
  }

  // Enter/Espaço na linha: abre a ficha na hora (sem esperar o duplo clique).
  function handleDriverActivate(driverId) {
    clearDriverClickTimeout();
    openDriverDetail(driverId);
  }

  function clearTeamClickTimeout() {
    if (teamClickTimeoutRef.current) {
      clearTimeout(teamClickTimeoutRef.current);
      teamClickTimeoutRef.current = null;
    }
  }

  function openTeamDossier(team) {
    setSelectedHistoryTeam(team);
    setActiveHistoryTab("records");
  }

  function handleTeamClick(team) {
    clearTeamClickTimeout();
    teamClickTimeoutRef.current = setTimeout(() => {
      openTeamDossier(team);
      teamClickTimeoutRef.current = null;
    }, TEAM_CLICK_DELAY_MS);
  }

  function handleTeamDoubleClick(team) {
    clearTeamClickTimeout();
    setSelectedHistoryTeam(null);
    onOpenGlobalTeams?.({
      ...team,
      categoria: team.categoria ?? viewCategory,
      classe: team.classe ?? team.class_name ?? null,
    });
  }

  useEffect(() => {
    let mounted = true;

    async function fetchStandings() {
      if (!careerId || !viewCategory) {
        setLoading(false);
        return;
      }

      setLoading(true);
      setError("");

      try {
        const [drivers, teams, previousChampions] = await Promise.all([
          invoke("get_drivers_by_category", {
            careerId,
            category: viewCategory,
          }),
          invoke("get_teams_standings", {
            careerId,
            category: viewCategory,
          }),
          invoke("get_previous_champions", {
            careerId,
            category: viewCategory,
          }),
        ]);

        if (!mounted) return;

        setDriverStandings(drivers);
        setTeamStandings(teams);
        setPreviousChampionId(previousChampions.driver_champion_id ?? null);
      } catch (invokeError) {
        if (!mounted) return;

        setError(
          typeof invokeError === "string"
            ? invokeError
            : i18n.t("standings.loadError"),
        );
      } finally {
        if (mounted) {
          setLoading(false);
        }
      }
    }

    fetchStandings();

    return () => {
      mounted = false;
    };
  }, [careerId, viewCategory, season?.ano, season?.rodada_atual, season?.fase]);

  const totalRodadas = driverStandings.length > 0
    ? Math.max(...driverStandings.map((d) => (d.results ?? []).length))
    : (season?.total_rodadas || 0);
  const completedRounds = viewCategory === playerTeam?.categoria
    ? Math.max(0, (season?.rodada_atual || 1) - 1)
    : totalRodadas;
  const positionDeltaMap = useMemo(
    () => buildPositionDeltaMap(driverStandings, completedRounds),
    [driverStandings, completedRounds],
  );
  // A classe da linha atual (Mazda→mazda, GT3→gt3, LMP2→lmp2) aparece primeiro
  // dentro da categoria multiclasse; as demais seguem na ordem padrão.
  const specialClassGroups = useMemo(
    () => orderSpecialGroupsForClass(SPECIAL_STANDING_GROUPS[viewCategory] ?? null, currentSeries.classId),
    [viewCategory, currentSeries.classId],
  );
  const isLegacyPhase = isLegacySeasonPhase(season?.fase);
  const driverStandingSections = useMemo(
    () => buildSpecialStandingSections(driverStandings, specialClassGroups),
    [driverStandings, specialClassGroups],
  );
  const showSpecialPendingNotice =
    isLegacyPhase
    && specialClassGroups != null
    && !hasSpecialStandingResults(driverStandings, teamStandings);

  useEffect(() => {
    if (!selectedHistoryTeam?.id) return;
    if (!teamStandings.some((team) => team.id === selectedHistoryTeam.id)) {
      setSelectedHistoryTeam(null);
    }
  }, [selectedHistoryTeam?.id, teamStandings]);


  if (loading) {
    if (!showLoadingUI) return null;
    return (
      <GlassCard hover={false} className="rounded-[28px] p-10">
        <p className="text-sm uppercase tracking-[0.22em] text-accent-primary">Dashboard</p>
        <h2 className="mt-3 text-3xl font-semibold text-text-primary">{t("standings.loading")}</h2>
        <p className="mt-3 text-sm text-text-secondary">
          Buscando pilotos, construtores e resultados da categoria atual.
        </p>
      </GlassCard>
    );
  }

  if (error) {
    return (
      <GlassCard hover={false} className="rounded-[28px] border border-status-red/30 p-10">
        <p className="text-sm font-semibold text-status-red">{error}</p>
      </GlassCard>
    );
  }

  return (
    <>
      <div className="grid gap-5 xl:grid-cols-[1.6fr_0.95fr]">
        <GlassCard hover={false} className="overflow-hidden rounded-[28px]">
          <SeriesNavigator
            currentSeries={currentSeries}
            viewCategory={viewCategory}
            navLocked={navLocked}
            hasTierAbove={hasTierAbove}
            hasTierBelow={hasTierBelow}
            driverCount={driverStandings.length}
            onSelectSeries={switchSeries}
            onTierUp={goUpTier}
            onTierDown={goDownTier}
          />

          {showSpecialPendingNotice ? (
            <SpecialPendingNotice category={viewCategory} phase={season?.fase} />
          ) : (
            <DriverStandingsTable
              sections={driverStandingSections}
              specialClassGroups={specialClassGroups}
              totalRodadas={totalRodadas}
              completedRounds={completedRounds}
              currentRound={season?.rodada_atual}
              positionDeltaMap={positionDeltaMap}
              previousChampionId={previousChampionId}
              selectedTeamId={selectedTeamId}
              selectedTeamColor={selectedTeamColor}
              onDriverHover={setHoveredDriverId}
              onDriverClick={handleDriverClick}
              onDriverDoubleClick={handleDriverDoubleClick}
              onDriverActivate={handleDriverActivate}
              onReviewRace={(rodada) => openSavedRaceScreen?.(viewCategory, rodada)}
            />
          )}
        </GlassCard>

        <TeamStandingsPanel
          teamStandings={teamStandings}
          viewCategory={viewCategory}
          specialClassGroups={specialClassGroups}
          showSpecialPendingNotice={showSpecialPendingNotice}
          selectedHistoryTeamId={selectedHistoryTeam?.id ?? null}
          onTeamDossierOpen={handleTeamClick}
          onTeamGlobalHistoryOpen={handleTeamDoubleClick}
        />
      </div>

      {selectedDriverId ? (
        <DriverDetailModal
          driverId={selectedDriverId}
          driverIds={driverStandings.map((driver) => driver.id)}
          onSelectDriver={setSelectedDriverId}
          onClose={() => setSelectedDriverId(null)}
        />
      ) : null}

      {selectedHistoryTeam ? (
        <TeamHistoryDrawer
          careerId={careerId}
          team={selectedHistoryTeam}
          teams={teamStandings}
          playerTeam={playerTeam}
          activeCategory={viewCategory}
          placement="left"
          activeTab={activeHistoryTab}
          onTabChange={setActiveHistoryTab}
          onSelectTeam={setSelectedHistoryTeam}
          onClose={() => setSelectedHistoryTeam(null)}
        />
      ) : null}
    </>
  );
}

export default StandingsTab;
