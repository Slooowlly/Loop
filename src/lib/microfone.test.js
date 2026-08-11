import { beforeEach, describe, expect, it, vi } from "vitest";

// A faixa que MORRE é o assunto deste arquivo.
//
// `stream` continuar não-nulo depois que o dispositivo sai debaixo da captura foi o
// defeito que deixou o engenheiro surdo em corrida: `estaArmado()` respondia `true`, o
// push-to-talk seguia em frente e o que subia para o Scribe era silêncio. Como nada
// disso lança, a única prova possível é esta — abrir, matar a faixa, e conferir que o
// módulo passou a admitir que não tem microfone.

/** Faixa de mentira, com o `readyState` na mão de quem escreve o teste. */
function criarFaixa(deviceId) {
  return {
    kind: "audio",
    label: `Microfone ${deviceId || "padrão"}`,
    readyState: "live",
    getSettings: () => ({ deviceId, sampleRate: 48000, channelCount: 1, echoCancellation: true }),
    stop() {
      this.readyState = "ended";
    },
  };
}

function criarStream(deviceId) {
  const faixa = criarFaixa(deviceId);
  return { faixa, getAudioTracks: () => [faixa], getTracks: () => [faixa] };
}

let aberto = null;
let aberturas = 0;

beforeEach(async () => {
  vi.resetModules();
  aberto = null;
  aberturas = 0;

  // O `exact` do módulo pede um dispositivo específico; aqui ele sempre existe, porque a
  // queda para o padrão já tem o seu caminho e não é o que se mede neste arquivo.
  navigator.mediaDevices = {
    getUserMedia: vi.fn(async (restricoes) => {
      aberturas += 1;
      aberto = criarStream(restricoes?.audio?.deviceId?.exact ?? "");
      return aberto;
    }),
    enumerateDevices: vi.fn(async () => []),
  };
  // O módulo desiste do medidor em silêncio quando o contexto falha, e o medidor não é o
  // assunto — deixar o `AudioContext` ausente é o caminho mais curto para isso.
  delete window.AudioContext;
  delete window.webkitAudioContext;

  globalThis.MediaRecorder = class {
    static isTypeSupported() {
      return true;
    }
  };
});

describe("microfone", () => {
  it("admite que não tem microfone quando a faixa aberta morre", async () => {
    const microfone = await import("./microfone.js");

    await microfone.armar({ deviceId: "usb-1" });
    expect(microfone.estaArmado()).toBe(true);

    // O headset de VR desligou, ou o iRacing tomou a placa. O objeto do stream continua
    // ali, idêntico; só a faixa mudou de estado.
    aberto.faixa.readyState = "ended";

    expect(microfone.estaArmado()).toBe(false);
  });

  it("rearma no MESMO dispositivo depois de a faixa morrer", async () => {
    const microfone = await import("./microfone.js");

    await microfone.armar({ deviceId: "usb-1" });
    expect(aberturas).toBe(1);

    // Pedir de novo com a faixa VIVA é o atalho de sempre: não custa uma abertura.
    await microfone.armar({ deviceId: "usb-1" });
    expect(aberturas).toBe(1);

    aberto.faixa.readyState = "ended";
    await microfone.armar({ deviceId: "usb-1" });

    // Sem a conferência de faixa, o atalho devolvia o retrato do microfone morto e esta
    // segunda abertura nunca acontecia — que é a corrida inteira sem captura.
    expect(aberturas).toBe(2);
    expect(microfone.estaArmado()).toBe(true);
  });

  it("rearma no PADRÃO depois de a faixa morrer", async () => {
    const microfone = await import("./microfone.js");

    await microfone.armar({});
    expect(aberturas).toBe(1);
    await microfone.armar({});
    expect(aberturas).toBe(1);

    aberto.faixa.readyState = "ended";
    await microfone.armar({});

    expect(aberturas).toBe(2);
    expect(microfone.estaArmado()).toBe(true);
  });

  it("desarmar deixa o módulo sem microfone", async () => {
    const microfone = await import("./microfone.js");

    await microfone.armar({ deviceId: "usb-1" });
    microfone.desarmar();

    expect(microfone.estaArmado()).toBe(false);
    expect(microfone.retratoAtual()).toBeNull();
  });
});
