// Reprodutor de PCM em STREAMING sobre Web Audio, com instrumentação.
//
// A regra da POC: não esperar a fala inteira. Assim que houver uma quantidade segura
// de áudio (o "pré-buffer"), começa a tocar; os blocos seguintes são AGENDADOS colados
// no fim do anterior, então não há junta audível entre eles.
//
// Por que agendar em vez de tocar na chegada: `AudioBufferSourceNode.start(t)` usa o
// relógio de áudio, que é preciso ao nível da amostra. Tocar "quando chegar" deixaria
// cada bloco à mercê do jitter do event loop e da IPC do Tauri, e o resultado seria
// estalo entre blocos.
//
// MEDIÇÃO DO PRIMEIRO SOM — a métrica que importa. Não dá para perguntar ao navegador
// "já saiu som?"; o que existe é o instante AGENDADO no relógio de áudio. Então o
// número principal é calculado:
//
//     primeiroSom = agora + (instanteAgendado - relogioDeAudio) + latenciaDeSaida
//
// `latenciaDeSaida` (`AudioContext.outputLatency`) é o atraso do buffer do driver até
// o alto-falante — ignorá-lo mentiria a favor do resultado. Em paralelo, um laço de
// rAF confirma o instante observado (±16 ms) e serve de checagem de sanidade contra
// a conta acima.

import { criarConversorPcm } from "./ttsPcm";
import { criarCadeiaRadio } from "./ttsRadio";

const TAXA_TTS = 24000; // Hz — o que a API devolve
const FOLGA_S = 0.02; // margem para nunca agendar no passado

export class ReprodutorStreaming {
  /**
   * @param {object} opcoes
   * @param {number} opcoes.prebufferMs quanto áudio acumular antes do primeiro som
   * @param {boolean} opcoes.radio aplicar a cadeia de rádio (Etapa 10)
   */
  constructor({ prebufferMs = 120, radio = false } = {}) {
    this.prebufferS = Math.max(0, prebufferMs) / 1000;
    this.radioLigado = radio;

    // `latencyHint: "interactive"` pede o menor buffer que o dispositivo aceita.
    // Pedir 24 kHz evita uma reamostragem no caminho; se o dispositivo recusar, o
    // AudioBuffer continua sendo criado a 24 kHz e o próprio contexto reamostra.
    this.ctx = new AudioContext({ sampleRate: TAXA_TTS, latencyHint: "interactive" });
    this.cadeia = criarCadeiaRadio(this.ctx, this.radioLigado);
    this.conversor = criarConversorPcm();

    this.fila = []; // blocos ainda não agendados (antes do pré-buffer encher)
    this.filaAmostras = 0;
    this.proximoInicio = 0; // instante (relógio de áudio) do próximo bloco
    this.iniciado = false;
    this.encerrado = false;
    this.fontes = [];

    this.metricas = {
      amostras: 0,
      blocos: 0,
      interrupcoes: 0, // buffer secou entre blocos = corte audível
      msPrimeiroSomEstimado: null,
      msPrimeiroSomObservado: null,
      latenciaSaidaMs: 0,
      latenciaBaseMs: 0,
      taxaContexto: this.ctx.sampleRate,
      radio: this.radioLigado,
      prebufferMs,
    };

    this.t0 = null; // marco do disparo, em performance.now()
    this._resolverPrimeiroSom = null;
    this.primeiroSom = new Promise((resolve) => {
      this._resolverPrimeiroSom = resolve;
    });
  }

  /** O contexto nasce suspenso até um gesto do usuário; o clique no botão libera. */
  async destravar() {
    if (this.ctx.state === "suspended") await this.ctx.resume();
  }

  /** Marca o instante do disparo. Tudo é medido a partir daqui. */
  iniciarCronometro(t0 = performance.now()) {
    this.t0 = t0;
  }

  get duracaoAudioMs() {
    return (this.metricas.amostras / TAXA_TTS) * 1000;
  }

