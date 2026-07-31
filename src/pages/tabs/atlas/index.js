// Seletor de versão do Atlas Histórico.
//
// O v1 é o atlas em produção (linha do tempo + scrubber + drawer) e continua
// exatamente onde sempre esteve: src/pages/tabs/GlobalTeamsTab.jsx. O v2 é o
// redesenho em andamento, isolado em ./GlobalTeamsTabV2.jsx.
//
// Para trocar de versão, mude o número abaixo — é o único ponto de decisão.
// Voltar para o v1 é reverter esta linha; nenhum arquivo do v1 é tocado pelo v2.
import GlobalTeamsTabV1 from "../GlobalTeamsTab";
import GlobalTeamsTabV2 from "./GlobalTeamsTabV2";

export const ATLAS_VERSION = 2;

const GlobalTeamsTab = ATLAS_VERSION === 2 ? GlobalTeamsTabV2 : GlobalTeamsTabV1;

export { GlobalTeamsTabV1, GlobalTeamsTabV2 };
export default GlobalTeamsTab;
