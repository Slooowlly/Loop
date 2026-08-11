// Geometria e normalização do atlas histórico de equipes.
//
// Tudo aqui é PURO: recebe o payload de `get_global_team_history` (ou pedaços dele) e
// devolve números/estilos. Vive fora da aba porque é compartilhado por ela e pelos
// componentes de v2/ (atlasV2Geometry, AtlasChart, AtlasRankings,
// AtlasChampionsPanel) — manter aqui evita ciclo de import entre eles.

export const DEFAULT_FAMILY = "mazda";
export const DEFAULT_WINDOW_SIZE = 20;
// Ano-base da janela de busca; também é o piso quando o payload não traz min/max.
export const DEFAULT_START_YEAR = 2000;
export const CHART_WIDTH = 1000;
export const CHART_HEADER_HEIGHT = 56;
export const MIN_CHART_HEIGHT = 360;
export const MIN_BAND_HEIGHT = 124;
export const BAND_LABEL_HEIGHT = 22;
export const ROW_HEIGHT = 32;
export const ROW_PILL_HEIGHT = 28;
export const ROW_TOP_OFFSET = 34;
export const BAND_BOTTOM_PADDING = 16;
export const INLINE_TABLE_WIDTH_PCT = 25;
// Most years the window may show at once before the chart gets too cramped/distorted
// — matches GT3's current widest range (1994–2027). Past this, the window stops
// growing and starts SCROLLING instead (the scrubber becomes functional).
export const MAX_VISIBLE_SPAN = 34;

export function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

export function round(value) {
  return Math.round(value * 10) / 10;
}

export function formatPercent(value) {
  return String(Math.round(value * 10000) / 10000);
}

