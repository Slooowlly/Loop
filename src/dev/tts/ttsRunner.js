// i18n-ignore-file — bancada da POC de TTS, fora do caminho do jogador.
// Orquestra UMA geração de ponta a ponta e devolve o registro medido.
//
// Sequência: abre o contexto de áudio -> assina os eventos -> zera o cronômetro ->
// invoca o Rust -> alimenta o reprodutor à medida que os blocos chegam -> fecha o
// registro. Nada aqui espera o áudio inteiro: o reprodutor começa sozinho quando o
// pré-buffer enche.
//
// Etapa 12 mora aqui: se o primeiro som não sair dentro do SLA, ou se der erro, a
// fala dinâmica é CANCELADA e uma frase local assume. A regra é que a geração remota
// nunca seja necessária — ela é um bônus que às vezes não vem.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ReprodutorStreaming } from "./ttsPlayer";
import { FALA_LOCAL_DE_RESERVA } from "./ttsScripts";
import { faseDaChamada } from "./ttsMetrics";

const IDIOMA = "pt-BR";

function novoId() {
  if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
  return `tts-${Date.now()}-${Math.floor(Math.random() * 1e6)}`;
}

/**
 * Reserva local da Etapa 12. Usa a síntese do próprio sistema (SAPI, no Windows):
 * é offline, instantânea e não depende de nada que a POC esteja testando. No produto
 * final isto vira um arquivo pré-gravado — o papel é o mesmo.
 */
export function tocarReservaLocal(texto = FALA_LOCAL_DE_RESERVA) {
  const sintese = globalThis.speechSynthesis;
  if (!sintese) return Promise.resolve({ disponivel: false, msAteFalar: null });

  return new Promise((resolve) => {
    const t0 = performance.now();
    const fala = new SpeechSynthesisUtterance(texto);
    fala.lang = IDIOMA;
    fala.rate = 1.05;
    let respondido = false;
    const responder = (disponivel) => {
      if (respondido) return;
      respondido = true;
      resolve({ disponivel, msAteFalar: performance.now() - t0 });
    };
    fala.onstart = () => responder(true);
    fala.onerror = () => responder(false);
    sintese.cancel();
    sintese.speak(fala);
    // Rede de segurança: se nem `onstart` nem `onerror` vierem, não trava a bateria.
    setTimeout(() => responder(false), 3000);
  });
}

/** Estado compartilhado entre execuções, para separar chamada fria de quente. */
const relogioDeUso = { ultimaEm: null, jaRodou: false };

export function reiniciarRelogioDeUso() {
  relogioDeUso.ultimaEm = null;
  relogioDeUso.jaRodou = false;
}

/**
 * @param {object} opcoes
 * @param {import("./ttsScripts").CATEGORIAS[number]} opcoes.categoria
 * @param {string} opcoes.texto
 * @param {number} opcoes.slaMs corte da Etapa 12 (0 = sem corte)
 * @param {(evento: object) => void} [opcoes.onEvento] progresso para a UI
 * @param {(cancelar: () => void) => void} [opcoes.exporCancelamento] Etapa 11
 */
