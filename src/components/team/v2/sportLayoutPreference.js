// Qual layout a seção Esportivo do dossiê desenha.
//
// São dois arranjos dos MESMOS seis blocos — nenhum dado existe em um e falta no
// outro. O "arrumado" é o padrão: agrupa os blocos por pergunta e cria a
// hierarquia que a pilha de seis irmãos não tinha. O "classico" é o arranjo
// original, preservado inteiro porque o empilhamento simples continua sendo o
// que melhor sobrevive a janela estreita e a equipe com pouco histórico.
//
// A escolha é por instalação, não por carreira: é preferência de leitura da
// interface, não estado do mundo do jogo.
const STORAGE_KEY = "loop.teamDossier.sportLayout";

export const SPORT_LAYOUT_ARRANGED = "arrumado";
export const SPORT_LAYOUT_CLASSIC = "classico";
export const SPORT_LAYOUT_DEFAULT = SPORT_LAYOUT_ARRANGED;

export function readSportLayout() {
  try {
    const salvo = localStorage.getItem(STORAGE_KEY);
    return salvo === SPORT_LAYOUT_CLASSIC || salvo === SPORT_LAYOUT_ARRANGED ? salvo : SPORT_LAYOUT_DEFAULT;
  } catch {
    // localStorage indisponível — a preferência só deixa de persistir, sem
    // quebrar a tela.
    return SPORT_LAYOUT_DEFAULT;
  }
}

export function writeSportLayout(layout) {
  try {
    localStorage.setItem(STORAGE_KEY, layout === SPORT_LAYOUT_CLASSIC ? SPORT_LAYOUT_CLASSIC : SPORT_LAYOUT_ARRANGED);
  } catch {
    /* idem: sem persistência, o layout volta ao padrão na próxima abertura */
  }
}