  /** Recebe um bloco base64 vindo do Rust. */
  aceitar(b64) {
    if (this.encerrado) return;
    const amostras = this.conversor.converter(b64);
    if (amostras.length === 0) return;

    this.metricas.blocos += 1;
    this.metricas.amostras += amostras.length;

    if (this.iniciado) {
      this._agendar(amostras);
      return;
    }

    this.fila.push(amostras);
    this.filaAmostras += amostras.length;
    if (this.filaAmostras / TAXA_TTS >= this.prebufferS) this._comecar();
  }

  /**
   * Fim do stream. Se o pré-buffer nunca encheu (fala curtíssima), toca o que houver —
   * do contrário uma fala de 200 ms com pré-buffer de 250 ms nunca sairia.
   */
  finalizar() {
    if (!this.iniciado && this.fila.length > 0) this._comecar();
  }

  _comecar() {
    this.iniciado = true;
    const relogio = this.ctx.currentTime;
    const agora = performance.now();
    const inicio = relogio + FOLGA_S;
    this.proximoInicio = inicio;

    const saida = this.ctx.outputLatency || 0;
    const base = this.ctx.baseLatency || 0;
    this.metricas.latenciaSaidaMs = saida * 1000;
    this.metricas.latenciaBaseMs = base * 1000;

    // A conta central da POC (ver cabeçalho).
    const previsto = agora + (inicio - relogio) * 1000 + saida * 1000;
    this.metricas.msPrimeiroSomEstimado = this.t0 == null ? null : previsto - this.t0;

    const pendentes = this.fila;
    this.fila = [];
    this.filaAmostras = 0;
    for (const bloco of pendentes) this._agendar(bloco);

    this._observarPrimeiroSom(inicio);
    if (this._resolverPrimeiroSom) this._resolverPrimeiroSom(this.metricas.msPrimeiroSomEstimado);
  }

  /** Confirmação independente, por rAF, de que o relógio de áudio passou do instante. */
  _observarPrimeiroSom(inicio) {
    const olhar = () => {
      if (this.encerrado) return;
      if (this.ctx.currentTime >= inicio) {
        this.metricas.msPrimeiroSomObservado =
          this.t0 == null ? null : performance.now() - this.t0;
        return;
      }
      requestAnimationFrame(olhar);
    };
    requestAnimationFrame(olhar);
  }

  _agendar(amostras) {
    const buffer = this.ctx.createBuffer(1, amostras.length, TAXA_TTS);
    buffer.copyToChannel(amostras, 0);

    // Se o instante do próximo bloco já passou, o buffer secou: houve silêncio no meio
    // da fala. Isso é o "corte" que a Etapa 2 pede para contar.
    if (this.proximoInicio < this.ctx.currentTime) {
      this.metricas.interrupcoes += 1;
      this.proximoInicio = this.ctx.currentTime + FOLGA_S;
    }

    const fonte = this.ctx.createBufferSource();
    fonte.buffer = buffer;
    fonte.connect(this.cadeia.entrada);
    fonte.start(this.proximoInicio);
    this.fontes.push(fonte);
    this.proximoInicio += buffer.duration;
  }

  /**
   * Resolve quando o último bloco agendado terminar de tocar. Só depois disso o
   * contexto pode ser fechado — fechar antes cortaria o fim da fala.
   */
  aguardarFim() {
    return new Promise((resolve) => {
      const olhar = () => {
        if (this.encerrado || !this.iniciado) return resolve();
        const restanteMs = (this.proximoInicio - this.ctx.currentTime) * 1000;
        if (restanteMs <= 0) return resolve();
        return setTimeout(olhar, Math.min(200, Math.max(20, restanteMs)));
      };
      olhar();
    });
  }

  /** Corta a fala na hora (Etapa 11: a vitória evaporou). */
  interromper() {
    for (const fonte of this.fontes) {
      try {
        fonte.stop();
      } catch {
        /* já terminou */
      }
    }
    this.fontes = [];
  }

  async encerrar({ interrompendo = false } = {}) {
    if (this.encerrado) return;
    this.encerrado = true;
    if (interrompendo) this.interromper();
    try {
      await this.ctx.close();
    } catch {
      /* contexto já fechado */
    }
  }
}
