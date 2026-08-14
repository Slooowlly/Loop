// O ORQUESTRADOR do push-to-talk: do dedo no botão à voz do engenheiro.
//
// Tudo abaixo já existia em pedaços — o gatilho no Rust, o microfone, o classificador, o
// acervo gravado, os dois endpoints do servidor. Este arquivo é o que os amarra, e a
// única coisa que ele decide sozinho é ORDEM e DESISTÊNCIA.
//
//     apertou ──► grava ──► soltou ──► Scribe ──► classifica ─┬─► toca peças    (~1,2 s)
//                                                             └─► modelo (~1,4 s quente)
//
// ## A frase de espera é SEGURO, não cerimônia
//
// O desenho original armava a espera por TEMPO GLOBAL (dispare se a resposta não chegou em
// 700 ms). Não sobrevive ao encanamento: aos 700 ms a transcrição ainda está no ar, então
// nem se sabe qual é o caminho, e a espera tocaria em TODA pergunta — inclusive nas que o
// acervo responde de graça, que passariam a esperar a espera terminar.
//
// A segunda versão foi para o outro extremo: incondicional no ramo do modelo. Também
// errado, e medido — 906 ms de "só um segundo" na frente de uma resposta que ficou pronta
// 462 ms depois. A espera não estava cobrindo silêncio; estava criando som.
//
// Agora ela CORRE contra o pedido, e só dentro do ramo do modelo — onde o caminho já é
// conhecido e a chamada já saiu. Servidor quente, ninguém ouve a espera; servidor frio
// (partida a zero do Cloud Run, ~4 s), ela cobre exatamente o buraco para o qual foi
// gravada. O preço da corrida é a faixa estreita em torno do limiar, onde a resposta chega
// logo depois de a espera começar e tem de aguardá-la terminar — cortar a própria frase no
// meio para falar por cima de si mesmo seria pior.
//
// ## Cancelar é do piloto
//
// Diferente do spotter, onde a fala mais nova apaga a anterior sozinha, aqui só um toque
// novo no botão cancela: cada resposta pertence a uma pergunta, e apagá-la por conta
// própria seria decidir que o piloto não quer mais saber.

import { CHAVE_RADIO_RUIM, CHAVES_FOCO, criarFreio } from "./pttFreio";

const CHAVE_DESISTENCIA = "nao_sei";
/** A pergunta que NÃO CHEGOU — botão apertado sem falar, ou a voz coberta pelo motor. */
const CHAVE_NAO_OUVI = "nao_ouvi";
const CHAVES_ESPERA = ["espera", "espera_2", "espera_3"];

/**
 * Quanto silêncio o engenheiro pode fazer antes de a espera entrar.
 *
 * 2,2 s, medido. A ida e volta completa ao servidor QUENTE — modelo mais síntese — deu
 * 1,14 / 1,64 / 1,64 / 1,82 s em cinco perguntas; a FRIA, 3,58 s. O limiar fica acima de
 * toda a nuvem quente e abaixo da partida a zero, que é o buraco que a espera existe para
 * tapar.
 *
 * A folga até 1,82 s não é generosidade: perto do limiar a corrida fica no pior dos
 * mundos, porque a resposta chega logo depois de a espera começar e ainda tem de esperar
 * a frase inteira terminar. Um limiar rente à mediana transformaria isso na metade dos
 * casos.
 *
 * Ele conta a partir do ROTEAMENTO, não de o piloto soltar o botão — a transcrição já
 * gastou ~1 s antes disso, e esse silêncio é o do engenheiro ouvindo, que ninguém estranha.
 */
const LIMIAR_ESPERA_MS = 2200;
/** Marca de quem venceu a corrida entre o pedido e o relógio. */
const CHEGOU = "chegou";

