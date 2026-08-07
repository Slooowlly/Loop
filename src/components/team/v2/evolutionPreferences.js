// Como o jogador escolheu LER o bloco "Como a equipe evolui" — os dois eixos de
// escolha do gráfico: QUANDO (entre campeonatos · campeonato atual) e O QUÊ
// (colocação · pontos).
//
// Isso é preferência de leitura, e não estado da ficha. Quem abriu a campanha do
// ano corrente para comparar equipes quer a campanha da PRÓXIMA equipe também —
// e o caminho até ela passa por desmontar o bloco: a aba inativa sai da árvore,
// e fechar o dossiê leva o drawer inteiro junto. Guardado só no `useState`, o
// gráfico voltava para a vista padrão a cada troca.
//
// O registro é por instalação, como a luz de descoberta do atlas: é conhecimento
// sobre a interface, não sobre o mundo do jogo. Nada aqui depende da carreira
// aberta, então não tem por que morrer com ela.
export const EVOLUTION_VIEW_SEASONS = "temporadas";
export const EVOLUTION_VIEW_RUN = "campanha";
export const RUN_MODE_POSITION = "posicao";
export const RUN_MODE_POINTS = "pontos";

const VIEW_KEY = "loop.teamHistory.evolutionView";
const MODE_KEY = "loop.teamHistory.evolutionMode";

const VIEWS = [EVOLUTION_VIEW_SEASONS, EVOLUTION_VIEW_RUN];
const MODES = [RUN_MODE_POSITION, RUN_MODE_POINTS];

// Valor guardado que não é mais um valor válido volta ao padrão em vez de
// atravessar a tela: a lista de vistas pode mudar entre versões, e o gráfico não
// deve ficar preso a uma opção que já não existe.
function ler(chave, validos, padrao) {
  try {
    const guardado = localStorage.getItem(chave);
    return validos.includes(guardado) ? guardado : padrao;
  } catch {
    // localStorage indisponível — a escolha só deixa de persistir, sem quebrar
    // a tela.
    return padrao;
  }
}

function guardar(chave, valor) {
  try {
    localStorage.setItem(chave, valor);
  } catch {
    /* idem: sem persistência, a vista volta ao padrão na próxima abertura */
  }
}

// A curva entre campeonatos é o padrão porque é a pergunta que o dossiê de uma
// equipe responde primeiro: quem é essa equipe ao longo dos anos.
export function lerVistaEvolucao() {
  return ler(VIEW_KEY, VIEWS, EVOLUTION_VIEW_SEASONS);
}

export function guardarVistaEvolucao(vista) {
  guardar(VIEW_KEY, vista);
}

// Colocação é o padrão porque é o modo que descomprime o eixo: em pontos, um
// líder disparado come a altura sozinho e o pelotão vira um feixe de retas.
export function lerModoEvolucao() {
  return ler(MODE_KEY, MODES, RUN_MODE_POSITION);
}

export function guardarModoEvolucao(modo) {
  guardar(MODE_KEY, modo);
}
