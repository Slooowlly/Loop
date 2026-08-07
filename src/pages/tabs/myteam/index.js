// Seletor de versão da aba Minha Equipe.
//
// O v1 é a aba que estava em produção (cabeçalho + dossiê financeiro + eixos
// técnicos + ranking) e continua exatamente onde sempre esteve:
// src/pages/tabs/MyTeamTab.jsx. O v2 é o redesenho, isolado em ./MyTeamTabV2.jsx.
//
// Para trocar de versão, mude o número abaixo — é o único ponto de decisão.
// Voltar para o v1 é reverter esta linha; nenhum arquivo do v1 é tocado pelo v2.
import MyTeamTabV1 from "../MyTeamTab";
import MyTeamTabV2 from "./MyTeamTabV2";

export const MY_TEAM_VERSION = 2;

const MyTeamTab = MY_TEAM_VERSION === 2 ? MyTeamTabV2 : MyTeamTabV1;

export { MyTeamTabV1, MyTeamTabV2 };
export default MyTeamTab;
