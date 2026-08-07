import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

// jsdom não tem Web Audio. O contexto falso abaixo é suficiente porque o que está
// sob teste é a ARITMÉTICA de agendamento — quando começar, onde encaixar cada bloco
// e quando o buffer secou. A síntese de fato é verificada no navegador.
class NoFalso {
  constructor() {
    this.ligadoEm = [];
    this.gain = { value: 1 };
    this.frequency = { value: 0 };
    this.Q = { value: 0 };
    this.threshold = { value: 0 };
    this.knee = { value: 0 };
    this.ratio = { value: 0 };
    this.attack = { value: 0 };
    this.release = { value: 0 };
  }

  connect(destino) {
    this.ligadoEm.push(destino);
    return destino;
  }
}

class AudioContextFalso {
  constructor({ sampleRate } = {}) {
    this.sampleRate = sampleRate ?? 48000;
    this.currentTime = 0;
    this.state = "suspended";
    this.outputLatency = 0.02;
    this.baseLatency = 0.01;
    this.destination = new NoFalso();
    this.agendamentos = [];
    this.fechado = false;
  }

  createGain() {
    return new NoFalso();
  }
  createBiquadFilter() {
    return new NoFalso();
  }
  createWaveShaper() {
    return new NoFalso();
  }
  createDynamicsCompressor() {
    return new NoFalso();
  }

  createBuffer(canais, amostras, taxa) {
    return { duration: amostras / taxa, length: amostras, copyToChannel: () => {} };
  }

  createBufferSource() {
    const ctx = this;
    return {
      buffer: null,
      connect: () => {},
      start(t) {
        ctx.agendamentos.push({ t, duracao: this.buffer.duration });
      },
      stop: () => {},
    };
  }

  async resume() {
    this.state = "running";
  }
  async close() {
    this.fechado = true;
  }
}

/** Base64 de N amostras silenciosas (2 bytes cada). */
function blocoDe(amostras) {
  let binario = "";
  for (let i = 0; i < amostras * 2; i += 1) binario += "\0";
  return btoa(binario);
}

let ReprodutorStreaming;

beforeEach(async () => {
  globalThis.AudioContext = AudioContextFalso;
  vi.stubGlobal("requestAnimationFrame", () => 0);
  ({ ReprodutorStreaming } = await import("./ttsPlayer"));
});

afterEach(() => {
  vi.unstubAllGlobals();
  delete globalThis.AudioContext;
});

describe("ReprodutorStreaming", () => {
  it("segura o áudio até o pré-buffer encher", async () => {
    const r = new ReprodutorStreaming({ prebufferMs: 200 });
    await r.destravar();
    r.iniciarCronometro(0);

    // 100 ms de áudio a 24 kHz = 2400 amostras. Metade do pré-buffer.
    r.aceitar(blocoDe(2400));
    expect(r.iniciado).toBe(false);
    expect(r.ctx.agendamentos).toHaveLength(0);

    r.aceitar(blocoDe(2400));
    expect(r.iniciado).toBe(true);
    // Os dois blocos represados entram de uma vez.
    expect(r.ctx.agendamentos).toHaveLength(2);
  });

  it("encaixa os blocos colados, sem buraco entre eles", async () => {
    const r = new ReprodutorStreaming({ prebufferMs: 0 });
    await r.destravar();
    r.iniciarCronometro(0);

    r.aceitar(blocoDe(2400)); // 100 ms
    r.aceitar(blocoDe(1200)); // 50 ms
    r.aceitar(blocoDe(2400)); // 100 ms

    const [a, b, c] = r.ctx.agendamentos;
    expect(b.t).toBeCloseTo(a.t + a.duracao, 6);
    expect(c.t).toBeCloseTo(b.t + b.duracao, 6);
    expect(r.metricas.interrupcoes).toBe(0);
  });

  it("conta interrupção quando o buffer seca entre blocos", async () => {
    const r = new ReprodutorStreaming({ prebufferMs: 0 });
    await r.destravar();
    r.iniciarCronometro(0);

    r.aceitar(blocoDe(2400)); // agenda 100 ms de áudio
    expect(r.metricas.interrupcoes).toBe(0);

    // O relógio de áudio avança MAIS que a duração agendada: houve silêncio.
    r.ctx.currentTime = 5;
    r.aceitar(blocoDe(2400));
    expect(r.metricas.interrupcoes).toBe(1);
  });

  it("toca o que tiver quando o stream acaba antes de encher o pré-buffer", async () => {
    const r = new ReprodutorStreaming({ prebufferMs: 500 });
    await r.destravar();
    r.iniciarCronometro(0);

    r.aceitar(blocoDe(1200)); // 50 ms, muito abaixo do pré-buffer
    expect(r.iniciado).toBe(false);

    r.finalizar();
    expect(r.iniciado).toBe(true);
    expect(r.ctx.agendamentos).toHaveLength(1);
  });

  it("inclui a latência de saída do dispositivo no tempo até o primeiro som", async () => {
    const r = new ReprodutorStreaming({ prebufferMs: 0 });
    await r.destravar();
    const t0 = performance.now();
    r.iniciarCronometro(t0);
    r.aceitar(blocoDe(2400));

    // folga de agendamento (20 ms) + latência de saída (20 ms) = 40 ms de piso.
    expect(r.metricas.msPrimeiroSomEstimado).toBeGreaterThanOrEqual(39);
    expect(r.metricas.latenciaSaidaMs).toBe(20);
  });

  it("contabiliza a duração do áudio recebido", async () => {
    const r = new ReprodutorStreaming({ prebufferMs: 0 });
    await r.destravar();
    r.iniciarCronometro(0);
    r.aceitar(blocoDe(24000)); // exatamente 1 s
    expect(r.duracaoAudioMs).toBeCloseTo(1000, 3);
  });

  it("monta a cadeia de rádio só quando pedida", async () => {
    const limpo = new ReprodutorStreaming({ radio: false });
    expect(limpo.cadeia.ligado).toBe(false);
    expect(limpo.cadeia.nos).toHaveLength(0);

    const comRadio = new ReprodutorStreaming({ radio: true });
    expect(comRadio.cadeia.ligado).toBe(true);
    // passa-altas, passa-baixas, presença, saturação, ganho, compressor,
    // recuperação de nível, o meio-ganho que encolhe a entrada para a faixa que o
    // `WaveShaper` enxerga, e o limitador.
    expect(comRadio.cadeia.nos).toHaveLength(9);
  });
});
