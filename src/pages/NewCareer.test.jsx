import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

import NewCareer, { totalDeMensagensDeCarregamento } from "./NewCareer";
import ptBR from "../i18n/locales/pt-BR/common.json";

const mockInvoke = vi.fn();
const mockLoadCareer = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args) => mockInvoke(...args),
}));

vi.mock("../stores/useCareerStore", () => ({
  default: (selector) =>
    selector({
      loadCareer: mockLoadCareer,
    }),
}));

const generatedDraft = {
  exists: true,
  career_id: "career_001",
  lifecycle_status: "draft",
  progress_year: 2025,
  error: null,
  categories: ["mazda_rookie", "toyota_rookie"],
  teams: [
    {
      id: "TEAM001",
      nome: "Racing Academy Red",
      nome_curto: "RAR",
      categoria: "mazda_rookie",
      cor_primaria: "#e63946",
      cor_secundaria: "#101010",
      car_performance: 68,
      reputacao: 42,
      n1_nome: "Ana Costa",
      n2_nome: "Bruno Lima",
    },
    {
      id: "TEAM002",
      nome: "Sakura Driver Academy",
      nome_curto: "SDA",
      categoria: "toyota_rookie",
      cor_primaria: "#d90429",
      cor_secundaria: "#101010",
      car_performance: 74,
      reputacao: 44,
      n1_nome: "Ken Mori",
      n2_nome: "Luis Rocha",
    },
  ],
};

function renderPage() {
  return render(
    <MemoryRouter>
      <NewCareer />
    </MemoryRouter>,
  );
}

function mockDraftCommands() {
  mockInvoke.mockImplementation(async (command) => {
    if (command === "get_career_draft") {
      return {
        exists: false,
        career_id: null,
        lifecycle_status: "active",
        progress_year: null,
        error: null,
        categories: [],
        teams: [],
      };
    }

    if (command === "create_historical_career_draft") {
      return generatedDraft;
    }

    if (command === "finalize_career_draft") {
      return { success: true, career_id: "career_001" };
    }

    if (command === "update_career_draft_identity") {
      return generatedDraft;
    }

    if (command === "discard_career_draft") {
      return null;
    }

    return null;
  });
}

