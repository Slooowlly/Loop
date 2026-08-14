import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import MainMenu from "./MainMenu";

// O menu principal, com foco na EXCLUSÃO DE CARREIRA — a ação mais destrutiva da
// interface, e a que estava sem rede nenhuma. O modo de falha que se guarda aqui é o
// silêncio: o `catch` da exclusão era vazio, então uma exclusão que falhou fechava o
// diálogo, recarregava a lista e devolvia a carreira ao jogador com a mesma cara de
// sucesso. Quem apertou "Excluir" saía sem saber se apagou.

const mockInvoke = vi.fn();
const mockNavigate = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args) => mockInvoke(...args),
}));

vi.mock("react-router-dom", async (importOriginal) => ({
  ...(await importOriginal()),
  useNavigate: () => mockNavigate,
}));

// Áudio (Web Audio) e auto-update não têm o que dizer sobre o menu.
vi.mock("../utils/sfx", () => ({
  hover: vi.fn(),
  resume: vi.fn(),
  startAmbient: vi.fn(),
  stopAmbient: vi.fn(),
  whoosh: vi.fn(),
}));

vi.mock("../components/system/UpdaterProvider", () => ({
  useUpdater: () => ({ checkStable: vi.fn(), checkBeta: vi.fn(), status: "idle" }),
}));

const mockLoadCareer = vi.fn();
vi.mock("../stores/useCareerStore", () => ({
  default: (selector) => selector({ loadCareer: mockLoadCareer }),
}));

const SAVES = [
  {
    career_id: "C_ANTIGA",
    player_name: "Ana Prado",
    category_name: "GT3 Championship",
    last_played: "2026-08-01T10:00:00Z",
  },
  {
    career_id: "C_RECENTE",
    player_name: "Bruno Lima",
    category_name: "Mazda Rookie Cup",
    last_played: "2026-08-10T10:00:00Z",
  },
];

function renderMenu() {
  return render(
    <MemoryRouter initialEntries={["/menu"]}>
      <MainMenu />
    </MemoryRouter>,
  );
}

/// Abre o submenu "Carregar carreira", onde vive a lista com o botão de lixeira.
async function abrirPainelDeSaves() {
  fireEvent.click(screen.getByRole("button", { name: "Carregar carreira" }));
  return screen.findAllByRole("button", { name: "Deletar save" });
}

/// A carreira mais recente aparece DUAS vezes na tela: no cartão "Continuar" e na lista.
/// Consultas sobre a lista precisam do recorte, senão batem nas duas.
function painel() {
  return document.querySelector(".mm-panel");
}

