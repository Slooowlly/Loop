import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";

import DriverDetailModal from "../../components/driver/DriverDetailModal";
import ResultBadge from "../../components/standings/ResultBadge";
import TrophyBadge from "../../components/standings/TrophyBadge";
import TeamLogoMarkShared from "../../components/team/TeamLogoMark";
import FlagIcon from "../../components/ui/FlagIcon";
import GlassCard from "../../components/ui/GlassCard";
import useDeferredLoading from "../../hooks/useLoading";
import useCareerStore from "../../stores/useCareerStore";
import { categoryLabel } from "../../utils/formatters";
import { isLegacySeasonPhase } from "../../utils/seasonPhases";
import { TeamHistoryDrawer } from "./MyTeamTab";

const ALL_CATEGORIES = [
  "mazda_rookie",
  "toyota_rookie",
  "mazda_amador",
  "toyota_amador",
  "bmw_m2",
  "production_challenger",
  "gt4",
  "gt3",
  "endurance",
];

// Escada apresentada como duas dimensões: a "série" (marca/linha de carro) que o
// jogador troca por um dropdown agrupado, e o "tier" dentro dela que sobe/desce
// com as setas ▲▼. As categorias multiclasse (production/endurance) NÃO são
// séries próprias: elas ficam no TOPO de várias séries ao mesmo tempo. Subindo a
// Mazda (rookie → championship) você "cai" na Production; a mesma Production também
// é o topo de Toyota e BMW. Endurance é o topo de GT4, GT3 e LMP2. `classId` diz
// qual classe da categoria multiclasse aparece PRIMEIRO quando você chega por
// aquela linha (ex.: Endurance pela linha GT3 lista GT3 antes de LMP2/GT4).
// `access` divide o conteúdo por licença: as linhas de entrada (Mazda/Toyota/BMW +
// Production) são "pro"; as de topo (GT4/GT3/LMP2 + Endurance) são "elite". O menu
// desenha uma separação física entre os dois grupos.
const CATEGORY_SERIES = [
  { id: "mazda", label: "Mazda", classId: "mazda", access: "pro", categories: ["mazda_rookie", "mazda_amador", "production_challenger"] },
  { id: "toyota", label: "Toyota", classId: "toyota", access: "pro", categories: ["toyota_rookie", "toyota_amador", "production_challenger"] },
  { id: "bmw", label: "BMW", classId: "bmw", access: "pro", categories: ["bmw_m2", "production_challenger"] },
  { id: "gt4", label: "GT4", classId: "gt4", access: "elite", categories: ["gt4", "endurance"] },
  { id: "gt3", label: "GT3", classId: "gt3", access: "elite", categories: ["gt3", "endurance"] },
  { id: "lmp2", label: "LMP2", classId: "lmp2", access: "elite", categories: ["endurance"] },
];

// `label` (série) + tier reconstroem o nome. Para as multiclasse o tier é só o
// nome da categoria ("Production"/"Endurance"), pois a série já dá a linha.
const CATEGORY_TIER_LABEL = {
  mazda_rookie: "Rookie",
  mazda_amador: "Cup",
  toyota_rookie: "Rookie",
  toyota_amador: "Cup",
  bmw_m2: "Cup",
  production_challenger: "Production",
  gt4: "Series",
  gt3: "Championship",
  endurance: "Endurance",
};

// A linha (série) de uma categoria. Categorias regulares pertencem a exatamente
// uma série; as multiclasse (production/endurance) pertencem a várias, então a
// pista de classe (do time do jogador) desempata.
function resolveSeriesForLane(category, classHint) {
  const hint = normalizeClassId(classHint);
  const candidates = CATEGORY_SERIES.filter((serie) => serie.categories.includes(category));
  if (candidates.length === 0) return CATEGORY_SERIES[0];
  if (candidates.length === 1) return candidates[0];
  return candidates.find((serie) => serie.classId === hint || serie.id === hint) ?? candidates[0];
}

// Nav inicial (série + categoria) a partir do time do jogador, respeitando o
// Bloco Especial (que força production/endurance na linha do próprio jogador).
function resolveInitialNav(phase, playerTeamCategory, classHint, acceptedSpecialOffer) {
  const forced = getForcedSpecialStandingCategory(phase, playerTeamCategory, acceptedSpecialOffer);
  const laneCategory = playerTeamCategory ?? ALL_CATEGORIES[0];
  const series = resolveSeriesForLane(laneCategory, classHint);
  const viewCategory = forced ?? laneCategory;
  if (!series.categories.includes(viewCategory)) {
    return { seriesId: resolveSeriesForLane(viewCategory, classHint).id, viewCategory };
  }
  return { seriesId: series.id, viewCategory };
}

