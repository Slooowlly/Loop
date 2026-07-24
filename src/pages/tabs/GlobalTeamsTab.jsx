import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import i18n from "../../i18n/index.js";
import TeamLogoMark from "../../components/team/TeamLogoMark";
import GlassCard from "../../components/ui/GlassCard";
import useCareerStore from "../../stores/useCareerStore";
import { TeamHistoryDrawer } from "../../components/team/TeamHistoryDrawer";
import goldTrophy from "../../assets/utilities/trophies/ouro.png";

/**
 * Maps each band_key to a dedicated trophy image path.
 * Replace `null` entries with an imported image once the art is produced.
 * Keys are stable identifiers from TeamHistoryBandDef (global_team_history.rs).
 *
 * Distinct band keys (11 total — one image slot per entry):
 *   Mazda  : mazda_rookie · mazda_amador · production_mazda
 *   Toyota : toyota_rookie · toyota_amador · production_toyota
 *   BMW    : bmw_m2 · production_bmw
 *   GT4    : gt4 · endurance_gt4
 *   GT3    : gt3 · endurance_gt3
 *   LMP2   : endurance_lmp2
 */
const BAND_TROPHY_IMAGES = {
  mazda_rookie:      null,
  mazda_amador:      null,
  production_mazda:  null,
  toyota_rookie:     null,
  toyota_amador:     null,
  production_toyota: null,
  bmw_m2:            null,
  production_bmw:    null,
  gt4:               null,
  endurance_gt4:     null,
  gt3:               null,
  endurance_gt3:     null,
  endurance_lmp2:    null,
};

const DEFAULT_FAMILY = "mazda";
const DEFAULT_WINDOW_SIZE = 20;
// Zoom "recente": quantos anos a linha do tempo mostra quando o zoom está ligado.
const ZOOM_RECENT_YEARS = 10;
const HISTORY_FETCH_WINDOW_SIZE = 32;
const HISTORY_FETCH_START_YEAR = 2000;
const TEAM_CLICK_DELAY_MS = 220;
const CHART_WIDTH = 1000;
const CHART_HEADER_HEIGHT = 56;
const MIN_CHART_HEIGHT = 360;
const MIN_BAND_HEIGHT = 124;
const BAND_LABEL_HEIGHT = 22;
const ROW_HEIGHT = 32;
const ROW_PILL_HEIGHT = 28;
const ROW_TOP_OFFSET = 34;
const BAND_BOTTOM_PADDING = 16;
// Most years the window may show at once before the chart gets too cramped/distorted
// — matches GT3's current widest range (1994–2027). Past this, the window stops
// growing and starts SCROLLING instead (the scrubber becomes functional).
const MAX_VISIBLE_SPAN = 34;

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
  const displayStartYear = useMemo(
    () => roundedDisplayStartYear(payload, previewStartYear ?? visibleStartYear, windowSize),
    [payload, previewStartYear, visibleStartYear, windowSize],
  );
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
            <button
              type="button"
              aria-pressed={zoomYears != null}
              onClick={() => setZoomYears((current) => (current == null ? ZOOM_RECENT_YEARS : null))}
              title={zoomYears != null ? i18n.t("globalTeams.zoomFullTitle") : i18n.t("globalTeams.zoomRecentTitle", { years: ZOOM_RECENT_YEARS })}
              className={`mr-1 rounded-full border px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.13em] transition-glass ${
                zoomYears != null
                  ? "border-accent-primary/50 bg-accent-primary/15 text-accent-primary"
                  : "border-white/10 bg-white/[0.04] text-text-muted hover:text-text-primary"
              }`}
            >
              {zoomYears != null ? i18n.t("globalTeams.zoomRecentLabel", { years: ZOOM_RECENT_YEARS }) : i18n.t("globalTeams.zoomAll")}
            </button>
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

