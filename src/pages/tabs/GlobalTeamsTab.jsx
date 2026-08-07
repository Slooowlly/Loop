import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import i18n from "../../i18n/index.js";
import GlassCard from "../../components/ui/GlassCard";
import Tooltip from "../../components/ui/Tooltip";
import useCareerStore from "../../stores/useCareerStore";
import { TeamHistoryDrawer } from "../../components/team/history";
import { TeamHistoryGrid } from "../../components/team/WorldTeamHistoryGrid";
import { YearWindowScrubber } from "../../components/team/YearWindowScrubber";
import {
  DEFAULT_START_YEAR,
  buildGeometry,
  buildTeamTracks,
  buildYears,
  clamp,
  clampVisibleStart,
  familyFromTeamContext,
  familyMaxYear,
  flattenTeams,
  latestWindowStart,
  normalizePayload,
  scrollMinYear,
  teamRowToTeam,
  teamToTeamRow,
  visibleWindowEndYear,
  visibleWindowSize,
} from "../../components/team/worldTeamChartGeometry";

// Zoom "recente": quantos anos a linha do tempo mostra quando o zoom está ligado.
const ZOOM_RECENT_YEARS = 10;
const HISTORY_FETCH_WINDOW_SIZE = 32;
// Mesmo ano-base do fallback da geometria — fonte única para não divergirem.
const HISTORY_FETCH_START_YEAR = DEFAULT_START_YEAR;
const TEAM_CLICK_DELAY_MS = 220;

