// Ponto de entrada do Atlas Histórico.
//
// Houve aqui um seletor de versão (`ATLAS_VERSION`) enquanto o v1 — linha do
// tempo + scrubber + drawer, em src/pages/tabs/GlobalTeamsTab.jsx — servia de
// rollback do redesenho. O v1 foi removido em 11/08/2026 junto com os
// componentes que só ele usava (WorldTeamHistoryGrid, YearWindowScrubber); a
// geometria compartilhada continua em components/team/worldTeamChartGeometry.js.
export { default } from "./GlobalTeamsTabV2";
export { default as GlobalTeamsTabV2 } from "./GlobalTeamsTabV2";