function YearWindowScrubber({
  payload,
  visibleStart,
  previewStart,
  windowSize = DEFAULT_WINDOW_SIZE,
  onPreviewChange,
  onChange,
  ariaLabel = i18n.t("globalTeams.moveWindow"),
  railTestId = "world-team-window-scrubber",
  compact = false,
}) {
  const [dragging, setDragging] = useState(false);
  const railRef = useRef(null);

  useEffect(() => {
    setDragging(false);
  }, [payload?.window_start, payload?.selected_family]);

  useEffect(() => {
    if (!dragging || !payload) return undefined;

    function handlePointerMove(event) {
      onPreviewChange(yearFromClientX(payload, railRef.current, event.clientX, windowSize));
    }

    function handlePointerUp(event) {
      const nextYear = yearFromClientX(payload, railRef.current, event.clientX, windowSize);
      const committedYear = Math.round(nextYear);
      setDragging(false);
      onPreviewChange(committedYear);
      onChange(committedYear);
    }

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);
    return () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [dragging, onChange, onPreviewChange, payload, windowSize]);

  if (!payload) {
    return (
      <div className="h-14 rounded-2xl border border-white/10 bg-white/[0.035]" />
    );
  }

  // The scrubber represents the NAVIGABLE range: its left end is the leftmost the
  // window can start (familyMin). The extra buffer year columns rendered before it
  // live only in the gutter as context, so they are intentionally NOT part of the
  // rail — that way the thumb visually reaches the left end at the scroll limit.
  const min = scrollMinYear(payload);
  const max = latestWindowStart(payload, windowSize);
  const displayStart = clamp(previewStart ?? visibleStart, min, max);
  const displayEnd = visibleWindowEndYear(payload, displayStart, windowSize);
  const value = Math.round(displayStart);
  const fillStyle = windowRailStyle(payload, displayStart, windowSize);

  function handlePointerDown(event) {
    event.preventDefault();
    setDragging(true);
    onPreviewChange(yearFromClientX(payload, railRef.current, event.clientX, windowSize));
    event.currentTarget.focus();
  }

  function handleKeyDown(event) {
    const current = Math.round(displayStart);
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      onChange(current - 1);
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      onChange(current + 1);
    } else if (event.key === "Home") {
      event.preventDefault();
      onChange(min);
    } else if (event.key === "End") {
      event.preventDefault();
      onChange(max);
    }
  }

  return (
    <div className={`grid gap-3 md:grid-cols-[96px_minmax(0,1fr)_96px] md:items-center ${compact ? "text-[11px]" : ""}`}>
      <div className="font-mono text-[12px] font-black text-text-secondary">{min}</div>
      <div>
        <div
          ref={railRef}
          data-testid={railTestId}
          className="relative h-9"
        >
          <div className="absolute inset-x-0 top-[15px] h-1 rounded-full bg-white/12" />
          <div
            role="slider"
            tabIndex={0}
            aria-label={ariaLabel}
            aria-valuemin={min}
            aria-valuemax={max}
            aria-valuenow={value}
            onPointerDown={handlePointerDown}
            onKeyDown={handleKeyDown}
            className={`absolute top-[7px] h-5 cursor-grab rounded-full border border-status-green/50 bg-status-green/18 shadow-[0_0_22px_rgba(94,231,168,0.2)] outline-none transition-[box-shadow,border-color] focus:border-status-green active:cursor-grabbing ${
              dragging ? "shadow-[0_0_0_5px_rgba(94,231,168,0.12),0_0_26px_rgba(94,231,168,0.28)]" : ""
            }`}
            style={fillStyle}
          />
          <div className="pointer-events-none absolute inset-x-0 top-[1px] flex justify-between px-1 font-mono text-[8px] font-black uppercase tracking-[0.12em] text-text-muted">
            <span>{i18n.t("globalTeams.start")}</span>
            <span>{i18n.t("globalTeams.end")}</span>
          </div>
        </div>
        <p className="mt-1 text-center text-[10px] font-semibold uppercase tracking-[0.16em] text-status-green">
          {i18n.t("globalTeams.visibleWindow", { start: Math.round(displayStart), end: displayEnd })}
        </p>
      </div>
      <div className="text-right font-mono text-[12px] font-black text-text-secondary">{axisEndYear(payload)}</div>
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

const INLINE_TABLE_WIDTH_PCT = 25;

// One name/logo table per category — a "current standings" panel anchored to the
// RIGHT edge. Its left edge meets the year where the visible lines end, so the lines
// flow into it; the header + ranked rows sit in the leftmost `tableWidth`, and the
// panel fills from there to the right edge in the table colour. Because every active
// category ends at the same latest year, the panels line up cleanly on the right
// (unlike category STARTS, which are staggered). Rows + panel glide in sync with the
// lines while scrolling.
function InlineTeamTables({
  payload,
  geometry,
  gridStartYear,
  displayEndYear,
  windowSize = DEFAULT_WINDOW_SIZE,
  focusedTeamId,
  pinnedTeamId,
  onFocus,
  onTeamClick,
  onTeamDoubleClick,
}) {
  const bands = (payload?.bands ?? []).filter((band) => !band.is_special);
  const tableWidth = `${INLINE_TABLE_WIDTH_PCT}%`;
  const dataWidthPct = 100 - INLINE_TABLE_WIDTH_PCT;
  return (
    <div className="pointer-events-none absolute inset-0 z-30" data-testid="world-team-name-rail">
      {bands.map((band) => {
        const bandBox = geometry.bands[band.key];
        if (!bandBox) return null;
        const referenceYear = bandReferenceYearRight(band, displayEndYear);
        const hasStarted = Number.isFinite(referenceYear);
        const displayRows = hasStarted ? visibleBandRows(band.rows, referenceYear) : [];
        // Left edge of the section = the year-end boundary of the latest visible
        // standings (where the lines stop). Clamp keeps the table within the chart: it
        // pins to the right gutter once that boundary scrolls off the right edge.
        const endScreenPct = hasStarted && Number.isFinite(gridStartYear)
          ? ((referenceYear + 1 - gridStartYear) / windowSize) * dataWidthPct
          : dataWidthPct;
        const panelLeftPct = clamp(endScreenPct, 0, dataWidthPct);

        return (
          <div key={band.key} className="pointer-events-none absolute inset-0">
            <div
              aria-hidden="true"
              data-testid={`world-team-name-panel-${band.key}`}
              className="absolute rounded-l-2xl border-y border-l border-white/10 bg-[#0a1322] shadow-[0_18px_44px_rgba(0,0,0,0.4)]"
              style={{
                left: 0,
                top: CHART_HEADER_HEIGHT + bandBox.top + 4,
                // Full-width element whose LEFT edge is parked at the line-end column
                // via a GPU transform (same mechanism/timing as the lines), so it stays
                // in sync while scrolling — no width/layout animation, no blur.
                width: "100%",
                height: Math.max(bandBox.height - 8, 0),
                transform: `translate3d(${formatPercent(panelLeftPct)}%, 0, 0)`,
                transition: "transform 80ms linear",
                willChange: "transform",
              }}
            />
            <div
              className="pointer-events-none absolute inset-0"
              style={{
                transform: `translate3d(${formatPercent(panelLeftPct)}%, 0, 0)`,
                transition: "transform 80ms linear",
                willChange: "transform",
              }}
            >
              <span
                className="pointer-events-auto absolute z-10 grid h-6 place-items-center rounded-full border border-status-yellow/35 bg-[#0b1526] px-3 text-[9px] font-black uppercase tracking-[0.14em] text-status-yellow"
                style={{ left: 0, top: CHART_HEADER_HEIGHT + bandBox.top + 10, width: tableWidth }}
              >
                {hasStarted ? `${band.label} ${referenceYear}` : band.label}
              </span>

              {displayRows.length === 0 ? (
                <div
                  className="pointer-events-auto absolute z-10 grid h-10 place-items-center rounded-lg border border-dashed border-white/15 bg-[#07101d] px-3 text-center text-[10px] font-semibold leading-4 text-text-muted"
                  style={{ left: 0, top: CHART_HEADER_HEIGHT + bandBox.top + 40, width: tableWidth }}
                >
                  {`${band.label} ainda nao existia`}
                </div>
              ) : null}

              {displayRows.map((row) => {
              const { isFocused, isDimmed } = teamHighlight(row.team_id, focusedTeamId, pinnedTeamId);
              const displayPosition = rowPositionAtYear(row, referenceYear);
              const y = bandRowOffsetY(bandBox.top, displayPosition);
              const teamColor = getReadableWorldTeamColor(row.cor_primaria);
              return (
                <button
                  key={`${band.key}-${row.team_id}`}
                  type="button"
                  onMouseEnter={() => onFocus(row.team_id)}
                  onFocus={() => onFocus(row.team_id)}
                  onClick={() => onTeamClick({ ...row, band_key: band.key, band_category: band.category })}
                  onDoubleClick={() => onTeamDoubleClick({ ...row, band_key: band.key, band_category: band.category })}
                  data-testid={`world-team-row-${row.team_id}-${band.key}`}
                  className={`pointer-events-auto absolute z-10 grid grid-cols-[28px_20px_30px_minmax(0,1fr)_auto] items-center gap-2 rounded-md pr-2 text-left transition-opacity ${
                    isFocused ? "bg-white/[0.06]" : ""
                  } ${isDimmed ? "opacity-35" : "opacity-100"}`}
                  style={{ left: 0, top: CHART_HEADER_HEIGHT + y - ROW_PILL_HEIGHT / 2, height: ROW_PILL_HEIGHT, width: tableWidth, "--team-color": teamColor }}
                >
                  {/* Connector to the incoming line on the LEFT — solid where the line
                      meets the row (left edge), fading into the row. */}
                  <span
                    className="h-1 rounded-full"
                    style={{
                      background: `linear-gradient(90deg, ${teamColor}, transparent)`,
                    }}
                  />
                  <span className="text-center font-mono text-[11px] font-black text-text-secondary">
                    {displayPosition}
                  </span>
                  <TeamLogoMark
                    teamName={row.nome}
                    color={teamColor}
                    size="xs"
                    testId="world-team-logo"
                  />
                  <span className="min-w-0">
                    <span
                      className="block truncate text-xs font-black"
                      style={{ color: teamColor }}
                    >
                      {row.nome}
                    </span>
                  </span>
                  <span className="flex justify-end">
                    <TeamTrophies titles={row.titles} isReigning={row.is_reigning_champion} />
                  </span>
                </button>
              );
            })}
            </div>
          </div>
        );
      })}
    </div>
  );
}

// Um time fica "aceso" se está sob o mouse (focused) OU fixado em análise (pinned —
// o time em evidência ao abrir daqui). Assim o highlight do time selecionado NÃO some
// ao passar o mouse por outra linha; a linha sob o mouse acende junto, pra comparação.
function teamHighlight(teamId, focusedTeamId, pinnedTeamId) {
  const isFocused = teamId === focusedTeamId || teamId === pinnedTeamId;
  const anyActive = focusedTeamId != null || pinnedTeamId != null;
  return { isFocused, isDimmed: anyActive && !isFocused };
}

function TeamHistoryGrid({ payload, years, geometry, teamTracks, previewStartYear, visibleStartYear, windowSize = DEFAULT_WINDOW_SIZE, focusedTeamId, pinnedTeamId, onFocus, onTeamClick, onTeamDoubleClick }) {
  const gridStartYear = previewStartYear ?? visibleStartYear;
  const displayStartYear = roundedDisplayStartYear(payload, gridStartYear, windowSize);
  const displayEndYear = displayStartYear + windowSize - 1;
  const movingGridStyle = chartTimelineStyle(payload, years, gridStartYear, windowSize);
  const bandByKey = new Map((payload?.bands ?? []).map((band) => [band.key, band]));
  const firstDataYear = familyMinYear(payload);
  const lastDataYear = familyMaxYear(payload);
  // First plotted (in-axis) data year of each band, so a line born with its
  // category can be anchored to that year's start edge (see buildPath).
  const yearSet = new Set(years);
  const bandFirstYear = new Map(
    (payload?.bands ?? []).map((band) => {
      let min = null;
      band.rows?.forEach((row) =>
        row.points?.forEach((point) => {
          if (yearSet.has(point.year) && (min === null || point.year < min)) {
            min = point.year;
          }
        }),
      );
      return [band.key, min];
    }),
  );

  return (
    <div
      className="relative h-full overflow-hidden bg-[#07101d]"
      data-testid="world-team-grid"
      onMouseLeave={() => onFocus(null)}
      style={{ height: geometry.totalHeight }}
    >
      <div
        className="absolute bottom-0 left-0 top-0"
        data-testid="world-team-moving-grid"
        style={movingGridStyle}
      >
        {(() => {
          const lz = axisEdgeZoneStyle("left", payload, years);
          const rz = axisEdgeZoneStyle("right", payload, years);
          return (
            <>
              {lz && (
                <div
                  data-testid="world-team-axis-hatch-left"
                  className="pointer-events-none absolute z-0 border-r border-status-yellow/20"
                  style={{ ...lz, background: "repeating-linear-gradient(135deg,rgba(242,196,109,0.10) 0 8px,rgba(242,196,109,0.03) 8px 16px)" }}
                />
              )}
              {rz && (
                <div
                  data-testid="world-team-axis-hatch-right"
                  className="pointer-events-none absolute z-0 border-l border-status-yellow/20"
                  style={{ ...rz, background: "repeating-linear-gradient(135deg,rgba(242,196,109,0.10) 0 8px,rgba(242,196,109,0.03) 8px 16px)" }}
                />
              )}
            </>
          );
        })()}
        {/* Grade de anos: colunas alternadas (zebra) + separadores verticais em cada
            ano, atrás das linhas. Alinhada às colunas do gráfico e rola junto — deixa
            fácil ler a posição de um time ao longo dos anos sem o fundo confundir. */}
        <div
          aria-hidden="true"
          data-testid="world-team-year-grid"
          className="pointer-events-none absolute inset-x-0 bottom-0 z-0 grid"
          style={{ top: CHART_HEADER_HEIGHT, gridTemplateColumns: `repeat(${Math.max(years.length, 1)}, minmax(0, 1fr))` }}
        >
          {years.map((year, i) => (
            <div
              key={year}
              className="border-l border-white/[0.08]"
              style={i % 2 === 1 ? { background: "rgba(255,255,255,0.025)" } : undefined}
            />
          ))}
        </div>
        <div className="absolute inset-x-0 top-0 z-20 grid h-14 border-b border-white/10 bg-[#07101d]/90" style={{ gridTemplateColumns: `repeat(${years.length}, minmax(0, 1fr))` }}>
          {years.map((year) => (
            <div key={year} data-testid={`world-team-year-${year}`} className="grid place-items-center border-l border-white/8 text-center">
              <strong className="font-mono text-sm font-black leading-none text-text-primary">{year}</strong>
            </div>
          ))}
        </div>

        {(payload?.bands ?? []).map((band) => {
          const bandBox = geometry.bands[band.key];
          const isFutureBand = band.starts_year > (payload?.window_end ?? 0);
          const preStartStyle = bandPreStartStyle(band, bandBox, years, firstDataYear);
          const startDividerStyle = bandStartDividerStyle(band, bandBox, years);
          return (
            <div key={band.key}>
              {preStartStyle ? (
                <div
                  data-testid={`world-team-pre-start-${band.key}`}
                  data-start-year={band.starts_year}
                  className="pointer-events-none absolute z-[2] bg-[repeating-linear-gradient(135deg,rgba(139,148,158,0.10)_0_8px,rgba(139,148,158,0.03)_8px_16px)]"
                  style={preStartStyle}
                />
              ) : null}
              {startDividerStyle ? (
                <div
                  data-testid={`world-team-start-divider-${band.key}`}
                  data-start-year={band.starts_year}
                  className="pointer-events-none absolute z-[3] w-px bg-status-yellow/70 shadow-[0_0_18px_rgba(242,196,109,0.45)]"
                  style={startDividerStyle}
                />
              ) : null}
              <div
                className="absolute inset-x-0 z-10 h-1 bg-white/15"
                style={{ top: CHART_HEADER_HEIGHT + bandBox.top }}
              />
              {/* The band's name + start year is shown by its floating name-table
                  header (InlineTeamTables) and the yellow start divider; a second
                  in-grid pill here was redundant and got clipped at the gutter edge
                  when the grid scrolled, so it was removed. */}
              {isFutureBand ? (
                <div
                  className="absolute inset-x-0 z-[1] border-y border-dashed border-white/12 bg-[repeating-linear-gradient(135deg,rgba(139,148,158,0.08)_0_8px,rgba(139,148,158,0.02)_8px_16px)]"
                  style={{ top: CHART_HEADER_HEIGHT + bandBox.top, height: bandBox.height }}
                />
              ) : null}
            </div>
          );
        })}

        <svg
          className="absolute left-0 top-14 z-10 w-full"
          viewBox={`0 0 ${CHART_WIDTH} ${geometry.chartHeight}`}
          preserveAspectRatio="none"
          aria-hidden="true"
          style={{ height: geometry.chartHeight }}
        >
          {teamTracks.flatMap((track) => trackLineGroups(track).map((line) => {
              const d = buildPath(line, geometry, years, lastDataYear, bandFirstYear);
              if (!d) return null;
              const { isFocused, isDimmed } = teamHighlight(track.team_id, focusedTeamId, pinnedTeamId);
              return (
                <path
                  key={`${track.team_id}-${line.line_key}`}
                  data-testid={`world-team-track-${track.team_id}-${line.line_key}`}
                  d={d}
                  fill="none"
                  stroke={track.cor_primaria}
                  strokeWidth={line.line_key === "special" ? (isFocused ? 5 : 3) : (isFocused ? 4 : 2.4)}
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  vectorEffect="non-scaling-stroke"
                  opacity={isDimmed ? 0.15 : line.line_key === "special" ? 0.9 : 0.66}
                  pointerEvents="stroke"
                  className="cursor-pointer"
                  onMouseEnter={() => onFocus(track.team_id)}
                  onClick={() => onTeamClick(teamTrackToTeamRow(track, line.points?.[0]?.band_key, bandByKey))}
                />
              );
            }))}
          {teamTracks.flatMap((track) => teamMovementMarkers(track, geometry, years, lastDataYear, bandFirstYear).map((marker) => (
            <SpecialMovementMarker
              key={`${track.team_id}-${marker.type}-${marker.year}`}
              marker={marker}
              teamId={track.team_id}
              focusedTeamId={focusedTeamId}
              pinnedTeamId={pinnedTeamId}
              onFocus={onFocus}
            />
          )))}
        </svg>
        {teamTracks.flatMap((track) => teamEntryLabels(track, geometry, years, payload, displayStartYear).map((label) => (
          <TeamEntryLabel
            key={`${track.team_id}-${label.line_key}-${label.year}`}
            label={label}
            team={track}
            band={bandByKey.get(label.band_key)}
            focusedTeamId={focusedTeamId}
            pinnedTeamId={pinnedTeamId}
            onFocus={onFocus}
            onClick={onTeamClick}
          />
        )))}
      </div>
      <InlineTeamTables
        payload={payload}
        geometry={geometry}
        gridStartYear={gridStartYear}
        displayEndYear={displayEndYear}
        windowSize={windowSize}
        focusedTeamId={focusedTeamId}
        pinnedTeamId={pinnedTeamId}
        onFocus={onFocus}
        onTeamClick={onTeamClick}
        onTeamDoubleClick={onTeamDoubleClick}
      />
    </div>
  );
}

function TeamEntryLabel({ label, team, band, focusedTeamId, pinnedTeamId, onFocus, onClick }) {
  const { isDimmed } = teamHighlight(team.team_id, focusedTeamId, pinnedTeamId);
  return (
    <button
      type="button"
      data-testid={`world-team-entry-label-${team.team_id}-${label.line_key}-${label.year}`}
      className="absolute z-20 grid max-w-[236px] cursor-pointer grid-cols-[42px_minmax(0,1fr)] items-center gap-2.5 overflow-hidden rounded border bg-[#07101d]/85 px-2 py-1 text-left text-[10px] font-black leading-4 shadow-[0_8px_20px_rgba(0,0,0,0.28)] backdrop-blur-sm"
      onMouseEnter={() => onFocus(team.team_id)}
      onFocus={() => onFocus(team.team_id)}
      onClick={() => onClick(teamTrackToTeamRow(team, label.band_key, new Map([[label.band_key, band]])))}
      style={{
        left: `${formatPercent((label.anchorX / CHART_WIDTH) * 100)}%`,
        top: CHART_HEADER_HEIGHT + label.y - 13,
        width: label.width,
        transform: "translateX(calc(-100% - 8px))",
        color: team.cor_primaria,
        borderColor: `${team.cor_primaria}73`,
        opacity: isDimmed ? 0.16 : 0.92,
      }}
    >
      <span className="grid h-5 w-[42px] shrink-0 place-items-center overflow-hidden">
        <TeamLogoMark
          teamName={team.nome}
          color={team.cor_primaria}
          size="xs"
          testId="world-team-entry-logo"
        />
      </span>
      <span className="min-w-0 truncate">{team.nome}</span>
    </button>
  );
}

function SpecialMovementMarker({ marker, teamId, focusedTeamId, pinnedTeamId, onFocus }) {
  const isPromotion = marker.type === "promotion";
  const color = isPromotion ? "#5ee7a8" : "#ff5b57";
  const points = isPromotion ? "-2.4,1.6 0,-1.6 2.4,1.6" : "-2.4,-1.6 0,1.6 2.4,-1.6";
  const { isDimmed } = teamHighlight(teamId, focusedTeamId, pinnedTeamId);

  return (
    <g
      data-testid={`world-team-${marker.type}-${teamId}-${marker.year}`}
      data-band-key={marker.band_key}
      transform={`translate(${round(marker.x)} ${round(marker.y)})`}
      opacity={isDimmed ? 0.15 : 0.9}
      onMouseEnter={() => onFocus(teamId)}
      pointerEvents="visiblePainted"
    >
      <polyline
        points={points}
        fill="none"
        stroke={color}
        strokeWidth="1.1"
        strokeLinecap="round"
        strokeLinejoin="round"
        vectorEffect="non-scaling-stroke"
      />
    </g>
  );
}

function normalizePayload(payload) {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  return {
    ...payload,
    families: Array.isArray(payload.families) ? payload.families : [],
    bands: Array.isArray(payload.bands) ? payload.bands.map((band) => ({
      ...band,
      rows: Array.isArray(band.rows) ? band.rows.map((row) => ({
        ...row,
        points: Array.isArray(row.points) ? row.points : [],
      })) : [],
    })) : [],
  };
}

function bandPreStartStyle(band, bandBox, years, firstDataYear) {
  if (!bandBox || !years.length || !Number.isFinite(band?.starts_year)) {
    return null;
  }

  const gridFirstYear = years[0];
  // The per-band "did not exist yet" zone starts at the family's first data year,
  // NOT the axis start, so it sits to the RIGHT of the global yellow "no data"
  // frame instead of stacking on top of it (the stack read as a double-yellow
  // band). Everything left of familyMin is already the single global frame.
  const zoneStart = Math.max(
    Number.isFinite(firstDataYear) ? firstDataYear : gridFirstYear,
    gridFirstYear,
  );
  if (band.starts_year <= zoneStart) {
    return null;
  }

  // Both edges use the LEFT-edge (year-start) anchor — the same as the global
  // "no data" frame's right edge, the start divider, and the category-founding
  // lines — so the grey "did not exist yet" zone sits flush between the frame and
  // the divider, with no stray border line half a column past the divider.
  const leftPos = clamp(zoneStart - gridFirstYear, 0, years.length);
  const rightPos = clamp(band.starts_year - gridFirstYear, 0, years.length);
  if (rightPos <= leftPos) {
    return null;
  }

  return {
    left: `${formatPercent((leftPos / years.length) * 100)}%`,
    top: CHART_HEADER_HEIGHT + bandBox.top,
    width: `${formatPercent(((rightPos - leftPos) / years.length) * 100)}%`,
    height: bandBox.height,
  };
}

function bandStartDividerStyle(band, bandBox, years) {
  if (!bandBox || !years.length || !Number.isFinite(band?.starts_year)) {
    return null;
  }

  const firstYear = years[0];
  const lastYear = years[years.length - 1];
  if (band.starts_year <= firstYear || band.starts_year > lastYear) {
    return null;
  }

  return {
    left: `${formatPercent((clamp(bandStartPosition(band, firstYear), 0, years.length) / years.length) * 100)}%`,
    top: CHART_HEADER_HEIGHT + bandBox.top,
    height: bandBox.height,
  };
}

function bandStartPosition(band, firstYear) {
  // Glue the pre-start band and divider to the left edge of the start year's
  // column. The per-slot offset is only for spacing data points within a
  // year, not for the "category did not exist" boundary.
  return band.starts_year - firstYear;
}

function buildYears(payload, zoomYears = null) {
  if (!payload) return [];
  let start = axisStartYear(payload);
  let end = renderEndYear(payload);
  // Zoom: recorta a linha do tempo para os últimos N anos (start sobe até maxYear-N+1)
  // e encolhe a folga futura proporcionalmente, pra não render dezenas de colunas vazias.
  if (zoomYears != null && Number.isFinite(zoomYears)) {
    const maxYear = familyMaxYear(payload);
    if (Number.isFinite(maxYear)) {
      start = Math.max(start, maxYear - zoomYears + 1);
      end = Math.min(end, maxYear + Math.ceil(zoomYears / 3) + 1);
    }
  }
  if (!Number.isFinite(start) || !Number.isFinite(end) || start > end) return [];
  const years = [];
  for (let year = start; year <= end; year += 1) years.push(year);
  return years;
}

function buildGeometry(payload, years) {
  const bands = {};
  let cursor = 0;

  (payload?.bands ?? []).forEach((band) => {
    // Size each band to the largest championship position it ever holds — i.e.
    // the number of simultaneous slots in the grid — not the count of distinct
    // teams that passed through over the years. Lines and name rows are placed by
    // position, so this is exactly the number of rows the band needs.
    const maxPointPosition = Math.max(
      1,
      ...band.rows.flatMap((row) => row.points.map((point) => Math.max(point.position ?? 1, 1))),
    );
    const bandHeight = Math.max(
      MIN_BAND_HEIGHT,
      ROW_TOP_OFFSET + BAND_LABEL_HEIGHT + maxPointPosition * ROW_HEIGHT + BAND_BOTTOM_PADDING,
    );
    bands[band.key] = { top: cursor, height: bandHeight };
    cursor += bandHeight;
  });

  const chartHeight = Math.max(cursor, MIN_CHART_HEIGHT);
  return { bands, yearCount: years.length, chartHeight, totalHeight: CHART_HEADER_HEIGHT + chartHeight };
}

// Vertical offset (within the chart, excluding the year header) of a given
// championship position inside its band.
function bandRowOffsetY(bandTop, position) {
  return bandTop + ROW_TOP_OFFSET + BAND_LABEL_HEIGHT + (Math.max(position ?? 1, 1) - 1) * ROW_HEIGHT;
}

function sortedBandRows(rows, displayStartYear) {
  return [...(rows ?? [])].sort((left, right) => {
    const leftPosition = rowSortPosition(left, displayStartYear);
    const rightPosition = rowSortPosition(right, displayStartYear);
    return leftPosition - rightPosition || String(left.nome).localeCompare(String(right.nome));
  });
}

function visibleBandRows(rows, displayStartYear) {
  return sortedBandRows(
    (rows ?? []).filter((row) => Number.isFinite(rowPositionAtYear(row, displayStartYear))),
    displayStartYear,
  );
}

function bandReferenceYear(band, displayStartYear, windowEnd) {
  if (!Number.isFinite(band?.starts_year)) {
    return displayStartYear;
  }
  if (band.starts_year > displayStartYear && band.starts_year <= windowEnd) {
    return band.starts_year;
  }
  return displayStartYear;
}

// Year whose standings the RIGHT-hand table shows: the band's most recent data year
// at or before the rightmost visible year. Returns null when the band has no data
// yet (it has not started by the rightmost visible year).
function bandReferenceYearRight(band, displayEndYear) {
  let latest = null;
  (band?.rows ?? []).forEach((row) =>
    (row.points ?? []).forEach((point) => {
      if (point.year <= displayEndYear && (latest === null || point.year > latest)) {
        latest = point.year;
      }
    }),
  );
  return latest;
}

function rowSortPosition(row, displayStartYear) {
  const exactPosition = rowPositionAtYear(row, displayStartYear);
  if (Number.isFinite(exactPosition)) {
    return exactPosition;
  }

  const nextPoint = [...(row.points ?? [])]
    .filter((point) => point.year > displayStartYear)
    .sort((left, right) => left.year - right.year || left.position - right.position)[0];
  if (nextPoint) {
    return 1000 + nextPoint.year - displayStartYear + nextPoint.position / 100;
  }

  const previousPoint = [...(row.points ?? [])]
    .filter((point) => point.year < displayStartYear)
    .sort((left, right) => right.year - left.year || left.position - right.position)[0];
  if (previousPoint) {
    return 2000 + displayStartYear - previousPoint.year + previousPoint.position / 100;
  }

  return 3000 + Math.max(row.base_position ?? 999, 1);
}

function rowPositionAtYear(row, year) {
  const point = (row.points ?? []).find((item) => item.year === year);
  return point ? Math.max(point.position ?? 1, 1) : null;
}

function buildTeamTracks(payload, geometry, years) {
  const tracks = new Map();
  (payload?.bands ?? []).forEach((band) => {
    band.rows.forEach((row) => {
      if (!tracks.has(row.team_id)) {
        tracks.set(row.team_id, {
          team_id: row.team_id,
          nome: row.nome,
          cor_primaria: getReadableWorldTeamColor(row.cor_primaria),
          points: [],
        });
      }
      const track = tracks.get(row.team_id);
      row.points.forEach((point) => {
        if (!years.includes(point.year)) return;
        track.points.push({
          ...point,
          band_key: band.key,
          team_id: row.team_id,
        });
      });
    });
  });

  return Array.from(tracks.values())
    .map((track) => ({
      ...track,
      points: track.points.sort((left, right) => {
        return left.year - right.year || slotOrder(left.slot) - slotOrder(right.slot);
      }),
    }))
    .filter((track) => track.points.some((point) => Number.isFinite(pointY(point, geometry))));
}

function trackLineGroups(track) {
  return ["regular", "special"]
    .map((lineKey) => ({
      ...track,
      line_key: lineKey,
      points: track.points.filter((point) => point.slot === lineKey),
    }))
    .filter((line) => line.points.length > 0);
}

function teamMovementMarkers(track, geometry, years, lastDataYear, bandFirstYear) {
  const points = [...(track.points ?? [])].sort((left, right) => left.year - right.year);
  const markers = [];

  points.forEach((point, index) => {
    const nextPoint = points[index + 1];
    if (!nextPoint || point.band_key === nextPoint.band_key || nextPoint.year !== point.year + 1) {
      return;
    }
    if (!years.includes(point.year)) return;

    const currentY = pointY(point, geometry);
    const nextY = pointY(nextPoint, geometry);
    if (!Number.isFinite(currentY) || !Number.isFinite(nextY)) return;

    // Match the line vertex anchor so the arrow sits exactly on the path.
    const x = anchoredPointX(point, years);

    if (nextY < currentY) {
      markers.push({
        type: "promotion",
        year: point.year,
        band_key: point.band_key,
        x,
        y: currentY - 6,
      });
    } else if (nextY > currentY) {
      markers.push({
        type: "demotion",
        year: point.year,
        band_key: point.band_key,
        x,
        y: currentY + 6,
      });
    }
  });

  return markers;
}

function teamEntryLabels(track, geometry, years, payload, displayStartYear) {
  const bandMap = new Map((payload?.bands ?? []).map((band) => [band.key, band]));
  return trackLineGroups(track)
    .map((line) => {
      const firstPoint = line.points?.[0];
      if (!firstPoint || !years.includes(firstPoint.year)) return null;
      const band = bandMap.get(firstPoint.band_key);
      if (band?.is_special) return null;
      // Label every team whose line is BORN inside the visible window (its debut
      // falls after the leftmost visible year), regardless of whether the band's
      // data starts on its official start year — so founding rosters are shown at
      // every category's birth, consistently across all divisions.
      if (firstPoint.year <= displayStartYear) return null;
      const pointYValue = pointY(firstPoint, geometry);
      if (!Number.isFinite(pointYValue)) return null;
      const width = clamp(track.nome.length * 7.2 + 70, 124, 236);
      const anchorX = clamp(pointX(firstPoint, years), 16, CHART_WIDTH - 6);
      const y = pointYValue - 9;
      return {
        line_key: line.line_key,
        band_key: firstPoint.band_key,
        year: firstPoint.year,
        anchorX,
        y,
        width,
      };
    })
    .filter(Boolean);
}

function buildPath(track, geometry, years, lastDataYear, bandFirstYear) {
  if (!track.points?.length || !years.length) return "";
  // Diagonal reta entre 2 ÂNCORAS: início do ano (posição daquele ano) → início do
  // ano seguinte (nova posição). Sem platô horizontal — a linha sobe/desce direto de
  // um começo de ano ao outro. A última temporada segura até a borda direita da sua
  // coluna, só pra encostar na tabela/hachura do fim.
  const yearCount = Math.max(years.length, 1);
  const anchors = [];
  track.points.forEach((point) => {
    const yearIndex = years.indexOf(point.year);
    if (yearIndex < 0) return;
    const y = pointY(point, geometry);
    if (!Number.isFinite(y)) return;
    anchors.push({
      leftX: (yearIndex / yearCount) * CHART_WIDTH,
      rightX: ((yearIndex + 1) / yearCount) * CHART_WIDTH,
      y,
    });
  });
  if (anchors.length === 0) return "";
  const coordinates = anchors.map((a) => [a.leftX, a.y]);
  const last = anchors[anchors.length - 1];
  coordinates.push([last.rightX, last.y]);
  return coordinates
    .map(([x, y], index) => `${index === 0 ? "M" : "L"} ${round(x)} ${round(y)}`)
    .join(" ");
}

function pointX(point, years) {
  const yearIndex = years.indexOf(point.year);
  if (yearIndex < 0) return NaN;
  return ((yearIndex + 0.5) / years.length) * CHART_WIDTH;
}

// Âncora X de um ponto = borda ESQUERDA da coluna (início do ano), igual aos vértices
// da linha (2 âncoras: início do ano → início do ano seguinte). Marcadores de
// promoção/rebaixamento usam a mesma âncora pra ficar exatamente sobre a linha.
function anchoredPointX(point, years) {
  const yearIndex = years.indexOf(point.year);
  if (yearIndex < 0) return NaN;
  return (yearIndex / Math.max(years.length, 1)) * CHART_WIDTH;
}

function pointY(point, geometry) {
  const bandBox = geometry.bands[point.band_key];
  if (!bandBox) return NaN;
  return bandRowOffsetY(bandBox.top, point.position);
}

function pointRowY(point, geometry) {
  return pointY(point, geometry);
}

function slotOrder(slot) {
  return slot === "special" ? 2 : 1;
}


function flattenTeams(payload) {
  const rows = [];
  (payload?.bands ?? []).forEach((band) => {
    band.rows.forEach((row) => rows.push({ ...row, band_category: band.category }));
  });
  return rows;
}

function teamRowToTeam(row) {
  return {
    id: row.team_id ?? row.id,
    nome: row.nome,
    nome_curto: row.nome_curto ?? row.nome,
    cor_primaria: row.cor_primaria,
    cor_secundaria: row.cor_secundaria,
    categoria: row.band_category ?? row.category ?? "",
    posicao: row.base_position ?? row.posicao,
    pontos: row.points?.[0]?.points ?? row.pontos ?? 0,
    vitorias: row.points?.[0]?.wins ?? row.vitorias ?? 0,
  };
}

function teamTrackToTeamRow(track, bandKey, bandByKey) {
  const band = bandByKey?.get?.(bandKey);
  return {
    ...track,
    band_key: bandKey,
    band_category: band?.category ?? track.band_category ?? "",
    category: band?.category ?? track.category ?? "",
    class_name: band?.class_name ?? track.class_name ?? null,
  };
}

function teamToTeamRow(team, fallback) {
  return {
    ...fallback,
    team_id: team.id,
    nome: team.nome,
    nome_curto: team.nome_curto,
    cor_primaria: team.cor_primaria,
    cor_secundaria: team.cor_secundaria,
    base_position: team.posicao ?? fallback.base_position,
  };
}

function familyFromTeamContext(category, className) {
  const categoryId = normalizeCategoryId(category);
  const classId = normalizeCategoryId(className);

  if (categoryId.includes("toyota") || classId === "toyota") {
    return "toyota";
  }
  if (categoryId.includes("bmw") || classId === "bmw") {
    return "bmw";
  }
  if (categoryId.includes("gt4") || classId === "gt4") {
    return "gt4";
  }
  if (categoryId.includes("gt3") || classId === "gt3") {
    return "gt3";
  }
  if (categoryId.includes("lmp2") || classId === "lmp2") {
    return "lmp2";
  }
  if (categoryId.includes("mazda") || classId === "mazda") {
    return "mazda";
  }
  if (categoryId === "production_challenger") {
    if (["toyota", "bmw", "mazda"].includes(classId)) {
      return classId;
    }
    return "mazda";
  }
  if (categoryId === "endurance") {
    if (["gt4", "gt3"].includes(classId)) {
      return classId;
    }
    return "gt3";
  }

  return DEFAULT_FAMILY;
}

function normalizeCategoryId(value) {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

function getReadableWorldTeamColor(color) {
  if (!color || !/^#([0-9a-f]{6})$/i.test(color)) {
    return "#7d8590";
  }

  const hex = color.slice(1);
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  const luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;

  if (luminance >= 0.32) {
    return color;
  }

  const mixWithWhite = 0.64;
  const boost = (channel) => Math.round(channel + (255 - channel) * mixWithWhite);
  return `rgb(${boost(r)}, ${boost(g)}, ${boost(b)})`;
}

// Earliest year the timeline may scroll to for the selected family: the oldest
// category start among its bands (no point scrolling before any of them raced),
// floored at the data's min year.
function familyMinYear(payload) {
  const starts = (payload?.bands ?? [])
    .map((band) => band.starts_year)
    .filter((year) => Number.isFinite(year));
  const dataMin = payload?.min_year ?? DEFAULT_START_YEAR;
  return starts.length ? Math.max(dataMin, Math.min(...starts)) : dataMin;
}

function windowRailStyle(payload, displayStart = payload?.window_start, windowSize = payload?.window_size ?? DEFAULT_WINDOW_SIZE) {
  if (!payload) return { left: "0%", width: "20%" };
  // Map the fill over the NAVIGABLE range [scrollMin, axisEnd], not the full
  // rendered axis, so the thumb's left edge reaches the rail's left end exactly at
  // the leftmost scroll position (no dead segment before it).
  const railStart = scrollMinYear(payload);
  const axisEnd = axisEndYear(payload);
  const total = Math.max(axisEnd - railStart + 1, 1);
  const left = ((displayStart - railStart) / total) * 100;
  const width = (windowSize / total) * 100;
  return {
    left: `${clamp(left, 0, 100)}%`,
    width: `${clamp(width, 3, 100)}%`,
  };
}

function chartTimelineStyle(payload, years, displayStartYear, visibleWindowSize) {
  if (!payload || !years.length || !Number.isFinite(displayStartYear)) {
    return { transform: "translate3d(0%, 0, 0)" };
  }
  // Reserve a RIGHT gutter for the standings table so the data area is drawn to its
  // left (not hidden behind it). The visible window spans [0, 100% - gutter].
  const gutter = INLINE_TABLE_WIDTH_PCT / 100;
  const widthPercent = round((1 - gutter) * (years.length / visibleWindowSize) * 100);
  const offsetYears = displayStartYear - years[0];
  const offsetPercent = -(offsetYears / years.length) * 100;
  return {
    left: "0%",
    width: `${widthPercent}%`,
    transform: `translate3d(${round(offsetPercent)}%, 0, 0)`,
    transition: "transform 80ms linear",
    willChange: "transform",
  };
}

function latestWindowStart(payload, windowSize = payload?.window_size ?? DEFAULT_WINDOW_SIZE) {
  return Math.max(scrollMinYear(payload), axisEndYear(payload) - windowSize + 1);
}

function clampVisibleStart(payload, startYear, windowSize = DEFAULT_WINDOW_SIZE) {
  if (!payload) return startYear;
  return clamp(startYear, scrollMinYear(payload), latestWindowStart(payload, windowSize));
}

function roundedDisplayStartYear(payload, value, windowSize = DEFAULT_WINDOW_SIZE) {
  if (!payload || !Number.isFinite(value)) return value;
  return Math.round(clamp(value, scrollMinYear(payload), latestWindowStart(payload, windowSize)));
}

function visibleWindowEndYear(payload, startYear, windowSize = DEFAULT_WINDOW_SIZE) {
  if (!payload || !Number.isFinite(startYear)) return null;
  const unclampedEnd = Math.round(startYear) + windowSize - 1;
  return Math.min(unclampedEnd, axisEndYear(payload));
}

function atlasMaxYear(payload) {
  return payload?.window_end ?? payload?.max_year ?? DEFAULT_START_YEAR;
}

// Leftmost RENDERED year: the past margin before the earliest category start, which
// is where the founding-team labels live.
function axisStartYear(payload) {
  return familyMinYear(payload) - pastMargin(payload);
}

// Leftmost year the window may start — same as the rendered start (the window is
// locked to the full range, so this equals the only window-start position).
function scrollMinYear(payload) {
  return familyMinYear(payload) - pastMargin(payload);
}

// Rightmost NAVIGABLE year: the last season itself. The window can't be scrolled
// past it — its right edge stops at the standings table, never sliding into the
// empty future beyond the "trophies".
function axisEndYear(payload) {
  return familyMaxYear(payload);
}

// Rightmost year RENDERED as a column. Extends past the navigable end just enough to
// keep the year header filled across the right gutter (under the table). These extra
// columns are header context only — the scrubber/scroll still stop at axisEndYear.
function renderEndYear(payload) {
  return axisEndYear(payload) + Math.ceil(visibleWindowSize(payload) / 3) + 1;
}

function familyMaxYear(payload) {
  let max = null;
  (payload?.bands ?? []).forEach((band) => {
    (band.rows ?? []).forEach((row) => {
      (row.points ?? []).forEach((point) => {
        if (max === null || point.year > max) max = point.year;
      });
    });
  });
  return max ?? atlasMaxYear(payload);
}

function familyDataWidth(payload) {
  if (!payload) return DEFAULT_WINDOW_SIZE;
  const min = familyMinYear(payload);
  const max = familyMaxYear(payload);
  if (!Number.isFinite(min) || !Number.isFinite(max)) return DEFAULT_WINDOW_SIZE;
  return Math.max(1, max - min + 1);
}

// Empty years rendered before the first category. The founding-team labels sit in
// this margin, so it must be wide enough that their names aren't cut — wider
// divisions (more years on screen → narrower columns) need more years of margin to
// give the labels the same pixel room. Clamped to [2, 7] so a huge future span
// doesn't waste the (capped) window.
function pastMargin(payload) {
  return clamp(Math.ceil(familyDataWidth(payload) / 5), 2, 7);
}

// Width of the visible window. Normally the whole data span PLUS the margin, so the
// chart shows the start (founding labels) AND the latest standings at once with no
// scrolling. But it is CAPPED at MAX_VISIBLE_SPAN: once a family accumulates more
// years than that, the window stops growing and the chart starts scrolling instead
// of cramming everything in (which would distort the image).
function visibleWindowSize(payload) {
  return Math.min(familyDataWidth(payload) + pastMargin(payload), MAX_VISIBLE_SPAN);
}

function axisEdgeZoneStyle(side, payload, years) {
  if (!payload || !years.length) return null;
  const firstDataYear = familyMinYear(payload);
  const lastDataYear = familyMaxYear(payload);
  if (side === "left") {
    const colCount = firstDataYear - years[0];
    if (colCount <= 0) return null;
    // Right edge at the LEFT edge (year-start) of the first data year — the same
    // anchor as the start divider and the category-founding lines — so the frame
    // ends exactly at the divider with no half-column overshoot into the debut year.
    return { top: 0, left: 0, bottom: 0, width: `${formatPercent((colCount / years.length) * 100)}%` };
  }
  const startColIndex = lastDataYear - years[0] + 1;
  if (startColIndex > years.length) return null;
  // Start the "no championship" zone at the boundary AFTER the last completed
  // season (the right edge of its column), not at its mid-column data point, so
  // the final season reads as a full, finished year instead of being cut in half.
  return { top: 0, left: `${formatPercent((startColIndex / years.length) * 100)}%`, right: 0, bottom: 0 };
}

function yearFromClientX(payload, railElement, clientX, windowSize = DEFAULT_WINDOW_SIZE) {
  const dragMin = scrollMinYear(payload);
  const latestStart = latestWindowStart(payload, windowSize);
  const rect = railElement?.getBoundingClientRect();
  if (!rect || rect.width <= 0) return dragMin;
  const progress = clamp((clientX - rect.left) / rect.width, 0, 1);
  return dragMin + progress * (latestStart - dragMin);
}

/**
 * Renders all-time constructor title badges for a team row.
 * Each badge shows a band-specific trophy image (falling back to the gold trophy)
 * and the count when > 1.  The reigning champion gets a star prefix.
 * Returns null when the team has no titles, leaving the cell empty.
 */
function TeamTrophies({ titles, isReigning }) {
  if (!titles || titles.length === 0) return null;
  return (
    // aria-hidden: trophy icons are decorative supplements to the team name; they
    // must not pollute the button's accessible name and confuse screen-reader
    // queries that look for the family-filter buttons by name.
    <span aria-hidden="true" className="flex items-center gap-0.5">
      {isReigning && (
        <span
          className="text-[9px] font-black leading-none text-yellow-400"
          title={i18n.t("globalTeams.currentChampion")}
        >
          ★
        </span>
      )}
      {titles.map((tc) => {
        const src = BAND_TROPHY_IMAGES[tc.band_key] ?? goldTrophy;
        return (
          <span
            key={tc.band_key}
            className="inline-flex items-center gap-px"
            title={`${tc.band_label}: ${tc.count}× campeão`}
          >
            <img
              src={src}
              alt={tc.band_label}
              className="h-3 w-3 object-contain drop-shadow-[0_0_4px_rgba(255,215,0,0.4)]"
              onError={(e) => {
                e.currentTarget.src = goldTrophy;
              }}
            />
            {tc.count > 1 && (
              <span
                data-testid="team-trophy-count"
                className="font-mono text-[9px] font-black leading-none text-text-secondary"
              >
                {tc.count}
              </span>
            )}
          </span>
        );
      })}
    </span>
  );
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function round(value) {
  return Math.round(value * 10) / 10;
}

function formatPercent(value) {
  return String(Math.round(value * 10000) / 10000);
}

export default GlobalTeamsTab;
