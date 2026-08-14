// Voz do engenheiro: toca a resposta ao push-to-talk.
//
// É o irmão do `spotterVoice.js` e diverge dele em três pontos, todos por causa da mesma
// diferença de fundo — o spotter INTERROMPE e o engenheiro RESPONDE:
//
//  1. **Sequência, não peça solta.** Uma resposta pode ser quatro frases ("Você está de
//     pneu seco." / "Ele está com cinco voltas." / "O pneu dele é três voltas mais velho
//     que o seu.") e elas saem em ordem, com a pausa curta de quem fala no rádio.
//  2. **Mais nova NÃO ganha.** Lá, uma fala nova apaga a anterior porque a informação
//     mais recente é a única que vale. Aqui, cada resposta pertence a uma pergunta: só
//     um push-to-talk novo cancela, e é o piloto quem decide isso. Fala NÃO SOLICITADA
//     (ver a fila de anúncios, mais abaixo) nem isso: ela espera a vez.
//  3. **Cede ao spotter.** Os dois são a mesma pessoa, e a mesma pessoa não fala por
//     cima de si mesma. Quando um aviso de segurança abre, a frase em curso PARA e é
//     repetida inteira depois — cortar no meio de "o pneu dele é três voltas" deixaria
//     uma meia-informação pior que o silêncio.
//
// Dois caminhos de áudio chegam aqui, e eles se encontram ANTES de tocar. As peças do
// acervo já saem do gerador com a cadeia de rádio gravada dentro. O áudio que o modelo
// escreveu chega cru da Cloud TTS e passa pela mesma cadeia num `OfflineAudioContext`,
// que é o mais perto que dá de repetir o que o gerador fez — e, principalmente, é o que
// permite MEDIR o resultado e casar o nível com o do acervo antes de tocar. Ver
// `processarComoPeca`. Daí para a frente os dois são a mesma coisa: um buffer pronto.

import { criarCadeiaRadio } from "./filtroRadio";
import { pausasDoRadio } from "./pausasDoRadio";
import { aoMudarVolume, aplicarEm, volumeRadio } from "./volumeRadio";
import { aoFalar as aoFalarSpotter, estaFalando as spotterFalando } from "./spotterVoice";
import { comLimite } from "./filaDeCarga";
import { registrar as registrarRadio } from "./radioRegistro";

// Opus, não WAV. O acervo tem 3.943 peças: em PCM 24 kHz são 328 MB de repositório, e WAV não
// delta-comprime — cada regravação somaria o arquivo inteiro ao histórico, para sempre. Os WAV
// seguem sendo os masters, no disco e fora do git (`scripts/audio-para-opus.mjs`).
const ARQUIVOS_ENGENHEIRO = import.meta.glob("../assets/engenheiro/*.opus", {
  eager: true,
  query: "?url",
  import: "default",
});
// As peças EMPRESTADAS. Três falas ("última volta", "amarela", "reparo") são as mesmas
// nos dois papéis, e gravá-las duas vezes num modelo não determinístico faria a mesma
// pessoa soar diferente conforme quem falou. A lista de quais são vive no Rust
// (`PECAS_DO_SPOTTER`), e um guard estrutural garante que elas continuem existindo lá;
// aqui basta procurar nas duas pastas.
const ARQUIVOS_SPOTTER = import.meta.glob("../assets/spotter/*.opus", {
  eager: true,
  query: "?url",
  import: "default",
});

function indexar(mod) {
  return Object.fromEntries(
    Object.entries(mod).map(([caminho, url]) => [
      caminho.split("/").pop().replace(/\.opus$/, ""),
      url,
    ]),
  );
}
// O acervo do engenheiro vence em caso de nome repetido: se um dia gravarmos a nossa
// versão de uma peça emprestada, ela passa a valer sem mais nenhuma mudança.
const URLS = { ...indexar(ARQUIVOS_SPOTTER), ...indexar(ARQUIVOS_ENGENHEIRO) };

const CHAVE_LIGADA = "loop.engenheiroVoz";

// Pausa entre frases da mesma resposta. Mais curta que a pausa natural da conversa, de
// propósito: rádio de equipe é comprimido e apressado, e uma resposta de quatro frases
// com pausa de fala normal levaria quase um segundo a mais — em corrida, um segundo é
// uma curva.
const PAUSA_ENTRE_FRASES_MS = 160;

// Quantas vezes uma frase espera o spotter antes de sair por cima dele. Sem teto, uma
// sequência de avisos numa relargada — que é exatamente quando o spotter mais fala —
// deixaria a resposta presa para sempre, e o piloto ficaria sem saber que perguntou.
const CESSOES_MAX = 3;

// Teto da fila de ANÚNCIOS. Backstop contra crescimento sem limite, não regra de produto —
// quem descarta na prática é a validade abaixo.
const FILA_MAX = 6;

// Por quanto tempo um anúncio ainda vale a pena ser dito depois de enfileirado.
//
// Vinte segundos. Uma quebra anunciada com dez segundos de atraso continua sendo novidade
// (o piloto não viu); anunciada com meio minuto, o rádio virou um noticiário atrasado e o
// jogador perde a ligação entre a fala e o que aconteceu na pista.
//
// Este número é um palpite informado, não uma medida — a medição do rádio inteiro numa linha
// do tempo só é que vai dizer se ele está certo. Está nomeado aqui para ser ajustável quando
// esse número existir.
const VALIDADE_ANUNCIO_MS = 20000;

