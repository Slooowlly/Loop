import { render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import DriverDetailModal from "./DriverDetailModal";

let mockState = {};

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("./DriverDetailModalSections", () => ({
  SummarySection: () => <section>Resumo Atual</section>,
  HistorySection: () => <section>Historico de Carreira</section>,
  RivalsSection: () => <section>Rivais do Piloto</section>,
  MarketSection: () => <section>Contrato e Mercado</section>,
  formatMoment: () => ({ label: "Estavel", color: "text-[#d29922]" }),
}));

function detail(overrides = {}) {
  return {
    id: "D_RET",
    nome: "Lenda Aposentada",
    nacionalidade: "Brasil",
    idade: 38,
    genero: "M",
    is_jogador: false,
    status: "aposentado",
    equipe_id: null,
    equipe_nome: null,
    equipe_cor_primaria: null,
    equipe_cor_secundaria: null,
    papel: null,
    personalidade_primaria: null,
    personalidade_secundaria: null,
    motivacao: 50,
    tags: [],
    stats_temporada: { corridas: 0, pontos: 0, vitorias: 0, podios: 0, poles: 0, melhor_resultado: 0, dnfs: 0 },
    stats_carreira: { corridas: 120, pontos: 1800, vitorias: 22, podios: 55, poles: 18, melhor_resultado: 1, dnfs: 7 },
    contrato: null,
    perfil: {
      nome: "Lenda Aposentada",
      bandeira: "",
      nacionalidade: "Brasil",
      idade: 38,
      genero: "M",
      status: "aposentado",
      is_jogador: false,
      equipe_nome: null,
      papel: null,
      licenca: { nivel: "Elite", sigla: "E" },
      badges: [],
      equipe_cor_primaria: null,
      equipe_cor_secundaria: null,
    },
    competitivo: {
      personalidade_primaria: null,
      personalidade_secundaria: null,
      motivacao: 50,
      qualidades: [],
      defeitos: [],
      neutro: true,
    },
    leitura_tecnica: { itens: [] },
    performance: { temporada: {}, carreira: {} },
    forma: { momento: "sem_dados", ultimas_10: [] },
    resumo_atual: {},
    leitura_desempenho: {},
    trajetoria: { titulos: 2, foi_campeao: true, historico: {}, marcos: [], categorias_timeline: [] },
    rankings_carreira: {},
    rivais: {},
    contrato_mercado: { contrato: null, mercado: null },
    relacionamentos: null,
    reputacao: null,
    saude: null,
    ...overrides,
  };
}

describe("DriverDetailModal", () => {
  beforeEach(() => {
    mockState = { careerId: "career-1" };
    invoke.mockReset();
  });

  it("shows only the history tab for retired drivers", async () => {
    invoke.mockResolvedValue(detail());

    render(
      <DriverDetailModal
        driverId="D_RET"
        driverIds={["D_RET"]}
        onSelectDriver={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    expect(await screen.findByText("Historico de Carreira")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Histórico/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Resumo/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Rivais/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Mercado/i })).not.toBeInTheDocument();
    expect(screen.queryByText("Resumo Atual")).not.toBeInTheDocument();
  });
});
