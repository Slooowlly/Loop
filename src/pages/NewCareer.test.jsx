import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

import NewCareer from "./NewCareer";

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

  it("generates the world before showing category and team selection", async () => {
    renderPage();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_career_draft");
    });

    fireEvent.click(screen.getByRole("button", { name: /proximo|próximo/i }));
    fireEvent.change(screen.getByPlaceholderText("João Silva"), {
      target: { value: "Rodrigo Teste" },
    });
    fireEvent.click(screen.getByRole("button", { name: /proximo|próximo/i }));

    expect(screen.queryByText("Mazda MX-5 Rookie Cup")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /gerar hist.rico/i }));

    expect((await screen.findAllByText("Mazda MX-5 Rookie Cup")).length).toBeGreaterThan(0);
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

    fireEvent.click(screen.getByRole("button", { name: /proximo|próximo/i }));
    fireEvent.change(screen.getByPlaceholderText("João Silva"), {
      target: { value: "Rodrigo Teste" },
    });
    fireEvent.click(screen.getByRole("button", { name: /proximo|próximo/i }));
    fireEvent.click(screen.getByRole("button", { name: /gerar hist.rico/i }));

    fireEvent.click((await screen.findAllByText("Mazda MX-5 Rookie Cup")).at(-1));
    fireEvent.click(screen.getByRole("button", { name: /proximo|próximo/i }));
    expect(await screen.findByText("Racing Academy Red")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /voltar/i }));
    fireEvent.click(screen.getByText("Toyota GR86 Rookie Cup"));
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
    fireEvent.click(screen.getByRole("button", { name: /pr.ximo/i }));
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

  it("discards generated draft when pending player identity changes", async () => {
    renderPage();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_career_draft");
    });

    fireEvent.click(screen.getByRole("button", { name: /pr.ximo/i }));
    fireEvent.change(screen.getByPlaceholderText(/Jo.o Silva/), {
      target: { value: "Rodrigo Teste" },
    });
    fireEvent.click(screen.getByRole("button", { name: /pr.ximo/i }));
    fireEvent.click(screen.getByRole("button", { name: /gerar hist.rico/i }));

    expect((await screen.findAllByText("Mazda MX-5 Rookie Cup")).length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole("button", { name: /voltar/i }));
    fireEvent.click(screen.getByRole("button", { name: /voltar/i }));
    fireEvent.change(screen.getByPlaceholderText(/Jo.o Silva/), {
      target: { value: "Rodrigo Novo" },
    });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("discard_career_draft");
    });
  });
});
