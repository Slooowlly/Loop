import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import MyTeamTabV2 from "./MyTeamTabV2";

// A aba Minha Equipe não tinha teste nenhum (A10.4). O que está coberto aqui é o
// esqueleto que o resto da tela pendura: a carga dos quatro comandos, o que a tela faz
// enquanto eles não voltam, o que ela faz quando um deles falha, e a troca de seção.

let mockState;

vi.mock("../../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const TIME = {
  id: "T1",
  nome: "Falcon Racing",
  categoria: "gt3",
  cor_primaria: "#58a6ff",
  cash_balance: 250000,
  salary_ceiling: 400000,
  last_round_income: 90000,
  last_round_expenses: 60000,
  last_round_net: 30000,
  presenca_publica: 55,
  piloto_1_id: "D1",
  piloto_2_id: "D2",
  piloto_1_nome: "Ana Prado",
  piloto_2_nome: "Bruno Lima",
  piloto_1_salario_anual: 120000,
  piloto_2_salario_anual: 80000,
  hierarquia_n1_id: "D1",
  hierarquia_n2_id: "D2",
  hierarquia_status: "estavel",
  hierarquia_tensao: 10,
};

const PILOTOS = [
  { id: "D1", nome: "Ana Prado", equipe_id: "T1", skill: 78, midia: 60, idade: 27, nacionalidade: "BR" },
  { id: "D2", nome: "Bruno Lima", equipe_id: "T1", skill: 71, midia: 40, idade: 31, nacionalidade: "BR" },
];

const CLASSIFICACAO = [
  { id: "T1", nome: "Falcon Racing", posicao: 3, pontos: 120, piloto_1_tenure_seasons: 2, piloto_2_tenure_seasons: 1 },
  { id: "T2", nome: "Isar Track", posicao: 1, pontos: 180 },
];

const FINANCAS = {
  grid_size: 12,
  current_position: 3,
  expected_constructor_prize: 500000,
  season: { net: 400000, constructor_prize_income: 0 },
  latest: { sponsorship_income: 50000, gate_income: 20000 },
};

/// Cada comando responde o que o caso mandar. `null` no lugar de uma resposta significa
/// "a promessa nunca resolve", que é como se testa o estado de carga.
function configurarBackend(respostas = {}) {
  mockState = {
    careerId: "C1",
    player: { id: "D1" },
    playerTeam: TIME,
    season: { id: "S1", ano: 2026 },
  };
  const padrao = {
    get_drivers_by_category: PILOTOS,
    get_teams_standings: CLASSIFICACAO,
    get_team_finance_report: FINANCAS,
    get_teams_car_parts: [],
    ...respostas,
  };
  invoke.mockReset();
  invoke.mockImplementation((comando) => {
    if (!(comando in padrao)) return Promise.reject(new Error(`Comando inesperado: ${comando}`));
    const r = padrao[comando];
    if (typeof r === "function") return r();
    return Promise.resolve(r);
  });
}

const pilulas = () => screen.getByTestId("my-team-v2-sections");

beforeEach(() => configurarBackend());

