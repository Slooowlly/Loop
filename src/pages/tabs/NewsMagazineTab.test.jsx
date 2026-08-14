import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import NewsMagazineTab from "./NewsMagazineTab";

// A revista não tinha teste nenhum (A10.4). O que ela decide, e é o que está coberto
// aqui, é QUAL das três faces abrir: o livro fechado (capa), o spread de pré-temporada e
// a edição da corrida. A escolha depende do calendário, que chega depois da montagem — e
// abrir a face errada por alguns quadros é o "pisca" que o `calendarLoaded` existe para
// evitar.

let mockState;

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const CORRIDAS = [
  { rodada: 1, track_name: "Interlagos", status: "Concluida" },
  { rodada: 2, track_name: "Spa", status: "Concluida" },
  { rodada: 3, track_name: "Monza", status: "Agendada" },
];

/// Toda a carga da revista cai em `catch` e vira vazio/placeholder por contrato. O padrão
/// aqui responde o mínimo; cada caso troca o que importa para ele.
function configurarBackend(respostas = {}) {
  mockState = {
    careerId: "C1",
    playerTeam: { id: "T1", categoria: "gt3" },
    season: { id: "S1", ano: 2026 },
    language: "pt-BR",
  };
  const padrao = {
    get_calendar_for_category: [],
    get_teams_standings: [],
    get_drivers_by_category: [],
    get_world_footer: { notes: [] },
    enrich_world_footer_ai: { source: "template", notes: [] },
    get_inbox_messages: [],
    player_race_news_id: null,
    enrich_season_preview_ai: {},
    ...respostas,
  };
  invoke.mockReset();
  invoke.mockImplementation((comando) => {
    if (!(comando in padrao)) return Promise.resolve(null);
    const r = padrao[comando];
    return typeof r === "function" ? r() : Promise.resolve(r);
  });
}

const revista = () => document.querySelector("article.mag");

beforeEach(() => configurarBackend());

describe("NewsMagazineTab — abertura", () => {
  it("enquanto o calendário não volta, segura a escolha em vez de piscar a face errada", () => {
    configurarBackend({ get_calendar_for_category: () => new Promise(() => {}) });
    render(<NewsMagazineTab />);

    expect(revista()).toHaveClass("mag--abrindo");
    // Folha em branco reservando a altura: nada de conteúdo dentro.
    expect(revista().children).toHaveLength(0);
  });

  it("com corridas concluídas, abre na edição mais recente", async () => {
    configurarBackend({ get_calendar_for_category: CORRIDAS });
    render(<NewsMagazineTab />);

    expect(await screen.findByText("Etapa 2 · Temporada 2026")).toBeInTheDocument();
    // Edição 2 de 3: o total conta o calendário inteiro, não só o que já correu.
    expect(screen.getByText(/Edição 2 de 3 · Temporada 2026/)).toBeInTheDocument();
    expect(revista()).not.toHaveClass("mag--abrindo");
    expect(revista()).not.toHaveClass("mag--cover");
  });

  it("sem nenhuma corrida disputada, abre o spread de pré-temporada", async () => {
    configurarBackend({
      get_calendar_for_category: [{ rodada: 1, track_name: "Interlagos", status: "Agendada" }],
    });
    render(<NewsMagazineTab />);

    expect(await screen.findByText("Expectativas da temporada")).toBeInTheDocument();
    expect(screen.getByText("Edição de Pré-Temporada · Temporada 2026")).toBeInTheDocument();
  });

  it("sem categoria, cai no livro fechado", async () => {
    configurarBackend();
    mockState = { ...mockState, playerTeam: null };
    render(<NewsMagazineTab />);

    await waitFor(() => expect(revista()).toHaveClass("mag--cover"));
    expect(
      screen.getByText("A revista abre quando você disputar a primeira corrida da temporada."),
    ).toBeInTheDocument();
  });
});

describe("NewsMagazineTab — erro e vazio", () => {
  it("calendário que falha não trava a revista: ela abre na pré-temporada", async () => {
    configurarBackend({
      get_calendar_for_category: () => Promise.reject(new Error("banco travado")),
    });
    render(<NewsMagazineTab />);

    expect(await screen.findByText("Expectativas da temporada")).toBeInTheDocument();
    expect(revista()).not.toHaveClass("mag--abrindo");
  });

  it("boletim de IA que falha vira placeholder, com a edição de pé", async () => {
    configurarBackend({
      get_calendar_for_category: CORRIDAS,
      player_race_news_id: () => Promise.reject(new Error("sem notícia")),
    });
    render(<NewsMagazineTab />);

    expect(await screen.findByText("Etapa 2 · Temporada 2026")).toBeInTheDocument();
    expect(screen.getByText("Boletim da corrida")).toBeInTheDocument();
  });

  it("classificações vazias dizem que estão indisponíveis, em vez de sumir", async () => {
    configurarBackend({ get_calendar_for_category: CORRIDAS });
    render(<NewsMagazineTab />);

    await screen.findByText("Etapa 2 · Temporada 2026");
    // A edição abre em Pilotos; o outro lado da chave conta a mesma verdade.
    expect(screen.getByText("Classificação de pilotos indisponível.")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Construtores" }));
    expect(await screen.findByText("Classificação de equipes indisponível.")).toBeInTheDocument();
  });

  it("sem carreira aberta, nenhum comando é disparado", () => {
    configurarBackend();
    mockState = { ...mockState, careerId: null, playerTeam: null };
    render(<NewsMagazineTab />);

    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("NewsMagazineTab — navegação entre edições", () => {
  it("pede o boletim da rodada aberta e troca ao folhear para trás", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    configurarBackend({ get_calendar_for_category: CORRIDAS });
    render(<NewsMagazineTab />);

    await screen.findByText("Etapa 2 · Temporada 2026");
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("player_race_news_id", {
        careerId: "C1",
        seasonId: "S1",
        rodada: 2,
      }),
    );

    fireEvent.click(screen.getByRole("button", { name: "Edição anterior" }));
    // A virada da página tem 200 ms de animação antes de trocar o índice.
    await vi.advanceTimersByTimeAsync(300);

    expect(await screen.findByText("Etapa 1 · Temporada 2026")).toBeInTheDocument();
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("player_race_news_id", {
        careerId: "C1",
        seasonId: "S1",
        rodada: 1,
      }),
    );
    vi.useRealTimers();
  });

  it("na edição mais antiga, não há para onde voltar", async () => {
    configurarBackend({
      get_calendar_for_category: [{ rodada: 1, track_name: "Interlagos", status: "Concluida" }],
    });
    render(<NewsMagazineTab />);

    await screen.findByText("Etapa 1 · Temporada 2026");
    // Os dois botões continuam desenhados, e desabilitados: sumir com eles mexeria a
    // diagramação do rodapé a cada folhear.
    expect(screen.getByRole("button", { name: "Edição anterior" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Próxima edição" })).toBeDisabled();
  });
});
