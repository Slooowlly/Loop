import TeamLogoMark from "./TeamLogoMark";
import Tooltip from "../ui/Tooltip";
import i18n from "../../i18n/index.js";
import goldTrophy from "../../assets/utilities/trophies/ouro.png";
import {
  CHART_HEADER_HEIGHT,
  CHART_WIDTH,
  DEFAULT_WINDOW_SIZE,
  INLINE_TABLE_WIDTH_PCT,
  ROW_PILL_HEIGHT,
  axisEdgeZoneStyle,
  bandPreStartStyle,
  bandReferenceYearRight,
  bandRowOffsetY,
  bandStartDividerStyle,
  buildPath,
  chartTimelineStyle,
  clamp,
  familyMaxYear,
  familyMinYear,
  formatPercent,
  getReadableWorldTeamColor,
  round,
  roundedDisplayStartYear,
  rowPositionAtYear,
  teamEntryLabels,
  teamHighlight,
  teamMovementMarkers,
  teamTrackToTeamRow,
  trackLineGroups,
  visibleBandRows,
} from "./worldTeamChartGeometry";

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

export function TeamHistoryGrid({ payload, years, geometry, teamTracks, previewStartYear, visibleStartYear, windowSize = DEFAULT_WINDOW_SIZE, focusedTeamId, pinnedTeamId, onFocus, onTeamClick, onTeamDoubleClick }) {
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
        <Tooltip texto={i18n.t("globalTeams.currentChampion")}>
          <span className="text-[9px] font-black leading-none text-yellow-400">★</span>
        </Tooltip>
      )}
      {titles.map((tc) => {
        const src = BAND_TROPHY_IMAGES[tc.band_key] ?? goldTrophy;
        return (
          <Tooltip
            key={tc.band_key}
            texto={i18n.t("globalTeams.bandTitlesCount", { count: tc.count, band: tc.band_label })}
          >
            <span className="inline-flex items-center gap-px">
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
          </Tooltip>
        );
      })}
    </span>
  );
}

export default TeamHistoryGrid;
