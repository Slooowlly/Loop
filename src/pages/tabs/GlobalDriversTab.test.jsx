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
    fama: 78,
    carisma: 88,
    fama_delta: 6,
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

  it("shows fama in the ranking with a rising arrow chip when it climbed", async () => {
    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    const table = await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });
    expect(within(table).getByRole("button", { name: /Fama/i })).toBeInTheDocument();

    const chip = within(table).getByText("▲6");
    expect(chip).toBeInTheDocument();
    expect(chip).toHaveClass("text-status-green");
    expect(chip.parentElement).toHaveAttribute(
      "title",
      expect.stringContaining("Ganhou 6 de fama"),
    );
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

  it("shows the champion's team logo next to each title year", async () => {
    const teamedRows = rows.map((row) =>
      row.id === "D003"
        ? {
            ...row,
            titulos_por_categoria: [
              {
                categoria: "gt3",
                titulos: 2,
                anos: [2024, 2023],
                anos_equipes: [
                  { ano: 2024, equipe: "Mercedes-AMG", equipe_cor: "#00a19b" },
                  { ano: 2023, equipe: "Ferrari", equipe_cor: "#ff2800" },
                ],
              },
            ],
          }
        : row,
    );
    invoke.mockResolvedValue({ selected_driver_id: "D001", rows: teamedRows, leaders: {} });

    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);
    await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });
    fireEvent.click(screen.getByRole("button", { name: /Ver campeoes de GT3/i }));

    const dialog = screen.getByRole("dialog", { name: /Campeoes de GT3/i });
    // Each year is now a chip with the champion team's logo, not a comma-joined string.
    expect(within(dialog).getByText("2024")).toBeInTheDocument();
    expect(within(dialog).getByText("2023")).toBeInTheDocument();
    expect(within(dialog).queryByText("2024, 2023")).not.toBeInTheDocument();
    expect(within(dialog).getByAltText("Mercedes-AMG logo")).toBeInTheDocument();
    expect(within(dialog).getByAltText("Ferrari logo")).toBeInTheDocument();
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
    // O filtro agora agrupa por país (código), uma entrada por nacionalidade.
    fireEvent.change(screen.getByLabelText(/Nacionalidade/i), { target: { value: "br" } });
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

  it("groups category options by manufacturer lineage in progression order", async () => {
    const lineageRows = rows.map((row) => {
      if (row.id === "D001") {
        return {
          ...row,
          categoria_atual: "production_challenger:mazda",
          categorias_historicas: ["mazda_rookie", "mazda_amador", "production_challenger:mazda"],
        };
      }
      if (row.id === "D004") {
        return {
          ...row,
          categoria_atual: "toyota_amador",
          categorias_historicas: ["toyota_rookie", "toyota_amador"],
        };
      }
      return { ...row, categoria_atual: "mazda_rookie", categorias_historicas: ["mazda_rookie"] };
    });
    invoke.mockResolvedValue({ selected_driver_id: "D001", rows: lineageRows, leaders: {} });

    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);
    const categoryFilter = await screen.findByLabelText(/Categoria/i);

    const optgroups = [...categoryFilter.querySelectorAll("optgroup")];
    const byLabel = Object.fromEntries(
      optgroups.map((group) => [
        group.label,
        [...group.querySelectorAll("option")].map((option) => option.textContent),
      ]),
    );

    // Mazda lineage comes first, then Toyota, each in rookie -> championship -> production order.
    expect(optgroups.map((group) => group.label)).toEqual(["Mazda", "Toyota"]);
    expect(byLabel.Mazda).toEqual(["Mazda Rookie", "Mazda Amador", "Mazda Production"]);
    expect(byLabel.Toyota).toEqual(["Toyota Rookie", "Toyota Amador"]);
  });

  it("pairs each GT/LMP2 class with its endurance variant and drops generic/no-category entries", async () => {
    const classRows = rows.map((row, index) => {
      const categories = [
        ["gt4", "endurance:gt4"],
        ["gt3", "endurance:gt3"],
        ["endurance:lmp2"],
        ["endurance"],
        ["production_challenger", "SemCategoria"],
      ][index] ?? ["gt4"];
      return { ...row, categoria_atual: categories[0], categorias_historicas: categories };
    });
    invoke.mockResolvedValue({ selected_driver_id: "D001", rows: classRows, leaders: {} });

    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);
    const categoryFilter = await screen.findByLabelText(/Categoria/i);

    const optgroups = [...categoryFilter.querySelectorAll("optgroup")];
    const byLabel = Object.fromEntries(
      optgroups.map((group) => [
        group.label,
        [...group.querySelectorAll("option")].map((option) => option.textContent),
      ]),
    );

    // Only class-based groups remain; there is no catch-all group.
    expect(optgroups.map((group) => group.label)).toEqual(["GT4", "GT3", "LMP2"]);
    expect(byLabel.GT4).toEqual(["GT4", "GT4 Endurance"]);
    expect(byLabel.GT3).toEqual(["GT3", "GT3 Endurance"]);
    expect(byLabel.LMP2).toEqual(["LMP2 Endurance"]);

    // Generic endurance/production and SemCategoria are not offered as filter options.
    const optionLabels = [...categoryFilter.options].map((option) => option.textContent);
    expect(optionLabels).not.toContain("Endurance");
    expect(optionLabels).not.toContain("Production Challenger");
    expect(optionLabels).not.toContain("SemCategoria");
  });

  it("collapses gendered nationalities into a single per-country filter option", async () => {
    const genderedRows = rows.map((row) => {
      if (row.id === "D001") return { ...row, nacionalidade: "🇧🇷 Brasileiro" };
      if (row.id === "D004") return { ...row, nacionalidade: "🇧🇷 Brasileira" };
      return row;
    });
    invoke.mockResolvedValue({ selected_driver_id: "D001", rows: genderedRows, leaders: {} });

    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    const table = await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });
    const nationalitySelect = screen.getByLabelText(/Nacionalidade/i);

    // Masculine and feminine Brazilians collapse to one option keyed by country code.
    const brazilOptions = [...nationalitySelect.options].filter((option) => /Brasil/i.test(option.textContent));
    expect(brazilOptions).toHaveLength(1);
    expect(brazilOptions[0].value).toBe("br");

    // Selecting it still matches drivers of both genders.
    fireEvent.change(nationalitySelect, { target: { value: "br" } });
    expect(within(table).getByText("Piloto Selecionado")).toBeInTheDocument();
    expect(within(table).getByText("Piloto Usuario")).toBeInTheDocument();
    expect(within(table).queryByText("Piloto Livre")).not.toBeInTheDocument();
  });

  it("recalculates the # column and the delta among active drivers when status is Ativo", async () => {
    // Globally: D001 rank 2 (climbed +2 → prev 4), D004 rank 4 (dropped -1 → prev 3).
    // Among only the two active drivers, current order is D001<D004 but previous order
    // was D004<D001, so within the filter D001 climbs +1 and D004 drops -1.
    const activeRows = rows.map((row) =>
      row.id === "D004" ? { ...row, historical_rank_delta: -1 } : row,
    );
    invoke.mockResolvedValue({ selected_driver_id: "D001", rows: activeRows, leaders: {} });

    render(<GlobalDriversTab selectedDriverId="D001" onBack={vi.fn()} />);

    const table = await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });

    const selectedRowGlobal = within(table).getByText("Piloto Selecionado").closest("tr");
    expect(within(selectedRowGlobal).getByText("02")).toBeInTheDocument();
    expect(within(selectedRowGlobal).getByText("↑2")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/Status/i), { target: { value: "Ativo" } });

    // Only the two active drivers remain, renumbered 01/02 by world index order.
    expect(within(table).queryByText("Piloto Livre")).not.toBeInTheDocument();
    expect(within(table).queryByText("Lenda Aposentada")).not.toBeInTheDocument();

    const selectedRow = within(table).getByText("Piloto Selecionado").closest("tr");
    const userRow = within(table).getByText("Piloto Usuario").closest("tr");

    // Ranks are recalculated within the filtered set...
    expect(within(selectedRow).getByText("01")).toBeInTheDocument();
    expect(within(userRow).getByText("02")).toBeInTheDocument();

    // ...and so are the delta badges, now relative to the filtered ranking.
    const selectedDelta = within(selectedRow).getByText("↑1");
    expect(selectedDelta).toHaveAttribute("title", expect.stringMatching(/neste ranking filtrado/i));
    expect(within(userRow).getByText("↓1")).toBeInTheDocument();

    // The global #2 climb is no longer shown as a badge in the filtered view.
    expect(within(table).queryByText("↑2")).not.toBeInTheDocument();
    expect(screen.getByText(/# recalculado entre pilotos ativos/i)).toBeInTheDocument();
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

  it("formats composite current divisions instead of showing raw keys", async () => {
    invoke.mockResolvedValueOnce({
      selected_driver_id: "D_END_GT3",
      rows: [
        {
          ...rows[0],
          id: "D_END_GT3",
          nome: "Piloto Endurance",
          equipe_nome: "Equipe Endurance",
          categoria_atual: "endurance:gt3",
          categorias_historicas: ["endurance:gt3"],
          titulos_por_categoria: [],
        },
      ],
      leaders: {},
    });

    render(<GlobalDriversTab selectedDriverId="D_END_GT3" onBack={vi.fn()} />);

    const table = await screen.findByRole("table", { name: /Ranking mundial de pilotos/i });
    expect(within(table).getByText(/Equipe Endurance \/ GT3 Endurance/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/Categoria/i)).toHaveTextContent("GT3 Endurance");
    expect(screen.queryByText(/endurance:gt3/i)).not.toBeInTheDocument();
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