// Reordena os grupos de classe de uma categoria multiclasse para que a classe da
// linha atual apareça primeiro (mantendo a ordem relativa das demais).
function orderSpecialGroupsForClass(groups, classId) {
  if (!groups) return null;
  const index = groups.findIndex((group) => group.id === classId);
  if (index <= 0) return groups;
  const reordered = [...groups];
  const [lead] = reordered.splice(index, 1);
  reordered.unshift(lead);
  return reordered;
}

const STANDARD_POINTS = {
  1: 25,
  2: 18,
  3: 15,
  4: 12,
  5: 10,
  6: 8,
  7: 6,
  8: 4,
  9: 2,
  10: 1,
};

const SPECIAL_STANDING_GROUPS = {
  production_challenger: [
    { id: "bmw", label: "BMW", color: "#bc8cff" },
    { id: "toyota", label: "Toyota", color: "#f2cc60" },
    { id: "mazda", label: "Mazda", color: "#c8102e" },
  ],
  endurance: [
    { id: "lmp2", label: "LMP2", color: "#f2cc60" },
    { id: "gt3", label: "GT3", color: "#e73f47" },
    { id: "gt4", label: "GT4", color: "#58a6ff" },
  ],
};

// Categorias especiais movem exatamente uma equipe por classe por temporada
// (production: bmw/toyota/mazda; endurance: gt4/gt3). LMP2 é fixa: não sobe nem
// desce (ver promotion/block3.rs — ENDURANCE_PAIRS exclui lmp2).
const NO_MOVEMENT_SPECIAL_CLASSES = new Set(["endurance:lmp2"]);

function getSpecialClassRelegationCount(category, classId) {
  if (NO_MOVEMENT_SPECIAL_CLASSES.has(`${category}:${classId}`)) {
    return 0;
  }
  return 1;
}

const PROMOTION_ONLY_TEAM_ZONE_CATEGORIES = new Set([
  "mazda_rookie",
  "toyota_rookie",
  "bmw_m2",
  "gt4",
  "gt3",
  "lmp2",
]);
const NO_MOVEMENT_TEAM_ZONE_CATEGORIES = new Set(["endurance"]);
const PRODUCTION_SPECIAL_FEEDERS = new Set([
  "mazda_rookie",
  "toyota_rookie",
  "mazda_amador",
  "toyota_amador",
  "bmw_m2",
]);
const ENDURANCE_SPECIAL_FEEDERS = new Set(["lmp2", "gt3", "gt4"]);
const SEVERE_INJURY_TYPES = new Set(["Grave", "Critica"]);
const DRIVER_CLICK_DELAY_MS = 220;
const TEAM_CLICK_DELAY_MS = 220;

function injuryStatusMarker(injuryType) {
  if (!injuryType) {
    return null;
  }

  if (SEVERE_INJURY_TYPES.has(injuryType)) {
    return { emoji: "🚑", title: "Lesão grave" };
  }

  return { emoji: "🩸", title: "Lesão ativa" };
}

function DriverStatusMarkers({ driver }) {
  const injuryMarker = injuryStatusMarker(driver.lesao_ativa_tipo);

  return (
    <>
      {driver.is_estreante_da_vida ? (
        <span className="shrink-0 text-xs" title="Estreante da vida" aria-label="Estreante da vida">
          {"\u{1F331}"}
        </span>
      ) : null}
      {driver.is_estreante && !driver.is_estreante_da_vida ? (
        <span className="shrink-0 text-xs" title="Estreante" aria-label="Estreante">
          {"\u2B50"}
        </span>
      ) : null}
      {injuryMarker ? (
        <span className="shrink-0 text-xs" title={injuryMarker.title} aria-label={injuryMarker.title}>
          {injuryMarker.emoji}
        </span>
      ) : null}
    </>
  );
}

function getForcedSpecialStandingCategory(phase, playerTeamCategory, acceptedSpecialOffer) {
  if (phase !== "BlocoEspecial") {
    return null;
  }

  const offeredSpecialCategory =
    typeof acceptedSpecialOffer?.special_category === "string"
      ? acceptedSpecialOffer.special_category.trim().toLowerCase()
      : null;

  if (offeredSpecialCategory === "production_challenger" || offeredSpecialCategory === "endurance") {
    return offeredSpecialCategory;
  }

  if (playerTeamCategory === "production_challenger" || playerTeamCategory === "endurance") {
    return playerTeamCategory;
  }

  if (PRODUCTION_SPECIAL_FEEDERS.has(playerTeamCategory)) {
    return "production_challenger";
  }

  if (ENDURANCE_SPECIAL_FEEDERS.has(playerTeamCategory)) {
    return "endurance";
  }

  return null;
}

