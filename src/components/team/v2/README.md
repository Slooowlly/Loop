# Atlas Histórico — componentes do v2

Componentes do Atlas Histórico (`src/pages/tabs/atlas/GlobalTeamsTabV2.jsx`) e o
dossiê de equipe (`TeamHistoryDrawerV2.jsx`).

O nome "v2" é histórico: durante o redesenho o atlas v1 vivia um nível acima
(`src/components/team/WorldTeamHistoryGrid.jsx`, `YearWindowScrubber.jsx`) e
servia de rollback, e a regra era não editá-lo. O v1 foi removido em 11/08/2026
— `src/pages/tabs/atlas/index.js` é apenas o ponto de entrada hoje.

O que continua valendo: `../worldTeamChartGeometry.js` (cálculo puro, sem
desenho) e `../teamHistoryDossier.js` (normalização do payload de
`get_team_history_dossier`) moram um nível acima porque são compartilhados.
Mudar o comportamento deles arrasta os dois consumidores.
