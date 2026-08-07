// O FREIO DO RÁDIO — o engenheiro perdendo a paciência.
//
// O push-to-talk não tinha teto nenhum. Segurar o botão e despejar pergunta atrás de
// pergunta funcionava, e funcionar é o problema: cada uma custa uma transcrição no Scribe
// e, na metade das vezes, uma ida ao modelo mais uma síntese. Um jogador entediado num
// treino livre esvazia a cota do mês sem nunca ter corrido.
//
// A resposta poderia ser um erro na tela. É pior: quebra a única coisa que o rádio tem, que
// é parecer uma pessoa do outro lado. Então o freio FALA — primeiro um empurrão de volta
// para a pista, depois o rádio "com problema" e um minuto de silêncio de verdade.
//
// ## Frequência, não conteúdo
//
// Seria tentador punir só as perguntas "sem importância", e não dá para saber quais são:
// "quanto falta de combustível" é vital numa volta e ansiedade na seguinte, e a mesma
// pergunta repetida seis vezes em um minuto é ansiedade nas seis. O que o freio mede é a
// FREQUÊNCIA — que é, aliás, o que um engenheiro de verdade nota. Ele não julga o mérito
// da pergunta; ele nota que você não para de falar.
//
// ## O silêncio precisa ser BARATO
//
// Durante o castigo o microfone nem abre. Não é economia de bateria: se o áudio subisse
// para ser descartado, o Scribe cobraria por transcrever uma pergunta que ninguém vai
// responder — exatamente o custo que o freio existe para conter.

/** Janela deslizante. Um minuto é o tempo em que o excesso ainda se lê como excesso. */
export const JANELA_MS = 60000;
/** A partir desta pergunta DENTRO da janela sai o empurrão, junto com a resposta. */
export const LIMITE_AVISO = 4;
/** E nesta o rádio cai. */
export const LIMITE_CORTE = 6;
/** Quanto dura o silêncio. O mesmo minuto da janela, e é o que a fala promete. */
export const CASTIGO_MS = 60000;

/** As redações do empurrão. Três, para ele não soar gravado quando sai duas vezes. */
export const CHAVES_FOCO = ["foco", "foco_2", "foco_3"];
/** A fala do corte. */
export const CHAVE_RADIO_RUIM = "radio_ruim";

/**
 * Monta um freio.
 *
 * É fábrica com `agora` injetável porque o que se testa aqui é TEMPO, e um teste de tempo
 * que depende do relógio de verdade ou é lento ou é intermitente.
 */
export function criarFreio({ agora = () => Date.now() } = {}) {
  let perguntas = [];
  let mudoAte = 0;
  let ultimoAviso = 0;

  /** O rádio está em silêncio agora? */
  function bloqueado() {
    return agora() < mudoAte;
  }

  /** Quanto falta do castigo, em milissegundos. Zero quando não há. */
  function restanteMs() {
    return Math.max(0, mudoAte - agora());
  }

  /**
   * Registra uma pergunta que VAI ser enviada e devolve o que fazer com ela:
   *
   * - `"ok"` — nada de especial.
   * - `"aviso"` — responda, e emende o empurrão no fim.
   * - `"corte"` — não envie nada; toque a fala do rádio ruim e cale por um minuto.
   */
  function registrar() {
    const t = agora();
    perguntas = perguntas.filter((x) => t - x < JANELA_MS);
    perguntas.push(t);

    if (perguntas.length >= LIMITE_CORTE) {
      mudoAte = t + CASTIGO_MS;
      // Cumprida a pena, a conta recomeça limpa. Sem isto, a primeira pergunta depois do
      // silêncio já encontraria a janela cheia e cairia noutro corte — um minuto de
      // castigo viraria castigo permanente, que não é freio, é porta fechada.
      perguntas = [];
      return "corte";
    }
    // Um empurrão por janela. Ele existe para ser notado, e notado é uma vez: repetido a
    // cada pergunta vira o próprio tipo de tagarelice que ele está reclamando.
    if (perguntas.length >= LIMITE_AVISO && t - ultimoAviso >= JANELA_MS) {
      ultimoAviso = t;
      return "aviso";
    }
    return "ok";
  }

  /** Larga tudo — trocou de sessão, ou a voz foi desligada. */
  function zerar() {
    perguntas = [];
    mudoAte = 0;
    ultimoAviso = 0;
  }

  return {
    bloqueado,
    restanteMs,
    registrar,
    zerar,
    naJanela: () => perguntas.length,
  };
}
