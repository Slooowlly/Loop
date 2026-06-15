import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import TeamLogoMark from "../../components/team/TeamLogoMark";
import GlassCard from "../../components/ui/GlassCard";
import useCareerStore from "../../stores/useCareerStore";
import { TeamHistoryDrawer } from "./MyTeamTab";

const DEFAULT_FAMILY = "mazda";
const DEFAULT_WINDOW_SIZE = 20;
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

function GlobalTeamsTab({
  selectedTeamId = null,
  selectedTeamCategory = null,
  selectedTeamClassName = null,
  onBack,
}) {
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const [family, setFamily] = useState(() => familyFromTeamContext(selectedTeamCategory, selectedTeamClassName));
  const [startYear, setStartYear] = useState(HISTORY_FETCH_START_YEAR);
  const windowSize = DEFAULT_WINDOW_SIZE;
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
        setError("Carreira nao carregada.");
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
        setError(typeof invokeError === "string" ? invokeError : "Nao foi possivel carregar o historico mundial de equipes.");
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
    setPreviewStartYear(null);
  }, [payload?.window_start, payload?.selected_family]);

  const visibleStartYear = useMemo(() => clampVisibleStart(payload, startYear, windowSize), [payload, startYear, windowSize]);
  const displayStartYear = useMemo(
    () => roundedDisplayStartYear(payload, previewStartYear ?? visibleStartYear, windowSize),
    [payload, previewStartYear, visibleStartYear, windowSize],
  );
  const visibleEndYear = useMemo(
    () => visibleWindowEndYear(payload, visibleStartYear, windowSize),
    [payload, visibleStartYear, windowSize],
  );
  const years = useMemo(() => buildYears(payload), [payload]);
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
    setStartYear(clamp(nextYear, familyMinYear(payload), latestStart));
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
          <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">Equipes mundiais</p>
          <h2 className="mt-2 text-3xl font-semibold text-text-primary">Histórico mundial de equipes</h2>
        </div>
        <button
          type="button"
          onClick={onBack}
          className="rounded-2xl border border-white/10 bg-white/[0.04] px-4 py-2 text-xs font-semibold uppercase tracking-[0.16em] text-text-secondary transition-glass hover:border-accent-primary/40 hover:bg-accent-primary/10 hover:text-text-primary"
        >
          Voltar para Classificacao
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
            <p className="text-[10px] uppercase tracking-[0.24em] text-accent-primary">Atlas histórico</p>
            <h3 className="mt-2 text-2xl font-semibold text-text-primary">
              {activeFamily?.label ?? "Mazda"}: janela {visibleStartYear ?? "-"}-{visibleEndYear ?? "-"}
            </h3>
          </div>
          <div className="flex flex-wrap justify-end gap-2">
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

        <div className="border-b border-white/10 bg-white/[0.025] px-5 py-4">
          <YearWindowScrubber
            payload={payload}
            visibleStart={visibleStartYear}
            previewStart={previewStartYear}
            windowSize={windowSize}
            onPreviewChange={setPreviewStartYear}
            onChange={handleWindowStartChange}
          />
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
              onFocus={setFocusedTeamId}
              onTeamClick={handleTeamClick}
              onTeamDoubleClick={handleTeamDoubleClick}
            />
          </div>
        </div>

        <div className="sticky bottom-0 z-40 border-t border-white/10 bg-[#07101d]/95 px-5 py-3 shadow-[0_-18px_36px_rgba(0,0,0,0.32)] backdrop-blur-xl">
          <YearWindowScrubber
            payload={payload}
            visibleStart={visibleStartYear}
            previewStart={previewStartYear}
            windowSize={windowSize}
            onPreviewChange={setPreviewStartYear}
            onChange={handleWindowStartChange}
            ariaLabel="Mover janela historica inferior"
            railTestId="world-team-window-scrubber-bottom"
            compact
          />
        </div>
      </GlassCard>

      {selectedTeam ? (
        <TeamHistoryDrawer
          careerId={careerId}
          team={teamRowToTeam(selectedTeam)}
          teams={allTeams.map(teamRowToTeam)}
          playerTeam={playerTeam}
          activeCategory={selectedTeam.category ?? selectedTeam.points?.[0]?.category ?? selectedTeam.band_category ?? ""}
          activeTab={activeHistoryTab}
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
  ariaLabel = "Mover janela historica",
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

  const min = familyMinYear(payload);
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
            <span>inicio</span>
            <span>fim</span>
          </div>
        </div>
        <p className="mt-1 text-center text-[10px] font-semibold uppercase tracking-[0.16em] text-status-green">
          Janela visivel: {Math.round(displayStart)}-{displayEnd}
        </p>
      </div>
      <div className="text-right font-mono text-[12px] font-black text-text-secondary">{atlasMaxYear(payload)}</div>
    </div>
  );
}

