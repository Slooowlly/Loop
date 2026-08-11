// Voz do spotter: toca a fala gravada de cada evento.
//
// O pacote vive em `src/assets/spotter/<chave>.opus` e é gerado por
// `scripts/spotter-pack.mjs` (que grava WAV) seguido de `scripts/audio-para-opus.mjs` — o
// nome do arquivo É a chave do evento que o Rust emite, então acrescentar uma fala é
// acrescentar um arquivo, sem tabela no meio. O WAV fica como master, fora do git.
//
// A cadeia de rádio já está GRAVADA nos arquivos (a POC decidiu assim: estas falas
// nunca são coladas umas nas outras, então não há a emenda que obrigaria a filtrar
// depois de juntar). Aqui sobra o essencial: decodificar uma vez, guardar, e tocar.
//
// MAIS NOVA GANHA. Se uma fala chega enquanto outra toca, a anterior é cortada. É o
// oposto de uma fila, e de propósito: o spotter que enfileira termina anunciando
// "esquerda" quando o carro já está à direita. Numa disputa, a informação mais
// recente é a única que ainda vale.

import { aoMudarVolume, aplicarEm, volumeRadio } from "./volumeRadio";
import { comLimite } from "./filaDeCarga";
import { registrar as registrarRadio } from "./radioRegistro";

const ARQUIVOS = import.meta.glob("../assets/spotter/*.opus", {
  eager: true,
  query: "?url",
  import: "default",
});

// { esquerda: "/assets/esquerda-a1b2.opus", ... }
const URLS = Object.fromEntries(
  Object.entries(ARQUIVOS).map(([caminho, url]) => [caminho.split("/").pop().replace(/\.opus$/, ""), url]),
);

// VARIAÇÕES: `chave.opus`, `chave_2.opus`, `chave_3.opus` são a mesma fala escrita de
// três jeitos, e o rodízio é feito aqui — o detector no Rust emite só `chave` e não
// sabe (nem precisa saber) quantas gravações existem. Acrescentar uma variação é
// acrescentar um arquivo.
//
// Isso importa mais no lembrete ("Ainda aí" / "Segura, segura" / "Mantém a posição"),
// que sai a cada 2,5 s enquanto a disputa dura: a mesma frase quatro vezes seguidas
// deixaria de ser informação e viraria um alarme de carro.
const CHAVE_LIGADA = "loop.spotterVoz";

// As falas que encerram uma situação. Ao tocar uma delas o rodízio volta ao começo:
// a próxima disputa recomeça pela primeira variação em vez de pegar o rodízio no meio.
//
// Sem isto o rodízio é global e eterno, e o efeito colateral é ruim de um jeito difícil
// de perceber: numa sessão longa, cada disputa nova entra numa variação arbitrária, e a
// PRIMEIRA fala da permanência — a que responde à dúvida mais quente ("ele ainda está
// aí?") — deixa de ser sempre a redação mais direta. Reiniciar aqui devolve a ordem
// pensada e mantém a alternância dentro da disputa, que é onde ela serve.
const LIBERACAO = new Set(["livre", "livre_esquerda", "livre_direita", "livre_atras", "pode_voltar", "verde"]);

