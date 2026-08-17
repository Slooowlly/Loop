import { fireEvent, render, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import PreSeasonView from "../../PreSeasonView";

let mockState = {};

vi.mock("../../../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Equipe do grid com os campos que o v2 desenha e o v1 descartava.
function equipe(overrides = {}) {
  return {
    id: "T1",
    nome: "Belgian Racing Team",
    nome_curto: "BRT",
    cor_primaria: "#58a6ff",
    classe: "gt3",
    temp_posicao: 1,
    pontos: 300,
    vitorias: 6,
    trofeus: [],
    cash_balance: 4_000_000,
    car_level: 9,
    car_performance: 90,
    confiabilidade: 88,
    pit_crew_quality: 91,
    founded_year: 1998,
    historico_vitorias: 200,
    historico_podios: 150,
    historico_titulos_construtores: 9,
    // Recorte da categoria em que a equipe corre AGORA (o card usa estes, não o
    // acumulado de carreira acima).
    categoria_titulos: 3,
    categoria_vitorias: 84,
    piloto_1_nome: "Wei Zhang",
    piloto_1_tenure_seasons: 10,
    piloto_2_nome: "Luis Cruz",
    piloto_2_tenure_seasons: 2,
    piloto_1_contrato_vence: false,
    piloto_2_contrato_vence: false,
    piloto_1_aposentado: false,
    piloto_2_aposentado: false,
    ...overrides,
  };
}

describe("PreSeasonView — layout v2", () => {
  beforeEach(() => {
    window.localStorage.setItem("loop.preseason.layout", "v2");
    // O diário só aparece na primeira visita e grava a marca ao montar — sem
    // limpar aqui, o primeiro teste do arquivo esconderia o diário de todos os
    // outros, e a falha apareceria em quem não mexeu nele.
    window.localStorage.removeItem("loop.preseason.diarySeen");
    invoke.mockReset();
    invoke.mockResolvedValue([]);
    mockState = {
      careerId: "career-1",
      preseasonState: {
        current_week: 1,
        total_weeks: 9,
        is_complete: false,
        signings_start_week: 3,
        current_display_date: "2026-12-08",
        player_has_team: false,
        player_interest_forecast: { min: 3, max: 5 },
      },
      preseasonWeeks: [],
      lastMarketWeekResult: null,
      playerProposals: [],
      preseasonFreeAgents: [],
      transferWindow: { player_offers: [], player_tier: 4, player_name: "Rodrigo Silva" },
      isAdvancingWeek: false,
      isRespondingProposal: false,
      advanceMarketWeek: vi.fn(),
      respondToProposal: vi.fn(),
      finalizePreseason: vi.fn(),
      player: { nome: "Rodrigo Silva", posicao_campeonato: 4 },
      playerTeam: { categoria: "gt3", nome: "Nordwand" },
    };
  });

  it("marca no trilho as semanas de fotografia e a semana corrente", async () => {
    render(<PreSeasonView />);

    const rail = await screen.findByTestId("preseason-week-rail");
    // Nove semanas, uma marca cada.
    expect(within(rail).getAllByText(/^S\d$/)).toHaveLength(9);
    // O portão vira texto: a regra de que ninguém contrata antes da semana 3
    // deixa de ser invisível.
    expect(screen.getByText("Contratações abrem na semana 3")).toBeInTheDocument();
  });

  it("conta títulos e vitórias da categoria em que a equipe corre, não da carreira", async () => {
    invoke.mockImplementation((cmd, args) => {
      if (cmd !== "get_teams_standings") return Promise.resolve([]);
      return Promise.resolve(args.category === "gt3" ? [equipe()] : []);
    });

    render(<PreSeasonView />);

    const card = await screen.findByTestId("preseason-team-card-v2");
    expect(within(card).getByText("Desde 1998")).toBeInTheDocument();
    expect(within(card).getByText("3 títulos")).toBeInTheDocument();
    expect(within(card).getByText("84 vitórias")).toBeInTheDocument();
    // O acumulado de carreira (9 títulos, 200 vitórias) mistura escalões e não
    // diz nada sobre o assento sendo olhado — não aparece.
    expect(within(card).queryByText("9 títulos")).not.toBeInTheDocument();
    expect(within(card).queryByText("200 vitórias")).not.toBeInTheDocument();
  });

  it("abre a ficha da equipe no clique do nome, e não só no logo", async () => {
    invoke.mockImplementation((cmd, args) => {
      if (cmd !== "get_teams_standings") return Promise.resolve([]);
      return Promise.resolve(args.category === "gt3" ? [equipe()] : []);
    });

    render(<PreSeasonView />);

    const card = await screen.findByTestId("preseason-team-card-v2");
    fireEvent.click(within(card).getByText("Belgian Racing Team"));

    const ficha = await screen.findByTestId("team-mini-card");
    // A ficha rápida completa: identidade, temporada e estrutura.
    expect(ficha.textContent).toContain("Belgian Racing Team");
    expect(ficha.textContent).toContain("Desde 1998");
  });

  it("não mostra carro, confiabilidade, pit nem faixa de orçamento", async () => {
    invoke.mockImplementation((cmd, args) => {
      if (cmd !== "get_teams_standings") return Promise.resolve([]);
      return Promise.resolve(args.category === "gt3" ? [equipe()] : []);
    });

    render(<PreSeasonView />);

    const card = await screen.findByTestId("preseason-team-card-v2");
    expect(within(card).queryByText("Carro")).not.toBeInTheDocument();
    expect(within(card).queryByText("Confiab.")).not.toBeInTheDocument();
    expect(within(card).queryByText("Pit")).not.toBeInTheDocument();
    expect(within(card).queryByText(/Orçamento/)).not.toBeInTheDocument();
  });

  it("conta os assentos que podem abrir quando nenhum está vazio ainda", async () => {
    invoke.mockImplementation((cmd, args) => {
      if (cmd !== "get_teams_standings") return Promise.resolve([]);
      if (args.category !== "gt3") return Promise.resolve([]);
      return Promise.resolve([
        equipe({ piloto_2_aposentado: true }),
        equipe({ id: "T2", nome: "Cool Racing", temp_posicao: 2, piloto_1_contrato_vence: true }),
      ]);
    });

    render(<PreSeasonView />);

    const strip = await screen.findByTestId("preseason-category-strip-gt3");
    // Quatro assentos, nenhum vazio ainda: a barra lê "4 de 4 ocupados"...
    expect(within(strip).getByText("4 de 4 assentos ocupados")).toBeInTheDocument();
    // ...mas dois deles estão em risco (uma aposentadoria e um contrato vencendo).
    // A legenda nomeia as três fatias da barra: ocupado, pode abrir, vago.
    expect(within(strip).getByText("2 podem abrir")).toBeInTheDocument();
    expect(within(strip).getByText("Sem vaga")).toBeInTheDocument();
  });

  it("só imprime número no filtro quando a vaga é real", async () => {
    invoke.mockImplementation((cmd, args) => {
      if (cmd !== "get_teams_standings") return Promise.resolve([]);
      if (args.category === "gt3") {
        // Vaga de verdade: assento sem piloto.
        return Promise.resolve([equipe({ piloto_2_nome: null, piloto_2_tenure_seasons: null })]);
      }
      if (args.category === "gt4") {
        // Só contrato vencendo: o número não vai para a barra, fica no tooltip.
        return Promise.resolve([equipe({ id: "T9", classe: "gt4", piloto_1_contrato_vence: true })]);
      }
      return Promise.resolve([]);
    });

    render(<PreSeasonView />);

    // Espera o grid chegar (os contadores dos chips saem do mesmo carregamento).
    await screen.findByTestId("preseason-seat-open");

    expect(screen.getByTestId("preseason-filter-gt3")).toHaveTextContent("GT31");
    // Contrato vencendo não vira dígito: em toda categoria ao mesmo tempo, uma
    // fileira de "24? 12? 20?" não aponta para lugar nenhum. Fica no tooltip.
    expect(screen.getByTestId("preseason-filter-gt4")).toHaveTextContent("GT4");
    expect(screen.getByTestId("preseason-filter-gt4")).not.toHaveTextContent("?");
  });

  it("mostra o assento livre como alvo, e não como linha vazia", async () => {
    invoke.mockImplementation((cmd, args) => {
      if (cmd !== "get_teams_standings") return Promise.resolve([]);
      if (args.category !== "gt3") return Promise.resolve([]);
      return Promise.resolve([equipe({ piloto_2_nome: null, piloto_2_tenure_seasons: null })]);
    });

    render(<PreSeasonView />);

    const seat = await screen.findByTestId("preseason-seat-open");
    expect(seat).toHaveTextContent("Vaga aberta");
  });

  it("não repete uma etiqueta de contrato em cada linha ocupada", async () => {
    invoke.mockImplementation((cmd, args) => {
      if (cmd !== "get_teams_standings") return Promise.resolve([]);
      if (args.category !== "gt3") return Promise.resolve([]);
      return Promise.resolve([
        equipe({ piloto_1_contrato_vence: true, piloto_2_contrato_vence: true }),
      ]);
    });

    render(<PreSeasonView />);

    const card = await screen.findByTestId("preseason-team-card-v2");
    // Na semana da fotografia quase todo contrato vence: uma etiqueta por linha
    // marcava o normal. O estado vive no contador de anos, que já estava ali.
    expect(within(card).queryByText("Vence")).not.toBeInTheDocument();
    expect(within(card).getAllByText(/anos$/)).toHaveLength(2);
  });

  it("dá conteúdo à coluna da direita na semana em que não há oferta nenhuma", async () => {
    render(<PreSeasonView />);

    // Status do jogador em vez de um zero solto.
    expect(await screen.findByText("Rodrigo Silva")).toBeInTheDocument();
    expect(screen.getByText("Sem Contrato")).toBeInTheDocument();
    // Expectativa como faixa.
    expect(screen.getByText("3–5")).toBeInTheDocument();
    // Diário da janela explicando por que ele não pode assinar ainda.
    expect(screen.getByText("Diário Da Janela")).toBeInTheDocument();
    expect(screen.getByText(/A foto do fim da temporada/)).toBeInTheDocument();
  });

  // A janela funciona sempre igual: o diário ensina um processo, não conta o
  // estado de uma temporada. Lido uma vez, vira moldura ocupando meia coluna.
  it("esconde o diário depois da primeira visita, e o traz de volta a um clique", async () => {
    const { unmount } = render(<PreSeasonView />);
    expect(await screen.findByText("Diário Da Janela")).toBeInTheDocument();
    unmount();

    render(<PreSeasonView />);
    expect(await screen.findByTestId("preseason-diary-show")).toBeInTheDocument();
    expect(screen.queryByText("Diário Da Janela")).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("preseason-diary-show"));
    expect(screen.getByText("Diário Da Janela")).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("preseason-diary-hide"));
    expect(screen.queryByText("Diário Da Janela")).not.toBeInTheDocument();
  });

  it("ordena os pilotos livres pelo último campeonato dentro da marca", async () => {
    mockState = {
      ...mockState,
      playerTeam: {},
      preseasonFreeAgents: [
        { driver_id: "d1", driver_name: "Adam Baker", categoria: "mazda_amador", market_tier: 1, seasons_idle: 0, is_rookie: false, last_championship_position: 10, last_championship_total_drivers: 12, eligible_categories: ["mazda_amador"] },
        { driver_id: "d2", driver_name: "Zoe Winters", categoria: "mazda_amador", market_tier: 1, seasons_idle: 0, is_rookie: false, last_championship_position: 3, last_championship_total_drivers: 12, eligible_categories: ["mazda_amador"] },
        { driver_id: "d3", driver_name: "Bob Carter", categoria: "mazda_amador", market_tier: 1, seasons_idle: 0, is_rookie: false, last_championship_position: 6, last_championship_total_drivers: 12, eligible_categories: ["mazda_amador"] },
      ],
    };

    render(<PreSeasonView />);

    await screen.findByText("Adam Baker");
    const order = screen
      .getAllByText(/^(Adam Baker|Zoe Winters|Bob Carter)$/)
      .map((node) => node.textContent);
    // 3º, 6º, 10º — e não a ordem alfabética, que colocaria o 10º no topo.
    expect(order).toEqual(["Zoe Winters", "Bob Carter", "Adam Baker"]);
  });

  it("esconde a carteira na ficha do piloto livre", async () => {
    mockState = {
      ...mockState,
      playerTeam: {},
      preseasonFreeAgents: [
        { driver_id: "d1", driver_name: "Adam Baker", categoria: "mazda_amador", market_tier: 1, seasons_idle: 0, is_rookie: false, license_sigla: "SP", last_championship_position: 10, last_championship_total_drivers: 12, eligible_categories: ["mazda_amador"] },
      ],
    };

    render(<PreSeasonView />);

    const card = (await screen.findByText("Adam Baker")).closest("[data-testid='preseason-free-agent-v2']");
    expect(within(card).queryByText("SP")).not.toBeInTheDocument();
    // O resultado continua lá: é ele que ordena a fila.
    expect(within(card).getByText(/10º\/12/)).toBeInTheDocument();
  });

  it("volta ao layout antigo pelo interruptor", async () => {
    window.localStorage.setItem("loop.preseason.layout", "v1");
    render(<PreSeasonView />);

    expect(await screen.findByTestId("preseason-layout-switch")).toHaveTextContent("Layout Antigo");
    expect(screen.queryByTestId("preseason-week-rail")).not.toBeInTheDocument();
    // Cabeçalho-pôster do v1 de volta.
    expect(screen.getByText("Mapeamento das equipes")).toBeInTheDocument();
  });
});
