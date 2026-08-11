// Captura do microfone para o push-to-talk do engenheiro.
//
// O ciclo é: ARMAR uma vez (ao entrar na sessão) e depois gravar/parar a cada toque no
// botão. A separação existe por causa da latência: `getUserMedia` custa de 100 a 500 ms
// na primeira chamada — enumeração de dispositivo, driver, negociação de formato — e
// pagar isso DEPOIS que o piloto já começou a falar significa perder a primeira sílaba.
// Com o stream aberto, `comecar()` custa alguns milissegundos.
//
// O preço de deixar aberto é honesto e visível: o Windows mostra o indicador de
// microfone em uso enquanto a sessão dura. É o comportamento de qualquer software de
// voz em jogo, e o jogador arma isso de propósito.
//
// O formato é WebM/Opus, que é o que o MediaRecorder do WebView2 entrega nativamente.
// Uma fala de três segundos sai com cerca de 9 KB — o upload some dentro do RTT.
//
// PERMISSÃO: o WebView2 não pergunta nada porque as janelas sobem com
// `--use-fake-ui-for-media-stream` (ver `additionalBrowserArgs` no tauri.conf.json). O
// nome do parâmetro engana: ele não falsifica o dispositivo, ele aceita automaticamente
// o pedido de permissão. Sem isso o WebView2 abriria um diálogo próprio — impossível de
// responder por cima de um overlay transparente e click-through, e pior ainda de VR.

// Abaixo disso a gravação é um toque acidental no botão, não uma pergunta. Quem chama
// decide o que fazer; aqui só marcamos.
const CURTA_DEMAIS_MS = 250;

// Ordem de preferência de container. O primeiro que o WebView2 aceitar ganha.
const FORMATOS = ["audio/webm;codecs=opus", "audio/webm", "audio/ogg;codecs=opus", ""];

// 24 kbps mono é folgado para voz em Opus — o Scribe recebe muito mais do que precisa e
// o arquivo continua pequeno o bastante para o upload não aparecer no orçamento.
const TAXA_BITS = 24000;

let stream = null;
let contexto = null;
let analisador = null;
let janelaDoNivel = null; // Float32Array reaproveitado, para o medidor não alocar a 60 Hz
let formato = "";
let gravacao = null; // { rec, pedacos, inicioMs }
// O que foi PEDIDO da última vez (não o que se conseguiu), para `armar()` saber se um
// chamado sem dispositivo é uma troca de volta ao padrão ou um "deixe como está".
let pedidoAnterior = null;
// O dispositivo pedido não existia e abrimos outro. Quem mostra isso na tela é a
// interface; o valor existe para ela não ter que adivinhar comparando rótulos.
let caiuNoPadrao = false;

/** O formato que o WebView2 aceita, na ordem da nossa preferência. */
function escolherFormato() {
  if (typeof MediaRecorder === "undefined") return null;
  for (const f of FORMATOS) {
    if (f === "" || MediaRecorder.isTypeSupported(f)) return f;
  }
  return null;
}

/**
 * A faixa aberta ainda grava?
 *
 * `stream` continuar não-nulo NÃO quer dizer que há captura. Quando o dispositivo sai
 * debaixo da captura — o headset de VR que desliga, o iRacing que toma a placa, o Windows
 * que troca o padrão — a `MediaStreamTrack` vai para `ended` e o objeto do stream continua
 * ali, idêntico ao de antes. Sem esta conferência `estaArmado()` mente, o `MediaRecorder`
 * nasce em cima de uma faixa morta, e o que sobe para o Scribe é silêncio: o piloto
 * pergunta, o engenheiro não responde, e nada na tela diz por quê.
 */
function faixaViva() {
  const faixa = stream?.getAudioTracks()[0];
  return Boolean(faixa) && faixa.readyState === "live";
}

export function estaArmado() {
  return faixaViva();
}

/** O que o navegador chama de microfone. Os rótulos só aparecem depois da permissão. */
export async function dispositivos() {
  if (!navigator.mediaDevices?.enumerateDevices) return [];
  const todos = await navigator.mediaDevices.enumerateDevices();
  return todos
    .filter((d) => d.kind === "audioinput")
    .map((d) => ({ id: d.deviceId, rotulo: d.label || "" }));
}

/**
 * Abre o microfone e deixa aberto. Chamar duas vezes com o MESMO dispositivo não faz
 * nada; com outro, troca (o jogador mexeu nas configurações no meio da sessão).
 *
 * `cancelamentoDeEco` liga o AEC do Chromium, que subtrai do microfone o que o próprio
 * webview está tocando. Vale a pena por um motivo específico: o engenheiro e o spotter
 * falam pelo mesmo alto-falante, e num headset de VR de concha aberta (Index, Quest) o
 * microfone os escuta. Sem o AEC, apertar o botão enquanto o engenheiro ainda fala faria
 * o Scribe transcrever a nossa própria voz junto com a do piloto.
 *
 * Devolve o retrato do que foi aberto, ou lança com uma mensagem em português.
 */