describe("MyTeamTabV2 — carga", () => {
  it("pede os quatro comandos com a categoria e a equipe do jogador", async () => {
    render(<MyTeamTabV2 />);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("get_drivers_by_category", {
        careerId: "C1",
        category: "gt3",
      });
      expect(invoke).toHaveBeenCalledWith("get_teams_standings", {
        careerId: "C1",
        category: "gt3",
      });
      expect(invoke).toHaveBeenCalledWith("get_team_finance_report", {
        careerId: "C1",
        category: "gt3",
        teamId: "T1",
      });
      expect(invoke).toHaveBeenCalledWith("get_teams_car_parts", {
        careerId: "C1",
        category: "gt3",
      });
    });
  });

  it("enquanto o backend não volta, a tela já desenha o esqueleto e não mostra erro", () => {
    configurarBackend({
      get_drivers_by_category: () => new Promise(() => {}),
      get_teams_standings: () => new Promise(() => {}),
      get_team_finance_report: () => new Promise(() => {}),
      get_teams_car_parts: () => new Promise(() => {}),
    });
    render(<MyTeamTabV2 />);

    // As três pílulas são a âncora: sem elas o jogador não teria nem para onde clicar.
    expect(pilulas().querySelectorAll("[role=tab]")).toHaveLength(3);
    expect(screen.queryByText(/Não foi possível carregar/i)).not.toBeInTheDocument();
  });

  it("sem carreira aberta, não chama comando nenhum", () => {
    configurarBackend();
    mockState = { ...mockState, careerId: null };
    render(<MyTeamTabV2 />);

    expect(invoke).not.toHaveBeenCalled();
  });

  it("sem equipe (piloto sem contrato), também não chama nada", () => {
    configurarBackend();
    mockState = { ...mockState, playerTeam: null };
    render(<MyTeamTabV2 />);

    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("MyTeamTabV2 — erro", () => {
  it("falha de ponte vira faixa vermelha com a mensagem do backend", async () => {
    configurarBackend({
      get_team_finance_report: () => Promise.reject("Save não encontrado: C1"),
    });
    render(<MyTeamTabV2 />);

    expect(await screen.findByText("Save não encontrado: C1")).toBeInTheDocument();
  });

  it("erro sem texto útil cai na mensagem padrão da aba", async () => {
    configurarBackend({
      get_drivers_by_category: () => Promise.reject(new Error("boom")),
    });
    render(<MyTeamTabV2 />);

    // O `Error` não é string, então a tela usa o texto próprio em vez de imprimir "[object Object]".
    expect(await screen.findByText(/Não foi possível carregar os dados da equipe/i)).toBeInTheDocument();
  });

  it("com erro, as pílulas continuam de pé — a tela não some inteira", async () => {
    configurarBackend({ get_teams_standings: () => Promise.reject("sem classificação") });
    render(<MyTeamTabV2 />);

    await screen.findByText("sem classificação");
    expect(pilulas().querySelectorAll("[role=tab]")).toHaveLength(3);
  });
});

describe("MyTeamTabV2 — vazio", () => {
  it("categoria sem pilotos e sem classificação não quebra a tela", async () => {
    configurarBackend({
      get_drivers_by_category: [],
      get_teams_standings: [],
      get_team_finance_report: null,
      get_teams_car_parts: [],
    });
    render(<MyTeamTabV2 />);

    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(4));
    expect(pilulas().querySelectorAll("[role=tab]")).toHaveLength(3);
    expect(screen.queryByText(/Não foi possível carregar/i)).not.toBeInTheDocument();
  });

  it("payload fora do formato (objeto no lugar de lista) é tratado como vazio", async () => {
    configurarBackend({ get_drivers_by_category: { erro: "nada" } });
    render(<MyTeamTabV2 />);

    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(4));
    expect(screen.queryByText(/Não foi possível carregar/i)).not.toBeInTheDocument();
  });
});

describe("MyTeamTabV2 — seções", () => {
  it("abre na Equipe, com a dupla lida pela hierarquia da garagem", async () => {
    render(<MyTeamTabV2 />);

    const abas = await waitFor(() => {
      const encontradas = pilulas().querySelectorAll("[role=tab]");
      expect(encontradas[0]).toHaveAttribute("aria-selected", "true");
      return encontradas;
    });
    expect(abas[1]).toHaveAttribute("aria-selected", "false");
    expect(abas[2]).toHaveAttribute("aria-selected", "false");
    expect(await screen.findByText("Ana Prado")).toBeInTheDocument();
    expect(screen.getByText("Bruno Lima")).toBeInTheDocument();
  });

  it("clicar numa pílula troca a seção e apaga a anterior", async () => {
    render(<MyTeamTabV2 />);
    await screen.findByText("Ana Prado");

    const [, dinheiro, grid] = pilulas().querySelectorAll("[role=tab]");
    fireEvent.click(dinheiro);

    await waitFor(() => expect(dinheiro).toHaveAttribute("aria-selected", "true"));
    expect(screen.queryByText("Ana Prado")).not.toBeInTheDocument();

    fireEvent.click(grid);
    await waitFor(() => expect(grid).toHaveAttribute("aria-selected", "true"));
    expect(dinheiro).toHaveAttribute("aria-selected", "false");
  });
});