export async function gerarFala({
  categoria,
  texto,
  voz,
  modelo,
  streaming = true,
  radio = false,
  prebufferMs = 120,
  usarDirecao = true,
  slaMs = 2500,
  onEvento,
  exporCancelamento,
}) {
  const requestId = novoId();
  const agoraMs = Date.now();
  const msDesdeUltima = relogioDeUso.ultimaEm == null ? null : agoraMs - relogioDeUso.ultimaEm;
  const fase = faseDaChamada({ primeiraDoProcesso: !relogioDeUso.jaRodou, msDesdeUltima });

  const registro = {
    id: requestId,
    quando: new Date(agoraMs).toISOString(),
    modelo,
    voz,
    idioma: IDIOMA,
    categoria: categoria.id,
    texto,
    caracteres: texto.length,
    palavras: texto.trim().split(/\s+/).length,
    streaming,
    radio,
    prebufferMs,
    direcao: usarDirecao ? categoria.direcao : null,
    fase,
    msDesdeUltima,
    // Métricas
    msPrimeiroBloco: null,
    msPrimeiroSom: null,
    msPrimeiroSomObservado: null,
    msTotal: null,
    duracaoAudioMs: null,
    msAteRespostaHttp: null,
    msPrimeiroBlocoRust: null,
    blocos: 0,
    bytesBase64: 0,
    interrupcoes: 0,
    latenciaSaidaMs: null,
    taxaContexto: null,
    // Desfecho
    sucesso: false,
    cancelado: false,
    estourouSla: false,
    reservaLocalUsada: false,
    msReservaLocal: null,
    erro: null,
  };

  const reprodutor = new ReprodutorStreaming({ prebufferMs, radio });
  const desassinar = [];
  // Marco do disparo. Declarado aqui porque os ouvintes abaixo o leem, mas só recebe
  // valor no instante exato do invoke — tudo depois dali é custo que o jogador paga.
  let t0 = 0;
  let finalizado = false;
  let resolverFim;
  const fim = new Promise((resolve) => {
    resolverFim = resolve;
  });

  const encerrar = (motivo) => {
    if (finalizado) return;
    finalizado = true;
    resolverFim(motivo);
  };

  const cancelar = (motivo = "cancelado") => {
    registro.cancelado = true;
    invoke("tts_poc_cancelar", { requestId }).catch(() => {});
    encerrar(motivo);
  };
  if (exporCancelamento) exporCancelamento(cancelar);

  try {
    await reprodutor.destravar();

    desassinar.push(
      await listen("tts-poc-chunk", (evento) => {
        const p = evento.payload;
        if (p.request_id !== requestId) return;
        if (registro.msPrimeiroBloco == null) {
          registro.msPrimeiroBloco = performance.now() - t0;
          registro.msPrimeiroBlocoRust = p.ms;
          onEvento?.({ tipo: "primeiroBloco", ms: registro.msPrimeiroBloco });
        }
        registro.blocos += 1;
        registro.bytesBase64 += p.bytes_b64;
        reprodutor.aceitar(p.b64);
        if (registro.msPrimeiroSom == null && reprodutor.metricas.msPrimeiroSomEstimado != null) {
          registro.msPrimeiroSom = reprodutor.metricas.msPrimeiroSomEstimado;
          onEvento?.({ tipo: "primeiroSom", ms: registro.msPrimeiroSom });
        }
      }),
    );

    desassinar.push(
      await listen("tts-poc-done", (evento) => {
        const p = evento.payload;
        if (p.request_id !== requestId) return;
        registro.msAteRespostaHttp = p.ms_ate_resposta;
        registro.msTotal = performance.now() - t0;
        encerrar("done");
      }),
    );

    desassinar.push(
      await listen("tts-poc-error", (evento) => {
        const p = evento.payload;
        if (p.request_id !== requestId) return;
        registro.erro = p.mensagem;
        registro.msTotal = performance.now() - t0;
        encerrar("erro");
      }),
    );

    // O cronômetro zera IMEDIATAMENTE antes do invoke.
    t0 = performance.now();
    reprodutor.iniciarCronometro(t0);
    relogioDeUso.jaRodou = true;
    relogioDeUso.ultimaEm = Date.now();

    // O SLA da Etapa 12 conta a partir do disparo, não do retorno do invoke — senão
    // o próprio tempo de ida e volta da IPC ficaria de fora do orçamento.
    let temporizadorSla = null;
    if (slaMs > 0) {
      temporizadorSla = setTimeout(() => {
        if (registro.msPrimeiroSom != null) return;
        registro.estourouSla = true;
        cancelar("sla");
      }, slaMs);
    }

    const inicio = await invoke("tts_poc_falar", {
      pedido: {
        request_id: requestId,
        texto,
        instrucao: usarDirecao ? categoria.direcao : null,
        voz,
        modelo,
        streaming,
      },
    });
    registro.modelo = inicio.modelo;
    registro.voz = inicio.voz;
    registro.origemChave = inicio.origem_chave;
    onEvento?.({ tipo: "enviado", inicio });

    await fim;
    if (temporizadorSla) clearTimeout(temporizadorSla);
  } catch (e) {
    registro.erro = String(e?.message ?? e);
  } finally {
    for (const fn of desassinar) {
      try {
        fn();
      } catch {
        /* ouvinte já removido */
      }
    }
  }

  // Fecha as contas com o que o reprodutor viu.
  reprodutor.finalizar();
  if (registro.msPrimeiroSom == null) {
    registro.msPrimeiroSom = reprodutor.metricas.msPrimeiroSomEstimado;
  }
  registro.msPrimeiroSomObservado = reprodutor.metricas.msPrimeiroSomObservado;
  registro.interrupcoes = reprodutor.metricas.interrupcoes;
  registro.duracaoAudioMs = reprodutor.duracaoAudioMs;
  registro.latenciaSaidaMs = reprodutor.metricas.latenciaSaidaMs;
  registro.taxaContexto = reprodutor.metricas.taxaContexto;
  registro.sucesso = !registro.erro && !registro.estourouSla && registro.msPrimeiroSom != null;

  if (registro.cancelado) {
    await reprodutor.encerrar({ interrompendo: true });
  } else {
    await reprodutor.aguardarFim();
    await reprodutor.encerrar();
  }

  // A fala dinâmica falhou -> a local entra. Registrar isso importa tanto quanto a
  // latência: é a taxa com que o jogador ouviria o plano B.
  if (!registro.sucesso && !registro.cancelado) {
    const reserva = await tocarReservaLocal();
    registro.reservaLocalUsada = reserva.disponivel;
    registro.msReservaLocal = reserva.msAteFalar;
  }

  // Persiste no JSONL antes de devolver: uma bateria interrompida não perde medição.
  try {
    await invoke("tts_poc_log_registrar", { linha: JSON.stringify(registro) });
  } catch {
    /* o log em disco é conveniência; a tabela na tela é a fonte imediata */
  }

  onEvento?.({ tipo: "fim", registro });
  return registro;
}