export async function armar({ deviceId = null, cancelamentoDeEco = true } = {}) {
  if (!navigator.mediaDevices?.getUserMedia) {
    throw new Error("Este webview não expõe captura de áudio.");
  }
  const fmt = escolherFormato();
  if (fmt === null) throw new Error("Este webview não sabe gravar áudio (sem MediaRecorder).");

  if (stream) {
    const atual = stream.getAudioTracks()[0]?.getSettings?.().deviceId;
    // Faixa morta não conta como "já aberto": os dois atalhos abaixo devolveriam o retrato
    // de um microfone que não grava mais nada, e quem chamou iria embora achando que
    // rearmou. É o caso que este módulo passou a existir para pegar — ver `faixaViva`.
    const viva = faixaViva();
    // Já aberto no dispositivo pedido: nada a fazer. Note que "sem pedido" NÃO conta como
    // acerto — quem chama sem `deviceId` e encontra outro aberto está pedindo o padrão, e
    // devolver o que está aberto esconderia a diferença.
    if (viva && deviceId && deviceId === atual) return retrato();
    if (viva && !deviceId && !pedidoAnterior) return retrato();
    desarmar();
  }

  const base = {
    channelCount: 1,
    echoCancellation: cancelamentoDeEco,
    // A cabine é barulhenta — motor, vento, pedal, o próprio volante batendo. As duas
    // ficam ligadas porque é o perfil em que o Scribe foi treinado (voz de chamada),
    // não o de estúdio.
    noiseSuppression: true,
    autoGainControl: true,
  };

  // `exact` de propósito, e não `ideal`: com `ideal` o navegador cai no padrão do Windows
  // em silêncio quando o dispositivo some, e o jogador descobre que o engenheiro está
  // ouvindo a placa de áudio errada só quando ninguém responde. Com `exact` a falha é
  // explícita — e a queda para o padrão acontece aqui embaixo, DECLARADA.
  caiuNoPadrao = false;
  try {
    stream = await navigator.mediaDevices.getUserMedia({
      audio: deviceId ? { ...base, deviceId: { exact: deviceId } } : base,
    });
  } catch (e) {
    const sumiu = e?.name === "OverconstrainedError" || e?.name === "NotFoundError";
    if (!deviceId || !sumiu) {
      stream = null;
      throw new Error(traduzirFalha(e));
    }
    // O microfone escolhido não existe mais (desconectado, ou o id mudou). Um microfone
    // qualquer é melhor que nenhum no meio de uma corrida — mas quem chamou precisa
    // SABER, senão vira exatamente o silêncio que o `exact` foi posto para evitar.
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: base });
      caiuNoPadrao = true;
    } catch (e2) {
      stream = null;
      throw new Error(traduzirFalha(e2));
    }
  }
  pedidoAnterior = deviceId || null;
  formato = fmt;

  // Medidor de nível. Existe para uma coisa só, e ela não é enfeite: sem ele, um
  // microfone mudo (cabo solto, dispositivo errado, mudo no hardware) é indistinguível
  // de um engenheiro que não entendeu a pergunta. O jogador precisa VER que foi ouvido.
  try {
    const AC = window.AudioContext || window.webkitAudioContext;
    contexto = new AC();
    analisador = contexto.createAnalyser();
    analisador.fftSize = 1024;
    janelaDoNivel = new Float32Array(analisador.fftSize);
    contexto.createMediaStreamSource(stream).connect(analisador);
  } catch {
    // Sem medidor a captura continua funcionando; só o retorno visual se perde.
    contexto = null;
    analisador = null;
  }

  return retrato();
}

function retrato() {
  const faixa = stream?.getAudioTracks()[0];
  const cfg = faixa?.getSettings?.() ?? {};
  return {
    rotulo: faixa?.label || "",
    deviceId: cfg.deviceId || "",
    taxa: cfg.sampleRate || contexto?.sampleRate || 0,
    canais: cfg.channelCount || 1,
    cancelamentoDeEco: Boolean(cfg.echoCancellation),
    caiuNoPadrao,
    formato,
  };
}

/**
 * O retrato do que está aberto AGORA, ou `null`.
 *
 * Existe porque duas telas mostram o mesmo microfone, e cada uma guardando a sua cópia é
 * como o painel de teste passou a exibir "Virtual Desktop Audio" depois de o jogador ter
 * trocado para o USB na linha de cima: a troca aconteceu no módulo, e a cópia dele não
 * soube. Há um microfone só; que haja uma fonte só.
 */
