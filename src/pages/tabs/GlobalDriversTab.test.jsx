import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import GlobalDriversTab from "./GlobalDriversTab";

let mockState = {};

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("../../components/driver/DriverDetailModal", () => ({
  default: ({ driverId, onClose }) => (
    <div role="dialog" aria-label={`Ficha ${driverId}`}>
      <button type="button" onClick={onClose}>
        Fechar ficha
      </button>
    </div>
  ),
}));

const rows = [
  {
    id: "D001",
    nome: "Piloto Selecionado",
    nacionalidade: "Brasil",
    idade: 28,
    status: "Ativo",
    status_tone: "active",
    is_jogador: false,
    is_lesionado: false,
    lesao_ativa_tipo: null,
    equipe_nome: "Equipe Azul",
    equipe_cor_primaria: "#58a6ff",
    categoria_atual: "gt4",
    categorias_historicas: ["mazda_rookie", "gt4"],
    salario_anual: 250000,
    ano_inicio_carreira: 2020,
    anos_carreira: 7,
    temporada_aposentadoria: null,
    anos_aposentado: null,
    historical_index: 510.2,
    historical_rank: 2,
    historical_rank_delta: 2,
    wins_rank: 2,
    titles_rank: 3,
    podiums_rank: 2,
    injuries_rank: 4,
    corridas: 42,
    pontos: 620,
    vitorias: 7,
    podios: 18,
    poles: 5,
    titulos: 1,
    titulos_por_categoria: [{ categoria: "gt4", titulos: 1, anos: [2024] }],
    dnfs: 3,
    lesoes: 1,
    lesoes_leves: 1,
    lesoes_moderadas: 0,
    lesoes_graves: 0,
  },
  {
    id: "D002",
    nome: "Piloto Livre",
    nacionalidade: "Argentina",
    idade: 31,
    status: "Livre",
    status_tone: "dimmed",
    is_jogador: false,
    is_lesionado: true,
    lesao_ativa_tipo: "Moderada",
    equipe_nome: null,
    equipe_cor_primaria: null,
    categoria_atual: "mazda_rookie",
    categorias_historicas: ["mazda_rookie"],
    salario_anual: null,
    ano_inicio_carreira: 2019,
    anos_carreira: 8,
    temporada_aposentadoria: null,
    anos_aposentado: null,
    historical_index: 320,
    historical_rank: 3,
    historical_rank_delta: -1,
    wins_rank: 3,
    titles_rank: 2,
    podiums_rank: 3,
    injuries_rank: 2,
    corridas: 36,
    pontos: 420,
    vitorias: 4,
    podios: 12,
    poles: 2,
    titulos: 1,
    titulos_por_categoria: [{ categoria: "mazda_rookie", titulos: 1, anos: [2023] }],
    dnfs: 4,
    lesoes: 3,
    lesoes_leves: 2,
    lesoes_moderadas: 1,
    lesoes_graves: 0,
  },
  {
    id: "D003",
    nome: "Lenda Aposentada",
    nacionalidade: "",
    idade: 0,
    status: "Aposentado",
    status_tone: "retired",
    is_jogador: false,
    is_lesionado: false,
    lesao_ativa_tipo: null,
    equipe_nome: null,
    equipe_cor_primaria: null,
    categoria_atual: "gt3",
    categorias_historicas: ["gt4", "gt3"],
    salario_anual: null,
    ano_inicio_carreira: 2018,
    anos_carreira: 7,
    temporada_aposentadoria: "2024",
    anos_aposentado: 2,
    historical_index: 680.7,
    historical_rank: 1,
    historical_rank_delta: null,
    wins_rank: 1,
    titles_rank: 1,
    podiums_rank: 1,
    injuries_rank: 1,
    corridas: 80,
    pontos: 1200,
    vitorias: 12,
    podios: 32,
    poles: 11,
    titulos: 3,
    titulos_por_categoria: [
      { categoria: "gt3", titulos: 2, anos: [2024, 2023] },
      { categoria: "production_challenger", classe: "mazda", titulos: 1, anos: [2022] },
    ],
    dnfs: 6,
    lesoes: 5,
    lesoes_leves: 3,
    lesoes_moderadas: 1,
    lesoes_graves: 1,
  },
  {
    id: "D004",
    nome: "Piloto Usuario",
    nacionalidade: "Brasil",
    idade: 25,
    status: "Ativo",
    status_tone: "active",
    is_jogador: true,
    is_lesionado: false,
    lesao_ativa_tipo: null,
    equipe_nome: "Equipe Verde",
    equipe_cor_primaria: "#2dd4bf",
    categoria_atual: "mazda_rookie",
    categorias_historicas: ["mazda_rookie"],
    salario_anual: 125000,
    ano_inicio_carreira: 2022,
    anos_carreira: 5,
    temporada_aposentadoria: null,
    anos_aposentado: null,
    historical_index: 220,
    historical_rank: 4,
    historical_rank_delta: 0,
    wins_rank: 4,
    titles_rank: 4,
    podiums_rank: 4,
    injuries_rank: 4,
    corridas: 20,
    pontos: 180,
    vitorias: 1,
    podios: 4,
    poles: 1,
    titulos: 0,
    titulos_por_categoria: [],
    dnfs: 2,
    lesoes: 1,
    lesoes_leves: 1,
    lesoes_moderadas: 0,
    lesoes_graves: 0,
  },
  {
    id: "D005",
    nome: "Veterano Distante",
    nacionalidade: "Chile",
    idade: 0,
    status: "Aposentado",
    status_tone: "retired",
    is_jogador: false,
    is_lesionado: false,
    lesao_ativa_tipo: null,
    equipe_nome: null,
    equipe_cor_primaria: null,
    categoria_atual: "mazda_rookie",
    categorias_historicas: ["mazda_rookie"],
    salario_anual: null,
    ano_inicio_carreira: 2012,
    anos_carreira: 4,
    temporada_aposentadoria: "2016",
    anos_aposentado: 10,
    historical_index: 180,
    historical_rank: 5,
    historical_rank_delta: null,
    wins_rank: 5,
    titles_rank: 5,
    podiums_rank: 5,
    injuries_rank: 5,
    corridas: 29,
    pontos: 160,
    vitorias: 2,
    podios: 5,
    poles: 1,
    titulos: 0,
    titulos_por_categoria: [],
    dnfs: 3,
    lesoes: 0,
    lesoes_leves: 0,
    lesoes_moderadas: 0,
    lesoes_graves: 0,
  },
];

