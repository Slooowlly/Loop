import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import CarreiraTab from "./CarreiraTab.jsx";

let mockState = {};

vi.mock("../../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// A ficha do piloto do jogador, no formato de `get_driver_detail`. Só os campos que
// as cinco seções realmente leem — o payload de verdade tem quatro vezes isto, e
// copiá-lo inteiro faria o teste falhar quando um campo que ninguém lê mudasse.
const DETALHE = {
  id: "D_PLAYER",
  nome: "Joao Silva",
  nacionalidade: "br",
  idade: 24,
  is_jogador: true,
  status: "ativo",
  papel: "Numero1",
  equipe_nome: "Falcon Racing",
  equipe_cor_primaria: "#58a6ff",
  perfil: {
    nome: "Joao Silva",
    bandeira: "🇧🇷",
    nacionalidade: "br",
    idade: 24,
    status: "ativo",
    is_jogador: true,
    equipe_nome: "Falcon Racing",
    licenca: { nivel: "Licença B", sigla: "B" },
    badges: [],
  },
  competitivo: { motivacao: 72, personalidade_primaria: null, personalidade_secundaria: null },
  estrelato: { fama: 44, carisma: 51, nivel_fama: "Conhecido", nivel_carisma: "Simpático", resumo: "Nome em ascensão." },
  arco: {
    idade: 24,
    fase: "Em ascensão",
    fase_chave: "ascensao",
    tom_fase: "success",
    nivel_experiencia: "Rodado",
    nivel_desenvolvimento: "Crescendo",
    resumo: "Ainda tem teto pela frente.",
  },
  forma: { momento: "forte", tendencia: "->", ultimas_10: [], ultimas_5: [], temporadas: [] },
  stats_temporada: { corridas: 6, pontos: 58, vitorias: 1, podios: 3, poles: 1, melhor_resultado: 1, dnfs: 0 },
  stats_carreira: { corridas: 41, pontos: 320, vitorias: 4, podios: 12, poles: 3, melhor_resultado: 1, dnfs: 5 },
  resumo_atual: {
    veredito: "Bom",
    tom: "success",
    posicao_campeonato: 3,
    pontos: 58,
    vitorias: 1,
    podios: 3,
    media_recente: 4.2,
    tendencia: "->",
  },
  leitura_tecnica: { itens: [] },
  trajetoria: {
    ano_estreia: 2024,
    equipe_estreia: "Track Day Heroes",
    titulos: 1,
    foi_campeao: true,
    temporadas_na_categoria: 2,
    corridas_na_categoria: 20,
    titulos_detalhe: [{ ano: 2025, categoria: "gt4", equipe: "Falcon Racing", equipe_cor: "#58a6ff" }],
    categorias_timeline: [
      { categoria: "mazda_rookie", ano_inicio: 2024, ano_fim: 2024 },
      { categoria: "gt4", ano_inicio: 2025, ano_fim: 2026 },
    ],
    marcos: [],
    curva_campeonato: [
      { season_number: 1, ano: 2024, categoria: "mazda_rookie", equipe_nome: "Track Day Heroes", posicao: 7, grid: 20, pontos: 84, vitorias: 0, podios: 1, corridas: 14, titulo: false, atual: false },
      { season_number: 2, ano: 2025, categoria: "gt4", equipe_nome: "Falcon Racing", posicao: 1, grid: 22, pontos: 178, vitorias: 3, podios: 8, corridas: 15, titulo: true, atual: false },
      { season_number: 3, ano: 2026, categoria: "gt4", equipe_nome: "Falcon Racing", posicao: 3, grid: 22, pontos: 58, vitorias: 1, podios: 3, corridas: 6, titulo: false, atual: true },
    ],
    historico: {
      presenca: { tempo_carreira: 3, temporadas_disputadas: 3, anos_desempregado: 0, periodos_desempregado: [], corridas: 41, categorias_disputadas: 2 },
      primeiros_marcos: { primeiro_podio_corrida: 9, primeira_vitoria_corrida: 16, primeiro_dnf_corrida: 4, primeiro_titulo: null },
      auge: { melhor_temporada: { ano: 2025, categoria: "gt4", posicao_campeonato: 1, pontos: 178, vitorias: 3, podios: 8 }, maior_sequencia_vitorias: 2, maior_sequencia_podios: 5, temporadas_no_top3: 2 },
      queda: {},
      confiabilidade: { abandonos: 5, corridas: 41, taxa_abandono: 12.2, maior_sequencia_chegadas: 18 },
      sabado: { poles: 3, poles_convertidas: 2, voltas_rapidas: 6 },
      duelos: {},
      referencias: {},
      detalhes: {},
      recordes: {},
      mobilidade: { promocoes: 1, rebaixamentos: 0, equipes_defendidas: 2, tempo_medio_por_equipe: 1.5 },
      lesoes: { leves: 0, moderadas: 0, graves: 0 },
      eventos_especiais: { participacoes: 0, convocacoes: 0, vitorias: 0, podios: 0, timeline: [] },
    },
  },
  rivais: {
    itens: [
      {
        driver_id: "D_RIVAL",
        nome: "Marco Bertone",
        tipo: "Colisao",
        nivel_chave: "forte",
        intensidade: 68,
        intensidade_historica: 70,
        atividade_recente: 40,
        confrontos: 18,
        vitorias: 11,
        derrotas: 6,
        vitorias_quali: 9,
        derrotas_quali: 9,
        gap_medio: -1.4,
        companheirismo: null,
        encontros: [],
        categoria_atual: "gt4",
        mesma_categoria: true,
        equipe_nome: "Isar Track",
        equipe_cor: "#f0b232",
      },
    ],
  },
  contrato_mercado: {
    contrato: {
      equipe_nome: "Falcon Racing",
      papel: "Numero1",
      salario_anual: 240000,
      temporada_inicio: 2,
      temporada_fim: 3,
      ano_inicio: 2025,
      ano_fim: 2026,
      anos_restantes: 1,
      status: "Ativo",
    },
    mercado: {
      valor_mercado: 1250000,
      salario_estimado: 320000,
      chance_transferencia: 34,
      posicao_valor: 5,
      total_valor: 22,
      categoria_valor: "gt4",
    },
    curva: [],
  },
  saude: { saude_geral: 100, lesao_ativa: null },
};

const BOARD = {
  player_categoria: "gt4",
  player_tier: 3,
  vagas_elegiveis: 1,
  vagas: [
    {
      team_id: "T9",
      team_name: "Nordschleife Works",
      team_color: "#3fb950",
      categoria: "gt4",
      classe: null,
      categoria_tier: 3,
      papel: "Numero2",
      car_performance_rating: 71,
      licenca_ok: true,
      tier_ok: true,
      salario_estimado: 195000,
    },
    {
      team_id: "T12",
      team_name: "Sebring Legends",
      team_color: "#f85149",
      categoria: "gt3",
      classe: null,
      categoria_tier: 5,
      papel: "Numero1",
      car_performance_rating: 88,
      licenca_ok: false,
      tier_ok: false,
      salario_estimado: null,
    },
  ],
};

const INBOX = {
  category: "gt4",
  head_to_head: null,
  title_favorite: null,
  team_interest: {
    player_fama: 44,
    teams: [{ team_name: "Nordschleife Works", category: "gt4" }],
  },
};

function respondeComando(overrides = {}) {
  invoke.mockImplementation((command) => {
    if (command in overrides) return Promise.resolve(overrides[command]);
    if (command === "get_driver_detail") return Promise.resolve(DETALHE);
    if (command === "get_season_market_board") return Promise.resolve(BOARD);
    if (command === "get_inbox_messages") return Promise.resolve(INBOX);
    if (command === "get_player_dossier") {
      return Promise.resolve({ total_races: 41, total_seasons: 3, attributes: [] });
    }
    if (command === "get_driver_world_rank") return Promise.resolve(null);
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  invoke.mockReset();
  mockState = { careerId: "career_001", player: { id: "D_PLAYER" }, playerInterests: null };
  respondeComando();
});

describe("CarreiraTab", () => {
  it("abre na seção do piloto com o cabeçalho do protagonista", async () => {
    render(<CarreiraTab />);

    await waitFor(() => expect(screen.getByTestId("carreira-header")).toBeInTheDocument());
    expect(screen.getByText("Joao Silva")).toBeInTheDocument();
    expect(screen.getByText("Falcon Racing")).toBeInTheDocument();
    // Campeão uma vez: a faixa dourada é o fato mais alto da ficha.
    expect(screen.getByTestId("carreira-header-titulos")).toBeInTheDocument();
    // P3 aparece no cabeçalho, e a seção do piloto abre por padrão.
    expect(screen.getByTestId("carreira-piloto-agora")).toBeInTheDocument();
    expect(screen.getByTestId("carreira-secao-piloto")).toHaveAttribute("aria-selected", "true");
  });

  it("a seção de história mostra a escada e a tabela por temporada", async () => {
    render(<CarreiraTab />);
    await waitFor(() => expect(screen.getByTestId("carreira-secoes")).toBeInTheDocument());

    fireEvent.click(screen.getByTestId("carreira-secao-historia"));

    expect(screen.getByTestId("carreira-historia-escada")).toBeInTheDocument();
    const tabela = screen.getByTestId("carreira-historia-temporadas");
    // Da mais recente para a mais antiga, e a temporada em curso marcada como
    // parcial — sem a marca, um P3 de abril se leria como resultado final do ano.
    expect(tabela).toHaveTextContent("2026");
    expect(tabela).toHaveTextContent("Em curso");
    expect(tabela).toHaveTextContent("P1");
  });

  it("a sala de troféus lista o título com ano e equipe", async () => {
    render(<CarreiraTab />);
    await waitFor(() => expect(screen.getByTestId("carreira-secoes")).toBeInTheDocument());

    fireEvent.click(screen.getByTestId("carreira-secao-trofeus"));

    const prateleira = screen.getByTestId("carreira-trofeus-titulos");
    expect(prateleira).toHaveTextContent("2025");
    expect(prateleira).toHaveTextContent("Falcon Racing");
    const numeros = screen.getByTestId("carreira-trofeus-numeros");
    expect(numeros).toHaveTextContent("41");
    expect(numeros).toHaveTextContent("Largadas");
  });

  it("a sala de rivais mostra o placar do confronto direto", async () => {
    render(<CarreiraTab />);
    await waitFor(() => expect(screen.getByTestId("carreira-secoes")).toBeInTheDocument());

    fireEvent.click(screen.getByTestId("carreira-secao-rivais"));

    const card = screen.getByTestId("carreira-rival-card");
    expect(card).toHaveTextContent("Marco Bertone");
    expect(card).toHaveTextContent("11");
    expect(card).toHaveTextContent("6");
    expect(card).toHaveTextContent("Rivalidade forte");
    expect(card).toHaveTextContent("Nasceu de um toque na pista.");
  });

  it("o mercado mostra contrato, quem está de olho e as vagas com elegibilidade", async () => {
    render(<CarreiraTab />);
    await waitFor(() => expect(screen.getByTestId("carreira-secoes")).toBeInTheDocument());

    fireEvent.click(screen.getByTestId("carreira-secao-mercado"));

    // Último ano de contrato é o único estado que muda o que o jogador faz agora.
    await waitFor(() =>
      expect(screen.getByTestId("carreira-mercado-prazo")).toHaveTextContent(
        "Último ano de contrato",
      ),
    );
    expect(screen.getByTestId("carreira-mercado-interesse")).toHaveTextContent(
      "Nordschleife Works",
    );

    const vagas = screen.getByTestId("carreira-mercado-vagas");
    const linhas = vagas.querySelectorAll("li");
    expect(linhas).toHaveLength(2);
    expect(linhas[0]).toHaveAttribute("data-elegivel", "true");
    // A vaga fora da faixa aparece, e diz por que não é dele — em vez de sumir.
    expect(linhas[1]).toHaveAttribute("data-elegivel", "false");
    expect(linhas[1]).toHaveTextContent("Sem a licença");
  });

  it("uma falha no painel de vagas não derruba o resto do mercado", async () => {
    invoke.mockImplementation((command) => {
      if (command === "get_driver_detail") return Promise.resolve(DETALHE);
      if (command === "get_season_market_board") return Promise.reject("boom");
      if (command === "get_inbox_messages") return Promise.resolve(INBOX);
      return Promise.resolve(null);
    });

    render(<CarreiraTab />);
    await waitFor(() => expect(screen.getByTestId("carreira-secoes")).toBeInTheDocument());
    fireEvent.click(screen.getByTestId("carreira-secao-mercado"));

    expect(screen.getByTestId("carreira-mercado-contrato")).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByTestId("carreira-mercado-vagas")).toHaveTextContent(
        "Nenhum assento aberto no mundo agora.",
      ),
    );
  });

  it("sem piloto do jogador a aba diz isso em vez de ficar carregando", async () => {
    mockState = { careerId: "career_001", player: null, playerInterests: null };

    render(<CarreiraTab />);

    await waitFor(() => expect(screen.getByTestId("carreira-error")).toBeInTheDocument());
    expect(screen.getByTestId("carreira-error")).toHaveTextContent(
      "Nenhum piloto do jogador nesta carreira.",
    );
  });
});
