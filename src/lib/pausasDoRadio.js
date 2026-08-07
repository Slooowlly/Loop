// As pausas ENTRE as peças de uma fala montada do rádio.
//
// Módulo separado do `engenheiroVoz.js` de propósito: aquele depende do `import.meta.glob` do
// Vite para achar os `.wav`, o que o torna incarregável fora do bundle. Esta derivação é
// aritmética pura sobre nomes de chave, e precisa rodar em três lugares — no app, nas audições
// (`scripts/tts-poc/`) e no guard estrutural. Duplicá-la nos três seria garantir que
// divergissem.
//
// ## Quais falas são montadas, e por quê
//
// A regra da casa é gravar a frase INTEIRA: uma tomada só tem a prosódia certa e não paga
// emenda nenhuma. Duas famílias fogem dela, as duas por aritmética:
//
// - **quebra** (`engenheiro::quebra`) — nomeia um piloto do grid. Frase inteira custaria 355
//   sobrenomes × 108 trechos × 8 enquadramentos.
// - **volta mais rápida** (`engenheiro::tempo_volta`) — nomeia um piloto E diz um tempo.
//   Inteira seria 355 × 2.101.
//
// O tempo de volta em si é fundido, e isso foi MEDIDO: partir o minuto (`"um,"` +
// `"trinta e dois e quatro."`) deixou a fala 22% mais longa no caso comum e 91% no pior,
// porque um monossílabo gravado sozinho sai com acento pleno e contorno de fim de frase.
//
// ## Os quatro tipos de junção
//
//   ARTIGO  — "…da" + "Kitsune": dentro do sintagma, o mais apertado que existe.
//   ORACAO  — "Cooper," + "está com…": fronteira dentro da mesma oração.
//   VIRGULA — "Seu rival," + "Cooper,": vírgula real no texto, respiro curto.
//   FRASE   — "…no motor." + "Ótima notícia": ponto final, a emenda barata — a prosódia de
//             fim de frase da peça anterior já é a certa ali.
//
// A resposta ao PUSH-TO-TALK usa as mesmas quatro. Ela é montada quando o vizinho tem nome
// ("Seu rival," + "Cooper," + "está a um e dois na sua frente.") e quando a fala da pista
// ganha a do campeonato atrás — e são exatamente as junções acima, na mesma ordem.

export const JUNCAO = { artigo: 40, oracao: 60, virgula: 90, frase: 220 };

/**
 * As pausas em ms entre as peças de uma fala montada, derivadas das CHAVES.
 *
 * Devolve `chaves.length - 1` valores. A gramática está no Rust e as chaves a carregam: o
 * prefixo diz que papel a peça cumpre, e o papel diz que tipo de fronteira vem depois dela.
 */
export function pausasDoRadio(chaves) {
  const pausas = [];
  for (let i = 0; i < chaves.length - 1; i += 1) {
    const atual = chaves[i];
    const proxima = chaves[i + 1];
    // O VOCATIVO abre a fala ("Novato," + "Você está em quinto.") e é a PRIMEIRA regra de
    // propósito: o que vem depois dele é uma frase inteira, e quase sempre uma das que a
    // regra seguinte manda emendar com respiro de ponto final. Fosse ela a decidir, o nome
    // do piloto ficaria pendurado 220 ms longe do resto — e um vocativo separado por esse
    // tanto para de ser vocativo e vira uma frase de uma palavra só.
    if (atual.startsWith("voc_")) pausas.push(JUNCAO.virgula);
    // DUAS FRASES INTEIRAS seguidas, e é `else if` porque a regra acima já respondeu. É a
    // forma da resposta ao push-to-talk desde que o
    // engenheiro passou a saber da carreira: a fala da pista e, depois dela, a do
    // campeonato ("No campeonato, você está em terceiro."), a da memória ("São quatro
    // décimos a menos que da última vez.") ou o empurrão do freio ("Olha a pista.").
    // Cada uma já traz a prosódia de ponto final gravada dentro, e sem esta linha as duas
    // cairiam na pausa de ORAÇÃO — 60 ms — e a segunda entraria atropelando a primeira.
    else if (
      proxima.startsWith("camp_") ||
      proxima.startsWith("mem_") ||
      proxima.startsWith("foco")
    )
      pausas.push(JUNCAO.frase);
    else if (atual.startsWith("ab_piloto")) pausas.push(JUNCAO.artigo);
    else if (atual.startsWith("ab_")) pausas.push(JUNCAO.virgula);
    else if (atual.startsWith("ap_")) pausas.push(JUNCAO.virgula);
    else if (proxima.startsWith("ap_")) pausas.push(JUNCAO.virgula);
    else if (proxima.startsWith("co_")) pausas.push(JUNCAO.frase);
    // A classificação monta uma fala só: o reconhecimento ("Perdeu essa.") mais a conta das
    // tentativas ("Ainda dá pra mais duas."). São duas frases inteiras, cada uma com a prosódia
    // de ponto final já gravada dentro — a emenda barata. Uma pausa de oração aqui atropelaria
    // a segunda em cima da primeira.
    else if (atual.startsWith("cl_")) pausas.push(JUNCAO.frase);
    // O tempo SEMPRE encerra a frase ("…um trinta e dois e quatro."). O que vier depois é
    // outra oração — "Faltam quatro décimos para a melhor volta." — e merece o respiro de
    // ponto final. Esta linha vem ANTES da de baixo de propósito: `t_` como peça atual manda
    // mais que `t_` como próxima.
    else if (atual.startsWith("t_")) pausas.push(JUNCAO.frase);
    // "…é do Cooper, um trinta e dois e quatro." — a vírgula é do texto, não da gramática do
    // sujeito. Sem esta linha o nome cairia em ORACAO (60 ms) e o tempo entraria atropelando.
    else if (proxima.startsWith("t_")) pausas.push(JUNCAO.virgula);
    else pausas.push(JUNCAO.oracao);
  }
  return pausas;
}
