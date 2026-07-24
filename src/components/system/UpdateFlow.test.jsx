import { fireEvent, render, screen } from "@testing-library/react";
import i18n from "../../i18n";

import UpdateGate from "./UpdateGate";
import UpdateChangelogModal from "./UpdateChangelogModal";

// Ancorado na chave, não no texto: rótulo de botão é cópia e muda.
const txt = (key, opts) => i18n.t(`updater.${key}`, opts);

let ctx;
vi.mock("./UpdaterProvider", () => ({
  useUpdater: () => ctx,
}));

const NOTES = "Primeira mudança\nSegunda mudança";

function setup(overrides = {}) {
  ctx = {
    update: { version: "0.12.0", body: NOTES },
    channel: "stable",
    status: "available",
    progress: null,
    showChangelog: false,
    install: vi.fn(),
    dismiss: vi.fn(),
    openChangelog: vi.fn(),
    closeChangelog: vi.fn(),
    ...overrides,
  };
  return ctx;
}

const click = (key) => fireEvent.click(screen.getByText(txt(key)));

// O ponto da mudança: atualizar não instala direto — leva ao "o que mudou", e a
// instalação é confirmada lá.
it("com notas, atualizar abre o changelog em vez de instalar", () => {
  const c = setup();
  render(<UpdateGate />);

  click("install");

  expect(c.openChangelog).toHaveBeenCalled();
  expect(c.install).not.toHaveBeenCalled();
});

// Sem notas a confirmação seria uma tela vazia pedindo "confirme" duas vezes.
it("sem notas, atualizar instala direto", () => {
  const c = setup({ update: { version: "0.12.0", body: "   " } });
  render(<UpdateGate />);

  click("install");

  expect(c.install).toHaveBeenCalled();
  expect(c.openChangelog).not.toHaveBeenCalled();
});

it("o changelog lista um item por linha das notas", () => {
  setup({ showChangelog: true });
  render(<UpdateChangelogModal />);

  expect(screen.getByText("Primeira mudança")).toBeInTheDocument();
  expect(screen.getByText("Segunda mudança")).toBeInTheDocument();
  expect(screen.getByText(txt("versionLabel", { version: "0.12.0" }))).toBeInTheDocument();
});

it("confirmar no changelog instala, e fecha antes para o progresso aparecer", () => {
  const order = [];
  const c = setup({ showChangelog: true });
  c.closeChangelog = vi.fn(() => order.push("close"));
  c.install = vi.fn(() => order.push("install"));
  render(<UpdateChangelogModal />);

  click("confirmInstall");

  expect(c.install).toHaveBeenCalled();
  // A barra de progresso vive no UpdateGate, ATRÁS desta tela: instalar sem fechar
  // deixaria o download rodando escondido.
  expect(order).toEqual(["close", "install"]);
});

it("adiar no changelog não instala nada", () => {
  const c = setup({ showChangelog: true });
  render(<UpdateChangelogModal />);

  click("later");

  expect(c.install).not.toHaveBeenCalled();
  expect(c.closeChangelog).toHaveBeenCalled();
  expect(c.dismiss).toHaveBeenCalled();
});

// Fora do fluxo de atualização o changelog é só leitura — não pode oferecer instalar.
it("com o update já em andamento, o changelog não oferece confirmar", () => {
  setup({ showChangelog: true, status: "downloading" });
  render(<UpdateChangelogModal />);

  expect(screen.queryByText(txt("confirmInstall"))).not.toBeInTheDocument();
  expect(screen.getByText(txt("close"))).toBeInTheDocument();
});

it("não renderiza nada com o changelog fechado", () => {
  setup({ showChangelog: false });
  const { container } = render(<UpdateChangelogModal />);

  expect(container).toBeEmptyDOMElement();
});
