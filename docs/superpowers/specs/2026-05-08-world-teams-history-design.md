# World Teams History Design

## Goal

Add a hidden "equipes mundiais" view parallel to the current global drivers view. From the standings team list, a single click opens the team dossier and a double click opens the world team history atlas.

## Approved UI

The atlas uses a dark fixed-grid slopegraph. The left column anchors each visible category band with team position, logo, team name in its original color, position delta, and a short colored segment that visually connects the name to the chart line. The right side shows a movable year window. Each year is split into two slots: `REG` at the start of the year and `ESP` at the end, allowing a regular category champion and a special category champion in the same year.

Families are selected by filter:

- Mazda: `production_challenger` class `mazda`, `mazda_amador`, `mazda_rookie`
- Toyota: `production_challenger` class `toyota`, `toyota_amador`, `toyota_rookie`
- BMW: `production_challenger` class `bmw`, `bmw_m2`
- GT4: `endurance` class `gt4`, `gt4`
- GT3: `endurance` class `gt3`, `gt3`

LMP2 is intentionally omitted from the filters.

## Behavior

The user can move the visible year window from early historical seasons through the playable start year. The left list dynamically reflects the first year in the current window. Categories that do not exist yet in that year range stay visible as inactive/hachured bands rather than showing fake results.

Hovering a team line focuses that team and fades all other lines. Clicking a team opens its dossier; double clicking opens or keeps the world history context for that team.

## Data

The backend should expose a dedicated payload for world team history built from `team_season_archive` plus current team metadata. It should include family definitions, visible years, category bands, rows, team colors, logo names, regular/special slot points, and min/max available years.