// PRIORIDADE. "Mais nova ganha" resolve bem enquanto todas as falas são sobre a mesma
// coisa — a vizinhança lateral, onde a leitura mais recente sempre substitui a anterior.
// Com o obstáculo à frente isso deixa de valer: são dois assuntos ao mesmo tempo, e um
// "Carro fora à frente" não pode cortar um "Esquerda" no meio. O carro do lado é o que
// machuca agora; o obstáculo é o que machuca daqui a três segundos.
//
// A regra é uma só: uma fala nunca interrompe outra de prioridade MAIOR. Entre iguais,
// segue valendo mais nova ganha.
// A tabela é a peça que decide se as famílias RARAS chegam a ser ouvidas. Medido sobre as
// duas capturas: `fora`, `parado` e `tras` juntas dão 1,24 fala por piloto-corrida, e
// `carro_atras` sozinha dá 8 falas em 82 pilotos-corrida. O lateral dá dezenas a centenas
// por corrida — duas ordens de grandeza acima. Entre as três famílias novas não houve uma
// única colisão no mesmo tique; contra o lateral, entre 16% e 80% das falas delas caem a
// menos de 2 s de uma lateral (a faixa é larga porque `car_left_right` é canal do jogador
// e o jogador ficou no box nas duas gravações — ver docs/spotter-obstaculo.md).
//
// Daí a forma: a família rara nunca divide degrau com a frequente. Se dividisse, "mais
// nova ganha" a apagaria por volume, sem erro de arbitragem nenhum.
const PRIORIDADE = {
  // RETORNO À PISTA — o topo absoluto. Quando este estado está aberto o piloto está parado
  // ou de ré, cego, decidindo se entra: nada que ele possa ouvir importa mais, e `livre` ou
  // `esquerda` chegando por cima de um "segura" seria a troca exata que mata.
  segura_volta: 6,
  pode_voltar: 6,
  // Entrada lateral — alguém chegou, ou a situação apertou. O topo do sistema.
  esquerda: 5,
  direita: 5,
  tres_largos: 5,
  duas_esquerda: 5,
  duas_direita: 5,
  // Obstáculo à frente: o piloto está indo em direção a ele, por vontade própria. As duas
  // famílias dividem degrau porque são o mesmo assunto e nunca falam do mesmo carro — o
  // detector abre um episódio por carro e escolhe o obstáculo MAIS PRÓXIMO, seja qual for
  // a família. Entre elas vale "mais nova ganha", que aqui significa "o mais perto".
  carro_fora_frente: 4,
  carro_parado_frente: 4,
  // Saída de box é a terceira do mesmo assunto: coisa lenta na sua trajetória.
  carro_saindo_box: 4,
  // Tráfego por trás: chega até o piloto, e ele não está dirigindo para dentro dele.
  // Abaixo do obstáculo por isso, acima de tudo que é lembrete ou liberação.
  carro_atras: 3,
  // Bandeiras. O `reparo` (meatball) é ORDEM sobre o seu carro — não obedecer custa a
  // corrida, e por isso fica acima do tráfego. A amarela é contexto de perigo à frente,
  // no degrau do obstáculo. A última volta é informação: não muda o que fazer no próximo
  // segundo, e não pode atropelar nada que mude.
  reparo: 4,
  amarela: 4,
  // O verde é a relargada. Fica no degrau da amarela porque é o par dela — e porque é a
  // fala mais perecível do sistema: chegar tarde é chegar depois de o campo acelerar.
  verde: 4,
  ultima_volta: 2,
  // Clima. Muda a corrida inteira, mas não o que fazer no próximo segundo — e é a família
  // mais rara do sistema, então nunca disputa canal na prática. Degrau de informação.
  pista_molhando: 2,
  pista_molhada: 2,
  pista_secando: 2,
  // Liberação — abriu a porta. `livre_atras` entra aqui, no degrau dos irmãos.
  livre: 2,
  livre_esquerda: 2,
  livre_direita: 2,
  livre_atras: 2,
  // Permanência: preenche silêncio, jamais atravessa notícia.
  ainda_esquerda: 1,
  ainda_direita: 1,
  ainda_ai: 1,
  ainda_atras: 1,
};
const PRIORIDADE_PADRAO = 1;

// Quanto uma fala adiada continua valendo (ms).
//
// Adiar em vez de descartar, porque descartar informação de segurança para preservar
// cadência é o erro que este projeto já cometeu uma vez, no detector lateral. Mas adiar
// para sempre é o mesmo erro de sinal trocado: um "carro fora à frente" que chega depois
// que o piloto já passou pelo ponto é pior que silêncio. As falas laterais duram cerca
// de um segundo, então 1,5 s cobre a espera real e ainda deixa margem dentro da janela
// de 2 a 5 segundos em que o aviso foi calibrado para sair.
const VALIDADE_ADIADA_MS = 1500;

