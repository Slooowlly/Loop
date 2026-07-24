import { useEffect, useRef, useState } from "react";

import i18n from "../../i18n/index.js";
import {
  DEFAULT_WINDOW_SIZE,
  axisEndYear,
  clamp,
  latestWindowStart,
  scrollMinYear,
  visibleWindowEndYear,
  windowRailStyle,
  yearFromClientX,
} from "./worldTeamChartGeometry";

// Barra que desliza a janela de anos do atlas histórico. O trilho representa o
// intervalo NAVEGÁVEL (não o eixo inteiro renderizado) — ver windowRailStyle.
export function YearWindowScrubber({
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

export default YearWindowScrubber;