describe("GlobalDriversTab", () => {
  beforeEach(() => {
    mockState = { careerId: "career-1" };
    invoke.mockReset();
    invoke.mockResolvedValue({
      selected_driver_id: "D001",
      rows,
      leaders: {
        historical_index_driver_id: "D003",
        wins_driver_id: "D003",
        titles_driver_id: "D003",
        injuries_driver_id: "D003",
      },
    });
  });

  it("shows a dedicated loading screen while the global ranking is loading", () => {
    let resolvePayload;
    invoke.mockReturnValue(new Promise((resolve) => {
      resolvePayload = resolve;
    }));

    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    expect(screen.getByText(/Montando ranking mundial/i)).toBeInTheDocument();
    expect(screen.getByText(/Reunindo pilotos ativos, livres e aposentados/i)).toBeInTheDocument();
    expect(screen.getByText(/Histórico/i)).toBeInTheDocument();
    expect(screen.getByText(/Contratos/i)).toBeInTheDocument();
    expect(screen.getByText(/Aposentadorias/i)).toBeInTheDocument();
    expect(screen.getByText(/Índice/i)).toBeInTheDocument();

    resolvePayload({ selected_driver_id: "D001", rows, leaders: {} });
  });

  it("stops loading with a clear message when the career id is missing", async () => {
    mockState = { careerId: null };

    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    await waitFor(() => {
      expect(screen.queryByText(/Montando ranking mundial/i)).not.toBeInTheDocument();
    });
    expect(screen.getByText(/Carreira não carregada/i)).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("get_global_driver_rankings", expect.anything());
  });

  it("renders the compact selected driver focus beside championship champion summaries", async () => {
    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    const focusHeading = await screen.findByRole("heading", { name: "Piloto Selecionado" });
    expect(invoke).toHaveBeenCalledWith("get_global_driver_rankings", {
      careerId: "career-1",
      selectedDriverId: "D001",
    });
    const table = screen.getByRole("table", { name: /Ranking mundial de pilotos/i });
    const focusCard = focusHeading.closest("article");
    expect(screen.getByLabelText(/Resumo do ranking mundial/i)).toHaveClass("lg:grid-cols-[minmax(0,1.22fr)_minmax(330px,0.78fr)]");
    expect(within(focusCard).getByText("Piloto em foco")).toBeInTheDocument();
    expect(within(focusCard).getByText("510,2")).toBeInTheDocument();
    expect(within(focusCard).getByText(/Rank #2/i)).toBeInTheDocument();
    expect(within(focusCard).getByText("Corridas")).toBeInTheDocument();
    expect(within(focusCard).getByText("42")).toBeInTheDocument();
    expect(within(focusCard).getAllByText(/Top #2/i).length).toBeGreaterThan(0);
    expect(within(focusCard).getAllByText("Vitorias").length).toBeGreaterThan(0);
    expect(within(focusCard).getAllByText("Titulos").length).toBeGreaterThan(0);
    expect(within(focusCard).getByText("Podios")).toBeInTheDocument();
    expect(within(focusCard).getAllByText("Carreira").length).toBeGreaterThan(0);
    expect(within(focusCard).getByText("Seu piloto")).toBeInTheDocument();
    expect(within(focusCard).getByRole("heading", { name: "Piloto Usuario" })).toBeInTheDocument();
    expect(within(focusCard).getByText(/Rank #4/i)).toBeInTheDocument();
    expect(within(focusCard).getByText("220,0")).toBeInTheDocument();
    expect(screen.getByText(/Campeoes por campeonato/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Ver campeoes de GT3/i })).toHaveTextContent("GT3");
    expect(screen.getByRole("button", { name: /Ver campeoes de GT3/i })).toHaveTextContent("1");
    expect(screen.getByRole("button", { name: /Ver campeoes de Production\/Mazda/i })).toHaveTextContent("Production/Mazda");
    expect(screen.getByText(/Eventos especiais/i)).toBeInTheDocument();
    const championshipButtons = screen.getAllByRole("button", { name: /Ver campeoes de/i });
    expect(championshipButtons.map((button) => button.textContent)).toEqual([
      "GT3Lenda Aposentada1",
      "GT4Piloto Selecionado1",
      "Mazda RookiePiloto Livre1",
      "Production/MazdaLenda Aposentada1",
    ]);
    expect(within(table).getByText("Piloto Selecionado").closest("tr")).toHaveClass("bg-accent-primary/12");
    expect(within(table).getByText("Piloto Livre").closest("tr")).toHaveClass("opacity-60");
    expect(within(table).getByText("Lenda Aposentada").closest("tr")).toHaveClass("opacity-50");
    expect(within(table).getAllByText("Lenda Aposentada")).toHaveLength(1);
    expect(within(table).getByText("Lesionado")).toBeInTheDocument();
    expect(within(table).queryByText(/Lesionado: Moderada/i)).not.toBeInTheDocument();
  });

  it("renders the user driver card from payload even when the player is not ranked", async () => {
    const playerRow = rows.find((row) => row.is_jogador);
    invoke.mockResolvedValueOnce({
      selected_driver_id: "D001",
      rows: rows.filter((row) => !row.is_jogador),
      player_driver: playerRow,
      leaders: {},
    });

    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    const focusHeading = await screen.findByRole("heading", { name: "Piloto Selecionado" });
    const focusCard = focusHeading.closest("article");
    const table = screen.getByRole("table", { name: /Ranking mundial de pilotos/i });

    expect(within(focusCard).getByText("Seu piloto")).toBeInTheDocument();
    expect(within(focusCard).getByRole("heading", { name: "Piloto Usuario" })).toBeInTheDocument();
    expect(within(table).queryByText("Piloto Usuario")).not.toBeInTheDocument();
  });

  it("opens a championship champion popup from the championship summary", async () => {
    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });
    fireEvent.click(screen.getByRole("button", { name: /Ver campeoes de GT3/i }));

    const dialog = screen.getByRole("dialog", { name: /Campeoes de GT3/i });
    expect(dialog).toHaveClass("max-h-[85vh]", "overflow-hidden");
    expect(within(dialog).getByText(/1 campeao/i)).toBeInTheDocument();
    expect(within(dialog).getByText("Lenda Aposentada")).toBeInTheDocument();
    expect(within(dialog).getByText("2 titulos")).toBeInTheDocument();
    expect(within(dialog).getByText("2024, 2023")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Fechar campeoes/i }));
    expect(screen.queryByRole("dialog", { name: /Campeoes de GT3/i })).not.toBeInTheDocument();
  });

  it("filters by status, historical category, nationality, champions, injured drivers, and age", async () => {
    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    const table = await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });
    expect(screen.getByText(/5 de 5 pilotos/i)).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/Status/i), { target: { value: "Livre" } });
    expect(within(table).getByText("Piloto Livre")).toBeInTheDocument();
    expect(within(table).queryByText("Piloto Selecionado")).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/Status/i), { target: { value: "Todos" } });
    fireEvent.change(screen.getByLabelText(/Categoria/i), { target: { value: "gt4" } });
    expect(within(table).getByText("Piloto Selecionado")).toBeInTheDocument();
    expect(within(table).getByText("Lenda Aposentada")).toBeInTheDocument();
    expect(within(table).queryByText("Piloto Usuario")).not.toBeInTheDocument();
    expect(within(table).getByText(/Atualmente em GT4/i)).toBeInTheDocument();
    expect(within(table).getByText(/Ja passaram por GT4/i)).toBeInTheDocument();

    const categoryRows = within(table).getAllByRole("row");
    const selectedIndex = categoryRows.findIndex((row) => within(row).queryByText("Piloto Selecionado"));
    const pastGroupIndex = categoryRows.findIndex((row) => within(row).queryByText(/Ja passaram por GT4/i));
    const retiredIndex = categoryRows.findIndex((row) => within(row).queryByText("Lenda Aposentada"));
    expect(selectedIndex).toBeGreaterThan(-1);
    expect(pastGroupIndex).toBeGreaterThan(selectedIndex);
    expect(retiredIndex).toBeGreaterThan(pastGroupIndex);

    fireEvent.change(screen.getByLabelText(/Categoria/i), { target: { value: "Todas" } });
    fireEvent.change(screen.getByLabelText(/Nacionalidade/i), { target: { value: "Brasil" } });
    expect(within(table).getByText("Piloto Selecionado")).toBeInTheDocument();
    expect(within(table).getByText("Piloto Usuario")).toBeInTheDocument();
    expect(within(table).queryByText("Piloto Livre")).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/Nacionalidade/i), { target: { value: "Todas" } });
    fireEvent.change(screen.getByLabelText(/Campeões/i), { target: { value: "champions" } });
    expect(within(table).getByText("Piloto Selecionado")).toBeInTheDocument();
    expect(within(table).queryByText("Piloto Usuario")).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/Campeões/i), { target: { value: "all" } });
    fireEvent.change(screen.getByLabelText(/Lesionados/i), { target: { value: "injured" } });
    expect(within(table).getByText("Piloto Livre")).toBeInTheDocument();
    expect(within(table).queryByText("Piloto Selecionado")).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/Lesionados/i), { target: { value: "all" } });
    fireEvent.change(screen.getByLabelText(/Idade mínima/i), { target: { value: "26" } });
    fireEvent.change(screen.getByLabelText(/Idade máxima/i), { target: { value: "30" } });
    expect(within(table).getByText("Piloto Selecionado")).toBeInTheDocument();
    expect(within(table).queryByText("Piloto Livre")).not.toBeInTheDocument();
    expect(within(table).queryByText("Piloto Usuario")).not.toBeInTheDocument();
  });

  it("keeps free and retired drivers out of the currently-in category section", async () => {
    render(<GlobalDriversTab selectedDriverId="D004" onBack={vi.fn()} />);

    const table = await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });
    fireEvent.change(screen.getByLabelText(/Categoria/i), { target: { value: "mazda_rookie" } });

    const bodyRows = within(table).getAllByRole("row").slice(1);
    const currentSectionIndex = bodyRows.findIndex((row) => within(row).queryByText(/Atualmente em Mazda Rookie/i));
    const pastSectionIndex = bodyRows.findIndex((row) => within(row).queryByText(/Ja passaram por Mazda Rookie/i));
    const userIndex = bodyRows.findIndex((row) => within(row).queryByText("Piloto Usuario"));
    const freeIndex = bodyRows.findIndex((row) => within(row).queryByText("Piloto Livre"));
    const retiredIndex = bodyRows.findIndex((row) => within(row).queryByText("Veterano Distante"));

    expect(currentSectionIndex).toBeGreaterThan(-1);
    expect(userIndex).toBeGreaterThan(currentSectionIndex);
    expect(userIndex).toBeLessThan(pastSectionIndex);
    expect(freeIndex).toBeGreaterThan(pastSectionIndex);
    expect(retiredIndex).toBeGreaterThan(pastSectionIndex);
  });

  it("orders category filter options by career progression and keeps options readable on dark UI", async () => {
    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });
    const categoryFilter = screen.getByLabelText(/Categoria/i);

    expect([...categoryFilter.options].map((option) => option.textContent)).toEqual([
      "Todas",
      "Mazda Rookie",
      "GT4",
      "GT3",
    ]);
    expect(categoryFilter).toHaveClass("bg-app-card");
    [...categoryFilter.options].forEach((option) => {
      expect(option).toHaveClass("bg-app-card");
      expect(option).toHaveClass("text-text-primary");
    });
  });

  it("changes the focused driver when a ranking row is clicked and keeps player emphasis", async () => {
    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    expect(await screen.findByRole("heading", { name: "Piloto Selecionado" })).toBeInTheDocument();
    const table = screen.getByRole("table", { name: /Ranking mundial de pilotos/i });
    fireEvent.click(within(table).getByText("Piloto Livre"));

    expect(screen.getByRole("heading", { name: "Piloto Livre" })).toBeInTheDocument();
    expect(screen.getByText("Seu piloto")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Piloto Usuario" })).toBeInTheDocument();
    expect(within(table).getByText("Piloto Usuario").closest("tr")).toHaveClass("border-l-accent-primary/70");
    expect(screen.getByText(/Voce/i)).toBeInTheDocument();
  });

  it("renders team/category, age, career years, salary, and retired tooltip", async () => {
    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    const table = await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });

    expect(within(table).getByText(/Equipe Azul \/ GT4/i)).toBeInTheDocument();
    expect(within(table).getByText("28")).toBeInTheDocument();
    expect(within(table).getAllByText(/7 anos/i).length).toBeGreaterThan(0);
    expect(within(table).getByText(/\$250k/i)).toBeInTheDocument();
    const retiredTeamCategory = within(table).getByText(/Há 2 anos \/ GT3/i);
    expect(retiredTeamCategory).toBeInTheDocument();
    expect(retiredTeamCategory).toHaveAttribute("title", "Aposentado em 2024");
    expect(within(table).queryByText(/Aposentado \/ GT3/i)).not.toBeInTheDocument();
    expect(within(table).queryByText(/GT3 \/ Aposentado/i)).not.toBeInTheDocument();
    within(table).getAllByText("Aposentado").forEach((status) => {
      expect(status).not.toHaveAttribute("title");
    });
  });

  it("sorts the global table by wins", async () => {
    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    const table = await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });
    fireEvent.click(screen.getByRole("button", { name: /Vit\./i }));

    const bodyRows = within(table).getAllByRole("row").slice(1);
    expect(within(bodyRows[0]).getByText("Lenda Aposentada")).toBeInTheDocument();
  });

  it("opens a title breakdown popup from the titles number", async () => {
    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    const table = await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });
    const retiredRow = within(table).getByText("Lenda Aposentada").closest("tr");

    fireEvent.click(within(retiredRow).getByRole("button", { name: /Ver titulos de Lenda Aposentada/i }));

    const dialog = screen.getByRole("dialog", { name: /Titulos de Lenda Aposentada/i });
    expect(within(dialog).getByText(/Total: 3/i)).toBeInTheDocument();
    expect(within(dialog).getByText("GT3")).toBeInTheDocument();
    expect(within(dialog).getByText("2 titulos")).toBeInTheDocument();
    expect(within(dialog).getByText("Production/Mazda")).toBeInTheDocument();
    expect(within(dialog).getByText("1 titulo")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Fechar titulos/i }));
    expect(screen.queryByRole("dialog", { name: /Titulos de Lenda Aposentada/i })).not.toBeInTheDocument();
  });

  it("opens the driver detail modal when double-clicking a driver row and highlights it", async () => {
    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    const table = await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });
    const retiredRow = within(table).getByText("Lenda Aposentada").closest("tr");

    fireEvent.doubleClick(retiredRow);

    expect(screen.getByRole("dialog", { name: /Ficha D003/i })).toBeInTheDocument();
    expect(retiredRow).toHaveClass("ring-2", "ring-accent-secondary/60");

    fireEvent.click(screen.getByRole("button", { name: /Fechar ficha/i }));
    expect(screen.queryByRole("dialog", { name: /Ficha D003/i })).not.toBeInTheDocument();
    expect(retiredRow).not.toHaveClass("ring-2", "ring-accent-secondary/60");
  });

  it("shows rank movement arrows beside the historical rank", async () => {
    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    const table = await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });
    const selectedRow = within(table).getByText("Piloto Selecionado").closest("tr");
    const freeRow = within(table).getByText("Piloto Livre").closest("tr");

    expect(within(selectedRow).getByText("↑2")).toHaveClass("text-status-green", "whitespace-nowrap");
    expect(within(selectedRow).getByText("↑2")).toHaveAttribute("title", "Subiu 2 posições desde a última corrida");
    expect(within(freeRow).getByText("↓1")).toHaveClass("text-status-red", "whitespace-nowrap");
    expect(within(freeRow).getByText("↓1")).toHaveAttribute("title", "Desceu 1 posição desde a última corrida");
  });

  it("sorts retired drivers by longest retirement from the team/category column", async () => {
    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    const table = await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });
    fireEvent.change(screen.getByLabelText(/Status/i), { target: { value: "Aposentado" } });
    fireEvent.click(screen.getByRole("button", { name: /Equipe\/Categoria/i }));

    const bodyRows = within(table).getAllByRole("row").slice(1);
    expect(within(bodyRows[0]).getByText("Veterano Distante")).toBeInTheDocument();
    expect(within(bodyRows[0]).getByText(/Há 10 anos \/ Mazda Rookie/i)).toHaveAttribute("title", "Aposentado em 2016");
    expect(within(bodyRows[1]).getByText("Lenda Aposentada")).toBeInTheDocument();
    expect(within(bodyRows[1]).getByText(/Há 2 anos \/ GT3/i)).toHaveAttribute("title", "Aposentado em 2024");
  });

  it("calls onBack from the hidden tab return action", async () => {
    const onBack = vi.fn();
    render(<GlobalDriversTab selectedDriverId="D001" onBack={onBack} />);

    await screen.findByText(/Panorama global de pilotos/i);
    fireEvent.click(await screen.findByRole("button", { name: /Voltar para Classificacao/i }));

    expect(onBack).toHaveBeenCalledTimes(1);
  });
});