let ctx = null;
let master = null;
let atual = null; // fonte tocando agora
let prioridadeAtual = 0; // prioridade do que está tocando
// Que fala está no ar. Existe só para o REGISTRO: `cortar()` acontece longe de quem escolheu a
// chave, e sem isto a fala interrompida entraria na linha do tempo sem nome — que é justamente
// a linha que se quer ler quando o rádio atropela alguma coisa.
let chaveAtual = null;
// Quando a fala no ar começou (`performance.now()`) e quanto ela tinha para durar. O par existe
// para a linha do CORTE poder dizer o quanto o jogador ouviu: a duração registrada no início é a
// pretendida, e "mais nova ganha" faz com que ela quase nunca seja a que soou. Sem estes dois
// números, uma frase cortada no primeiro décimo e uma cortada na última sílaba são a mesma linha.
//
// Relógio de PAREDE e não `AudioContext.currentTime`: o áudio segue soando quando o JS da janela
// coberta é estrangulado, e é justamente aí que a medida importa.
let inicioAtual = 0;
let duracaoAtual = 0;
let adiada = null; // { chave, prio, ate } — a fala que perdeu a vez
// A fala que já foi ESCOLHIDA para tocar e ainda está no `await` da decodificação:
// { seq, prio, chave }. Existe porque entre decidir tocar e o som sair há um ponto de
// suspensão, e as falas de um mesmo lote de poll caem todas ali dentro. Sem este registro,
// quem chegasse por último ganhava a corrida SEM olhar prioridade — um "esquerda" morria
// calado porque um "carro atrás" veio no mesmo lote, que é exatamente a troca que a tabela
// de prioridade existe para impedir. `null` quando nada está entre a decisão e o início.
let decidida = null;
// Ordem de chegada das chamadas a `falar` — o desempate entre iguais (mais nova ganha).
let sequencia = 0;
// Quem quer saber se o spotter está com a palavra. Existe por causa do ENGENHEIRO: os
// dois são a mesma pessoa, separados só pelo imediatismo, e a mesma pessoa não fala por
// cima de si mesma. O engenheiro assina isto para calar enquanto o aviso de segurança
// sai e retomar a frase depois. Notificação em vez de poll porque a decisão dele é por
// frase, e um poll erraria a borda por até um intervalo inteiro.
const ouvintes = new Set();
const buffers = new Map(); // chave -> AudioBuffer
const carregando = new Map(); // chave -> Promise
const rodizio = new Map(); // chave base -> índice da próxima variação

/** O spotter tem a palavra neste instante? */
export function estaFalando() {
  return Boolean(atual);
}

/**
 * Assina o começo e o fim de cada fala do spotter. Devolve a função que desassina.
 * O retorno do callback é ignorado; ele recebe `true` ao abrir e `false` ao fechar.
 */
export function aoFalar(cb) {
  ouvintes.add(cb);
  return () => ouvintes.delete(cb);
}

// Último estado ANUNCIADO. Sem ele, uma fala substituindo outra (que é o caso comum:
// `cortar()` seguido de `start()` no mesmo tique) anunciaria fechou-e-abriu, e o
// engenheiro tentaria retomar a frase no vão de zero milissegundo entre as duas.
let ultimoAviso = false;

/** Avisa os assinantes que o canal abriu ou fechou — no fim do tique, e só se mudou de
 *  verdade. Um ouvinte que estoura não pode derrubar a fala do spotter, que é o que
 *  importa aqui. */
function avisarOuvintes() {
  queueMicrotask(() => {
    const falando = Boolean(atual);
    if (falando === ultimoAviso) return;
    ultimoAviso = falando;
    for (const cb of ouvintes) {
      try {
        cb(falando);
      } catch {
        /* problema de quem assinou */
      }
    }
  });
}

/** `chave`, `chave_2`, `chave_3`… — as gravações que existem para uma fala. */
function variacoes(base) {
  const achadas = [base];
  for (let n = 2; URLS[`${base}_${n}`]; n += 1) achadas.push(`${base}_${n}`);
  return achadas;
}

/** A próxima variação no rodízio. Com uma gravação só, devolve sempre ela. */
function proximaVariacao(base) {
  const lista = variacoes(base);
  if (lista.length < 2) return lista[0];
  const i = (rodizio.get(base) ?? 0) % lista.length;
  rodizio.set(base, i + 1);
  return lista[i];
}

/** As chaves que o pacote tem — o que a UI pode oferecer para testar. */
export function chavesDisponiveis() {
  return Object.keys(URLS).sort();
}

export function estaLigada() {
  try {
    return localStorage.getItem(CHAVE_LIGADA) !== "0";
  } catch {
    return true; // sem storage, o padrão é falar
  }
}

export function ligar(on) {
  try {
    localStorage.setItem(CHAVE_LIGADA, on ? "1" : "0");
  } catch {
    /* sem persistência */
  }
  if (!on) parar();
}

function contexto() {
  if (!ctx) {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return null;
    ctx = new AC();
    master = ctx.createGain();
    master.gain.value = volumeRadio();
    master.connect(ctx.destination);
  }
  // A política de autoplay suspende o contexto até haver gesto. Retomar aqui é
  // barato e resolve o caso comum: o jogador clicou em algo no app antes de correr.
  if (ctx.state === "suspended") ctx.resume().catch(() => {});
  return ctx;
}