function StandingsTab({ onOpenGlobalDrivers = null, onOpenGlobalTeams = null }) {
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const season = useCareerStore((state) => state.season);
  const acceptedSpecialOffer = useCareerStore((state) => state.acceptedSpecialOffer);
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
  const seriesTriggerRef = useRef(null);
  const seriesMenuRef = useRef(null);
  const [seriesMenuOpen, setSeriesMenuOpen] = useState(false);
  const [seriesMenuPos, setSeriesMenuPos] = useState(null);

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
  function toggleSeriesMenu() {
    if (navLocked) return;
    if (seriesMenuOpen) {
      setSeriesMenuOpen(false);
      return;
    }
    const rect = seriesTriggerRef.current?.getBoundingClientRect();
    setSeriesMenuPos(
      rect ? { top: rect.bottom + 8, left: rect.left, minWidth: rect.width } : { top: 0, left: 0, minWidth: 0 },
    );
    setSeriesMenuOpen(true);
  }
  function selectSeries(nextSeriesIndex) {
    switchSeries(nextSeriesIndex);
    setSeriesMenuOpen(false);
  }
  function goUpTier() {
    if (hasTierAbove) setViewCategory(currentSeries.categories[tierIndex + 1]);
  }
  function goDownTier() {
    if (hasTierBelow) setViewCategory(currentSeries.categories[tierIndex - 1]);
  }

  // Fecha o menu de linha ao clicar fora, com Escape, ou se a página rolar/mudar
  // de tamanho (a posição é fixa, capturada na abertura, então rolar invalida).
  useEffect(() => {
    if (!seriesMenuOpen) return undefined;
    function onPointerDown(event) {
      if (
        seriesTriggerRef.current?.contains(event.target) ||
        seriesMenuRef.current?.contains(event.target)
      ) {
        return;
      }
      setSeriesMenuOpen(false);
    }
    function onKeyDown(event) {
      if (event.key === "Escape") setSeriesMenuOpen(false);
    }
    function dismiss() {
      setSeriesMenuOpen(false);
    }
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", dismiss);
    window.addEventListener("scroll", dismiss, true);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", dismiss);
      window.removeEventListener("scroll", dismiss, true);
    };
  }, [seriesMenuOpen]);
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
            : "Não foi possível carregar a classificação.",
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
  const teamStandingSections = useMemo(
    () => buildSpecialStandingSections(teamStandings, specialClassGroups),
    [teamStandings, specialClassGroups],
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
        <h2 className="mt-3 text-3xl font-semibold text-text-primary">Carregando classificação</h2>
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
      <div
        className="grid gap-5 xl:grid-cols-[1.6fr_0.95fr]"
      >
        <GlassCard hover={false} className="overflow-hidden rounded-[28px]">
          <div className="flex items-center justify-between gap-4">
            <div>
              {/* Série (linha de carro): dropdown agrupado troca de linha. */}
              <button
                type="button"
                ref={seriesTriggerRef}
                onClick={toggleSeriesMenu}
                disabled={navLocked}
                aria-haspopup="listbox"
                aria-expanded={seriesMenuOpen}
                className="flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-[0.22em] text-accent-primary transition-colors hover:enabled:text-text-primary disabled:cursor-default disabled:opacity-70"
                title="Trocar de linha"
              >
                <span className="whitespace-nowrap">{currentSeries.label}</span>
                {!navLocked ? (
                  <span
                    className={[
                      "text-[9px] text-text-muted transition-transform",
                      seriesMenuOpen ? "rotate-180" : "",
                    ].join(" ")}
                  >
                    ▾
                  </span>
                ) : null}
              </button>
              {seriesMenuOpen && seriesMenuPos && !navLocked
                ? createPortal(
                    <div
                      ref={seriesMenuRef}
                      role="listbox"
                      aria-label="Linha de carro"
                      className="fixed z-[120] rounded-2xl border border-white/10 bg-[#161b22]/95 p-1.5 shadow-[0_16px_48px_rgba(0,0,0,0.55)] backdrop-blur-md"
                      style={{
                        top: seriesMenuPos.top,
                        left: seriesMenuPos.left,
                        minWidth: Math.max(seriesMenuPos.minWidth, 224),
                      }}
                    >
                      {/* Ordem invertida: topo = ápice (LMP2), base = entrada
                          (Mazda), pra deixar clara a "subida" da escada. Os dois
                          grupos de acesso (premium em cima, free embaixo) ganham
                          um cabeçalho e uma separação física entre eles. */}
                      {CATEGORY_SERIES.slice().reverse().map((serie, displayIndex, ordered) => {
                        const index = CATEGORY_SERIES.indexOf(serie);
                        const isCurrent = serie.id === currentSeries.id;
                        const previous = ordered[displayIndex - 1];
                        const startsGroup = !previous || previous.access !== serie.access;
                        return (
                          <Fragment key={serie.id}>
                            {startsGroup ? (
                              <div
                                className={[
                                  "flex items-center gap-2 px-3 pb-1.5",
                                  displayIndex === 0
                                    ? "pt-1"
                                    : "mt-1.5 border-t border-white/10 pt-2.5",
                                ].join(" ")}
                              >
                                <span
                                  className={[
                                    "text-[9px] font-semibold uppercase tracking-[0.22em]",
                                    serie.access === "elite" ? "text-accent-primary" : "text-text-muted",
                                  ].join(" ")}
                                >
                                  {serie.access === "elite" ? "Elite" : "Pro"}
                                </span>
                              </div>
                            ) : null}
                            <button
                              type="button"
                              role="option"
                              aria-selected={isCurrent}
                              onClick={() => selectSeries(index)}
                              className={[
                                "block w-full rounded-xl px-3 py-2 text-left transition-glass",
                                isCurrent ? "bg-accent-primary/15" : "hover:bg-white/5",
                              ].join(" ")}
                            >
                              <span
                                className={[
                                  "block text-xs font-semibold uppercase tracking-[0.12em]",
                                  isCurrent ? "text-accent-primary" : "text-text-primary",
                                ].join(" ")}
                              >
                                {serie.label}
                              </span>
                              <span className="mt-0.5 block text-[10px] leading-relaxed text-text-muted">
                                {serie.categories.map((cat, catIndex) => (
                                  <Fragment key={cat}>
                                    {catIndex > 0 ? " · " : ""}
                                    <span className={isCurrent && cat === viewCategory ? "text-accent-primary" : ""}>
                                      {CATEGORY_TIER_LABEL[cat] ?? cat}
                                    </span>
                                  </Fragment>
                                ))}
                              </span>
                            </button>
                          </Fragment>
                        );
                      })}
                    </div>,
                    document.body,
                  )
                : null}
              {/* Tier dentro da série: setas verticais sobem/descem o nível. */}
              <div className="mt-1.5 flex items-center gap-2">
                <div className="flex flex-col">
                  <button
                    type="button"
                    onClick={goUpTier}
                    disabled={!hasTierAbove}
                    className="text-[10px] leading-[1.1] transition-colors disabled:cursor-default disabled:opacity-20 text-text-muted hover:enabled:text-text-primary"
                    title="Tier superior"
                  >
                    ▲
                  </button>
                  <button
                    type="button"
                    onClick={goDownTier}
                    disabled={!hasTierBelow}
                    className="text-[10px] leading-[1.1] transition-colors disabled:cursor-default disabled:opacity-20 text-text-muted hover:enabled:text-text-primary"
                    title="Tier inferior"
                  >
                    ▼
                  </button>
                </div>
                <h2 className="text-2xl font-semibold text-text-primary">
                  {CATEGORY_TIER_LABEL[viewCategory] ?? categoryLabel(viewCategory)}
                </h2>
              </div>
            </div>
            <p className="text-sm text-text-secondary">{driverStandings.length} pilotos</p>
          </div>

          {showSpecialPendingNotice ? (
            <SpecialPendingNotice category={viewCategory} phase={season?.fase} />
          ) : (
          <div className="mt-6 overflow-x-auto">
            <table className="w-full table-fixed" style={{ minWidth: 536 + totalRodadas * 68 }}>
              <colgroup>
                <col style={{ width: "62px" }} />
                <col style={{ width: "34px" }} />
                <col style={{ width: "42px" }} />
                <col style={{ width: "214px" }} />
                <col style={{ width: "118px" }} />
                {Array.from({ length: totalRodadas }, (_, index) => (
                  <col key={`rodada-col-${index + 1}`} style={{ width: "68px" }} />
                ))}
                <col style={{ width: "66px" }} />
              </colgroup>
              <thead>
                <tr className="border-b border-white/10 text-left text-[11px] uppercase tracking-[0.18em] text-text-muted">
                  <th className="py-3 pr-0.5">Pos</th>
                  <th className="py-3 pr-0.5 text-center" />
                  <th className="py-3 pr-1 text-center">Id</th>
                  <th className="py-3 pr-1">Piloto</th>
                  <th className="py-3 pr-1">Equipe</th>
                  {Array.from({ length: totalRodadas }, (_, index) => {
                    const rodada = index + 1;
                    const isCompleted = rodada <= completedRounds;
                    return (
                      <th
                        key={rodada}
                        className={[
                          "px-1.5 py-3 text-center",
                          rodada > completedRounds ? "opacity-30" : "",
                          rodada === season?.rodada_atual ? "text-accent-primary" : "",
                          isCompleted ? "cursor-pointer hover:text-accent-primary transition-colors" : "",
                        ].join(" ")}
                        title={isCompleted ? "Duplo clique: rever a classificação desta corrida" : undefined}
                        onDoubleClick={
                          isCompleted ? () => openSavedRaceScreen?.(viewCategory, rodada) : undefined
                        }
                      >
                        R{rodada}
                      </th>
                    );
                  })}
                  <th className="py-3 pl-3 text-right">Pts</th>
                </tr>
              </thead>
              <tbody>
                {driverStandingSections.map((section) => (
                  <Fragment key={`drivers-${section.id}`}>
                    {specialClassGroups ? (
                      <tr>
                        <td colSpan={totalRodadas + 6} className="px-0 pt-4 pb-2">
                          <SpecialClassHeader section={section} sticky />
                        </td>
                      </tr>
                    ) : null}
                    {section.items.map((driver, index) => {
                  const isInSelectedTeam =
                    selectedTeamId != null && driver.equipe_id === selectedTeamId;
                  const teamColor = selectedTeamColor;
                  const displayPosition = specialClassGroups
                    ? index + 1
                    : driver.posicao_campeonato;

                  return (
                    <tr
                      key={driver.id}
                      role="button"
                      tabIndex={0}
                      onMouseEnter={() => setHoveredDriverId(driver.id)}
                      onMouseLeave={() => setHoveredDriverId(null)}
                      onClick={() => handleDriverClick(driver.id)}
                      onDoubleClick={() => handleDriverDoubleClick(driver.id)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          clearDriverClickTimeout();
                          openDriverDetail(driver.id);
                        }
                      }}
                      className={[
                        "cursor-pointer border-b border-white/5 transition-glass",
                        !isInSelectedTeam && driver.is_jogador
                          ? "bg-accent-primary/8 hover:bg-accent-primary/15"
                          : !isInSelectedTeam
                            ? "hover:bg-white/5 focus-visible:bg-white/5"
                            : "",
                      ].join(" ")}
                      style={
                        isInSelectedTeam && teamColor
                          ? {
                              backgroundColor: `${teamColor}22`,
                              boxShadow: `inset 3px 0 0 0 ${teamColor}`,
                            }
                          : undefined
                      }
                    >
                      <td className="py-3 pr-0.5 text-sm font-semibold">
                        <div className="flex items-center gap-1">
                          <span className={podiumClass(index)}>{displayPosition}</span>
                          <PositionDelta delta={positionDeltaMap.get(driver.id)} />
                        </div>
                      </td>
                      <td className="py-3 pr-0.5 text-center">
                        <FlagIcon nacionalidade={driver.nacionalidade} className="mx-auto" />
                      </td>
                      <td className="py-3 pr-1 text-center text-sm font-medium text-text-secondary">
                        {driver.idade ?? "—"}
                      </td>
                      <td className="py-3 pr-1">
                        <div className="flex items-center gap-2">
                          <span
                            className={[
                              "block truncate text-sm whitespace-nowrap",
                              driver.is_jogador
                                ? "font-semibold text-accent-primary"
                                : "text-text-primary",
                            ].join(" ")}
                            title={driver.nome}
                          >
                            {driver.is_jogador ? "▸ " : ""}
                            {driver.nome}
                            {driver.is_jogador ? " ◂" : ""}
                          </span>
                          <DriverStatusMarkers driver={driver} />
                          {driver.id === previousChampionId ? (
                            <span className="shrink-0 text-sm" title="Campeão da temporada anterior">
                              🏆
                            </span>
                          ) : null}
                        </div>
                      </td>
                      <td className="py-3 pr-1 pl-0 text-sm font-semibold uppercase tracking-[0.02em]">
                        <div className="flex items-center gap-1.5">
                          <TeamLogoMarkShared
                            teamName={driver.equipe_nome}
                            color={driver.equipe_cor}
                            size="sm"
                          />
                          <span
                            className="block truncate"
                            style={{ color: getReadableTeamColor(driver.equipe_cor) }}
                            title={driver.equipe_nome_curto ?? driver.equipe_nome ?? "—"}
                          >
                            {driver.equipe_nome_curto ?? driver.equipe_nome ?? "—"}
                          </span>
                        </div>
                      </td>
                      {(driver.results ?? []).map((result, rodadaIndex) => (
                        <td key={`${driver.id}-r${rodadaIndex + 1}`} className="px-1 py-3 text-center">
                          <ResultBadge result={result} />
                        </td>
                      ))}
                      <td className="py-3 pl-3 text-right font-mono text-sm font-semibold text-text-primary">
                        {driver.pontos}
                      </td>
                    </tr>
                  );
                    })}
                  </Fragment>
                ))}
              </tbody>
            </table>
          </div>
          )}
        </GlassCard>

        <GlassCard hover={false} className="rounded-[28px]">
          <div className="flex items-center justify-between gap-4">
            <div>
              <p className="text-[11px] uppercase tracking-[0.22em] text-accent-primary">
                Construtores
              </p>
              <h2 className="mt-2 text-2xl font-semibold text-text-primary">
                Classificação de equipes
              </h2>
            </div>
            <p className="text-sm text-text-secondary">{teamStandings.length} equipes</p>
          </div>

          <div className="mt-6 space-y-2">
            {showSpecialPendingNotice ? (
              <SpecialPendingTeamsNotice />
            ) : specialClassGroups ? (
              teamStandingSections.map((section) => {
                const relegationCount = getSpecialClassRelegationCount(viewCategory, section.id);
                return (
                <div key={`teams-${section.id}`} className="space-y-2">
                  <SpecialClassHeader section={section} />
                  {section.items.map((team, index) => {
                    const isRelegationZone =
                      relegationCount > 0
                      && section.items.length > relegationCount
                      && index >= section.items.length - relegationCount;
                    return (
                      <TeamStandingCard
                        key={team.id}
                        team={team}
                        position={index + 1}
                        index={index}
                        isRelegationZone={isRelegationZone}
                        isHistoryActive={selectedHistoryTeam?.id === team.id}
                        onTeamDossierOpen={handleTeamClick}
                        onTeamGlobalHistoryOpen={handleTeamDoubleClick}
                      />
                    );
                  })}
                </div>
                );
              })
            ) : (() => {
              const { promotionCount, relegationCount } = getZoneCutoffs(viewCategory);
              const total = teamStandings.length;
              const items = [];
              teamStandings.forEach((team, index) => {
                if (index === promotionCount && promotionCount > 0) {
                  items.push(<ZoneDivider key="divider-promo" label="PROMOÇÃO ↑" variant="green" />);
                }
                if (relegationCount > 0 && index === total - relegationCount) {
                  items.push(<ZoneDivider key="divider-relego" label="REBAIXAMENTO ↓" variant="red" />);
                }
                items.push(
                  <div
                    key={team.id}
                    onClick={() => handleTeamClick(team)}
                    onDoubleClick={() => handleTeamDoubleClick(team)}
                    className={[
                      "flex items-center justify-between rounded-2xl border px-4 py-3 transition-glass hover:bg-white/[0.05]",
                      selectedHistoryTeam?.id === team.id
                        ? "border-status-yellow/45 bg-status-yellow/10 shadow-[inset_4px_0_0_rgba(242,196,109,0.95)]"
                        : "border-white/6 bg-white/[0.03]",
                    ].join(" ")}
                  >
                    <div className="flex min-w-0 flex-1 items-center gap-3">
                      <span className={["w-7 text-center text-sm font-semibold", podiumClass(index)].join(" ")}>
                        {team.posicao}
                      </span>
                      <TeamLogoMarkShared teamName={team.nome} color={team.cor_primaria} />
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-1.5">
                          <p
                            className="truncate text-sm font-semibold"
                            style={{ color: getReadableTeamColor(team.cor_primaria) }}
                          >
                            {team.nome}
                          </p>
                          {(team.trofeus ?? []).map((trofeu, trophyIndex) => (
                            <TrophyBadge key={`${team.id}-t${trophyIndex}`} trofeu={trofeu} />
                          ))}
                        </div>
                        <TeamDriverLine team={team} />
                      </div>
                    </div>
                    <div className="shrink-0 pl-4 text-right">
                      <p className="font-mono text-base font-semibold text-text-primary">{team.pontos}</p>
                      <p className="text-xs text-text-secondary">{team.vitorias} vit.</p>
                    </div>
                  </div>
                );
              });
              return items;
            })()}
          </div>
        </GlassCard>
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

