// O tempo de abertura de um painel de ficha — a ficha do piloto e o dossiê da
// equipe.
//
// Não é atraso artificial por capricho: é o compasso que faz a abertura ser uma
// SEQUÊNCIA em vez de um estalo. O painel entra vazio com o aviso de carga, as
// setas laterais deslizam, e só então o conteúdo aparece. Num save pequeno o
// `invoke` volta em 20ms e as três coisas acontecem no mesmo quadro — o jogador
// vê um pulo, e não uma tela abrindo.
//
// Vale só na ABERTURA. Navegar entre pilotos ou entre equipes com o painel já
// na tela não paga o piso: ali a espera seria uma pausa no meio de uma
// navegação que o jogador quer rápida.
// Este número é o dial da abertura, e o único lugar onde ele existe.
//
// Quem consome precisa marcar a primeira carga como gasta só QUANDO ELA
// ENTREGA, e não ao pedir o piso: em dev o StrictMode monta, desmonta e remonta
// o efeito, então a primeira passagem é descartada. Gastar a bandeira nela faz a
// segunda — a que de fato desenha — pedir o piso como navegação e não esperar
// nada. O sintoma é traiçoeiro porque só aparece em dev, e porque a ficha do
// piloto continua parecendo ter compasso: o `invoke` dela custa perto de um
// segundo por conta própria (varre o mundo inteiro para montar os recordes).
// Enquanto isso o dossiê de equipe, que volta em dezenas de milissegundos, abria
// de estalo — e mexer neste número não mudava nada em lugar nenhum.
export const ABERTURA_MS = 600;

// Debaixo do vitest o compasso não corre. Ele é encenação, e 600ms por abertura
// multiplicados pelas dezenas de casos que abrem ficha e conferem o conteúdo
// logo em seguida viram minutos de suíte esperando uma animação. Quem testa o
// compasso em si passa `forcar`.
const ENCENACAO_ATIVA = !import.meta.env?.VITEST;

/// Resolve quando o compasso de abertura tiver passado — ou na hora, se este
/// carregamento não for uma abertura.
export function pisoDeAbertura(ehAbertura, { forcar = false } = {}) {
  if (!ehAbertura || (!ENCENACAO_ATIVA && !forcar)) return Promise.resolve();
  return new Promise((resolve) => setTimeout(resolve, ABERTURA_MS));
}