// ─── O portão do MOMENTO ─────────────────────────────────────────────────────
//
// A fila já esperava o canal. Agora espera também a PISTA: largada, duelo e última volta
// são instantes em que uma notícia da grade rodaria por cima da única coisa que importa, e
// o jogador não perderia só a fala — perderia a curva.
//
// Quem decide o que é quente é o Rust (`engenheiro::momento`), porque é ele que tem a
// telemetria. Aqui fica só o portão, e ele é uma FUNÇÃO injetada em vez de um `invoke`
// direto: este módulo é de áudio e roda em teste sem Tauri nenhum.
//
// O portão NÃO vale para `falarPecas`. Resposta a push-to-talk e fala do spotter continuam
// saindo — a primeira porque o piloto perguntou, a segunda porque é segurança.
let portao = null;
/// De quanto em quanto se reconfere. Meio segundo: um duelo abre e fecha em segundos, e
/// perguntar mais que isso é pagar um `invoke` para saber o que não mudou.
const RECONFERE_MOMENTO_MS = 500;

/**
 * Instala o portão. `fn` devolve (ou promete) o motivo do silêncio, ou algo falso para
 * liberar. Sem portão instalado a fila se comporta como sempre se comportou.
 */
export function definirPortao(fn) {
  portao = typeof fn === "function" ? fn : null;
}

/**
 * Espera o momento esfriar, no máximo até `ate`.
 *
 * O teto é a própria validade do anúncio, e é o que impede a fila de travar numa disputa
 * longa: passado o prazo, quem descarta é a regra de validade que já existia. Um anúncio
 * que esperou vinte segundos virou notícia velha, e notícia velha não melhora esperando
 * mais.
 */
async function esperarMomentoCalmo(ate) {
  const t0 = Date.now();
  // O que o portão devolveu na PRIMEIRA recusa. Vai ao registro: sem o motivo, uma fala que
  // esperou oito segundos e outra que saiu na hora são a mesma linha no arquivo, e o portão do
  // momento fica sem prova nenhuma de ter funcionado.
  let motivo = null;
  if (!portao) return { esperou_ms: 0, motivo };
  while (Date.now() < ate) {
    let quente = false;
    try {
      quente = await portao();
    } catch {
      return { esperou_ms: Date.now() - t0, motivo }; // sem resposta do portão, o rádio fala
    }
    if (!quente) return { esperou_ms: Date.now() - t0, motivo };
    if (motivo === null) motivo = typeof quente === "string" ? quente : true;
    await dormir(Math.min(RECONFERE_MOMENTO_MS, Math.max(0, ate - Date.now())));
  }
  return { esperou_ms: Date.now() - t0, motivo };
}

// ─── Nível ───────────────────────────────────────────────────────────────────
//
// O acervo é notavelmente uniforme: medidos os 3.226 arquivos, o RMS DA FALA é 0,185 na
// mediana (p10 0,176, p90 0,196). Não é coincidência — todas as peças passaram pela mesma
// cadeia offline, e o limitador do fim segura o pico.
//
// ## Por que "da fala" e não do arquivo
//
// A primeira versão media o RMS do buffer inteiro, e isso divide a energia pelo SILÊNCIO
// também. Medido nas três respostas reais que o jogador ouviu, a fração de fala foi de
// 57%, 71% e 84% — uma frase curta tem proporcionalmente mais pausa que uma longa. Levar
// todas ao mesmo RMS cheio é, portanto, deixá-las em alturas DIFERENTES de fala, e foi
// exatamente essa a queixa: "a diferença de altura de uma pra outra é absurda".
//
// As peças do acervo são aparadas antes de gravadas (fração de fala mediana: 90%), então
// nelas os dois números quase coincidem — 0,175 cheio contra 0,185 com gate. Na voz do
// modelo, que não é aparada, não coincidem nada.
//
// O critério do gate é o MESMO de `faixaUtil` no gerador (janela de 10 ms, 32 dB abaixo
// do pico, piso em -50 dBFS). Tem de ser: o alvo saiu de medir o acervo com ele.
const RMS_ALVO = 0.185;
// Mesmo teto do limitador da cadeia: subir o RMS não pode custar um clipe.
const PICO_TETO = 0.97;
const GATE_JANELA_S = 0.01;
const GATE_QUEDA_DB = 32;
const GATE_PISO_DB = -50;
// Rabo extra no render offline. O compressor do Chromium adia o sinal alguns
// milissegundos, e sem esta folga o render devolveria a frase com o fim comido.
const CAUDA_RENDER_S = 0.05;

let ctx = null;
let master = null;
let entradaRadio = null;
let fonteAtual = null;
let ultimo = null;
let sequencia = 0; // ordem das respostas; só a última vale
const buffers = new Map(); // chave -> AudioBuffer
const carregando = new Map();

// ─── Fila de anúncios ────────────────────────────────────────────────────────
//
// O engenheiro ganhou TRÊS bocas — quebra na grade, peça do nosso carro e volta mais rápida —
// e continua com uma voz. `falarPecas` foi escrita para o push-to-talk, onde "mais nova ganha"
// é a semântica certa: o piloto perguntou outra coisa, a resposta anterior não interessa mais.
//
// Para fala NÃO SOLICITADA essa mesma regra corta no meio. As falas são longas (a quebra chega
// a 7,1 s, o anúncio da melhor volta a 4,2 s), então a sobreposição não é hipótese remota — e o
// resultado é `"A volta mais rápida da corrida é do Coo—"` seguido de outra frase. Meia
// informação, que é o mesmo argumento pelo qual este módulo já cede ao spotter sem cortar.
//
// A regra passa a ser: **quem responde corta, quem anuncia espera**.