function podiumClass(index) {
  if (index === 0) return "text-[#ffd700]";
  if (index === 1) return "text-[#c0c0c0]";
  if (index === 2) return "text-[#cd7f32]";
  return "text-text-secondary";
}

function getReadableTeamColor(color) {
  if (!color || !/^#([0-9a-f]{6})$/i.test(color)) {
    return "#7d8590";
  }

  const hex = color.slice(1);
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  const luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;

  if (luminance < 0.32) {
    const mixWithWhite = 0.58;
    const boost = (channel) => Math.round(channel + (255 - channel) * mixWithWhite);
    return `rgb(${boost(r)}, ${boost(g)}, ${boost(b)})`;
  }

  return color;
}

function formatTeamDriverName(name) {
  return typeof name === "string" && name.trim().length > 0 ? name.trim() : "-";
}

function formatTeamDriverPair(team) {
  return `${formatTeamDriverName(team.piloto_1_nome)} / ${formatTeamDriverName(team.piloto_2_nome)}`;
}

function TeamDriverLine({ team }) {
  const driverNames = formatTeamDriverPair(team);

  return (
    <p
      className="block truncate whitespace-nowrap text-xs text-text-secondary"
      title={driverNames}
    >
      {driverNames}
    </p>
  );
}

function hasSpecialStandingResults(driverStandings, teamStandings) {
  return (
    driverStandings.some((driver) => {
      const hasRoundResult = (driver.results ?? []).some(Boolean);
      return hasRoundResult || driver.pontos > 0 || driver.vitorias > 0 || driver.podios > 0;
    })
    || teamStandings.some((team) => team.pontos > 0 || team.vitorias > 0)
  );
}

