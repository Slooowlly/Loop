# Driver Detail Current Quality Design

## Goal

Redesign the driver detail drawer so it answers one primary question: "esse piloto e bom agora?"

## Approved Structure

Keep the existing profile header, including identity, license, motivation, personality, pontos fortes and pontos de atencao. Replace the current tab set with six connected dossier tabs:

- `Resumo`: current verdict, championship position, season wins/podiums/top 10, recent average and a compact trend chart.
- `Qualidade`: intrinsic driver quality independent of the car, with attribute bars, strengths, weaknesses and potential-oriented reading.
- `Leitura de desempenho`: expected-versus-delivered reading against car/team/teammate context.
- `Historico`: career totals with global rank in parentheses, e.g. `62 (82º)`, plus timeline. Include podiums.
- `Rivais`: active rivalries, intensity, type and comparison against the main rival.
- `Mercado`: contract, value/risk/renewal reading. Team name should use the team color.

## Data Design

The backend should extend `DriverDetail` instead of making separate frontend calls. The existing payload already has performance, form, profile, contract and tags. Add focused blocks for:

- current-season championship summary;
- global career stat ranks for races, wins, podiums and titles;
- performance context;
- rivalries using the existing `rivalries` table;
- market color metadata where needed.

When data is missing, the UI should show calm empty states rather than hiding the whole tab.

## UI Design

The drawer remains dense and operational, not a marketing page. Tabs should be equal-width controls that can wrap on smaller widths. The Resumo tab should be the default and preserve adjacent-driver navigation without resetting the active tab.

Use the existing dark dossier visual language: compact framed sections, restrained colors, team color as accent, and no large decorative surfaces.

## Testing

Add structure tests for the six-tab navigation and key textual anchors. Add backend tests for career ranking and rival payload construction. Run focused frontend structure tests and Rust tests around driver detail.