let fila = [];
let bombeando = false;
/// Qual sequência está em curso (0 = nenhuma). É a GERAÇÃO, não um booleano, e por dois
/// motivos. Primeiro: entre duas peças da mesma fala não há fonte tocando, então `fonteAtual`
/// não serve — um anúncio se enfiaria no meio da resposta. Segundo: quando uma pergunta
/// atropela um anúncio, os dois `tocarSequencia` convivem por um instante, e só o dono da
/// geração corrente pode soltar o canal. Com um booleano, o anúncio moribundo liberaria a fila
/// por cima da resposta que acabou de começar.
let geracaoAtiva = 0;
let descartados = 0;

// Reexportada para quem toca uma fala montada chegar por um import só.
export { pausasDoRadio };

// O controle de volume mexe no mestre desta boca em tempo real — quem estiver ouvindo o
// teste no painel de opções ouve a mudança enquanto arrasta, que é a única forma de
// escolher um volume.
aoMudarVolume((v) => aplicarEm(master, ctx, v));

export function chavesDisponiveis() {
  return [...new Set([...Object.keys(URLS), ...proprias])].sort();
}

// As peças PRÓPRIAS: chaves que existem nesta instalação e em mais nenhuma. Hoje é uma só —
// o sobrenome do jogador, sintetizado uma vez por carreira e guardado no save (ver
// `commands::engenheiro::voz_propria`). Elas não estão em `URLS` porque não vêm do bundle:
// chegam em base64 pela ponte e vivem direto no mapa de buffers.
const proprias = new Set();

/**
 * **Registra uma peça própria.** Devolve `true` quando ela ficou pronta para tocar.
 *
 * Passa pelo MESMO `processarComoPeca` da resposta do modelo — cadeia de rádio no
 * `OfflineAudioContext` e nivelamento ao alvo do acervo. É isso que faz o nome do jogador sair
 * na mesma cor e no mesmo volume das outras 3.900 peças, em vez de ser a única fala do rádio
 * que soa como um arquivo de fora.
 *
 * Idempotente: chamar de novo com uma chave já registrada não refaz o render. Quem chama é o
 * boot do app, que não sabe se já rodou nesta sessão.
 */
export async function registrarPecaPropria(chave, audioB64, mime = "audio/mpeg") {
  if (!chave || !audioB64) return false;
  if (buffers.has(chave)) return true;
  const c = contexto();
  if (!c) return false;
  void mime; // o decodificador reconhece pelo conteúdo; o tipo vai junto para o rastro
  try {
    const bruto = await c.decodeAudioData(bytesDeBase64(audioB64).buffer);
    // Sem o render a peça tocaria crua — sem cadeia e sem nivelamento —, e o nome do piloto
    // sairia com outro timbre e outro volume no meio da própria frase dele. Melhor não tocar.
    const pronto = await processarComoPeca(bruto);
    if (!pronto) return false;
    buffers.set(chave, pronto);
    proprias.add(chave);
    return true;
  } catch {
    return false;
  }
}

export function estaLigada() {
  try {
    return localStorage.getItem(CHAVE_LIGADA) !== "0";
  } catch {
    return true; // sem storage, o padrão é responder
  }
}

export function ligar(on) {
  try {
    localStorage.setItem(CHAVE_LIGADA, on ? "1" : "0");
  } catch {
    /* sem persistência */
  }
  if (!on) cancelar();
}

function contexto() {
  if (!ctx) {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return null;
    ctx = new AC();
    master = ctx.createGain();
    master.gain.value = volumeRadio();
    master.connect(ctx.destination);
    // A cadeia de rádio é montada UMA vez e fica ligada no mestre. Montá-la a cada
    // resposta custaria os nós e, pior, o look-ahead do compressor entraria como atraso
    // no primeiro som — justo no caminho que já é o mais lento dos dois.
    entradaRadio = criarCadeiaRadio(ctx, { saida: master }).entrada;
  }
  if (ctx.state === "suspended") ctx.resume().catch(() => {});
  return ctx;
}

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
 * Decodifica ANTES da hora as peças que uma fala vai usar.
 *
 * Recebe a lista de propósito. A versão anterior desta função não recebia nada e
 * percorria `chavesDisponiveis()` — o acervo INTEIRO — ao abrir a sessão. Isso não era
 * caro, era impossível: 3.943 peças, 324 MB em disco e cerca de 650 MB depois de
 * decodificadas em `AudioBuffer` (o acervo é PCM 16 bits e o buffer é float de 32).
 * Na prática ela nunca terminava; derrubava o carregamento das falas do spotter junto e
 * enchia o console de `ERR_INSUFFICIENT_RESOURCES`.
 *
 * O aquecimento em massa também protegia menos do que aparentava: `tocarSequencia` já
 * carrega peça a peça, então o único milissegundo que ele evitava era o da PRIMEIRA —
 * as outras decodificam sozinhas enquanto a anterior toca. Aquecer a sequência é a
 * versão proporcional da mesma ideia, e cabe de sobra no teto de [`comLimite`].
 */
export async function preaquecer(chaves) {
  await Promise.all((chaves ?? []).map(carregar));
}

export function estaFalando() {
  return Boolean(fonteAtual);
}

/** Cala o engenheiro. Chamado quando o piloto aperta o botão de novo — perguntar outra
 *  coisa é dizer que a resposta anterior não interessa mais.
 *
 *  Esvazia a fila de anúncios junto: o piloto acabou de fazer uma pergunta, e despejar nele
 *  três notícias represadas antes da resposta é o oposto de atender. */