function GlobalTeamsLoading({ onBack }) {
  return (
    <div className="space-y-5">
      <header className="flex flex-wrap items-start justify-between gap-4 px-1">
        <div>
          <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">Equipes mundiais</p>
          <h2 className="mt-2 text-3xl font-semibold text-text-primary">Montando histórico mundial de equipes</h2>
        </div>
        <button
          type="button"
          onClick={onBack}
          className="rounded-2xl border border-white/10 bg-white/[0.04] px-4 py-2 text-xs font-semibold uppercase tracking-[0.16em] text-text-secondary"
        >
          Voltar para Classificacao
        </button>
      </header>
      <GlassCard hover={false} className="rounded-[30px]">
        <div className="h-96 animate-pulse rounded-[24px] border border-white/8 bg-white/[0.035]" />
      </GlassCard>
    </div>
  );
}

const INLINE_TABLE_WIDTH_PCT = 25;

// Name/logo tables that mark the start of each category's lines.
//
// Rendered in viewport space (over the chart, not inside the translated moving
// grid) so the table can pin to the left edge and stay readable when the user
// advances past a category's start year. Each category's table sits at left: 0
// and is moved horizontally with a GPU `transform: translateX` that mirrors the
// moving grid's transform transition, so it glides in perfect sync with the
// lines (no per-element layout/`left` animation, which was janky). The translate
// is clamped so the table's right edge meets the first line when the start is in
// view, and pins to the left edge once the start scrolls off-screen.
function InlineTeamTables({
  payload,
  geometry,
  gridStartYear,
  displayStartYear,
  windowSize = DEFAULT_WINDOW_SIZE,
  focusedTeamId,
  onFocus,
  onTeamClick,
  onTeamDoubleClick,
}) {
  const windowEnd = payload?.window_end ?? displayStartYear;
  return (
    <div className="pointer-events-none absolute inset-0 z-30" data-testid="world-team-name-rail">
      {(payload?.bands ?? []).filter((band) => !band.is_special).map((band) => {
        const bandBox = geometry.bands[band.key];
        if (!bandBox) return null;
        const isFutureBand = band.starts_year > windowEnd;
        const startsInsideWindow = band.starts_year > (payload?.window_start ?? 0);
        const referenceYear = bandReferenceYear(band, displayStartYear, windowEnd);
        const displayRows = visibleBandRows(band.rows, referenceYear);
        // Where the category start sits horizontally, as a % of the viewport.
        // The data area is offset by the left gutter (the table width), so the
        // first visible year sits at the gutter edge and the table's right edge
        // meets it.
        const startScreenPct = Number.isFinite(band.starts_year) && Number.isFinite(gridStartYear)
          ? INLINE_TABLE_WIDTH_PCT + ((band.starts_year - gridStartYear) / windowSize) * (100 - INLINE_TABLE_WIDTH_PCT)
          : INLINE_TABLE_WIDTH_PCT;
        // Table left edge, in viewport %, so the right edge meets the start;
        // clamp pins it to the left edge once the start scrolls off-screen and to
        // the right edge for not-yet-existing categories. Applied as a translate.
        const tableLeftPct = clamp(startScreenPct - INLINE_TABLE_WIDTH_PCT, 0, 100 - INLINE_TABLE_WIDTH_PCT);
        const tableWidth = `${INLINE_TABLE_WIDTH_PCT}%`;

        const rowYs = displayRows.map(
          (row) => bandRowOffsetY(bandBox.top, rowPositionAtYear(row, referenceYear)),
        );
        const minRowY = rowYs.length ? Math.min(...rowYs) : bandBox.top + ROW_TOP_OFFSET;
        const maxRowY = rowYs.length ? Math.max(...rowYs) : minRowY;

        return (
          <div
            key={band.key}
            className="pointer-events-none absolute inset-0"
            style={{
              transform: `translate3d(${formatPercent(tableLeftPct)}%, 0, 0)`,
              transition: "transform 80ms linear",
              willChange: "transform",
            }}
          >
            <span
              className={`pointer-events-auto absolute z-10 grid h-6 place-items-center rounded-full border px-3 text-[9px] font-black uppercase tracking-[0.14em] ${
                isFutureBand
                  ? "border-white/12 bg-[#07101d] text-text-muted"
                  : "border-status-yellow/35 bg-[#0a1322] text-status-yellow"
              }`}
              style={{ left: 0, top: CHART_HEADER_HEIGHT + bandBox.top + 10, width: tableWidth }}
            >
              {startsInsideWindow ? `${band.label} ${band.starts_year}` : band.label}
            </span>

            {displayRows.length === 0 ? (
              <div
                className="pointer-events-auto absolute z-10 grid h-10 place-items-center rounded-lg border border-dashed border-white/15 bg-[#07101d] px-3 text-center text-[10px] font-semibold leading-4 text-text-muted"
                style={{ left: 0, top: CHART_HEADER_HEIGHT + bandBox.top + 40, width: tableWidth }}
              >
                {isFutureBand || band.starts_year > referenceYear ? `${band.label} ainda nao existia` : "Sem equipes neste ano"}
              </div>
            ) : (
              <div
                aria-hidden="true"
                className="absolute rounded-lg border border-white/10 bg-[#0a1322]"
                style={{
                  left: 0,
                  top: CHART_HEADER_HEIGHT + minRowY - ROW_PILL_HEIGHT / 2,
                  width: tableWidth,
                  height: maxRowY - minRowY + ROW_PILL_HEIGHT,
                }}
              />
            )}

            {displayRows.map((row) => {
              const isFocused = focusedTeamId === row.team_id;
              const isDimmed = focusedTeamId && !isFocused;
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
                  className={`pointer-events-auto absolute z-10 grid grid-cols-[20px_30px_minmax(0,1fr)_30px_28px] items-center gap-2 rounded-md px-2 text-left transition-opacity ${
                    isFocused ? "bg-white/[0.06]" : ""
                  } ${isDimmed ? "opacity-35" : "opacity-100"}`}
                  style={{ left: 0, top: CHART_HEADER_HEIGHT + y - ROW_PILL_HEIGHT / 2, height: ROW_PILL_HEIGHT, width: tableWidth, "--team-color": teamColor }}
                >
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
                  <span className="text-right font-mono text-[11px] font-black" style={{ color: teamColor }}>
                    {formatDelta(row.delta)}
                  </span>
                  <span
                    className="h-1 rounded-full"
                    style={{
                      background: `linear-gradient(90deg, ${teamColor}, transparent)`,
                    }}
                  />
                </button>
              );
            })}
          </div>
        );
      })}
    </div>
  );
}