function specialPendingMessage(phase) {
  if (phase === "JanelaConvocacao") {
    return "A competição começa quando a janela de convocação for finalizada. Os resultados aparecem aqui quando a primeira corrida for simulada.";
  }
  if (phase === "BlocoEspecial") {
    return "O bloco especial já foi aberto, mas ainda não existe resultado registrado. A classificação aparece quando a primeira corrida for simulada.";
  }
  return "Esta competição acontece depois da temporada regular e da janela de convocação. Os resultados aparecem aqui quando o bloco especial for simulado.";
}

function SpecialPendingNotice({ category, phase }) {
  return (
    <div className="mt-6 rounded-3xl border border-white/10 bg-white/[0.035] p-6 text-center shadow-[inset_0_1px_0_rgba(255,255,255,0.05)]">
      <p className="text-[11px] font-semibold uppercase tracking-[0.22em] text-accent-primary">
        {categoryLabel(category)}
      </p>
      <h3 className="mt-3 text-xl font-semibold text-text-primary">
        Competição especial ainda não aconteceu
      </h3>
      <p className="mx-auto mt-3 max-w-xl text-sm leading-6 text-text-secondary">
        {specialPendingMessage(phase)}
      </p>
    </div>
  );
}

function SpecialPendingTeamsNotice() {
  return (
    <div className="rounded-2xl border border-white/8 bg-white/[0.025] px-4 py-5 text-sm leading-6 text-text-secondary">
      A classificação de equipes será liberada junto com os resultados do bloco especial.
    </div>
  );
}