export function cancelar() {
  sequencia += 1;
  pararFonte();
  descartarFila();
}

/** Larga os anúncios represados. Chamado por TODA resposta, não só pelo cancelamento: o
 *  piloto acabou de falar com o engenheiro, e despejar nele três notícias antigas antes da
 *  resposta é o oposto de atender. Elas já eram velhas quando a pergunta chegou. */
function descartarFila() {
  if (!fila.length) return;
  descartados += fila.length;
  for (const item of fila) {
    // Jogada fora porque o piloto perguntou algo. É descarte legítimo e é o mais invisível de
    // todos: nada no áudio indica que existiram três notícias, e o contador global não diz
    // QUAIS eram.
    registrarRadio(item.canal ?? "engenheiro", {
      chaves: item.chaves ?? [],
      texto: item.texto ?? "",
      desfecho: "pergunta",
    });
    item.resolve(false);
  }
  fila = [];
}

/** Quantos anúncios foram descartados nesta sessão (fila cheia, validade vencida, ou uma
 *  pergunta do piloto que os tornou irrelevantes). É o número que a medição do rádio inteiro
 *  precisa para saber se o canal está saturado — sem ele, "o rádio parece calmo" e "o rádio
 *  está engolindo metade das falas" são indistinguíveis. */
export function anunciosDescartados() {
  return descartados;
}

function pararFonte() {
  if (!fonteAtual) return;
  const f = fonteAtual;
  fonteAtual = null;
  try {
    f.stop();
  } catch {
    /* já terminou */
  }
}