function GlobalTeamsTab({
  selectedTeamId = null,
  selectedTeamCategory = null,
  selectedTeamClassName = null,
  initialZoomYears = null,
  pinnedTeamId = null,
  drawerPlacement = "right",
  onBack,
}) {
  const { t } = useTranslation();
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  // Zoom da linha do tempo: null = janela cheia; N = mostra só os últimos N anos.
  const [zoomYears, setZoomYears] = useState(initialZoomYears);
  const [family, setFamily] = useState(() => familyFromTeamContext(selectedTeamCategory, selectedTeamClassName));
  const [startYear, setStartYear] = useState(HISTORY_FETCH_START_YEAR);
  const [payload, setPayload] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [focusedTeamId, setFocusedTeamId] = useState(selectedTeamId);
  const [selectedTeam, setSelectedTeam] = useState(null);
  const [activeHistoryTab, setActiveHistoryTab] = useState("records");
  const [previewStartYear, setPreviewStartYear] = useState(null);
  const teamClickTimeoutRef = useRef(null);

  useEffect(() => {
    setFocusedTeamId(selectedTeamId);
  }, [selectedTeamId]);

  useEffect(() => {
    setFamily(familyFromTeamContext(selectedTeamCategory, selectedTeamClassName));
  }, [selectedTeamCategory, selectedTeamClassName]);

  useEffect(() => {
    let mounted = true;

    async function load() {
      if (!careerId) {
        setPayload(null);
        setError(i18n.t("globalTeams.notLoaded"));
        setLoading(false);
        return;
      }

      try {
        setLoading(true);
        setError("");
        const data = await invoke("get_global_team_history", {
          careerId,
          family,
          startYear: HISTORY_FETCH_START_YEAR,
          windowSize: HISTORY_FETCH_WINDOW_SIZE,
        });
        if (!mounted) return;
        setPayload(normalizePayload(data));
      } catch (invokeError) {
        if (!mounted) return;
        setError(typeof invokeError === "string" ? invokeError : i18n.t("globalTeams.loadError"));
      } finally {
        if (mounted) {
          setLoading(false);
        }
      }
    }

    load();
    return () => {
      mounted = false;
    };
  }, [careerId, family]);

  useEffect(() => () => {
    if (teamClickTimeoutRef.current) {
      clearTimeout(teamClickTimeoutRef.current);
    }
  }, []);

  useEffect(() => {
    // Open at the latest end: when the window is capped (lots of data) this keeps the
    // current-standings table on screen; when it isn't capped it clamps to the single
    // locked start, so the whole timeline shows either way.
    setStartYear(payload ? familyMaxYear(payload) : HISTORY_FETCH_START_YEAR);
    setPreviewStartYear(null);
  }, [payload?.window_start, payload?.selected_family]);

  const windowSize = useMemo(() => {
    const base = visibleWindowSize(payload);
    return zoomYears ? Math.min(base, zoomYears) : base;
  }, [payload, zoomYears]);
  const visibleStartYear = useMemo(() => clampVisibleStart(payload, startYear, windowSize), [payload, startYear, windowSize]);
  const visibleEndYear = useMemo(
    () => visibleWindowEndYear(payload, visibleStartYear, windowSize),
    [payload, visibleStartYear, windowSize],
  );
  const years = useMemo(() => buildYears(payload, zoomYears), [payload, zoomYears]);
  const geometry = useMemo(() => buildGeometry(payload, years), [payload, years]);
  const teamTracks = useMemo(() => buildTeamTracks(payload, geometry, years), [payload, geometry, years]);
  const allTeams = useMemo(() => flattenTeams(payload), [payload]);
  const activeFamily = payload?.families?.find((item) => item.id === payload?.selected_family);

  function selectFamily(nextFamily) {
    setFamily(nextFamily);
  }

  function handleWindowStartChange(nextYear) {
    if (!payload) return;
    const latestStart = latestWindowStart(payload, windowSize);
    setStartYear(clamp(nextYear, scrollMinYear(payload), latestStart));
  }

  function clearTeamClickTimeout() {
    if (teamClickTimeoutRef.current) {
      clearTimeout(teamClickTimeoutRef.current);
      teamClickTimeoutRef.current = null;
    }
  }

  function openTeamDossier(team) {
    setSelectedTeam(team);
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
    setFocusedTeamId(team.team_id);
  }

  if (loading && !payload) {
    return <GlobalTeamsLoading onBack={onBack} />;
  }

  return (
    <div className="space-y-5">
      <header className="flex flex-wrap items-start justify-between gap-4 px-1">
        <div>
          <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">{t("globalTeams.eyebrow")}</p>
          <h2 className="mt-2 text-3xl font-semibold text-text-primary">{t("globalTeams.worldTeamHistory")}</h2>
        </div>
        <button
          type="button"
          onClick={onBack}
          className="rounded-2xl border border-white/10 bg-white/[0.04] px-4 py-2 text-xs font-semibold uppercase tracking-[0.16em] text-text-secondary transition-glass hover:border-accent-primary/40 hover:bg-accent-primary/10 hover:text-text-primary"
        >
          {i18n.t("globalTeams.backToStandings")}
        </button>
      </header>

      {error ? (
        <div className="rounded-2xl border border-status-red/30 bg-status-red/10 px-4 py-3 text-sm text-status-red">
          {error}
        </div>
      ) : null}

      <GlassCard hover={false} className="overflow-hidden rounded-[30px] p-0">
        <div className="flex flex-wrap items-start justify-between gap-4 border-b border-white/10 bg-black/10 px-5 py-4">
          <div>
            <p className="text-[10px] uppercase tracking-[0.24em] text-accent-primary">{t("globalTeams.historicAtlas")}</p>
            <h3 className="mt-2 text-2xl font-semibold text-text-primary">
              {i18n.t("globalTeams.familyWindow", { family: activeFamily?.label ?? "Mazda", start: visibleStartYear ?? "-", end: visibleEndYear ?? "-" })}
            </h3>
          </div>
          <div className="flex flex-wrap justify-end gap-2">
            <Tooltip
              texto={zoomYears != null ? i18n.t("globalTeams.zoomFullTitle") : i18n.t("globalTeams.zoomRecentTitle", { years: ZOOM_RECENT_YEARS })}
            >
              <button
                type="button"
                aria-pressed={zoomYears != null}
                onClick={() => setZoomYears((current) => (current == null ? ZOOM_RECENT_YEARS : null))}
                className={`mr-1 rounded-full border px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.13em] transition-glass ${
                  zoomYears != null
                    ? "border-accent-primary/50 bg-accent-primary/15 text-accent-primary"
                    : "border-white/10 bg-white/[0.04] text-text-muted hover:text-text-primary"
                }`}
              >
                {zoomYears != null ? i18n.t("globalTeams.zoomRecentLabel", { years: ZOOM_RECENT_YEARS }) : i18n.t("globalTeams.zoomAll")}
              </button>
            </Tooltip>
            {(payload?.families ?? []).map((item) => (
              <button
                key={item.id}
                type="button"
                aria-pressed={item.id === payload?.selected_family}
                onClick={() => selectFamily(item.id)}
                className={`rounded-full border px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.13em] transition-glass ${
                  item.id === payload?.selected_family
                    ? "border-status-yellow/45 bg-status-yellow/12 text-status-yellow"
                    : "border-white/10 bg-white/[0.04] text-text-muted hover:text-text-primary"
                }`}
              >
                {item.label}
              </button>
            ))}
          </div>
        </div>

        <div className="overflow-x-auto">
          <div className="min-w-[1180px]" style={{ height: geometry.totalHeight }}>
            <TeamHistoryGrid
              payload={payload}
              years={years}
              geometry={geometry}
              teamTracks={teamTracks}
              previewStartYear={previewStartYear}
              visibleStartYear={visibleStartYear}
              windowSize={windowSize}
              focusedTeamId={focusedTeamId}
              pinnedTeamId={pinnedTeamId}
              onFocus={setFocusedTeamId}
              onTeamClick={handleTeamClick}
              onTeamDoubleClick={handleTeamDoubleClick}
            />
          </div>
        </div>

        {/* No zoom "recente" a janela é fixa (últimos N anos) → o scrubber não faz
            sentido (rolar desincronizaria o gráfico recortado); só aparece na visão cheia. */}
        {zoomYears == null && (
          <div className="sticky bottom-0 z-40 border-t border-white/10 bg-[#07101d]/95 px-5 py-3 shadow-[0_-18px_36px_rgba(0,0,0,0.32)] backdrop-blur-xl">
            <YearWindowScrubber
              payload={payload}
              visibleStart={visibleStartYear}
              previewStart={previewStartYear}
              windowSize={windowSize}
              onPreviewChange={setPreviewStartYear}
              onChange={handleWindowStartChange}
              compact
            />
          </div>
        )}
      </GlassCard>

      {selectedTeam ? (
        <TeamHistoryDrawer
          careerId={careerId}
          team={teamRowToTeam(selectedTeam)}
          teams={allTeams.map(teamRowToTeam)}
          playerTeam={playerTeam}
          activeCategory={selectedTeam.category ?? selectedTeam.points?.[0]?.category ?? selectedTeam.band_category ?? ""}
          activeTab={activeHistoryTab}
          placement={drawerPlacement}
          onTabChange={setActiveHistoryTab}
          onSelectTeam={(team) => setSelectedTeam(teamToTeamRow(team, selectedTeam))}
          onClose={() => setSelectedTeam(null)}
        />
      ) : null}
    </div>
  );
}

function GlobalTeamsLoading({ onBack }) {
  const { t } = useTranslation();
  return (
    <div className="space-y-5">
      <header className="flex flex-wrap items-start justify-between gap-4 px-1">
        <div>
          <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">{t("globalTeams.eyebrow")}</p>
          <h2 className="mt-2 text-3xl font-semibold text-text-primary">{t("globalTeams.buildingWorldTeamHistory")}</h2>
        </div>
        <button
          type="button"
          onClick={onBack}
          className="rounded-2xl border border-white/10 bg-white/[0.04] px-4 py-2 text-xs font-semibold uppercase tracking-[0.16em] text-text-secondary"
        >
          {i18n.t("globalTeams.backToStandings")}
        </button>
      </header>
      <GlassCard hover={false} className="rounded-[30px]">
        <div className="h-96 animate-pulse rounded-[24px] border border-white/8 bg-white/[0.035]" />
      </GlassCard>
    </div>
  );
}

export default GlobalTeamsTab;