describe("NewCareer", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockLoadCareer.mockReset();
    mockDraftCommands();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  // O funil abre direto na identidade: a escolha de dificuldade saiu em 16/08/2026, porque
  // a dificuldade da IA passou a ser adaptativa. O valor continua indo para o backend fixo
  // em "medio", que é o que o teste de geração abaixo trava.
  it("abre no passo do piloto, sem pedir dificuldade", async () => {
    renderPage();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_career_draft");
    });

    expect(screen.getByPlaceholderText("João Silva")).toBeInTheDocument();
    expect(screen.queryByText("Lendario")).not.toBeInTheDocument();
    expect(screen.queryByText("Dificuldade")).not.toBeInTheDocument();
  });

  it("generates the world before showing category and team selection", async () => {
    renderPage();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_career_draft");
    });

    fireEvent.change(screen.getByPlaceholderText("João Silva"), {
      target: { value: "Rodrigo Teste" },
    });
    fireEvent.click(screen.getByRole("button", { name: /proximo|próximo/i }));

    expect(screen.queryByText("Mazda Rookie")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /gerar hist.rico/i }));

    expect((await screen.findAllByText("Mazda Rookie")).length).toBeGreaterThan(0);
    expect(mockInvoke).toHaveBeenCalledWith("create_historical_career_draft", {
      input: {
        player_name: "Rodrigo Teste",
        player_nationality: "br",
        player_age: 20,
        difficulty: "medio",
      },
    });
    expect(mockInvoke).not.toHaveBeenCalledWith("create_career", expect.anything());
  });

  it("does not regenerate the draft when navigating back after generation", async () => {
    renderPage();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_career_draft");
    });

    fireEvent.change(screen.getByPlaceholderText("João Silva"), {
      target: { value: "Rodrigo Teste" },
    });
    fireEvent.click(screen.getByRole("button", { name: /proximo|próximo/i }));
    fireEvent.click(screen.getByRole("button", { name: /gerar hist.rico/i }));

    fireEvent.click((await screen.findAllByText("Mazda Rookie")).at(-1));
    fireEvent.click(screen.getByRole("button", { name: /proximo|próximo/i }));
    expect(await screen.findByText("Racing Academy Red")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /voltar/i }));
    fireEvent.click(screen.getByText("Toyota Rookie"));
    fireEvent.click(screen.getByRole("button", { name: /proximo|próximo/i }));

    expect(screen.getByText("Sakura Driver Academy")).toBeInTheDocument();
    expect(mockInvoke.mock.calls.filter(([command]) => command === "create_historical_career_draft"))
      .toHaveLength(1);
  });

  it("polls draft progress while historical generation is running", async () => {
    vi.useFakeTimers();
    let resolveGeneration;
    let progressYear = 2004;
    let generationStarted = false;
    mockInvoke.mockImplementation((command) => {
      if (command === "get_career_draft") {
        if (!generationStarted) return Promise.resolve(null);
        return Promise.resolve({
          ...generatedDraft,
          teams: [],
          categories: [],
          progress_year: progressYear,
        });
      }
      if (command === "create_historical_career_draft") {
        generationStarted = true;
        return new Promise((resolve) => {
          resolveGeneration = resolve;
        });
      }
      return Promise.resolve(null);
    });

    renderPage();
    fireEvent.change(screen.getByPlaceholderText(/Jo.o Silva/), {
      target: { value: "Rodrigo Teste" },
    });
    fireEvent.click(screen.getByRole("button", { name: /pr.ximo/i }));
    fireEvent.click(screen.getByRole("button", { name: /gerar hist.rico/i }));

    progressYear = 2012;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });

    expect(screen.getByText("Simulando temporada 2012")).toBeInTheDocument();

    await act(async () => {
      resolveGeneration(generatedDraft);
      await Promise.resolve();
    });
  });

  // O Atlas embaixo das mensagens de espera lê o banco do rascunho enquanto ele é
  // escrito. O gatilho tem que ser o ANO fechado, e não o tique de um segundo do
  // polling: a leitura disputa lock com a geração, e uma por segundo devolveria em
  // espera parte do tempo que a criação do save acabou de ganhar.
  it("recarrega o atlas a cada temporada concluída, e não a cada tique do polling", async () => {
    vi.useFakeTimers();
    let resolveGeneration;
    let progressYear = 2004;
    let generationStarted = false;
    const historyCalls = [];
    mockInvoke.mockImplementation((command, args) => {
      if (command === "get_career_draft") {
        if (!generationStarted) return Promise.resolve(null);
        return Promise.resolve({
          ...generatedDraft,
          teams: [],
          categories: [],
          progress_year: progressYear,
        });
      }
      if (command === "create_historical_career_draft") {
        generationStarted = true;
        return new Promise((resolve) => {
          resolveGeneration = resolve;
        });
      }
      if (command === "get_global_team_history") {
        historyCalls.push(args);
        return Promise.resolve(null);
      }
      return Promise.resolve(null);
    });

    renderPage();
    fireEvent.change(screen.getByPlaceholderText(/Jo.o Silva/), {
      target: { value: "Rodrigo Teste" },
    });
    fireEvent.click(screen.getByRole("button", { name: /pr.ximo/i }));
    fireEvent.click(screen.getByRole("button", { name: /gerar hist.rico/i }));

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(historyCalls).toHaveLength(1);
    expect(historyCalls[0].careerId).toBe("career_001");
    // A escada nasce no GT3, e é nele que o painel abre.
    expect(historyCalls[0].family).toBe("gt3");

    // Três tiques do polling no mesmo ano: nenhuma leitura nova.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(historyCalls).toHaveLength(1);

    // Temporada fechada: uma leitura, e só uma.
    progressYear = 2005;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(historyCalls).toHaveLength(2);

    await act(async () => {
      resolveGeneration(generatedDraft);
      await Promise.resolve();
    });
  });

  it("restores the pending player identity when resuming a saved draft", async () => {
    mockInvoke.mockImplementation(async (command) => {
      if (command === "get_career_draft") {
        return {
          ...generatedDraft,
          player_name: "Carlos Magno",
          player_nationality: "us",
          player_age: 24,
          difficulty: "dificil",
        };
      }
      return null;
    });

    renderPage();

    // O resumo lateral é o que mostrava "Piloto novo" ao retomar o draft.
    expect(await screen.findByText("Carlos Magno")).toBeInTheDocument();
    expect(screen.queryByText("Piloto novo")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /voltar/i }));
    fireEvent.click(screen.getByRole("button", { name: /voltar/i }));
    expect(screen.getByPlaceholderText(/Jo.o Silva/)).toHaveValue("Carlos Magno");

    // Reabrir o passo de identidade não pode descartar o mundo já simulado.
    expect(mockInvoke).not.toHaveBeenCalledWith("discard_career_draft");
  });

  it("keeps the generated draft and rewrites the identity when the name changes", async () => {
    renderPage();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_career_draft");
    });

    fireEvent.change(screen.getByPlaceholderText(/Jo.o Silva/), {
      target: { value: "Rodrigo Teste" },
    });
    fireEvent.click(screen.getByRole("button", { name: /pr.ximo/i }));
    fireEvent.click(screen.getByRole("button", { name: /gerar hist.rico/i }));

    expect((await screen.findAllByText("Mazda Rookie")).length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole("button", { name: /voltar/i }));
    fireEvent.click(screen.getByRole("button", { name: /voltar/i }));
    fireEvent.change(screen.getByPlaceholderText(/Jo.o Silva/), {
      target: { value: "Carlos Magno" },
    });
    fireEvent.click(screen.getByRole("button", { name: /pr.ximo/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("update_career_draft_identity", {
        input: {
          career_id: "career_001",
          player_name: "Carlos Magno",
          player_nationality: "br",
          player_age: 20,
        },
      });
    });
    expect(mockInvoke).not.toHaveBeenCalledWith("discard_career_draft");
  });

  // O ciclo de mensagens do overlay é `(i + 1) % total`. Se o `total` vier de qualquer lista
  // que não seja o próprio locale, o índice passa do fim das chaves e a tela mostra
  // `newCareer.loadingMessages.msg75` cru para o jogador.
  it("conta as mensagens de carregamento na mesma fonte que a tela renderiza", () => {
    const doLocale = Object.keys(ptBR.newCareer.loadingMessages).length;

    expect(doLocale).toBeGreaterThan(0);
    expect(totalDeMensagensDeCarregamento()).toBe(doLocale);
  });
});