function dormir(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

/** Espera o spotter fechar o canal. Volta na hora se ele já estiver calado. */
function esperarSpotter() {
  if (!spotterFalando()) return Promise.resolve();
  return new Promise((resolve) => {
    const desassinar = aoFalarSpotter((falando) => {
      if (falando) return;
      desassinar();
      resolve();
    });
  });
}

/**
 * Toca um buffer e resolve com o que aconteceu:
 * `"fim"` (tocou inteiro), `"cancelado"` (outra pergunta chegou), `"cedeu"` (o spotter
 * abriu o canal no meio e esta frase precisa ser repetida).
 *
 * `medida` é um objeto de SAÍDA: quem passa um recebe de volta `tocado_s`, o tempo que o som
 * de fato saiu. Vai por parâmetro em vez de virar valor de retorno porque o retorno é uma
 * string comparada em três lugares — trocá-lo por um objeto renderia a mesma informação com
 * uma refatoração no caminho de áudio, que é onde menos vale mexer por conforto.
 */
function tocarBuffer(buf, minhaVez, { pelaCadeia = false, medida = null } = {}) {
  const c = contexto();
  if (!c) return Promise.resolve("cancelado");

  return new Promise((resolve) => {
    const fonte = c.createBufferSource();
    fonte.buffer = buf;
    fonte.connect(pelaCadeia ? entradaRadio : master);

    // Relógio de PAREDE: o áudio continua saindo quando o JS da janela coberta é estrangulado,
    // e é justamente nesse caso que a medida vale.
    const inicio = performance.now();
    let resolvido = false;
    const terminar = (motivo) => {
      if (resolvido) return;
      resolvido = true;
      desassinar();
      if (medida) medida.tocado_s = Math.round((performance.now() - inicio) / 10) / 100;
      if (fonteAtual === fonte) fonteAtual = null;
      resolve(motivo);
    };

    // O spotter abrindo o canal no meio da frase. Para e devolve "cedeu" — quem chamou
    // repete a frase inteira depois, porque meia frase de rádio é pior que nenhuma.
    const desassinar = aoFalarSpotter((falando) => {
      if (!falando) return;
      try {
        fonte.stop();
      } catch {
        /* já terminou */
      }
      terminar("cedeu");
    });

    fonte.onended = () => terminar(sequencia === minhaVez ? "fim" : "cancelado");
    fonte.start();
    fonteAtual = fonte;
  });
}

/**
 * **Toca a resposta gravada**, na ordem em que o Rust decidiu.
 *
 * Devolve `true` se a resposta saiu inteira. `false` significa que outra pergunta a
 * atropelou, ou que a voz está desligada — nunca que faltou arquivo, que é o caso de uma
 * peça sumida e resolve-se pulando: uma frase a menos é melhor que a resposta toda muda.
 *
 * `pausasMs` sobrepõe a pausa padrão junção a junção — é o que a fala de quebra usa, porque
 * lá as peças são fragmentos de oração e não frases (ver [`pausasDoRadio`]).
 */
export async function falarPecas(chaves, { pausasMs = null, canal = "resposta", texto = "" } = {}) {
  if (!estaLigada()) {
    registrarRadio(canal, { chaves, texto, desfecho: "voz_desligada" });
    return false;
  }
  sequencia += 1;
  pararFonte();
  descartarFila();
  return tocarSequencia(chaves, pausasMs, sequencia, { canal, texto });
}

/**
 * **Anuncia** — fala NÃO SOLICITADA (quebra na grade, peça do nosso carro, volta mais rápida).
 *
 * Espera a vez em vez de cortar. Devolve `true` se saiu inteira; `false` se foi descartada
 * (fila cheia, validade vencida) ou atropelada por uma pergunta do piloto.
 *
 * O descarte por VALIDADE é o que impede a fila de virar noticiário atrasado: numa relargada
 * com três quebras seguidas, dizer todas em fila daria vinte segundos falando de coisas que o
 * piloto já passou. Vale contar quantas caíram — [`anunciosDescartados`].
 */
export function anunciar(chaves, { pausasMs = null, canal = "engenheiro", texto = "" } = {}) {
  if (!estaLigada()) {
    registrarRadio(canal, { chaves, texto, desfecho: "voz_desligada" });
    return Promise.resolve(false);
  }
  if (fila.length >= FILA_MAX) {
    descartados += 1;
    registrarRadio(canal, { chaves, texto, desfecho: "fila_cheia" });
    return Promise.resolve(false);
  }
  return new Promise((resolve) => {
    fila.push({ chaves, pausasMs, resolve, entrou: Date.now(), canal, texto });
    bombear();
  });
}

/** Toca a fila de anúncios, um por vez, sempre com o canal livre. */
async function bombear() {
  if (bombeando) return;
  bombeando = true;
  try {
    while (fila.length) {
      // Espia sem tirar da fila: as duas esperas abaixo podem demorar, e um item já
      // removido ficaria fora do descarte por validade e fora do esvaziamento que uma
      // pergunta do piloto provoca.
      const item = fila[0];
      const espera = await esperarMomentoCalmo(item.entrou + VALIDADE_ANUNCIO_MS);
      await esperarCanalLivre();
      // Uma pergunta do piloto esvazia a fila, e ela pode ter chegado durante a espera. Se
      // a cabeça mudou, este item já foi resolvido por quem a esvaziou — recomeça.
      if (fila[0] !== item) continue;
      fila.shift();
      // `>=`, e não `>`: a espera do portão termina EXATAMENTE em `entrou + VALIDADE`, e
      // com o `>` o item saía de lá vencido e mesmo assim tocava — a fila esperava a pista
      // esfriar por vinte segundos e no fim falava a notícia velha assim mesmo. As duas
      // regras têm de concordar na borda, senão a espera contradiz o descarte.
      if (Date.now() - item.entrou >= VALIDADE_ANUNCIO_MS) {
        descartados += 1;
        // Morreu na fila. O `motivo` diz se foi o portão do momento que a segurou até vencer ou
        // se foi o canal ocupado — são dois problemas diferentes e a correção de cada um é outra.
        registrarRadio(item.canal ?? "engenheiro", {
          chaves: item.chaves ?? [],
          texto: item.texto ?? "",
          desfecho: espera.motivo ? "suprimida" : "validade",
          detalhe: { esperou_ms: Date.now() - item.entrou, motivo: espera.motivo },
        });
        item.resolve(false);
        continue;
      }
      sequencia += 1;
      // SEM `pararFonte()`: o canal está livre por construção, e chamá-lo aqui reintroduziria
      // exatamente o corte que esta fila existe para evitar.
      const meta = {
        canal: item.canal ?? "engenheiro",
        texto: item.texto ?? "",
        espera_ms: espera.esperou_ms || undefined,
        motivo: espera.motivo ?? undefined,
      };
      if (item.audioB64) {
        // Áudio do modelo. Segura o canal na mão, porque `tocarRemoto` não o faz — quem
        // decide se corta ou se espera é sempre o chamador, e aqui o chamador é a fila.
        geracaoAtiva = sequencia;
        const minhaVez = sequencia;
        try {
          item.resolve(await tocarRemoto(item.audioB64, item.mime, minhaVez, meta));
        } finally {
          liberarCanal(minhaVez);
        }
      } else {
        item.resolve(await tocarSequencia(item.chaves, item.pausasMs, sequencia, meta));
      }
    }
  } finally {
    bombeando = false;
  }
}

/** Resolve quando nenhuma sequência estiver em curso. */
function esperarCanalLivre() {
  if (geracaoAtiva === 0) return Promise.resolve();
  return new Promise((resolve) => {
    ouvintesLivre.push(resolve);
  });
}

let ouvintesLivre = [];

function liberarCanal(minhaVez) {
  // Só o dono da geração corrente solta o canal. Ver o comentário de `geracaoAtiva`.
  if (geracaoAtiva !== minhaVez) return;
  geracaoAtiva = 0;
  const espera = ouvintesLivre;
  ouvintesLivre = [];
  for (const r of espera) r();
}

/** O laço de tocar, comum à resposta e ao anúncio. Quem decide se corta é o chamador. */
async function tocarSequencia(chaves, pausasMs, minhaVez, meta = null) {
  geracaoAtiva = minhaVez;
  // Registrado no COMEÇO, e não no fim: é o instante em que o som saiu que casa com a decisão
  // do Rust, e uma fala de sete segundos registrada no fim aparece sete segundos depois do que
  // o jogador ouviu. Quem não completa ganha a segunda linha, com o desfecho.
  const canal = meta?.canal ?? "engenheiro";
  let completou = false;
  // `comecou` separa dois desfechos que o `completou` sozinho confunde: a fala que soou e foi
  // cortada no meio, e a que nunca chegou a soar porque uma pergunta do piloto passou na frente.
  let comecou = false;
  // Onde a fala parou, quando ela para no meio: a peça que estava no ar e quanto dela soou. Uma
  // resposta são ~dez frases, e "cortada" sem isto não diz se o piloto ouviu nove ou uma.
  let pecaNoAr = -1;
  const medida = {};
  // Quando o som saiu, para o FECHAMENTO poder dizer quanto tempo esta fala ocupou o canal. A
  // duração de uma fala de peças não é conhecida no começo: são várias gravações, com pausas
  // entre elas e com as esperas que o spotter impõe no meio. Sem isto, uma quebra na grade e um
  // aviso de bandeira contam zero segundo na ocupação do canal — e a ocupação é justamente o
  // número que diz se o rádio está falando demais.
  let comecouEm = 0;
  // Puxa a sequência inteira já, em vez de esperar cada peça chegar na sua vez: são
  // poucas (uma resposta tem uma dezena), e assim a peça 2 decodifica enquanto a 1 toca.
  // Sem `await` de propósito — o loop abaixo espera a peça de que precisa, e esperar
  // aqui pela ÚLTIMA seria atrasar a primeira por causa das que só falam depois.
  void preaquecer(chaves);
  try {
    for (let i = 0; i < chaves.length; i += 1) {
      const buf = await carregar(chaves[i]);
      if (sequencia !== minhaVez) return false;
      if (!buf) continue; // peça que não existe no disco: segue para a próxima

      let cessoes = 0;
      for (;;) {
        await esperarSpotter();
        if (sequencia !== minhaVez) return false;
        // A abertura sai na PRIMEIRA peça que de fato começa a tocar, e não no índice 0.
        // Enquanto a condição era `i === 0`, uma sequência cuja primeira peça não existisse
        // no disco (o `continue` acima) passava a tocar da segunda em diante sem abertura —
        // e, sem `comecouEm`, também sem a linha de fim. Uma fala que o jogador ouviu
        // inteira não deixava rastro nenhum no registro, que é o mesmo arquivo em branco de
        // uma fala que nunca saiu.
        if (!comecou) {
          comecou = true;
          comecouEm = performance.now();
          registrarRadio(canal, {
            chaves,
            texto: meta?.texto ?? "",
            desfecho: "ok",
            detalhe: {
              pecas: chaves.length,
              espera_ms: meta?.espera_ms,
              motivo: meta?.motivo,
            },
          });
        }
        pecaNoAr = i;
        const motivo = await tocarBuffer(buf, minhaVez, { medida });
        if (motivo === "cancelado") return false;
        if (motivo === "fim") break;
        // "cedeu": repete, até o teto. Estourou o teto, sai por cima, porque duas vozes por um
        // segundo é melhor que uma resposta que nunca chega.
        //
        // A linha é da PEÇA que cedeu, e não da sequência: aqui o que recomeça é a frase, não a
        // fala inteira, e sem a chave específica ficaria impossível saber onde a resposta travou.
        registrarRadio(canal, {
          chaves: [chaves[i]],
          desfecho: "cedeu",
          detalhe: {
            peca: i,
            pecas: chaves.length,
            cessao: cessoes + 1,
            tocado_s: medida.tocado_s,
            por: "spotter",
          },
        });
        cessoes += 1;
        if (cessoes >= CESSOES_MAX) {
          const ultima = await tocarBuffer(buf, minhaVez, { medida });
          if (ultima === "cancelado") return false;
          break;
        }
      }

      if (i < chaves.length - 1) {
        await dormir(pausasMs?.[i] ?? PAUSA_ENTRE_FRASES_MS);
        if (sequencia !== minhaVez) return false;
      }
    }
    completou = true;
    return true;
  } finally {
    // A fala que começou e não terminou. É o desfecho que explica um rádio com meia frase, e
    // sem esta linha ele fica idêntico no arquivo a uma fala que saiu inteira.
    const noAr_s = comecouEm ? Math.round((performance.now() - comecouEm) / 10) / 100 : undefined;
    if (!completou) {
      registrarRadio(canal, {
        chaves,
        texto: meta?.texto ?? "",
        desfecho: comecou ? "cortada" : "atropelada",
        detalhe: {
          // A peça é o índice DENTRO da sequência registrada no começo, e é o que permite ler no
          // arquivo a metade que o jogador ouviu sem ter de adivinhar pelas durações.
          peca: pecaNoAr >= 0 ? pecaNoAr : undefined,
          pecas: chaves.length,
          // Duas medidas diferentes, e as duas importam: `dur_s` é o canal ocupado do começo até
          // aqui, `tocado_s` é só a última peça — uma resposta que morreu na nona frase ocupou o
          // rádio por oito segundos e perdeu meio.
          dur_s: noAr_s,
          tocado_s: medida.tocado_s,
        },
      });
    } else if (comecouEm) {
      // O FECHAMENTO da fala que saiu inteira. Linha própria porque a de abertura sai no instante
      // em que o som começa (é ela que casa com a decisão do Rust) e a duração só existe agora.
      // O leitor não a mostra na lista: ela serve ao resumo, e repeti-la ali seria contar cada
      // fala duas vezes.
      registrarRadio(canal, {
        chaves,
        desfecho: "fim",
        detalhe: { pecas: chaves.length, dur_s: noAr_s },
      });
    }
    // Solta o canal mesmo no caminho do `return false`. Sem o `finally`, uma resposta
    // atropelada deixaria a fila de anúncios travada para sempre — e o sintoma seria o rádio
    // emudecendo de vez depois de um push-to-talk, que ninguém ligaria à causa.
    liberarCanal(minhaVez);
  }
}

/** Base64 do servidor -> bytes. O áudio não é decodificado no Rust; ele só releia. */
function bytesDeBase64(b64) {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

/**
 * Nível DA FALA: RMS e pico contando só as janelas que têm sinal.
 *
 * As janelas mudas ficam de fora porque a proporção delas varia demais de uma frase para
 * outra (57% a 84% de fala, medido) — incluí-las faz uma frase com muita pausa ser
 * empurrada para cima até o arquivo inteiro bater no alvo, e ela sai gritando.
 */
function medirFala(amostras, taxa) {
  const jan = Math.max(1, Math.round(GATE_JANELA_S * taxa));
  const energias = [];
  let topo = 0;
  for (let i = 0; i < amostras.length; i += jan) {
    const ate = Math.min(i + jan, amostras.length);
    let soma = 0;
    for (let j = i; j < ate; j += 1) soma += amostras[j] * amostras[j];
    const ms = Math.sqrt(soma / (ate - i));
    energias.push({ i, ate, ms });
    if (ms > topo) topo = ms;
  }
  const limiar = Math.max(topo * 10 ** (-GATE_QUEDA_DB / 20), 10 ** (GATE_PISO_DB / 20));

  let soma = 0;
  let quadros = 0;
  let pico = 0;
  for (const w of energias) {
    if (w.ms < limiar) continue;
    for (let j = w.i; j < w.ate; j += 1) {
      const v = amostras[j];
      soma += v * v;
      const a = v < 0 ? -v : v;
      if (a > pico) pico = a;
    }
    quadros += w.ate - w.i;
  }
  // Peça sem nenhuma janela acima do limiar (silêncio puro): devolve zero em vez de NaN,
  // que num `gain` emudeceria o nó inteiro sem nada aparecer em lugar nenhum.
  return quadros ? { rms: Math.sqrt(soma / quadros), pico } : { rms: 0, pico: 0 };
}

/**
 * Passa o áudio cru da Cloud TTS pela cadeia de rádio FORA do tempo real e devolve o
 * buffer já processado e no volume do acervo — isto é, indistinguível de uma peça.
 *
 * Devolve `null` quando o render não é possível; quem chama volta para a cadeia ao vivo,
 * que soa igual e só erra o nível.
 *
 * O render custa uma vez por resposta e acontece antes do primeiro som. É o preço de
 * poder MEDIR: sem ele, o nível da voz do modelo seria uma constante chutada contra a
 * implementação do compressor de um navegador específico.
 */
async function processarComoPeca(buf) {
  ultimo = null;
  const OAC = window.OfflineAudioContext || window.webkitOfflineAudioContext;
  if (!OAC) return null;
  const t0 = performance.now();
  try {
    const quadros = buf.length + Math.round(CAUDA_RENDER_S * buf.sampleRate);
    // Um canal: a cadeia de rádio é monofônica por natureza, e o mixdown de um estéreo
    // acontece sozinho na conexão.
    const off = new OAC(1, quadros, buf.sampleRate);
    const fonte = off.createBufferSource();
    fonte.buffer = buf;
    fonte.connect(criarCadeiaRadio(off, { saida: off.destination }).entrada);
    fonte.start();
    const pronto = await off.startRendering();

    const amostras = pronto.getChannelData(0);
    const { rms, pico } = medirFala(amostras, buf.sampleRate);
    if (!(rms > 0)) return pronto; // silêncio: nada a nivelar, e a divisão seria infinita

    // O menor dos dois: chegar ao alvo de RMS, sem passar do teto de pico.
    const ganho = Math.min(RMS_ALVO / rms, PICO_TETO / (pico || 1));
    if (ganho < 0.98 || ganho > 1.02) {
      for (let i = 0; i < amostras.length; i += 1) amostras[i] *= ganho;
    }
    ultimo = { ms: performance.now() - t0, rms, pico, ganho };
    return pronto;
  } catch {
    return null;
  }
}

/**
 * O último nivelamento: `{ms, rms, pico, ganho}`, ou `null` se ele não aconteceu.
 *
 * Vai para o rastro do painel de diagnóstico, e por dois motivos. O `ms` é o único trecho
 * do caminho do modelo que roda na máquina do jogador ANTES do primeiro som — pode piorar
 * a latência sem aparecer em medida de rede nenhuma. E o `null` precisa ser VISÍVEL: sem
 * ele, "a voz saiu alta porque o nivelamento não rodou" e "a voz saiu alta mesmo depois de
 * nivelada" são a mesma queixa, e só a segunda quer dizer que o alvo está errado.
 */
export function ultimoRender() {
  return ultimo;
}

/**
 * **Toca a frase que o modelo escreveu**, em base64.
 *
 * Cede ao spotter igual às peças, com uma diferença: aqui não dá para repetir do começo
 * sem repetir uma frase inteira e longa, então a resposta do modelo simplesmente ESPERA
 * o canal abrir antes de começar, e se o spotter cortar no meio ela recomeça uma vez só.
 */
export async function falarRemoto(audioB64, mime = "audio/mpeg", { canal = "resposta", texto = "" } = {}) {
  if (!estaLigada()) {
    // A fala do MODELO descartada por voz desligada é a mais cara do rádio: o servidor já
    // cobrou a redação e a síntese. Sem linha, ela desaparece depois de paga.
    registrarRadio(canal, { texto, desfecho: "voz_desligada" });
    return false;
  }
  sequencia += 1;
  const minhaVez = sequencia;
  pararFonte();
  descartarFila();
  // Segura o canal como qualquer resposta. Sem isto, a resposta do modelo — que é a MAIS
  // longa do rádio e ainda paga um a três segundos de espera antes de começar — seria a mais
  // fácil de um anúncio atropelar.
  geracaoAtiva = minhaVez;
  try {
    return await tocarRemoto(audioB64, mime, minhaVez, { canal, texto });
  } finally {
    liberarCanal(minhaVez);
  }
}

/**
 * O laço de tocar áudio remoto, comum à RESPOSTA e ao ANÚNCIO.
 *
 * Está separado pelo mesmo motivo que [`tocarSequencia`]: quem decide se corta ou se espera
 * é o chamador. Aqui dentro só se decodifica, nivela e toca.
 */
async function tocarRemoto(audioB64, mime, minhaVez, meta = null) {
  const canal = meta?.canal ?? "resposta";
  const c = contexto();
  if (!c) return false;
  let buf;
  try {
    buf = await c.decodeAudioData(bytesDeBase64(audioB64).buffer);
  } catch {
    // Áudio ilegível. O servidor cobrou a síntese e o jogador não ouviu nada — é o desfecho
    // que precisa de nome próprio, senão a fala simplesmente não aparece em lugar nenhum.
    registrarRadio(canal, { texto: meta?.texto ?? "", desfecho: "sem_audio" });
    return false;
  }
  if (sequencia !== minhaVez) return false;
  void mime; // o decodificador do Chromium reconhece pelo conteúdo; o tipo vai junto p/ log

  // Vira peça aqui. Só quando o render não é possível é que a cadeia ao vivo entra —
  // ela soa igual, e o que se perde no caminho de exceção é a igualdade de VOLUME.
  const pronto = await processarComoPeca(buf);
  if (sequencia !== minhaVez) return false;
  const paraTocar = pronto ?? buf;
  const medida = {};
  const opcoes = { pelaCadeia: !pronto, medida };
  // A DURAÇÃO da fala longa. É o número que o rádio na classificação mais precisa: dez
  // segundos de dossiê lido em cima de uma volta de preparação é o defeito inteiro, e sem
  // isto ele continua sendo impressão de quem ouviu.
  const detalhe = {
    dur_s: Math.round(paraTocar.duration * 100) / 100,
    espera_ms: meta?.espera_ms,
    motivo: meta?.motivo,
  };

  for (let tentativa = 0; tentativa < 2; tentativa += 1) {
    await esperarSpotter();
    if (sequencia !== minhaVez) return false;
    // A REPETIÇÃO tem linha própria, e ela é a que explica o rádio da classificação: quando o
    // spotter abre o canal no meio, esta fala não continua de onde parou, ela começa de novo do
    // zero. Sem esta linha, dezesseis segundos de dossiê tocados duas vezes aparecem no arquivo
    // como dezesseis segundos tocados uma vez, e a diferença é tudo o que o jogador ouviu.
    registrarRadio(canal, {
      texto: meta?.texto ?? "",
      desfecho: tentativa === 0 ? "ok" : "repetida",
      detalhe: tentativa === 0 ? detalhe : { ...detalhe, tentativa },
    });
    const motivo = await tocarBuffer(paraTocar, minhaVez, opcoes);
    if (motivo === "fim") return true;
    if (motivo === "cedeu") {
      // O spotter passou na frente. Fica registrado quanto desta fala tinha saído até aqui: é o
      // par (cedeu, repetida) que mostra o dossiê recomeçando.
      registrarRadio(canal, {
        texto: meta?.texto ?? "",
        desfecho: "cedeu",
        detalhe: { ...detalhe, tocado_s: medida.tocado_s, tentativa, por: "spotter" },
      });
    }
    if (motivo === "cancelado") {
      registrarRadio(canal, {
        texto: meta?.texto ?? "",
        desfecho: "cortada",
        // Quanto dos dez segundos de dossiê o jogador ouviu antes de a fala morrer. Aqui é a
        // medida mais importante do rádio inteiro: a fala longa é a única que dura o bastante
        // para ser cortada pela metade sem ninguém notar no meio de uma volta.
        detalhe: { ...detalhe, tocado_s: medida.tocado_s },
      });
      return false;
    }
  }
  // Cedeu duas vezes: sai por cima na terceira, pelo mesmo motivo do teto acima. A linha diz
  // `por_cima`, porque aqui o rádio tem duas vozes ao mesmo tempo de propósito — e isso precisa
  // ser reconhecível no arquivo, senão vira "o spotter estava ininteligível" sem causa.
  registrarRadio(canal, {
    texto: meta?.texto ?? "",
    desfecho: "repetida",
    detalhe: { ...detalhe, tentativa: 2, por_cima: true },
  });
  const fim = (await tocarBuffer(paraTocar, minhaVez, opcoes)) === "fim";
  if (!fim) {
    registrarRadio(canal, {
      texto: meta?.texto ?? "",
      desfecho: "cortada",
      detalhe: { ...detalhe, tocado_s: medida.tocado_s },
    });
  }
  return fim;
}

/**
 * **Anuncia áudio que o modelo escreveu** — a fala longa dos momentos sem pressa.
 *
 * `anunciar` para peças, isto para o que veio do servidor. Entra na MESMA fila e paga as
 * mesmas regras: espera a vez, espera a pista esfriar, e vence pela validade se demorar
 * demais. A diferença com [`falarRemoto`] é toda a semântica — lá o piloto perguntou e a
 * resposta corta; aqui ninguém pediu, e quem não foi pedido espera.
 */
export function anunciarRemoto(audioB64, mime = "audio/mpeg", { canal = "ocasiao", texto = "" } = {}) {
  if (!estaLigada() || !audioB64) {
    registrarRadio(canal, {
      texto,
      desfecho: audioB64 ? "voz_desligada" : "sem_audio",
    });
    return Promise.resolve(false);
  }
  if (fila.length >= FILA_MAX) {
    descartados += 1;
    registrarRadio(canal, { texto, desfecho: "fila_cheia" });
    return Promise.resolve(false);
  }
  return new Promise((resolve) => {
    fila.push({ audioB64, mime, resolve, entrou: Date.now(), canal, texto });
    bombear();
  });
}
