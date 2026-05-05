# Rookie Star Standings Design

## Goal

Show compact status markers next to drivers in the driver standings table.

## Decision

The standings backend will expose explicit debut and active-injury information on each `DriverSummary`. The frontend will render compact emoji markers beside the driver name and apply a small priority rule so life-debut rookies do not also show the category-rookie star.

## Behavior

- `🌱` appears only in `Classificação de pilotos` for drivers making their life debut.
- `⭐` appears only for drivers in their first season in the current category, and is hidden when `🌱` is already shown.
- `🩸` appears for active light/moderate injuries.
- `🚑` appears for active grave/critical injuries.
- Life debut uses `stats_carreira.corridas == 0`.
- Category debut uses `temporadas_na_categoria == 0`.
- Injury markers use the existing `injuries.active` and `InjuryType` data.
- Markers do not change sorting, points, rows, category selection, or team standings.
- Existing champion trophy display remains after status markers.

## Files

- `src-tauri/src/commands/career_types.rs`: add serialized debut status fields to `DriverSummary`.
- `src-tauri/src/commands/career.rs`: populate life-debut, category-debut, and injury fields for normal and special standings.
- `src-tauri/src/db/queries/injuries.rs`: expose active injury lookup by driver IDs if needed.
- `src/pages/tabs/StandingsTab.jsx`: render status markers beside the driver name with life-debut priority over the category star.
- `src/pages/tabs/StandingsTab.test.jsx`: cover the marker combinations.
