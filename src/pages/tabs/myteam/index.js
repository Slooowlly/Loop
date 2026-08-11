// Ponto de entrada da aba Minha Equipe.
//
// Houve aqui um seletor de versão (`MY_TEAM_VERSION`) enquanto o v1 — cabeçalho
// + dossiê financeiro + eixos técnicos + ranking, em src/pages/tabs/MyTeamTab.jsx
// — servia de rollback do redesenho. O v1 foi removido em 11/08/2026 junto com
// os painéis que só ele usava (CommandHeader, CostChart, DriverPanel,
// FinanceDossier, GaragePanel, RankingTable, TechPanel).
export { default } from "./MyTeamTabV2";
export { default as MyTeamTabV2 } from "./MyTeamTabV2";