// O volume é o do RÁDIO, não o do spotter: ele e o engenheiro são a mesma pessoa, e um
// controle por boca seria um jeito de deixá-los em alturas diferentes. Ver `volumeRadio`.
aoMudarVolume((v) => aplicarEm(master, ctx, v));

async function carregar(chave) {
  if (buffers.has(chave)) return buffers.get(chave);
  if (carregando.has(chave)) return carregando.get(chave);
  const url = URLS[chave];
  if (!url) return null;
  const c = contexto();
  if (!c) return null;

  const promessa = comLimite(() =>
    fetch(url)
      .then((r) => r.arrayBuffer())
      .then((bytes) => c.decodeAudioData(bytes)),
  )
    .then((buf) => {
      buffers.set(chave, buf);
      carregando.delete(chave);
      return buf;
    })
    .catch(() => {
      carregando.delete(chave);
      return null;
    });
  carregando.set(chave, promessa);
  return promessa;
}

/**
 * Decodifica o pacote inteiro antecipadamente. Vale chamar ao entrar numa sessão:
 * a primeira decodificação custa alguns milissegundos, e pagá-los no meio de uma
 * disputa é exatamente onde eles doem.
 */
export async function preaquecer() {
  await Promise.all(chavesDisponiveis().map(carregar));
}

/** Corta o que estiver tocando, sem mexer na fila de espera. */
function cortar() {
  prioridadeAtual = 0;
  if (!atual) return;
  try {
    atual.stop();
  } catch {
    /* já terminou */
  }
  // A fala interrompida NO MEIO. É o desfecho que mais interessa medir aqui: "mais nova ganha"
  // é a regra do spotter, e é ela que decide o que o jogador não chegou a ouvir inteiro.
  if (chaveAtual) {
    const tocado = Math.max(0, (performance.now() - inicioAtual) / 1000);
    registrarRadio("spotter", {
      chaves: [chaveAtual],
      desfecho: "cortada",
      detalhe: {
        tocado_s: Math.round(tocado * 100) / 100,
        dur_s: duracaoAtual,
        // O que NÃO soou. É a diferença entre o mecanismo funcionando e informação perdida:
        // cortar o último décimo de um "esquerda" é a regra fazendo o seu trabalho, cortar 1,4 s
        // de uma fala de 1,6 s é um aviso que o jogador não recebeu.
        restou_s: Math.max(0, Math.round((duracaoAtual - tocado) * 100) / 100),
      },
    });
    chaveAtual = null;
  }
  atual = null;
  avisarOuvintes();
}

/** Silêncio total: corta, esquece o que estava esperando e cancela o que decodificava. */
export function parar() {
  adiada = null;
  decidida = null;
  cortar();
}

function prioridade(chave) {
  return PRIORIDADE[chave] ?? PRIORIDADE_PADRAO;
}

/** A fala adiada ainda vale? */
function aindaVale(p) {
  return p && Date.now() <= p.ate;
}

/**
 * Guarda a fala para depois. O lugar de espera é UM só, e quem fica é a de MAIOR
 * prioridade: um "verde" que cedeu a vez não pode ser esquecido porque um lembrete
 * chegou depois e também cedeu. A mesma chave (e prioridade igual ou maior) substitui —
 * é a informação mais recente do mesmo assunto, e renova a validade.
 */
function adiar(chave) {
  const prio = prioridade(chave);
  if (adiada && adiada.chave !== chave && prio < adiada.prio && aindaVale(adiada)) return;
  adiada = { chave, prio, ate: Date.now() + VALIDADE_ADIADA_MS };
}

/**
 * Toca a fala da chave. `forcar` ignora o interruptor e a prioridade — é o botão
 * "Testar", que precisa soar mesmo com a voz desligada, senão não testa nada.
 *
 * Devolve `false` quando não tocou. Um `false` por prioridade não é uma perda: a fala
 * fica guardada e sai sozinha quando a mais urgente terminar, desde que ainda esteja
 * dentro da validade.
 */