export function normalizePayload(payload) {
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

export function bandPreStartStyle(band, bandBox, years, firstDataYear) {
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

export function bandStartDividerStyle(band, bandBox, years) {
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

export function buildYears(payload, zoomYears = null) {
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

export function buildGeometry(payload, years) {
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
export function bandRowOffsetY(bandTop, position) {
  return bandTop + ROW_TOP_OFFSET + BAND_LABEL_HEIGHT + (Math.max(position ?? 1, 1) - 1) * ROW_HEIGHT;
}

function sortedBandRows(rows, displayStartYear) {
  return [...(rows ?? [])].sort((left, right) => {
    const leftPosition = rowSortPosition(left, displayStartYear);
    const rightPosition = rowSortPosition(right, displayStartYear);
    return leftPosition - rightPosition || String(left.nome).localeCompare(String(right.nome));
  });
}

export function visibleBandRows(rows, displayStartYear) {
  return sortedBandRows(
    (rows ?? []).filter((row) => Number.isFinite(rowPositionAtYear(row, displayStartYear))),
    displayStartYear,
  );
}

// Year whose standings the RIGHT-hand table shows: the band's most recent data year
// at or before the rightmost visible year. Returns null when the band has no data
// yet (it has not started by the rightmost visible year).
export function bandReferenceYearRight(band, displayEndYear) {
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

export function rowPositionAtYear(row, year) {
  const point = (row.points ?? []).find((item) => item.year === year);
  return point ? Math.max(point.position ?? 1, 1) : null;
}

export function buildTeamTracks(payload, geometry, years) {
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

export function trackLineGroups(track) {
  return ["regular", "special"]
    .map((lineKey) => ({
      ...track,
      line_key: lineKey,
      points: track.points.filter((point) => point.slot === lineKey),
    }))
    .filter((line) => line.points.length > 0);
}

export function teamMovementMarkers(track, geometry, years, lastDataYear, bandFirstYear) {
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

export function teamEntryLabels(track, geometry, years, payload, displayStartYear) {
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

export function buildPath(track, geometry, years, lastDataYear, bandFirstYear) {
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

function slotOrder(slot) {
  return slot === "special" ? 2 : 1;
}

export function flattenTeams(payload) {
  const rows = [];
  (payload?.bands ?? []).forEach((band) => {
    band.rows.forEach((row) => rows.push({ ...row, band_category: band.category }));
  });
  return rows;
}

export function teamRowToTeam(row) {
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

export function teamTrackToTeamRow(track, bandKey, bandByKey) {
  const band = bandByKey?.get?.(bandKey);
  return {
    ...track,
    band_key: bandKey,
    band_category: band?.category ?? track.band_category ?? "",
    category: band?.category ?? track.category ?? "",
    class_name: band?.class_name ?? track.class_name ?? null,
  };
}

export function teamToTeamRow(team, fallback) {
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

export function familyFromTeamContext(category, className) {
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

export function getReadableWorldTeamColor(color) {
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
export function familyMinYear(payload) {
  const starts = (payload?.bands ?? [])
    .map((band) => band.starts_year)
    .filter((year) => Number.isFinite(year));
  const dataMin = payload?.min_year ?? DEFAULT_START_YEAR;
  return starts.length ? Math.max(dataMin, Math.min(...starts)) : dataMin;
}

export function windowRailStyle(payload, displayStart = payload?.window_start, windowSize = payload?.window_size ?? DEFAULT_WINDOW_SIZE) {
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

export function chartTimelineStyle(payload, years, displayStartYear, visibleWindowSizeValue) {
  if (!payload || !years.length || !Number.isFinite(displayStartYear)) {
    return { transform: "translate3d(0%, 0, 0)" };
  }
  // Reserve a RIGHT gutter for the standings table so the data area is drawn to its
  // left (not hidden behind it). The visible window spans [0, 100% - gutter].
  const gutter = INLINE_TABLE_WIDTH_PCT / 100;
  const widthPercent = round((1 - gutter) * (years.length / visibleWindowSizeValue) * 100);
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

export function latestWindowStart(payload, windowSize = payload?.window_size ?? DEFAULT_WINDOW_SIZE) {
  return Math.max(scrollMinYear(payload), axisEndYear(payload) - windowSize + 1);
}

export function clampVisibleStart(payload, startYear, windowSize = DEFAULT_WINDOW_SIZE) {
  if (!payload) return startYear;
  return clamp(startYear, scrollMinYear(payload), latestWindowStart(payload, windowSize));
}

export function roundedDisplayStartYear(payload, value, windowSize = DEFAULT_WINDOW_SIZE) {
  if (!payload || !Number.isFinite(value)) return value;
  return Math.round(clamp(value, scrollMinYear(payload), latestWindowStart(payload, windowSize)));
}

export function visibleWindowEndYear(payload, startYear, windowSize = DEFAULT_WINDOW_SIZE) {
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
export function scrollMinYear(payload) {
  return familyMinYear(payload) - pastMargin(payload);
}

// Rightmost NAVIGABLE year: the last season itself. The window can't be scrolled
// past it — its right edge stops at the standings table, never sliding into the
// empty future beyond the "trophies".
export function axisEndYear(payload) {
  return familyMaxYear(payload);
}

// Rightmost year RENDERED as a column. Extends past the navigable end just enough to
// keep the year header filled across the right gutter (under the table). These extra
// columns are header context only — the scrubber/scroll still stop at axisEndYear.
function renderEndYear(payload) {
  return axisEndYear(payload) + Math.ceil(visibleWindowSize(payload) / 3) + 1;
}

export function familyMaxYear(payload) {
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
export function visibleWindowSize(payload) {
  return Math.min(familyDataWidth(payload) + pastMargin(payload), MAX_VISIBLE_SPAN);
}

export function axisEdgeZoneStyle(side, payload, years) {
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

export function yearFromClientX(payload, railElement, clientX, windowSize = DEFAULT_WINDOW_SIZE) {
  const dragMin = scrollMinYear(payload);
  const latestStart = latestWindowStart(payload, windowSize);
  const rect = railElement?.getBoundingClientRect();
  if (!rect || rect.width <= 0) return dragMin;
  const progress = clamp((clientX - rect.left) / rect.width, 0, 1);
  return dragMin + progress * (latestStart - dragMin);
}

// Um time fica "aceso" se está sob o mouse (focused) OU fixado em análise (pinned —
// o time em evidência ao abrir daqui). Assim o highlight do time selecionado NÃO some
// ao passar o mouse por outra linha; a linha sob o mouse acende junto, pra comparação.
export function teamHighlight(teamId, focusedTeamId, pinnedTeamId) {
  const isFocused = teamId === focusedTeamId || teamId === pinnedTeamId;
  const anyActive = focusedTeamId != null || pinnedTeamId != null;
  return { isFocused, isDimmed: anyActive && !isFocused };
}
