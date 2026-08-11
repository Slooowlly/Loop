// A voz do spotter tem dois pontos onde uma fala pode morrer calada, e os dois já
// morderam: a corrida da decodificação (duas falas do mesmo lote de poll disputam o
// canal dentro do `await`) e o lugar único de espera (uma fala adiada sobrescrevia a
// anterior sem olhar prioridade). Estes testes escrevem a regra como verificação:
// prioridade decide as duas disputas, e o que cede a vez sai depois — nunca some.

import { beforeEach, describe, expect, it, vi } from "vitest";

// O que o áudio "tocou", na ordem. Cada fonte guarda a chave da fala via buffer.
let tocadas;
let fontes;

class FakeGain {
  constructor() {
    this.gain = {
      value: 1,
      cancelScheduledValues() {},
      setValueAtTime() {},
      linearRampToValueAtTime() {},
    };
  }
  connect() {}
}

class FakeAudioContext {
  constructor() {
    this.state = "running";
    this.currentTime = 0;
    this.destination = {};
  }
  resume() {
    return Promise.resolve();
  }
  createGain() {
    return new FakeGain();
  }
  // O `fetch` falso etiqueta os "bytes" com a chave; aqui a etiqueta vira o buffer.
  decodeAudioData(bytes) {
    return Promise.resolve({ chave: bytes.__chave });
  }
  createBufferSource() {
    const fonte = {
      buffer: null,
      onended: null,
      connect() {},
      start() {
        tocadas.push(fonte.buffer.chave);
        fontes.push(fonte);
      },
      stop() {},
    };
    return fonte;
  }
}

/** Espera as microtarefas e timers de 0 ms do `falar` assentarem. */
async function assentar() {
  await new Promise((r) => setTimeout(r, 0));
  await new Promise((r) => setTimeout(r, 0));
}

/** Encerra a fala que está tocando agora (dispara o `onended` da última fonte). */
async function terminarAtual() {
  fontes[fontes.length - 1].onended();
  await assentar();
}

async function montar() {
  tocadas = [];
  fontes = [];
  vi.resetModules();
  window.AudioContext = FakeAudioContext;
  // A URL termina no nome do arquivo, que É a chave — a etiqueta viaja no lugar dos bytes.
  global.fetch = vi.fn((url) => {
    const chave = String(url).split("/").pop().replace(/\.opus$/, "").replace(/_\d+$/, "");
    return Promise.resolve({ arrayBuffer: () => Promise.resolve({ __chave: chave }) });
  });
  return import("./spotterVoice");
}

beforeEach(() => {
  localStorage.clear();
});

describe("a corrida da decodificação respeita prioridade", () => {
  it("a fala mais urgente do lote vence, mesmo chegando primeiro", async () => {
    const { falar } = await montar();
    // O lote do poll entrega na ordem dos ids: "esquerda" (prioridade 5) e depois
    // "carro_atras" (3). Sem a guarda, quem chegasse por último ganhava a corrida e o
    // "esquerda" morria calado — a troca exata que a tabela de prioridade proíbe.
    const a = falar("esquerda");
    const b = falar("carro_atras");
    await Promise.all([a, b]);
    await assentar();
    expect(tocadas).toEqual(["esquerda"]);
  });

  it("quem perdeu a vez no lote sai depois, em vez de sumir", async () => {
    const { falar } = await montar();
    await Promise.all([falar("esquerda"), falar("carro_atras")]);
    await assentar();
    await terminarAtual();
    expect(tocadas).toEqual(["esquerda", "carro_atras"]);
  });

  it("entre iguais continua valendo a mais nova", async () => {
    const { falar } = await montar();
    // "esquerda" e "tres_largos" dividem o degrau 5; a escalada é a notícia mais nova.
    await Promise.all([falar("esquerda"), falar("tres_largos")]);
    await assentar();
    expect(tocadas).toEqual(["tres_largos"]);
  });
});

describe("o lugar de espera guarda a fala mais importante", () => {
  it("um lembrete não apaga um verde que já cedeu a vez", async () => {
    const { falar } = await montar();
    await falar("esquerda");
    await assentar();
    expect(tocadas).toEqual(["esquerda"]);
    // As duas cedem a vez ao "esquerda" que toca. O "verde" (4) chega primeiro; o
    // lembrete (1) chega depois e, antes da correção, o sobrescrevia — um aviso de tiro
    // único que o Rust já tinha dado por entregue sumia para sempre.
    await falar("verde");
    await falar("ainda_esquerda");
    await terminarAtual();
    expect(tocadas).toEqual(["esquerda", "verde"]);
  });
});
