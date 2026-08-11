import { renderHook } from "@testing-library/react";

import useTempoDeTela from "./useTempoDeTela";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args) => mockInvoke(...args),
}));

const envios = () => mockInvoke.mock.calls.filter(([cmd]) => cmd === "telemetria_tela");

beforeEach(() => {
  vi.useFakeTimers();
  mockInvoke.mockReset();
  mockInvoke.mockResolvedValue(undefined);
});

afterEach(() => {
  vi.useRealTimers();
});

it("reporta o tempo ao sair da tela", () => {
  const { unmount } = renderHook(() => useTempoDeTela("noticias"));
  vi.advanceTimersByTime(12_000);
  expect(envios()).toHaveLength(0);

  unmount();
  expect(envios()).toEqual([["telemetria_tela", { tela: "noticias", segundos: 12 }]]);
});

// Sem isto, quem lê o debriefing e fecha o Loop sem sair da tela levaria a leitura
// inteira embora — e é justamente a leitura mais longa do jogo.
it("descarrega o parcial a cada minuto com a tela aberta", () => {
  renderHook(() => useTempoDeTela("debriefing"));

  vi.advanceTimersByTime(60_000);
  expect(envios()).toHaveLength(1);
  expect(envios()[0][1]).toEqual({ tela: "debriefing", segundos: 60 });

  vi.advanceTimersByTime(60_000);
  expect(envios()).toHaveLength(2);
});

// O relógio reancora a cada descarga: dois minutos abertos são 60 + 60, e não 60 + 120.
it("não conta o mesmo segundo duas vezes", () => {
  const { unmount } = renderHook(() => useTempoDeTela("briefing"));
  vi.advanceTimersByTime(60_000);
  vi.advanceTimersByTime(30_000);
  unmount();

  const total = envios().reduce((soma, [, args]) => soma + args.segundos, 0);
  expect(total).toBe(90);
});

// Troca de aba com o dedo pesado e remontagem por `key` não são leitura.
it("ignora permanência de menos de dois segundos", () => {
  const { unmount } = renderHook(() => useTempoDeTela("noticias"));
  vi.advanceTimersByTime(900);
  unmount();
  expect(envios()).toHaveLength(0);
});

it("não reporta nada sem tela", () => {
  const { unmount } = renderHook(() => useTempoDeTela(null));
  vi.advanceTimersByTime(120_000);
  unmount();
  expect(envios()).toHaveLength(0);
});

// Telemetria não pode derrubar a tela do jogador. O stub síncrono é o mesmo caso dos
// testes das telas que montam este hook.
it("sobrevive a um invoke que não devolve promessa", () => {
  mockInvoke.mockReturnValue(undefined);
  const { unmount } = renderHook(() => useTempoDeTela("noticias"));
  vi.advanceTimersByTime(5000);
  expect(() => unmount()).not.toThrow();
  expect(envios()).toHaveLength(1);
});
