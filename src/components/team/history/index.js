// Ponto de entrada do dossiê de equipe.
//
// Houve aqui um seletor de versão (`TEAM_HISTORY_VERSION`) enquanto o v1 — o
// drawer de borda com abas em pílula — servia de rollback do redesenho. O v1 foi
// removido em 11/08/2026 e a normalização do payload, que morava dentro dele,
// ficou em ../teamHistoryDossier.js.
export { TeamHistoryDrawerV2 as TeamHistoryDrawer } from "../v2/TeamHistoryDrawerV2";
export { TeamHistoryDrawerV2 as default } from "../v2/TeamHistoryDrawerV2";