export async function falar(chave, { forcar = false } = {}) {
  if (!forcar && !estaLigada()) {
    // A voz DESLIGADA descarta a fala antes de qualquer decisão do canal. Sem esta linha, a
    // decisão que o Rust registrou aparece na leitura como fala perdida — e "o rádio está
    // engolindo falas" é o diagnóstico errado para "a voz está no zero".
    registrarRadio("spotter", { chaves: [chave], desfecho: "voz_desligada" });
    return false;
  }

  // Cedeu a vez ao que está TOCANDO: guarda e sai. `atual` só é limpo no `onended`,
  // então "há algo tocando" é uma pergunta que a própria fonte responde.
  if (!forcar && atual && prioridade(chave) < prioridadeAtual) {
    adiar(chave);
    registrarRadio("spotter", {
      chaves: [chave],
      desfecho: "adiada",
      detalhe: { cedeu_a: chaveAtual, prio: prioridade(chave), prio_no_ar: prioridadeAtual },
    });
    return false;
  }
  // Cedeu a vez ao que está DECODIFICANDO. Vale a mesma regra da interrupção:
  // prioridade menor não atropela — espera a vez na fila de adiadas.
  if (!forcar && decidida && prioridade(chave) < decidida.prio) {
    adiar(chave);
    registrarRadio("spotter", {
      chaves: [chave],
      desfecho: "adiada",
      detalhe: { cedeu_a: decidida.chave, prio: prioridade(chave), prio_no_ar: decidida.prio },
    });
    return false;
  }
  // O contrário: esta fala toma a vez de uma menos urgente que ainda decodificava. A
  // destronada não é descartada — fica guardada e sai depois, como na interrupção.
  if (!forcar && decidida && decidida.chave !== chave && prioridade(chave) > decidida.prio) {
    adiar(decidida.chave);
  }

  sequencia += 1;
  const minhaVez = sequencia;
  decidida = { seq: minhaVez, prio: forcar ? 0 : prioridade(chave), chave };
  const variacao = proximaVariacao(chave);
  if (LIBERACAO.has(chave)) rodizio.clear();
  const buf = await carregar(variacao);
  const c = contexto();
  // Alguém de prioridade IGUAL OU MAIOR entrou depois de mim enquanto eu decodificava
  // (as menores nem chegam aqui — viram adiadas na entrada). Ela é a informação mais
  // recente e mais urgente; desistir é o certo.
  if (decidida?.seq !== minhaVez) {
    registrarRadio("spotter", {
      chaves: [variacao],
      desfecho: "atropelada",
      detalhe: { por: decidida?.chave ?? null },
    });
    return false;
  }
  if (!buf || !c) {
    // A fala escolhida não tem como tocar; solta a vez para não represar as próximas.
    decidida = null;
    // Peça sem gravação, ou contexto de áudio indisponível. Aparece como fala muda na linha do
    // tempo, e é o único desfecho aqui que é bug e não decisão.
    registrarRadio("spotter", { chaves: [variacao], desfecho: "sem_audio" });
    return false;
  }
  // A que estava na espera é esta mesma — não precisa mais esperar. Uma fala de OUTRA
  // chave continua guardada mesmo através de uma interrupção: quem cedeu a vez a um
  // "esquerda" não deve perdê-la porque um "três largos" chegou logo depois.
  if (adiada?.chave === chave) adiada = null;
  cortar();
  decidida = null;
  const fonte = c.createBufferSource();
  fonte.buffer = buf;
  fonte.connect(master);
  fonte.onended = () => {
    if (atual !== fonte) return;
    atual = null;
    prioridadeAtual = 0;
    chaveAtual = null;
    avisarOuvintes();
    // A vez de quem cedeu — se ainda valer. Fora da validade, some em silêncio: um
    // aviso de obstáculo entregue depois do obstáculo é ruído, não informação.
    const espera = adiada;
    adiada = null;
    if (aindaVale(espera)) falar(espera.chave);
    else if (espera) {
      // Venceu esperando. Some do áudio, não do arquivo: uma sessão cheia destas linhas diz
      // que o canal está saturado, e é a única evidência disso que existe.
      registrarRadio("spotter", { chaves: [espera.chave], desfecho: "validade" });
    }
  };
  fonte.start();
  atual = fonte;
  chaveAtual = variacao;
  inicioAtual = performance.now();
  duracaoAtual = Math.round(buf.duration * 100) / 100;
  prioridadeAtual = forcar ? 0 : prioridade(chave);
  // TOCOU. A variação é a que soou, e a duração vem do próprio buffer — é o número que
  // transforma uma lista de falas em ocupação do canal.
  registrarRadio("spotter", {
    chaves: [variacao],
    desfecho: "ok",
    detalhe: {
      base: chave,
      dur_s: duracaoAtual,
      prio: forcar ? 0 : prioridade(chave),
      forcada: forcar || undefined,
    },
  });
  avisarOuvintes();
  return true;
}