beforeEach(() => {
  mockInvoke.mockReset();
  mockNavigate.mockReset();
  mockLoadCareer.mockReset();
  // Padrão: a carga nunca termina. `enterCareer` só navega DEPOIS da animação de saída
  // (`CUT_MS`), então uma carga que resolve sozinha deixa um `navigate` agendado para
  // depois do fim do teste — e ele cai no `mockNavigate` de outro caso, sob carga. Quem
  // precisa da carga concluída resolve ou rejeita explicitamente e espera pelo efeito.
  mockLoadCareer.mockReturnValue(new Promise(() => {}));
  // O fundo animado é canvas; em jsdom `getContext` devolve null e o menu estouraria.
  HTMLCanvasElement.prototype.getContext = () => ({
    setTransform: vi.fn(),
    clearRect: vi.fn(),
    fillRect: vi.fn(),
    beginPath: vi.fn(),
    arc: vi.fn(),
    fill: vi.fn(),
    createRadialGradient: () => ({ addColorStop: vi.fn() }),
  });
  mockInvoke.mockImplementation(async (comando) => {
    if (comando === "list_saves") return SAVES.map((s) => ({ ...s }));
    return null;
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("MainMenu — exclusão de carreira", () => {
  it("pede confirmação antes de apagar: a lixeira sozinha não exclui nada", async () => {
    renderMenu();
    const lixeiras = await abrirPainelDeSaves();

    fireEvent.click(lixeiras[0]);

    expect(screen.getByRole("button", { name: "Excluir" })).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("delete_career", expect.anything());
  });

  it("cancelar fecha a confirmação sem tocar no backend", async () => {
    renderMenu();
    const lixeiras = await abrirPainelDeSaves();

    fireEvent.click(lixeiras[0]);
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));

    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Excluir" })).not.toBeInTheDocument(),
    );
    expect(mockInvoke).not.toHaveBeenCalledWith("delete_career", expect.anything());
  });

  it("exclui a carreira CONFIRMADA, e não outra da lista", async () => {
    renderMenu();
    const lixeiras = await abrirPainelDeSaves();

    // A lista vem ordenada por última partida: Bruno (10/08) antes de Ana (01/08).
    expect(within(painel()).getByText("Bruno Lima")).toBeInTheDocument();
    fireEvent.click(lixeiras[1]); // a segunda linha é Ana
    fireEvent.click(screen.getByRole("button", { name: "Excluir" }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("delete_career", { careerId: "C_ANTIGA" }),
    );
  });

  it("recarrega a lista depois de apagar, para a carreira sumir da tela", async () => {
    let jaApagou = false;
    mockInvoke.mockImplementation(async (comando) => {
      if (comando === "delete_career") {
        jaApagou = true;
        return null;
      }
      if (comando === "list_saves") {
        return jaApagou ? [{ ...SAVES[1] }] : SAVES.map((s) => ({ ...s }));
      }
      return null;
    });

    renderMenu();
    const lixeiras = await abrirPainelDeSaves();

    fireEvent.click(lixeiras[1]);
    fireEvent.click(screen.getByRole("button", { name: "Excluir" }));

    await waitFor(() => expect(screen.queryByText("Ana Prado")).not.toBeInTheDocument());
    expect(within(painel()).getByText("Bruno Lima")).toBeInTheDocument();
  });

  it("falha na exclusão AVISA e mantém a confirmação aberta — não finge sucesso", async () => {
    mockInvoke.mockImplementation(async (comando) => {
      if (comando === "delete_career") throw new Error("arquivo em uso");
      if (comando === "list_saves") return SAVES.map((s) => ({ ...s }));
      return null;
    });

    renderMenu();
    const lixeiras = await abrirPainelDeSaves();

    fireEvent.click(lixeiras[1]);
    fireEvent.click(screen.getByRole("button", { name: "Excluir" }));

    // O aviso é o ponto do teste: sem ele, a carreira reaparecendo na lista parecia bug.
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    // A carreira continua lá, e a confirmação também — o jogador pode tentar de novo.
    expect(screen.getByText("Ana Prado")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Excluir" })).toBeInTheDocument();
  });

  it("o aviso de falha some ao reabrir o painel", async () => {
    mockInvoke.mockImplementation(async (comando) => {
      if (comando === "delete_career") throw new Error("arquivo em uso");
      if (comando === "list_saves") return SAVES.map((s) => ({ ...s }));
      return null;
    });

    renderMenu();
    const lixeiras = await abrirPainelDeSaves();
    fireEvent.click(lixeiras[1]);
    fireEvent.click(screen.getByRole("button", { name: "Excluir" }));
    await screen.findByRole("alert");

    // Fecha e reabre pelo mesmo botão do menu.
    fireEvent.click(screen.getByRole("button", { name: "Carregar carreira" }));
    fireEvent.click(screen.getByRole("button", { name: "Carregar carreira" }));

    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });
});

describe("MainMenu — lista de saves", () => {
  it("o cartão 'Continuar' aponta para a carreira jogada mais recentemente", async () => {
    renderMenu();

    await screen.findByText("Continuar carreira");
    // Bruno jogou em 10/08, Ana em 01/08 — mesmo vindo depois na resposta do backend.
    expect(screen.getByText("Bruno Lima")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Continuar carreira"));
    await waitFor(() => expect(mockLoadCareer).toHaveBeenCalledWith("C_RECENTE"));
  });

  it("sem nenhuma carreira, não oferece 'Continuar' e convida a criar uma", async () => {
    mockInvoke.mockImplementation(async (comando) => (comando === "list_saves" ? [] : null));

    renderMenu();
    fireEvent.click(screen.getByRole("button", { name: "Carregar carreira" }));

    expect(await screen.findByText("Nenhuma carreira ainda.")).toBeInTheDocument();
    expect(screen.queryByText("Continuar carreira")).not.toBeInTheDocument();
  });

  it("falha ao listar não vira 'nenhuma carreira' — o menu não inventa estado", async () => {
    mockInvoke.mockImplementation(async (comando) => {
      if (comando === "list_saves") throw new Error("pasta de saves inacessível");
      return null;
    });

    renderMenu();
    fireEvent.click(screen.getByRole("button", { name: "Carregar carreira" }));

    // Com a lista vazia por falha, o painel mostra o convite — mas nada é apagado nem
    // sobrescrito, e "Continuar" some em vez de apontar para uma carreira inexistente.
    expect(await screen.findByText("Nenhuma carreira ainda.")).toBeInTheDocument();
    expect(screen.queryByText("Continuar carreira")).not.toBeInTheDocument();
  });

  it("clicar num save da lista carrega aquela carreira", async () => {
    renderMenu();
    await abrirPainelDeSaves();

    fireEvent.click(screen.getByText("Ana Prado"));
    await waitFor(() => expect(mockLoadCareer).toHaveBeenCalledWith("C_ANTIGA"));
  });
});

// O outro silêncio do menu: entrar numa carreira. O `catch` de `enterCareer` só desfazia a
// animação de saída, então um save corrompido devolvia o jogador ao menu com a mesma cara de
// um clique que não pegou. Ele clicava de novo, no mesmo cartão, e nada acontecia de novo.
describe("MainMenu — entrar numa carreira", () => {
  it("save que não abre AVISA e mantém o jogador no menu", async () => {
    mockLoadCareer.mockRejectedValue(new Error("banco do save corrompido"));

    renderMenu();
    await screen.findByText("Continuar carreira");

    fireEvent.click(screen.getByText("Continuar carreira"));

    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(mockNavigate).not.toHaveBeenCalled();
    // O menu continua inteiro: dá para tentar outra carreira sem reabrir o app.
    expect(screen.getByText("Continuar carreira")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Carregar carreira" })).toBeInTheDocument();
  });

  it("a falha vale para a lista de saves também, e não só para 'Continuar'", async () => {
    mockLoadCareer.mockRejectedValue(new Error("banco do save corrompido"));

    renderMenu();
    await abrirPainelDeSaves();

    fireEvent.click(screen.getByText("Ana Prado"));

    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it("a tentativa seguinte limpa o aviso e entra na carreira", async () => {
    mockLoadCareer.mockRejectedValueOnce(new Error("banco do save corrompido"));

    renderMenu();
    await screen.findByText("Continuar carreira");

    fireEvent.click(screen.getByText("Continuar carreira"));
    await screen.findByRole("alert");

    mockLoadCareer.mockResolvedValue({ career_id: "C_RECENTE" });
    fireEvent.click(screen.getByText("Continuar carreira"));

    await waitFor(() => expect(mockNavigate).toHaveBeenCalledWith("/dashboard"));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