function SpecialClassHeader({ section, sticky = false }) {
  return (
    <div
      className={[
        "flex items-center justify-center gap-3 py-2.5",
        sticky ? "sticky left-0 z-10 w-[min(760px,calc(100vw-3rem))]" : "w-full",
      ].join(" ")}
    >
      <span
        className="h-px flex-1"
        style={{
          background: `linear-gradient(90deg, transparent 0%, ${section.color}4d 58%, ${section.color}c2 100%)`,
        }}
      />
      <span
        className="shrink-0 px-3 text-center text-[17px] font-black uppercase leading-none tracking-[0.22em]"
        style={{
          color: section.color,
          textShadow: `0 0 18px ${section.color}55`,
        }}
      >
        {section.label}
      </span>
      <span
        className="h-px flex-1"
        style={{
          background: `linear-gradient(90deg, ${section.color}c2 0%, ${section.color}4d 42%, transparent 100%)`,
        }}
      />
    </div>
  );
}

function TeamStandingCard({
  team,
  position,
  index,
  isRelegationZone = false,
  isHistoryActive = false,
  onTeamDossierOpen,
  onTeamGlobalHistoryOpen,
}) {
  const cardClassName = [
    "flex items-center justify-between rounded-2xl border px-4 py-3 transition-glass",
    isHistoryActive
      ? "border-status-yellow/45 bg-status-yellow/10 shadow-[inset_4px_0_0_rgba(242,196,109,0.95)] hover:bg-status-yellow/15"
      : "",
    isRelegationZone
      ? "border-status-red/35 bg-status-red/[0.12] shadow-[inset_3px_0_0_0_rgba(248,81,73,0.75)] hover:bg-status-red/[0.18]"
      : !isHistoryActive
        ? "border-white/6 bg-white/[0.03] hover:bg-white/[0.05]"
        : "",
  ].join(" ");

  return (
    <div
      onClick={() => onTeamDossierOpen?.(team)}
      onDoubleClick={() => onTeamGlobalHistoryOpen?.(team)}
      className={cardClassName}
      data-relegation-zone={isRelegationZone ? "true" : undefined}
    >
      <div className="flex min-w-0 flex-1 items-center gap-3">
        <span
          className={[
            "w-7 text-center text-sm font-semibold",
            isRelegationZone ? "text-status-red" : podiumClass(index),
          ].join(" ")}
        >
          {position}
        </span>
        <TeamLogoMarkShared teamName={team.nome} color={team.cor_primaria} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <p
              className="block truncate text-sm font-semibold"
              style={{ color: getReadableTeamColor(team.cor_primaria) }}
            >
              {team.nome}
            </p>
            {(team.trofeus ?? []).map((trofeu, trophyIndex) => (
              <TrophyBadge key={`${team.id}-t${trophyIndex}`} trofeu={trofeu} />
            ))}
          </div>
          <TeamDriverLine team={team} />
        </div>
      </div>
      <div className="shrink-0 pl-4 text-right">
        <p className="font-mono text-base font-semibold text-text-primary">{team.pontos}</p>
        <p className="text-xs text-text-secondary">{team.vitorias} vit.</p>
      </div>
    </div>
  );
}