function dormir(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

/** Os estados que a interface pode mostrar. */
export const OCIOSO = "ocioso";
export const OUVINDO = "ouvindo";
export const PENSANDO = "pensando";
export const FALANDO = "falando";

/**
 * Bytes -> base64, em blocos.
 *
 * `String.fromCharCode(...bytes)` de uma vez estoura a pilha de argumentos com poucas
 * dezenas de milhares de bytes — e três segundos de Opus já passam disso quando o
 * WebView2 escolhe uma taxa mais alta. O bloco de 8 KB é folgado e não custa nada.
 */
export function base64De(bytes) {
  const BLOCO = 0x2000;
  let bin = "";
  for (let i = 0; i < bytes.length; i += BLOCO) {
    bin += String.fromCharCode.apply(null, bytes.subarray(i, i + BLOCO));
  }
  return btoa(bin);
}

/**
 * Monta o orquestrador sobre as dependências dadas.
 *
 * É fábrica, e não módulo com estado, para o teste poder montar um com microfone e voz de
 * mentira. A máquina de estados é a parte que mais merece teste do push-to-talk inteiro —
 * é onde moram o toque acidental, o cancelamento e a desistência — e ela não precisa de
 * áudio nenhum para ser exercida.
 */
export function criarOrquestrador({
  microfone,
  voz,
  invoke,
  aleatorio = Math.random,
  // O painel de diagnóstico pede o caminho LONGO de propósito. Sem isto não há como
  // sentir a latência do modelo: o curto-circuito de telemetria responde antes de rotear,
  // e com o simulador aberto a maioria das perguntas cai no acervo — que é o objetivo do
  // desenho, e o motivo de o caminho do modelo ser difícil de provocar de propósito.
  forcarModelo = () => false,
  // Fatos de mentira para quando não há corrida nenhuma. Vivem em quem chama, e não aqui:
  // um estado de corrida fabricado dentro do orquestrador acabaria escapando para o jogo.
  linhasDemo = () => [],
  // Qual carreira está aberta. O Rust precisa dela para ler a TABELA do campeonato do
  // save — é a única coisa que o engenheiro fala que não sai da telemetria. Vazio é
  // legítimo (menu, painel sem carreira) e custa uma frase a menos, nunca a resposta.
  careerId = () => "",
  limiarEsperaMs = LIMIAR_ESPERA_MS,
  // O freio do rádio. Injetável para o teste poder mover o relógio; ver `pttFreio.js`.
  freio = criarFreio(),
} = {}) {
  let estado = OCIOSO;
  // Rastro dos passos, para o painel de diagnóstico. Existe porque o encanamento tem seis
  // pernas e todas falham do MESMO jeito visto de fora — o engenheiro diz que não sabe.
  // Sem o rastro, "o Scribe não respondeu" e "o acervo não cobre a pergunta" são a mesma
  // tela em branco.
  const passos = new Set();
  let marco = 0;

  function passo(nome, detalhe) {
    const agora = performance.now();
    const ms = marco ? agora - marco : 0;
    marco = agora;
    for (const cb of passos) {
      try {
        cb({ nome, ms, detalhe });
      } catch {
        /* problema de quem assinou */
      }
    }
  }

  /** Assina o rastro dos passos. Devolve a função que desassina. */
  function aoPasso(cb) {
    passos.add(cb);
    return () => passos.delete(cb);
  }

  // Cada pergunta ganha um número. Tudo que volta da rede confere o seu antes de agir:
  // sem isso, uma transcrição lenta chegaria DEPOIS de o piloto já ter perguntado outra
  // coisa e responderia a pergunta velha com a voz confiante do engenheiro.
  let geracao = 0;
  const ouvintes = new Set();
  let ultimoErro = "";

  function ir(novo) {
    if (estado === novo) return;
    estado = novo;
    for (const cb of ouvintes) {
      try {
        cb(estado);
      } catch {
        /* problema de quem assinou */
      }
    }
  }

  /** Assina as trocas de estado (para a interface acender o indicador). */
  function aoMudar(cb) {
    ouvintes.add(cb);
    return () => ouvintes.delete(cb);
  }

  async function desistir(minhaVez, chave = CHAVE_DESISTENCIA) {
    if (geracao !== minhaVez) return;
    passo("desistiu", ultimoErro || "sem detalhe");
    ir(FALANDO);
    await voz.falarPecas([chave]);
    if (geracao === minhaVez) ir(OCIOSO);
  }

  /** O piloto apertou: cala o que estiver saindo e abre o microfone. */
  async function apertar() {
    geracao += 1;
    voz.cancelar();
    // O rádio está de castigo. O microfone NEM ABRE: gravar para depois descartar ainda
    // pagaria a transcrição de uma pergunta que ninguém vai responder, e é esse custo que
    // o freio existe para conter. O engenheiro já avisou que voltava em um minuto — o
    // silêncio aqui é a promessa sendo cumprida, não uma falha.
    if (freio.bloqueado()) {
      marco = performance.now();
      passo("bloqueado", `rádio mudo por mais ${Math.ceil(freio.restanteMs() / 1000)} s`);
      // OCIOSO explícito, e não o estado que estava aí. As duas linhas acima já desfizeram a
      // fala anterior — o `geracao += 1` órfã quem estava no ar e o `voz.cancelar()` cala o
      // áudio —, então quem tocava não volta mais para pôr o estado no lugar: o `ir(OCIOSO)`
      // do fim daquele fluxo é guardado por `geracao === minhaVez`, e a geração já mudou.
      // Sem esta linha o estado ficava preso em FALANDO para sempre, com o indicador aceso e
      // o `soltar()` recusando tudo (ele só age vindo de OUVINDO) — o botão emudecia de vez.
      ir(OCIOSO);
      return;
    }
    if (!microfone.estaArmado()) {
      // Sai no RASTRO, e não só em `ultimoErro`. Esta é a perna que falha calada: o piloto
      // segura o botão, nada acontece, e "o engenheiro não me ouviu" fica idêntico a "o
      // servidor não respondeu" visto de fora. Com o passo, a bancada mostra qual das duas
      // é — que é a razão de o rastro existir.
      ultimoErro = "Microfone não está armado.";
      marco = performance.now();
      passo("sem_microfone", ultimoErro);
      // Mesmo motivo do freio, acima: a fala anterior já foi órfã e calada, e ninguém mais
      // vem devolver o estado.
      ir(OCIOSO);
      return;
    }
    try {
      await microfone.comecar();
      marco = performance.now();
      passo("gravando");
      ir(OUVINDO);
    } catch (e) {
      ultimoErro = String(e?.message || e);
      ir(OCIOSO);
    }
  }

  /** O piloto soltou: fecha o microfone e vai atrás da resposta. */
  async function soltar() {
    if (estado !== OUVINDO) return;
    const minhaVez = geracao;
    const captura = await microfone.terminar();
    if (geracao !== minhaVez) return;

    // Toque acidental. Some em silêncio de propósito: um engenheiro que responde a um
    // esbarrão no volante é pior que um que não responde.
    if (!captura || captura.curtaDemais) {
      passo("curta", "toque acidental, nada foi enviado");
      ir(OCIOSO);
      return;
    }
    passo("gravou", `${(captura.duracaoMs / 1000).toFixed(1)}s · ${(captura.bytes.length / 1024).toFixed(1)} KB`);

    // O freio conta AQUI, e não no aperto: o toque acidental já foi descartado acima, e
    // punir alguém por esbarrar no volante seria o oposto do que ele quer medir.
    const verdade = freio.registrar();
    if (verdade === "corte") {
      passo("corte", `${Math.ceil(freio.restanteMs() / 1000)} s de silêncio — nada foi enviado`);
      ir(FALANDO);
      await voz.falarPecas([CHAVE_RADIO_RUIM]);
      if (geracao === minhaVez) ir(OCIOSO);
      return;
    }
    ir(PENSANDO);

    let transcricao;
    try {
      transcricao = await invoke("ptt_transcrever", {
        audioB64: base64De(captura.bytes),
        mime: captura.mime,
      });
    } catch (e) {
      ultimoErro = String(e?.message || e);
      return desistir(minhaVez);
    }
    passo("transcreveu", transcricao ? `"${transcricao}"` : "(vazio)");
    if (geracao !== minhaVez) return;
    // Transcrição vazia é o piloto tendo apertado sem falar, ou falado com o motor por
    // cima. Vira desistência — o classificador de palavra-chave em cima de string vazia
    // devolveria a intenção aberta, e o engenheiro responderia com um retrato completo da
    // corrida a uma pergunta que ninguém fez —, mas com a fala CERTA: "não te ouvi" e
    // não "não consegui ver isso", que descreveria falta de dado onde houve falta de voz.
    if (!transcricao) {
      ultimoErro = "Transcrição vazia — nada foi entendido.";
      return desistir(minhaVez, CHAVE_NAO_OUVI);
    }

    const resposta = await rotear(transcricao, minhaVez);
    if (!resposta) return;
    return responder(resposta, transcricao, minhaVez, verdade === "aviso");
  }

  /** A redação do empurrão desta vez. */
  function chaveFoco() {
    return CHAVES_FOCO[Math.floor(aleatorio() * CHAVES_FOCO.length)];
  }

  /**
   * Decide o caminho da resposta. Devolve `null` se já desistiu ou se a pergunta foi
   * atropelada por outra.
   *
   * Normalmente é o Rust quem decide, em `engenheiro_responder`. Com o modelo FORÇADO o
   * caminho muda de comando: `engenheiro_dossie` devolve os fatos sem escolher nada, e é
   * a única forma de chegar ao modelo sem esperar uma pergunta que o acervo não cubra.
   */
  async function rotear(transcricao, minhaVez) {
    let resposta;
    try {
      if (forcarModelo()) {
        const dossie = await invoke("engenheiro_dossie", { transcricao, careerId: careerId() });
        // A demonstração entra quando NÃO HÁ TELEMETRIA, e não quando o dossiê vem vazio.
        // Sem simulador o dossiê traz uma linha — a que diz que não há dados —, e ela é
        // "não vazia" o bastante para o teste ingênuo passar: o modelo recebia um único
        // fato inútil e respondia sobre a falta dele, que é o oposto do que se quer medir.
        const semCorrida = !dossie?.estado?.conectado;
        const linhas = semCorrida || !dossie?.linhas?.length ? linhasDemo() : dossie.linhas;
        resposta = {
          caminho: "modelo",
          intencao: dossie?.intencao,
          linhas,
          tratamento: dossie?.tratamento,
        };
      } else {
        resposta = await invoke("engenheiro_responder", { transcricao, careerId: careerId() });
      }
    } catch (e) {
      ultimoErro = String(e?.message || e);
      await desistir(minhaVez);
      return null;
    }
    if (geracao !== minhaVez) return null;
    if (resposta.caminho === "modelo" && !resposta.linhas?.length) {
      // Sem um fato sequer o modelo só teria o que inventar, e o servidor recusa por
      // isso mesmo. Melhor desistir aqui, com a razão no rastro.
      ultimoErro = "Nenhum fato disponível — sem corrida e sem dossiê de demonstração.";
      await desistir(minhaVez);
      return null;
    }
    passo(
      "roteou",
      resposta.caminho === "gravada"
        ? `acervo · ${resposta.intencao} · ${resposta.pecas.join(" + ")}`
        : `modelo · ${resposta.intencao} · ${resposta.linhas.length} fato(s)` +
            (forcarModelo() ? " · FORÇADO" : ""),
    );
    return resposta;
  }

  /**
   * Toca a resposta decidida, pelo caminho que ela pedir.
   *
   * Com `empurrar`, emenda o "olha a pista" no FIM — depois de responder, nunca no lugar
   * da resposta. Quem recusa a informação é o corte; o empurrão é só um engenheiro
   * atendendo e mandando você voltar a correr.
   */
  async function responder(resposta, transcricao, minhaVez, empurrar = false) {
    if (resposta.caminho === "gravada") {
      ir(FALANDO);
      // Entra na MESMA sequência: assim ele respeita as pausas do rádio e cede ao spotter
      // como qualquer outra frase, em vez de ser uma fala solta atrás da resposta.
      // As PAUSAS derivadas das chaves, e não a padrão de 160 ms. A resposta gravada
      // deixou de ser sempre uma frase só: com o vizinho nomeado ela é montada
      // ("Seu rival," + "Cooper," + "está a um e dois na sua frente.") e com o campeonato
      // ela ganha uma segunda frase inteira atrás. Cada junção pede um respiro diferente,
      // e 160 ms para todas soa como leitura de lista.
      const pecas = empurrar ? [...resposta.pecas, chaveFoco()] : resposta.pecas;
      // A pergunta vai ao registro do rádio junto: uma resposta sem a pergunta é metade da
      // conversa, e é a metade que não deixa julgar se ele respondeu o que foi perguntado.
      await voz.falarPecas(pecas, {
        pausasMs: voz.pausasDoRadio(pecas),
        canal: "resposta",
        texto: transcricao ? `[${transcricao}]` : "",
      });
      passo("falou", empurrar ? "peças gravadas + empurrão" : "peças gravadas");
      if (geracao === minhaVez) ir(OCIOSO);
      return;
    }

    // Caminho do modelo. O pedido sai ANTES da espera tocar, não depois: são ~2,9 s de
    // servidor contra ~1,2 s de espera, e serializá-los somaria os dois.
    const pedido = invoke("ptt_responder", {
      transcricao,
      intencao: String(resposta.intencao ?? ""),
      linhas: resposta.linhas ?? [],
      // Como ele deve te chamar nesta fala, quando for a vez. O Rust já racionou; aqui é
      // só repasse.
      tratamento: resposta.tratamento ?? null,
    });
    // Sem isto, um servidor fora do ar rejeita a promessa antes de alguém dar `await`
    // nela, e o webview registra um "unhandled rejection" que não é erro de ninguém.
    pedido.catch(() => {});

    // A corrida. A rejeição também conta como "chegou": um servidor fora do ar não merece
    // um "só um segundo" na frente do "não consegui ver isso agora".
    const venceu = await Promise.race([
      pedido.then(
        () => CHEGOU,
        () => CHEGOU,
      ),
      dormir(limiarEsperaMs),
    ]);
    if (geracao !== minhaVez) return;
    if (venceu !== CHEGOU) {
      ir(FALANDO);
      await voz.falarPecas([CHAVES_ESPERA[Math.floor(aleatorio() * CHAVES_ESPERA.length)]]);
      passo("espera", "frase de espera tocada");
      if (geracao !== minhaVez) return;
    }

    let falada;
    try {
      falada = await pedido;
    } catch (e) {
      ultimoErro = String(e?.message || e);
      return desistir(minhaVez);
    }
    if (geracao !== minhaVez) return;
    passo("modelo", `"${falada.texto}"`);
    // Redundante quando a espera tocou; obrigatório quando ela não tocou, que é o caso
    // normal desde que a corrida entrou.
    ir(FALANDO);

    // `audio_b64`, e não `audioB64`: o serde do Rust serializa os campos como estão
    // escritos, e a ponte do Tauri não renomeia a VOLTA — só os argumentos de ida.
    const ok = await voz.falarRemoto(falada.audio_b64, falada.mime, {
      canal: "resposta",
      texto: falada.texto ?? "",
    });
    if (geracao !== minhaVez) return;
    // O áudio chegou mas não tocou (ilegível, ou o contexto morreu). Desistir aqui é o
    // que separa "o engenheiro disse que não sabe" de "o engenheiro emudeceu".
    if (!ok) return desistir(minhaVez);
    // Aqui o empurrão é uma fala à parte: a resposta do modelo é áudio remoto, e não há
    // sequência de peças em que ele possa entrar.
    if (empurrar) {
      await voz.falarPecas([chaveFoco()]);
      if (geracao !== minhaVez) return;
    }
    const r = voz.ultimoRender?.();
    passo(
      "falou",
      r
        ? `voz do modelo · render ${Math.round(r.ms)} ms · rms ${r.rms.toFixed(2)} ` +
            `× ${r.ganho.toFixed(2)} = ${(r.rms * r.ganho).toFixed(3)}`
        : "voz do modelo · SEM NIVELAMENTO",
    );
    ir(OCIOSO);
  }

  /**
   * O MESMO encanamento, a partir de uma pergunta ESCRITA — pula só a transcrição.
   *
   * Existe para o painel de diagnóstico. Com o modelo forçado ela vai até o fim: pede ao
   * servidor, sintetiza e fala, com o rastro medindo cada perna. É a única forma de
   * sentir a latência do caminho longo sem depender de o acervo falhar por acaso.
   *
   * NÃO passa pelo freio, de propósito: o painel é onde se afere o encanamento, e ser
   * cortado no meio de uma aferição custa mais do que as perguntas que ele economizaria.
   * Quem paga a conta de verdade é o botão do volante, e esse é freado.
   */
  async function perguntarEscrito(texto) {
    geracao += 1;
    const minhaVez = geracao;
    voz.cancelar();
    marco = performance.now();
    passo("transcreveu", `"${texto}" (escrita, sem Scribe)`);

    const resposta = await rotear(texto, minhaVez);
    if (!resposta) return;
    if (resposta.caminho === "modelo") passo("dossie", resposta.linhas.join(" | "));
    return responder(resposta, texto, minhaVez);
  }

  /** Aborta tudo — a voz desligada nas configurações, a sessão fechando. */
  function cancelar() {
    geracao += 1;
    voz.cancelar();
    microfone.abortar?.();
    ir(OCIOSO);
  }

  return {
    apertar,
    soltar,
    cancelar,
    perguntarEscrito,
    aoMudar,
    aoPasso,
    freio,
    estado: () => estado,
    ultimoErro: () => ultimoErro,
  };
}