export function retratoAtual() {
  return stream ? retrato() : null;
}

/** Fecha o microfone. O indicador do Windows apaga. */
export function desarmar() {
  if (gravacao) {
    try {
      gravacao.rec.stop();
    } catch {
      /* já parou */
    }
    gravacao = null;
  }
  stream?.getTracks().forEach((t) => t.stop());
  stream = null;
  pedidoAnterior = null;
  caiuNoPadrao = false;
  contexto?.close().catch(() => {});
  contexto = null;
  analisador = null;
  janelaDoNivel = null;
}

/**
 * O volume de agora, de 0 a 1 (RMS da janela). Serve para desenhar o medidor; não é
 * medida de nada. Devolve 0 sem microfone armado.
 */
export function nivel() {
  if (!analisador || !janelaDoNivel) return 0;
  analisador.getFloatTimeDomainData(janelaDoNivel);
  let soma = 0;
  for (let i = 0; i < janelaDoNivel.length; i += 1) soma += janelaDoNivel[i] * janelaDoNivel[i];
  return Math.sqrt(soma / janelaDoNivel.length);
}

export function estaGravando() {
  return Boolean(gravacao);
}

/**
 * Começa a gravar. Resolve quando o MediaRecorder está de fato gravando — não quando
 * pedimos —, e devolve o atraso entre uma coisa e outra. Esse número é o que decide se
 * dá para começar a gravar no toque do botão ou se precisamos de um anel de pré-captura.
 */
export async function comecar() {
  if (!stream) throw new Error("Microfone não está armado.");
  if (gravacao) return { atrasoMs: 0, jaGravava: true };

  const pedido = performance.now();
  const rec = new MediaRecorder(stream, {
    ...(formato ? { mimeType: formato } : {}),
    audioBitsPerSecond: TAXA_BITS,
  });
  const pedacos = [];
  rec.ondataavailable = (e) => {
    if (e.data && e.data.size) pedacos.push(e.data);
  };

  return new Promise((resolve, reject) => {
    rec.onerror = (e) => reject(new Error(`Falha ao gravar: ${e?.error?.name || "desconhecida"}`));
    rec.onstart = () => {
      const inicioMs = performance.now();
      gravacao = { rec, pedacos, inicioMs };
      resolve({ atrasoMs: inicioMs - pedido, jaGravava: false });
    };
    // Sem fatiar: um pedaço só, entregue no `stop`. Fatiar serviria para streaming, e
    // aqui o áudio inteiro sobe de uma vez depois que o piloto solta o botão.
    rec.start();
  });
}

/**
 * Para de gravar e devolve o que foi capturado:
 * `{ bytes, mime, duracaoMs, curtaDemais }`. `bytes` é o corpo que vai para o proxy.
 *
 * `curtaDemais` marca o toque acidental. Não descartamos aqui de propósito: quem chama
 * sabe se está num teste (onde tudo interessa) ou no meio de uma corrida (onde 120 ms de
 * ruído não merecem uma viagem ao Scribe).
 */
export async function terminar() {
  if (!gravacao) return null;
  const { rec, pedacos, inicioMs } = gravacao;
  gravacao = null;

  return new Promise((resolve) => {
    rec.onstop = async () => {
      const duracaoMs = performance.now() - inicioMs;
      const blob = new Blob(pedacos, { type: formato || "audio/webm" });
      const bytes = new Uint8Array(await blob.arrayBuffer());
      resolve({
        bytes,
        mime: blob.type,
        duracaoMs,
        curtaDemais: duracaoMs < CURTA_DEMAIS_MS,
      });
    };
    try {
      rec.stop();
    } catch {
      resolve(null);
    }
  });
}

/** Descarta a gravação em andamento sem devolver nada. */
export async function abortar() {
  if (!gravacao) return;
  await terminar();
}

/** As recusas do `getUserMedia` que o jogador tem alguma chance de consertar sozinho. */
function traduzirFalha(e) {
  switch (e?.name) {
    case "NotAllowedError":
    case "SecurityError":
      return "Permissão de microfone negada. O Windows pode estar bloqueando o app em Privacidade → Microfone.";
    case "NotFoundError":
    case "OverconstrainedError":
      return "Nenhum microfone encontrado — ou o dispositivo escolhido foi desconectado.";
    case "NotReadableError":
      return "O microfone existe mas outro programa está com ele preso.";
    default:
      return `Falha ao abrir o microfone: ${e?.name || e?.message || "desconhecida"}`;
  }
}