function buildSpecialStandingSections(items, classGroups) {
  if (!classGroups) {
    return [{ id: "all", label: null, color: "#7d8590", items }];
  }

  const knownIds = new Set(classGroups.map((group) => group.id));
  const sections = classGroups
    .map((group) => ({
      ...group,
      items: items.filter((item) => normalizeClassId(item.classe) === group.id),
    }))
    .filter((section) => section.items.length > 0);

  const unknownItems = items.filter((item) => !knownIds.has(normalizeClassId(item.classe)));
  if (unknownItems.length > 0) {
    sections.push({
      id: "outros",
      label: "Outros",
      color: "#7d8590",
      items: unknownItems,
    });
  }

  return sections;
}

function normalizeClassId(value) {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

function buildPositionDeltaMap(drivers, completedRounds) {
  const deltaMap = new Map();

  if (!Array.isArray(drivers) || completedRounds <= 1) {
    return deltaMap;
  }

  const previousRoundCount = completedRounds - 1;
  const previousStandings = [...drivers]
    .map((driver) => ({
      id: driver.id,
      nome: driver.nome,
      currentPosition: driver.posicao_campeonato ?? Number.MAX_SAFE_INTEGER,
      previousPoints: calculatePointsThroughRound(driver.results, previousRoundCount),
      previousBestFinish: calculateBestFinish(driver.results, previousRoundCount),
    }))
    .sort((left, right) => {
      if (right.previousPoints !== left.previousPoints) {
        return right.previousPoints - left.previousPoints;
      }
      if (left.previousBestFinish !== right.previousBestFinish) {
        return left.previousBestFinish - right.previousBestFinish;
      }
      if (left.currentPosition !== right.currentPosition) {
        return left.currentPosition - right.currentPosition;
      }
      return left.nome.localeCompare(right.nome, "pt-BR");
    });

  previousStandings.forEach((driver, index) => {
    deltaMap.set(driver.id, index + 1);
  });

  return new Map(
    drivers.map((driver) => {
      const previousPosition = deltaMap.get(driver.id);
      const currentPosition = driver.posicao_campeonato ?? 0;
      return [driver.id, previousPosition ? previousPosition - currentPosition : 0];
    }),
  );
}

function calculatePointsThroughRound(results, roundCount) {
  if (!Array.isArray(results) || roundCount <= 0) {
    return 0;
  }

  return results.slice(0, roundCount).reduce((total, result) => total + pointsForResult(result), 0);
}

function calculateBestFinish(results, roundCount) {
  if (!Array.isArray(results) || roundCount <= 0) {
    return Number.MAX_SAFE_INTEGER;
  }

  return results.slice(0, roundCount).reduce((best, result) => {
    if (!result || result.is_dnf) {
      return best;
    }
    return Math.min(best, result.position ?? Number.MAX_SAFE_INTEGER);
  }, Number.MAX_SAFE_INTEGER);
}

function pointsForResult(result) {
  if (!result || result.is_dnf) {
    return 0;
  }

  return STANDARD_POINTS[result.position] ?? 0;
}

function getZoneCutoffs(categoria) {
  if (PROMOTION_ONLY_TEAM_ZONE_CATEGORIES.has(categoria)) {
    return { promotionCount: 1, relegationCount: 0 };
  }
  if (NO_MOVEMENT_TEAM_ZONE_CATEGORIES.has(categoria)) {
    return { promotionCount: 0, relegationCount: 0 };
  }
  return { promotionCount: 1, relegationCount: 1 };
}

function ZoneDivider({ label, variant }) {
  const colorClass = variant === "green" ? "text-status-green border-status-green/30" : "text-status-red border-status-red/30";
  const lineClass = variant === "green" ? "border-status-green/20" : "border-status-red/20";
  return (
    <div className="flex items-center gap-3 py-1">
      <div className={["flex-1 border-t border-dashed", lineClass].join(" ")} />
      <span className={["text-[10px] font-semibold uppercase tracking-[0.18em] px-2 py-0.5 rounded border", colorClass].join(" ")}>
        {label}
      </span>
      <div className={["flex-1 border-t border-dashed", lineClass].join(" ")} />
    </div>
  );
}

function PositionDelta({ delta }) {
  if (!delta) {
    return <span className="text-[10px] font-semibold text-text-muted">•</span>;
  }

  if (delta > 0) {
    return (
      <span className="inline-flex items-center justify-center gap-0.5 text-[10px] font-semibold leading-none text-status-green">
        ▲{delta}
      </span>
    );
  }

  return (
    <span className="inline-flex items-center justify-center gap-0.5 text-[10px] font-semibold leading-none text-status-red">
      ▼{Math.abs(delta)}
    </span>
  );
}

export default StandingsTab;