function TeamHistoryGrid({ payload, years, geometry, teamTracks, previewStartYear, visibleStartYear, windowSize = DEFAULT_WINDOW_SIZE, focusedTeamId, onFocus, onTeamClick, onTeamDoubleClick }) {
  const gridStartYear = previewStartYear ?? visibleStartYear;
  const displayStartYear = roundedDisplayStartYear(payload, gridStartYear, windowSize);
  const movingGridStyle = chartTimelineStyle(payload, years, gridStartYear, windowSize);
  const bandByKey = new Map((payload?.bands ?? []).map((band) => [band.key, band]));

  return (
    <div
      className="relative h-full overflow-hidden bg-[#07101d]"
      data-testid="world-team-grid"
      onMouseLeave={() => onFocus(null)}
      style={{
        height: geometry.totalHeight,
        backgroundImage:
          "linear-gradient(90deg, rgba(255,255,255,0.075) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,0.035) 1px, transparent 1px), linear-gradient(180deg, rgba(255,255,255,0.035) 1px, transparent 1px)",
        backgroundSize: `calc(100% / ${Math.max(years.length, 1)}) 100%, calc(100% / ${Math.max(years.length, 1) * 2}) 100%, 100% ${ROW_HEIGHT}px`,
        backgroundPosition: `0 0, calc(100% / ${Math.max(years.length, 1) * 4}) 0, 0 ${CHART_HEADER_HEIGHT}px`,
      }}
    >
      <div
        className="absolute bottom-0 left-0 top-0"
        data-testid="world-team-moving-grid"
        style={movingGridStyle}
      >
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
          const startsInsideWindow = band.starts_year > (payload?.window_start ?? 0);
          const preStartStyle = bandPreStartStyle(band, bandBox, years);
          const startDividerStyle = bandStartDividerStyle(band, bandBox, years);
          return (
            <div key={band.key}>
              {preStartStyle ? (
                <div
                  data-testid={`world-team-pre-start-${band.key}`}
                  data-start-year={band.starts_year}
                  className="pointer-events-none absolute z-[2] border-r border-status-yellow/45 bg-[repeating-linear-gradient(135deg,rgba(242,196,109,0.13)_0_8px,rgba(242,196,109,0.045)_8px_16px)]"
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
              <span
                className={`absolute left-4 z-20 rounded-full border px-3 py-1 text-[9px] font-black uppercase tracking-[0.12em] ${
                  isFutureBand
                    ? "border-white/12 bg-white/[0.045] text-text-muted"
                    : "border-status-yellow/35 bg-status-yellow/12 text-status-yellow"
                }`}
                style={{ top: CHART_HEADER_HEIGHT + bandBox.top + 14 }}
              >
                {startsInsideWindow ? `${band.label} comeca em ${band.starts_year}` : band.label}
              </span>
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
              const d = buildPath(line, geometry, years);
              if (!d) return null;
              const isFocused = focusedTeamId === track.team_id;
              const isDimmed = focusedTeamId && !isFocused;
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
          {teamTracks.flatMap((track) => teamMovementMarkers(track, geometry, years).map((marker) => (
            <SpecialMovementMarker
              key={`${track.team_id}-${marker.type}-${marker.year}`}
              marker={marker}
              teamId={track.team_id}
              focusedTeamId={focusedTeamId}
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
            onFocus={onFocus}
            onClick={onTeamClick}
          />
        )))}
      </div>
      <InlineTeamTables
        payload={payload}
        geometry={geometry}
        gridStartYear={gridStartYear}
        displayStartYear={displayStartYear}
        windowSize={windowSize}
        focusedTeamId={focusedTeamId}
        onFocus={onFocus}
        onTeamClick={onTeamClick}
        onTeamDoubleClick={onTeamDoubleClick}
      />
    </div>
  );
}

function TeamEntryLabel({ label, team, band, focusedTeamId, onFocus, onClick }) {
  const isDimmed = focusedTeamId && focusedTeamId !== team.team_id;
  return (
    <button
      type="button"
      data-testid={`world-team-entry-label-${team.team_id}-${label.line_key}-${label.year}`}
      className="absolute z-20 grid max-w-[208px] cursor-pointer grid-cols-[42px_minmax(0,1fr)] items-center gap-2.5 overflow-hidden rounded border bg-[#07101d]/85 px-2 py-1 text-left text-[10px] font-black leading-4 shadow-[0_8px_20px_rgba(0,0,0,0.28)] backdrop-blur-sm"
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

function SpecialMovementMarker({ marker, teamId, focusedTeamId, onFocus }) {
  const isPromotion = marker.type === "promotion";
  const color = isPromotion ? "#5ee7a8" : "#ff5b57";
  const points = isPromotion ? "-2.4,1.6 0,-1.6 2.4,1.6" : "-2.4,-1.6 0,1.6 2.4,-1.6";
  const isDimmed = focusedTeamId && focusedTeamId !== teamId;

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

function bandPreStartStyle(band, bandBox, years) {
  if (!bandBox || !years.length || !Number.isFinite(band?.starts_year)) {
    return null;
  }

  const firstYear = years[0];
  if (band.starts_year <= firstYear) {
    return null;
  }

  const preStartPosition = bandStartPosition(band, firstYear);
  if (preStartPosition <= 0) {
    return null;
  }

  return {
    left: "0%",
    top: CHART_HEADER_HEIGHT + bandBox.top,
    width: `${formatPercent((clamp(preStartPosition, 0, years.length) / years.length) * 100)}%`,
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

function buildYears(payload) {
  if (!payload?.window_start || !payload?.window_end) {
    return [];
  }
  const years = [];
  for (let year = payload.window_start; year <= payload.window_end; year += 1) {
    years.push(year);
  }
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

function teamMovementMarkers(track, geometry, years) {
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

    if (nextY < currentY) {
      markers.push({
        type: "promotion",
        year: point.year,
        band_key: point.band_key,
        x: pointX(point, years),
        y: currentY - 6,
      });
    } else if (nextY > currentY) {
      markers.push({
        type: "demotion",
        year: point.year,
        band_key: point.band_key,
        x: pointX(point, years),
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
      const referenceYear = bandReferenceYear(band, displayStartYear, years[years.length - 1] ?? displayStartYear);
      if (firstPoint.year <= referenceYear) return null;
      const pointYValue = pointY(firstPoint, geometry);
      if (!Number.isFinite(pointYValue)) return null;
      const width = clamp(track.nome.length * 5.8 + 66, 118, 208);
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

function buildPath(track, geometry, years) {
  if (!track.points?.length || !years.length) {
    return "";
  }
  const coordinates = [];
  const firstVisiblePoint = track.points.find((point) => point.year === years[0]);
  const firstVisibleY = firstVisiblePoint ? pointRowY(firstVisiblePoint, geometry) : null;
  if (Number.isFinite(firstVisibleY)) {
    coordinates.push([0, firstVisibleY]);
  }
  track.points.forEach((point) => {
    if (!years.includes(point.year)) return;
    const y = pointY(point, geometry);
    if (!Number.isFinite(y)) return;
    coordinates.push([pointX(point, years), y]);
  });
  if (coordinates.length === 0) return "";
  if (coordinates.length === 1) {
    const [x, y] = coordinates[0];
    return `M ${round(Math.max(0, x - 7))} ${round(y)} L ${round(Math.min(CHART_WIDTH, x + 7))} ${round(y)}`;
  }
  return coordinates
    .map(([x, y], index) => `${index === 0 ? "M" : "L"} ${round(x)} ${round(y)}`)
    .join(" ");
}

function pointX(point, years) {
  // Anchor every point to the left edge (start) of its year column, so team
  // lines, markers and the "category did not exist" divider all line up at the
  // beginning of the year regardless of regular/special slot.
  const yearIndex = years.indexOf(point.year);
  return (yearIndex / years.length) * CHART_WIDTH;
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
  if (!payload) {
    return { left: "0%", width: "20%" };
  }
  const minYear = familyMinYear(payload);
  const total = Math.max(atlasMaxYear(payload) - minYear + 1, 1);
  const left = ((displayStart - minYear) / total) * 100;
  const width = (windowSize / total) * 100;
  return {
    left: `${clamp(left, 0, 100)}%`,
    width: `${clamp(width, 6, 100)}%`,
  };
}

function chartTimelineStyle(payload, years, displayStartYear, visibleWindowSize) {
  if (!payload || !years.length || !Number.isFinite(displayStartYear)) {
    return { transform: "translate3d(0%, 0, 0)" };
  }
  // Reserve a left gutter for the name tables so the data area is drawn to their
  // right (not hidden behind them). The visible window spans [gutter, 100%].
  const gutter = INLINE_TABLE_WIDTH_PCT / 100;
  const widthPercent = round((1 - gutter) * (years.length / visibleWindowSize) * 100);
  const offsetYears = displayStartYear - years[0];
  const offsetPercent = -(offsetYears / years.length) * 100;
  return {
    left: `${INLINE_TABLE_WIDTH_PCT}%`,
    width: `${widthPercent}%`,
    transform: `translate3d(${round(offsetPercent)}%, 0, 0)`,
    transition: "transform 80ms linear",
    willChange: "transform",
  };
}

function latestWindowStart(payload, windowSize = payload?.window_size ?? DEFAULT_WINDOW_SIZE) {
  return Math.max(familyMinYear(payload), atlasMaxYear(payload) - windowSize + 1);
}

function clampVisibleStart(payload, startYear, windowSize = DEFAULT_WINDOW_SIZE) {
  if (!payload) {
    return startYear;
  }
  return clamp(startYear, familyMinYear(payload), latestWindowStart(payload, windowSize));
}

function roundedDisplayStartYear(payload, value, windowSize = DEFAULT_WINDOW_SIZE) {
  if (!payload || !Number.isFinite(value)) {
    return value;
  }
  return Math.round(clamp(value, familyMinYear(payload), latestWindowStart(payload, windowSize)));
}

function visibleWindowEndYear(payload, startYear, windowSize = DEFAULT_WINDOW_SIZE) {
  if (!payload || !Number.isFinite(startYear)) {
    return null;
  }
  const unclampedEnd = Math.round(startYear) + windowSize - 1;
  return Math.min(unclampedEnd, atlasMaxYear(payload));
}

function atlasMaxYear(payload) {
  return payload?.window_end ?? payload?.max_year ?? DEFAULT_START_YEAR;
}

function yearFromClientX(payload, railElement, clientX, windowSize = DEFAULT_WINDOW_SIZE) {
  const minYear = familyMinYear(payload);
  const latestStart = latestWindowStart(payload, windowSize);
  const rect = railElement?.getBoundingClientRect();
  if (!rect || rect.width <= 0) {
    return payload.window_start;
  }
  const progress = clamp((clientX - rect.left) / rect.width, 0, 1);
  return minYear + progress * (latestStart - minYear);
}

function formatDelta(value) {
  if (!Number.isFinite(value) || value === 0) return "0";
  return value > 0 ? `+${value}` : String(value);
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
