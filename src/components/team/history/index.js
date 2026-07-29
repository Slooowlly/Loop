// Seletor de versão do dossiê de equipe.
//
// O v1 é o painel em produção (drawer de borda com abas em pílula) e continua
// exatamente onde sempre esteve: src/components/team/TeamHistoryDrawer.jsx. O v2
// é o redesenho — tela centralizada, seções na lateral, records com barra de
// posição e trajetória por temporada — isolado em ../v2/TeamHistoryDrawerV2.jsx.
//
// Para trocar de versão, mude o número abaixo — é o único ponto de decisão.
// Voltar para o v1 é reverter esta linha; nenhum arquivo do v1 desenha diferente
// por causa do v2 (ele só passou a EXPORTAR o construtor do dossiê).
import { TeamHistoryDrawer as TeamHistoryDrawerV1 } from "../TeamHistoryDrawer";
import { TeamHistoryDrawerV2 } from "../v2/TeamHistoryDrawerV2";

export const TEAM_HISTORY_VERSION = 2;

export const TeamHistoryDrawer = TEAM_HISTORY_VERSION === 2 ? TeamHistoryDrawerV2 : TeamHistoryDrawerV1;

export { TeamHistoryDrawerV1, TeamHistoryDrawerV2 };
export default TeamHistoryDrawer;
