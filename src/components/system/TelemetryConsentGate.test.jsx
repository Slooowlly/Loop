import { fireEvent, render, screen } from "@testing-library/react";
import i18n from "../../i18n";

import TelemetryConsentGate from "./TelemetryConsentGate";

// Ancorado na CHAVE, não no texto: a cópia deste aviso já mudou duas vezes, e um teste
// que casa a frase literal quebra a cada ajuste de redação sem nenhum bug envolvido.
const txt = (key) => i18n.t(`telemetryConsent.${key}`);

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args) => mockInvoke(...args),
}));

// Store fatiado como o componente consome (selector puro), no mesmo molde do
// Dashboard.test.jsx. Trocar o objeto + rerender simula a corrida terminando.
let storeState = { showResult: false, lastRaceOrigem: null };
vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(storeState),
}));

const baseConfig = { language: "pt-BR", autosave_enabled: true, version: "1.0.0" };

function mockConfig(telemetryEnabled) {
  mockInvoke.mockImplementation((cmd) => {
    if (cmd === "get_config") {
      return Promise.resolve({ ...baseConfig, telemetry_enabled: telemetryEnabled });
    }
    return Promise.resolve(undefined);
  });
}

// Adiantar o relógio de forma assíncrona também drena o `await invoke("get_config")`.
async function settle(ms = 1000) {
  await vi.advanceTimersByTimeAsync(ms);
}

function renderGate() {
  const view = render(<TelemetryConsentGate />);
  return {
    ...view,
    async openResult(origem) {
      storeState = { showResult: true, lastRaceOrigem: origem };
      view.rerender(<TelemetryConsentGate />);
      await settle(0);
    },
    async closeResult() {
      storeState = { ...storeState, showResult: false };
      view.rerender(<TelemetryConsentGate />);
      await settle();
    },
  };
}

// Correu de verdade: resultado importado do iRacing e a tela fechada.
async function raceInIracing(gate) {
  await gate.openResult("iracing");
  await gate.closeResult();
}

// Simulou o fim de semana dentro do app: mesma tela, outro caminho. Também gera
// evento de produto (`race_sim`), então também vale a pergunta.
async function simulateRace(gate) {
  await gate.openResult("simulada");
  await gate.closeResult();
}

// Reabriu uma corrida antiga pela Home. Não gera evento nenhum.
async function reopenOldRace(gate) {
  await gate.openResult("reaberta");
  await gate.closeResult();
}

const asked = () => screen.queryByText(txt("title")) !== null;
const lastUpdate = () =>
  mockInvoke.mock.calls.filter(([cmd]) => cmd === "update_config").at(-1);

beforeEach(() => {
  vi.useFakeTimers();
  mockInvoke.mockReset();
  storeState = { showResult: false, lastRaceOrigem: null };
});

afterEach(() => {
  vi.useRealTimers();
});

// O ponto do redesenho: no dia 1 o jogador não sabe o que é o Loop, então não é hora
// de perguntar. Nem `get_config` deve ser chamado antes de haver uma corrida.
it("não pergunta na abertura do app", async () => {
  mockConfig(null);
  renderGate();
  await settle(5000);
  expect(asked()).toBe(false);
  expect(mockInvoke).not.toHaveBeenCalled();
});

it("não pergunta com a tela de resultado ainda aberta", async () => {
  mockConfig(null);
  const gate = renderGate();
  await gate.openResult("iracing");
  await settle(5000);
  expect(asked()).toBe(false);
});

it("pergunta ao fechar o resultado da primeira corrida DIRIGIDA no iRacing", async () => {
  mockConfig(null);
  const gate = renderGate();
  await raceInIracing(gate);
  expect(asked()).toBe(true);
});

// O simulado passou a contar quando o `race_sim` nasceu: quem joga só simulando é
// justamente a população que a telemetria não enxergava, e sem a pergunta ela
// continuaria invisível por mais que o evento exista.
it("pergunta ao fechar o resultado de uma corrida SIMULADA", async () => {
  mockConfig(null);
  const gate = renderGate();
  await simulateRace(gate);
  expect(asked()).toBe(true);
});

// Reabertura não gera evento nenhum — perguntar ali é interromper por nada.
it("não pergunta a quem só reabriu uma corrida antiga", async () => {
  mockConfig(null);
  const gate = renderGate();
  await reopenOldRace(gate);
  await reopenOldRace(gate);
  expect(asked()).toBe(false);
  expect(mockInvoke).not.toHaveBeenCalled();
});

it("pergunta quando o jogador corre depois de só reabrir corridas antigas", async () => {
  mockConfig(null);
  const gate = renderGate();
  await reopenOldRace(gate);
  expect(asked()).toBe(false);

  await raceInIracing(gate);
  expect(asked()).toBe(true);
});

it.each([
  ["aceitou antes", true],
  ["recusou antes", false],
])("não pergunta de novo quando o jogador já %s", async (_label, answered) => {
  mockConfig(answered);
  const gate = renderGate();
  await raceInIracing(gate);
  expect(asked()).toBe(false);
});

it("aceitar grava true e fecha", async () => {
  mockConfig(null);
  const gate = renderGate();
  await raceInIracing(gate);

  fireEvent.click(screen.getByText(txt("accept")));
  await settle(0);

  expect(lastUpdate()[1].newConfig.telemetry_enabled).toBe(true);
  expect(asked()).toBe(false);
});

// O ponto do tri-estado: recusar precisa gravar `false` EXPLÍCITO. Deixar em `null`
// faria a pergunta voltar na corrida seguinte, insistindo com quem já disse não.
it("recusar grava false explícito, não null", async () => {
  mockConfig(null);
  const gate = renderGate();
  await raceInIracing(gate);

  fireEvent.click(screen.getByText(txt("decline")));
  await settle(0);

  expect(lastUpdate()[1].newConfig.telemetry_enabled).toBe(false);
  expect(asked()).toBe(false);
});

it("não volta a perguntar na corrida seguinte depois de respondida", async () => {
  mockConfig(null);
  const gate = renderGate();
  await raceInIracing(gate);
  fireEvent.click(screen.getByText(txt("decline")));
  await settle(0);

  // Segunda corrida: agora o config no disco tem a resposta.
  mockConfig(false);
  await raceInIracing(gate);
  expect(asked()).toBe(false);
});

it("preserva o resto do config ao responder", async () => {
  mockConfig(null);
  const gate = renderGate();
  await raceInIracing(gate);

  fireEvent.click(screen.getByText(txt("accept")));
  await settle(0);

  expect(lastUpdate()[1].newConfig).toMatchObject(baseConfig);
});

// Fechar no fundo deixaria o config em `null` e traria a pergunta de volta.
it("não fecha por clique no fundo", async () => {
  mockConfig(null);
  const gate = renderGate();
  await raceInIracing(gate);

  fireEvent.click(gate.container.querySelector(".fixed > .absolute"));
  await settle(0);

  expect(asked()).toBe(true);
  expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "update_config")).toBe(false);
});

it("some em silêncio se o config não carregar", async () => {
  mockInvoke.mockRejectedValue(new Error("sem config"));
  const gate = renderGate();
  await raceInIracing(gate);
  expect(asked()).toBe(false);
});
