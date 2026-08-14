import { render, act } from "@testing-library/react";

import OverlayVrWriter from "./OverlayVrWriter";

// O escritor da torre no VR. O que este teste guarda é o fim do fluxo, não o começo: quando
// os dados ao vivo somem (o iRacing fechou, a carreira foi descarregada, o gate de VR
// desligou), o escritor precisa mandar UM frame transparente e parar.
//
// Sem essa limpeza a última torre desenhada fica parada na memória compartilhada, e a layer
// dentro do iRacing continua subindo aquela imagem para o quad: o jogador enxerga, dentro do
// headset, um grid de dez voltas atrás com cara de agora. É o mesmo latch que o
// EngineerVrWriter já usava no fim da mensagem, e é o par do lado C++ que passou a rejeitar
// frame com contador parado — os dois juntos fecham o caso, cada um pela sua ponta.
//
// A outra metade do contrato é a limpeza ser UMA: um jato de transparente a 10 Hz custaria o
// `getImageData` de 8 MB e a travessia da ponte o tempo todo, com a tela vazia.

const invoke = vi.fn(() => Promise.resolve());
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args) => invoke(...args) }));
vi.mock("../lib/tauri", () => ({ estaNoTauri: () => true }));

// Dados ao vivo do barramento — o teste liga e desliga entre renders.
let dadosAoVivo = null;
vi.mock("./useOverlayData", () => ({ useOverlayData: () => dadosAoVivo }));
vi.mock("./useOverlayFlags", () => ({ useOverlayFlags: () => ({ vrOverlay: true }) }));

// Canvas minúsculo: o real é 1024×2048 (8 MB por getImageData) e aqui nada olha os pixels.
const drawTower = vi.fn();
vi.mock("./towerCanvas", () => ({
  VR_W: 4,
  VR_H: 2,
  drawTower: (...args) => drawTower(...args),
  preloadAssets: () => Promise.resolve({ carregado: true }),
}));
vi.mock("./towerAnimation", () => ({ createTowerAnimator: () => ({ hasMotion: () => false }) }));
vi.mock("./towerRows", () => ({ createTowerWindow: () => ({}) }));
vi.mock("./towerThemes", () => ({ VR_THEME: {} }));

const ctx = {
  clearRect: vi.fn(),
  getImageData: vi.fn(() => ({ data: new Uint8ClampedArray(4 * 2 * 4) })),
};

/// Deixa o laço rodar `ticks` voltas do gate ocioso (100 ms cada), com as promessas de cada
/// volta resolvidas antes da próxima.
async function avanca(ticks = 1) {
  for (let i = 0; i < ticks; i += 1) {
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
  }
}

beforeEach(() => {
  vi.useFakeTimers();
  invoke.mockClear();
  drawTower.mockClear();
  ctx.clearRect.mockClear();
  dadosAoVivo = null;
  HTMLCanvasElement.prototype.getContext = vi.fn(() => ctx);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("OverlayVrWriter", () => {
  it("sem dados ao vivo, limpa o quad uma vez e para de escrever", async () => {
    render(<OverlayVrWriter />);
    // O `tick` dispara já na montagem; a primeira volta é só microtask.
    await act(async () => {});

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("vr_overlay_write_frame", expect.any(Uint8Array));
    expect(ctx.clearRect).toHaveBeenCalledWith(0, 0, 4, 2);
    expect(drawTower).not.toHaveBeenCalled();

    // O latch segura: mais dez voltas do gate e nenhuma escrita nova.
    await avanca(10);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("com dados, desenha e escreve a cada volta do gate", async () => {
    dadosAoVivo = { cars: [] };
    render(<OverlayVrWriter />);
    await act(async () => {});
    invoke.mockClear();
    drawTower.mockClear();

    await avanca(3);
    expect(drawTower).toHaveBeenCalledTimes(3);
    expect(invoke).toHaveBeenCalledTimes(3);
  });

  it("ao perder os dados, manda a limpeza uma vez e volta a escrever quando eles voltam", async () => {
    dadosAoVivo = { cars: [] };
    const { rerender } = render(<OverlayVrWriter />);
    await act(async () => {});
    await avanca(2);
    invoke.mockClear();
    drawTower.mockClear();
    ctx.clearRect.mockClear();

    // Perdeu a fonte: o efeito zera o sourceRef na hora.
    dadosAoVivo = null;
    rerender(<OverlayVrWriter />);
    await avanca(1);

    expect(ctx.clearRect).toHaveBeenCalledTimes(1);
    expect(drawTower).not.toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledTimes(1);

    // Cinco voltas depois, continua uma só: a limpeza não vira jato de transparente.
    await avanca(5);
    expect(invoke).toHaveBeenCalledTimes(1);

    // Os dados voltam: o quad volta a receber a torre desenhada.
    dadosAoVivo = { cars: [] };
    rerender(<OverlayVrWriter />);
    await avanca(2);
    expect(drawTower).toHaveBeenCalled();
    expect(invoke.mock.calls.length).toBeGreaterThan(1);
  });
});
