import { StrictMode } from "react";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import { pisoDeAbertura } from "../../ui/aberturaDePainel.js";
import { DriverDetailModalV2 } from "./DriverDetailModalV2";

// O piso não corre debaixo do vitest de qualquer jeito; o espião existe para
// poder afirmar COM QUE ARGUMENTO ele foi pedido, que é onde o bug morava.
vi.mock("../../ui/aberturaDePainel.js", () => ({
  ABERTURA_MS: 0,
  pisoDeAbertura: vi.fn(() => Promise.resolve()),
}));

let mockState = {};

vi.mock("../../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// O dossiê de habilidade do jogador é um bloco fechado do v1 com invoke próprio:
// aqui interessa só se a ABA aparece para o jogador, não o conteúdo dela.
vi.mock("../detalhes/PlayerSkillSection.jsx", () => ({
  PlayerSkillSection: () => <section>dossie-habilidade</section>,
}));

function detail(overrides = {}) {
  return {
    id: "D1",
    nome: "Ana Vitoria",
    nacionalidade: "Brasil",
    idade: 27,
    genero: "F",
    is_jogador: false,
    is_favorito: false,
    status: "ativo",
    equipe_id: "T1",
    equipe_nome: "First Gear Motorsport",
    equipe_cor_primaria: "#1f6feb",
    papel: "Numero1",
    motivacao: 72,
    stats_temporada: { corridas: 8, pontos: 96, vitorias: 2, podios: 4, poles: 1, dnfs: 0 },
    stats_carreira: { corridas: 120, pontos: 1800, vitorias: 22, podios: 55, poles: 18, dnfs: 7 },
    perfil: {
      nome: "Ana Vitoria",
      bandeira: "",
      nacionalidade: "Brasil",
      idade: 27,
      status: "ativo",
      is_jogador: false,
      equipe_nome: "First Gear Motorsport",
      papel: "Numero1",
      licenca: { nivel: "Elite", sigla: "E" },
      badges: [],
      equipe_cor_primaria: "#1f6feb",
    },
    competitivo: {
      personalidade_primaria: { tipo: "Calculista", emoji: "🧠", descricao: "Pensa a corrida." },
      personalidade_secundaria: null,
      motivacao: 72,
      qualidades: [{ attribute_name: "skill", tag_text: "Rapida", level: "alto", color: "#3fb950" }],
      defeitos: [],
      neutro: true,
    },
    leitura_tecnica: {
      itens: [
        {
          chave: "ritmo",
          grupo: "volta_seca",
          label: "Ritmo",
          nivel: "Elite",
          tom: "elite",
          escala: 92,
          referencia: 51,
          delta: 4,
        },
        {
          chave: "classificacao",
          grupo: "volta_seca",
          label: "Classificacao",
          nivel: "Forte",
          tom: "info",
          escala: 68,
          delta: -3,
        },
        {
          chave: "mentalidade",
          grupo: "corrida",
          label: "Sob pressao",
          nivel: "Forte",
          tom: "info",
          escala: 66,
        },
        {
          chave: "confianca",
          grupo: "estilo",
          label: "Confianca",
          nivel: "Confiante",
          tom: "neutral",
          escala: 58,
          estilo: true,
          polo_min: "Cauteloso",
          polo_max: "Confiante",
        },
        {
          chave: "racecraft",
          grupo: "corrida",
          label: "Racecraft",
          nivel: "Competente",
          tom: "neutral",
          escala: 55,
        },
        {
          chave: "agressividade",
          grupo: "estilo",
          label: "Agressividade",
          nivel: "Calculista",
          tom: "neutral",
          escala: 40,
          estilo: true,
          referencia: 62,
          polo_min: "Calculista",
          polo_max: "Agressivo",
        },
        {
          chave: "chuva",
          grupo: "condicoes",
          label: "Chuva",
          nivel: "Muito forte",
          tom: "success",
          escala: 80,
        },
      ],
    },
    arco: {
      idade: 27,
      fase: "Em ascensao",
      fase_chave: "ascensao",
      tom_fase: "info",
      nivel_experiencia: "Rodado",
      nivel_desenvolvimento: "Rapida",
      nivel_margem: "Boa",
      resumo: "Ainda tem estrada ate o proprio teto.",
    },
    estrelato: null,
    performance: { temporada: {}, carreira: {} },
    forma: {
      tendencia: "->",
      momento: "forte",
      media_chegada: 4.2,
      temporadas: [
        {
          season_number: 5,
          ano: 2025,
          atual: false,
          resultados: [
            { rodada: 1, chegada: 4, dnf: false },
            { rodada: 2, chegada: 2, dnf: false },
            { rodada: 3, chegada: null, dnf: true },
            { rodada: 4, chegada: 6, dnf: false },
          ],
        },
        {
          season_number: 6,
          ano: 2026,
          atual: true,
          resultados: [
            { rodada: 1, chegada: 1, dnf: false },
            { rodada: 2, chegada: 12, dnf: false },
          ],
        },
      ],
      ultimas_10: [
        { rodada: 1, chegada: 1, dnf: false },
        { rodada: 2, chegada: 12, dnf: false },
        { rodada: 3, chegada: null, dnf: true },
      ],
    },
    resumo_atual: { veredito: "Em alta", tom: "success", posicao_campeonato: 2, pontos: 96, vitorias: 2, podios: 4 },
    leitura_desempenho: { piloto_pontos: 96, companheiro_pontos: 48, leitura: "Domina o lado do box." },
    trajetoria: {
      ano_estreia: 2019,
      equipe_estreia: "Thunderline Academy",
      categoria_atual: "gt3",
      titulos: 2,
      foi_campeao: true,
      titulos_detalhe: [
        { ano: 2023, categoria: "gt3", equipe: "Ferrari", equipe_cor: "#dc0000" },
        { ano: 2021, categoria: "gt4", equipe: "Aures Racing", equipe_cor: "#3fb950" },
      ],
      categorias_timeline: [
        { categoria: "gt4", ano_inicio: 2019, ano_fim: 2021 },
        { categoria: "gt3", ano_inicio: 2022, ano_fim: 2024 },
      ],
      curva_campeonato: curvaCampeonato(),
      historico: {
        presenca: { tempo_carreira: 6, temporadas_disputadas: 6, anos_desempregado: 0, categorias_disputadas: 2 },
        primeiros_marcos: {
          primeira_vitoria_corrida: 6,
          primeiro_podio_corrida: 1,
          primeiro_dnf_corrida: 74,
          primeiro_titulo: { ano: 2021, categoria: "gt4", posicao_campeonato: 1 },
        },
        auge: {
          melhor_temporada: { ano: 2023, categoria: "gt3", posicao_campeonato: 1 },
          maior_sequencia_vitorias: 3,
          sequencia_ano_inicio: 2023,
          sequencia_ano_fim: 2023,
          maior_sequencia_podios: 9,
          sequencia_podios_ano_inicio: 2022,
          sequencia_podios_ano_fim: 2023,
          temporadas_no_top3: 4,
        },
        queda: {
          maior_seca_vitorias: 24,
          seca_ano_inicio: 2018,
          seca_ano_fim: 2021,
          maior_seca_podios: 31,
          seca_podios_ano_inicio: 2018,
          seca_podios_ano_fim: 2022,
          pior_temporada: { ano: 2019, categoria: "gt4", posicao_campeonato: 11 },
          temporadas_sem_podio: 2,
        },
        confiabilidade: {
          abandonos: 3,
          corridas: 40,
          taxa_abandono: 7.5,
          maior_sequencia_chegadas: 18,
        },
        sabado: { poles: 18, poles_convertidas: 11, grid_medio: 4.2, voltas_rapidas: 14 },
        duelos: {
          companheiros: 5,
          temporadas: 9,
          temporadas_vencidas: 7,
          rival_mais_duro: { nome: "Igor Petrov", temporadas: 3, vitorias: 1, derrotas: 2 },
        },
        referencias: { taxa_abandono: 4.2, grid_medio: 10.5 },
        detalhes: {
          equipes: [
            {
              equipe: "Aures Racing",
              equipe_cor: "#3fb950",
              equipe_id: "T1",
              periodo: "2019-2021",
              contagem: 42,
            },
            // Sem `equipe_id`: equipe que sumiu do mundo nao tem tela para abrir.
            { equipe: "Ferrari", equipe_cor: "#dc0000", periodo: "2022-2024", contagem: 78 },
          ],
          promocoes: [{ categoria_origem: "gt4", categoria: "gt3", periodo: "2022" }],
          // O que abre no hover dos quatro cards de carreira: ano, equipe e
          // categoria de cada conquista. Sem colocacao no campeonato — numa lista
          // de vitorias ela se le como posicao de chegada.
          vitorias: [
            {
              periodo: "2021",
              categoria: "gt4",
              equipe: "Aures Racing",
              equipe_cor: "#3fb950",
              equipe_id: "T1",
              contagem: 4,
            },
            {
              periodo: "2023",
              categoria: "gt3",
              equipe: "Ferrari",
              equipe_cor: "#dc0000",
              contagem: 6,
            },
          ],
          podios: [
            {
              periodo: "2023",
              categoria: "gt3",
              equipe: "Ferrari",
              equipe_cor: "#dc0000",
              contagem: 11,
            },
          ],
          titulos: [
            {
              periodo: "2021",
              categoria: "gt4",
              equipe: "Aures Racing",
              equipe_cor: "#3fb950",
              resumo: "240 pts · 4V · 9P",
            },
          ],
          temporadas: [
            {
              periodo: "2019",
              categoria: "gt4",
              equipe: "Aures Racing",
              equipe_cor: "#3fb950",
              texto: "P11",
              contagem: 12,
              resumo: "48 pts · 0V · 1P",
            },
            {
              periodo: "2023",
              categoria: "gt3",
              equipe: "Ferrari",
              equipe_cor: "#dc0000",
              texto: "P1",
              contagem: 14,
              resumo: "310 pts · 6V · 11P",
            },
          ],
          primeira_vitoria: [
            {
              periodo: "2019",
              data: "2019-05-13",
              rodada: 6,
              pista: "Circuito de Navarra",
              categoria: "gt4",
              equipe: "Aures Racing",
              equipe_cor: "#3fb950",
            },
          ],
          companheiros: [
            {
              texto: "Igor Petrov",
              equipe: "Vector Racing",
              equipe_cor: "#1f6feb",
              nacionalidade: "🇷🇺 Russo",
              idade: 29,
              contagem: 3,
              resumo: "1-2",
            },
            {
              texto: "Owen Clark",
              nacionalidade: "🇬🇧 Britânico",
              idade: 25,
              contagem: 1,
              resumo: "1-0",
            },
          ],
          lesoes_leves: [
            {
              texto: "Dor no pescoço",
              resumo: "Leve",
              periodo: "2023",
              rodada: 4,
              pista: "Oulton Park",
              equipe: "Ferrari",
              equipe_cor: "#dc0000",
            },
          ],
          lesoes_moderadas: [
            {
              texto: "Fratura no punho",
              resumo: "Moderada",
              periodo: "2020",
              rodada: 9,
              pista: "Interlagos",
              equipe: "Aures Racing",
              equipe_cor: "#3fb950",
            },
          ],
          tempo_carreira: [
            {
              periodo: "2013",
              data: "2013-04-07",
              rodada: 1,
              pista: "Oran Park Raceway",
              categoria: "toyota_amador",
              equipe: "Flat Six Motorsport",
              equipe_cor: "#f0883e",
            },
            {
              periodo: "2026",
              data: "2026-09-20",
              rodada: 1,
              pista: "Autodromo Nazionale Monza",
              categoria: "gt3",
              equipe: "Blackwell",
              equipe_cor: "#3fb950",
            },
          ],
          anos_parados: [
            {
              periodo: "2021",
              equipe: "Aures Racing",
              equipe_cor: "#3fb950",
              categoria: "gt4",
              contagem: 1,
              resumo: "→ 2022",
            },
          ],
          temporadas_sem_podio: [
            {
              periodo: "2019",
              categoria: "gt4",
              equipe: "Aures Racing",
              equipe_cor: "#3fb950",
              texto: "P11",
              contagem: 14,
              resumo: "38 pts · 0V · 0P",
              melhor_resultado: 5,
            },
          ],
          taxa_abandono: [
            {
              categoria: "gt3",
              periodo: "2022-2024",
              equipe: "Ferrari",
              equipe_cor: "#dc0000",
              contagem: 70,
              resumo: "3/70 · 4.3%",
            },
          ],
          rival_mais_duro: [
            {
              periodo: "2022",
              categoria: "gt3",
              equipe: "Ferrari",
              equipe_cor: "#dc0000",
              resumo: "388 x 412",
            },
          ],
        },
        recordes: {
          poles: { grid: 2, grid_total: 24, mundo: 41, mundo_total: 610 },
          // Sem posição no grid: piloto aposentado não tem pelotão de domingo.
          taxa_abandono: { grid: null, grid_total: 0, mundo: 7, mundo_total: 610 },
        },
        mobilidade: {},
        lesoes: {},
        eventos_especiais: {},
      },
      marcos: [],
    },
    rankings_carreira: { corridas: 40, vitorias: 3, podios: 8, titulos: 12, total: 240 },
    rivais: { itens: [] },
    contrato_mercado: { contrato: null, mercado: null },
    saude: null,
    ...overrides,
  };
}

// A ficha faz DUAS buscas: o payload do piloto e, à parte, a posição no ranking
// mundial. `worldRank: undefined` = o comando não respondeu, que é o caso normal
// aqui — a marca do ranking tem teste próprio.
function renderFicha(props = {}, payload = detail(), worldRank = null) {
  invoke.mockImplementation((command) =>
    Promise.resolve(command === "get_driver_world_rank" ? worldRank : payload),
  );
  return render(
    <DriverDetailModalV2
      driverId="D1"
      driverIds={["D0", "D1", "D2"]}
      onSelectDriver={vi.fn()}
      onClose={vi.fn()}
      {...props}
    />,
  );
}

// A ficha abre no Histórico; os casos da temporada corrente precisam trocar de
// aba antes de procurar o conteúdo.
async function abrirTemporada() {
  await screen.findByTestId("driver-detail-hero");
  fireEvent.click(screen.getByTestId("driver-detail-tab-temporada"));
}

async function abrirPerfil() {
  await screen.findByTestId("driver-detail-hero");
  fireEvent.click(screen.getByTestId("driver-detail-tab-perfil"));
}

async function abrirRivais() {
  await screen.findByTestId("driver-detail-hero");
  fireEvent.click(screen.getByTestId("driver-detail-tab-rivais"));
}

async function abrirMercado() {
  await screen.findByTestId("driver-detail-hero");
  fireEvent.click(screen.getByTestId("driver-detail-tab-mercado"));
}

// Cinco temporadas subindo a escada: salário atrás do que o mercado pagaria.
// A carreira em posicao de campeonato: seis temporadas, dois titulos (2021 na
// gt4, 2023 na gt3) e a de 2024 ainda em disputa. O grid muda de tamanho junto
// com a categoria — 24 na escada de entrada, 18 na gt3 — que e justamente o que
// o chao do grafico existe para mostrar.
function curvaCampeonato() {
  return [
    { season_number: 1, ano: 2019, categoria: "gt4", equipe_nome: "Thunderline Academy", equipe_cor: "#3fb950", posicao: 9, grid: 24, esperado: 11, pontos: 88, vitorias: 0, podios: 1, corridas: 12, titulo: false, atual: false },
    { season_number: 2, ano: 2020, categoria: "gt4", equipe_nome: "Thunderline Academy", equipe_cor: "#3fb950", posicao: 4, grid: 24, esperado: 6, pontos: 160, vitorias: 1, podios: 5, corridas: 12, titulo: false, atual: false },
    { season_number: 3, ano: 2021, categoria: "gt4", equipe_nome: "Aures Racing", equipe_cor: "#3fb950", posicao: 1, grid: 22, esperado: 3, pontos: 240, vitorias: 6, podios: 10, corridas: 12, titulo: true, atual: false },
    { season_number: 4, ano: 2022, categoria: "gt3", equipe_nome: "Aures Racing", equipe_cor: "#3fb950", posicao: 12, grid: 18, esperado: 8, pontos: 54, vitorias: 0, podios: 0, corridas: 14, titulo: false, atual: false },
    { season_number: 5, ano: 2023, categoria: "gt3", equipe_nome: "Ferrari", equipe_cor: "#dc0000", posicao: 1, grid: 18, esperado: 2, pontos: 310, vitorias: 7, podios: 12, corridas: 14, titulo: true, atual: false },
    { season_number: 6, ano: 2024, categoria: "gt3", equipe_nome: "Ferrari", equipe_cor: "#dc0000", posicao: 3, grid: 18, esperado: 4, pontos: 96, vitorias: 2, podios: 4, corridas: 6, titulo: false, atual: true },
  ];
}

function curva() {
  return [
    { season_number: 1, ano: 2022, categoria: "gt4", equipe_nome: "Sunday Speed Club", equipe_cor: "#3fb950", salario_contrato: 42000, salario_mercado: 60000, atual: false },
    { season_number: 2, ano: 2023, categoria: "gt4", equipe_nome: "Sunday Speed Club", equipe_cor: "#3fb950", salario_contrato: 42000, salario_mercado: 110000, atual: false },
    { season_number: 3, ano: 2024, categoria: "gt3", equipe_nome: "Arclight", equipe_cor: "#dc0000", salario_contrato: 310000, salario_mercado: 420000, atual: false },
    { season_number: 4, ano: 2025, categoria: "gt3", equipe_nome: "Arclight", equipe_cor: "#dc0000", salario_contrato: 960534, salario_mercado: 980000, atual: false },
    { season_number: 5, ano: 2026, categoria: "gt3", equipe_nome: "Arclight", equipe_cor: "#dc0000", salario_contrato: 960534, salario_mercado: 1300000, atual: true },
  ];
}

function contrato(overrides = {}) {
  return {
    equipe_nome: "Arclight",
    papel: "Numero2",
    salario_anual: 960534,
    ano_inicio: 2026,
    ano_fim: 2026,
    anos_restantes: 0,
    status: "ativo",
    ...overrides,
  };
}

// Um rival completo: o motor de rivalidade de um lado, o confronto direto de
// `race_results` do outro.
function rival(overrides = {}) {
  return {
    driver_id: "D9",
    nome: "Tiago Sousa",
    tipo: "Colisao",
    nivel_chave: "clara",
    intensidade: 47,
    intensidade_historica: 38,
    atividade_recente: 52,
    confrontos: 23,
    vitorias: 13,
    derrotas: 10,
    vitorias_quali: 9,
    derrotas_quali: 14,
    gap_medio: 2.35,
    companheirismo: null,
    encontros: [
      { ano: 2026, season_number: 2, rodada: 1, pista: "Monza", vencedor: "piloto", gap: -0.4 },
      { ano: 2026, season_number: 2, rodada: 2, pista: "Spa", vencedor: "rival", gap: 12 },
      { ano: 2027, season_number: 3, rodada: 5, pista: "Interlagos", vencedor: "piloto", gap: -3 },
    ],
    categoria_atual: "GT3",
    mesma_categoria: true,
    equipe_nome: "Aures Racing",
    equipe_cor: "#3fb950",
    ...overrides,
  };
}

// O jsdom nao faz layout: `scrollHeight` e `clientHeight` sao 0 em tudo, entao
// nenhum painel se declararia rolavel e o mecanismo de prender ficaria morto nos
// testes. Estes dois helpers dizem ao painel se ha ou nao o que alcancar.
function fingeQueRola(rola) {
  Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
    configurable: true,
    get: () => (rola ? 640 : 320),
  });
  Object.defineProperty(HTMLElement.prototype, "clientHeight", {
    configurable: true,
    get: () => 320,
  });
}

function restauraLayout() {
  delete HTMLElement.prototype.scrollHeight;
  delete HTMLElement.prototype.clientHeight;
}

describe("DriverDetailModalV2", () => {
  beforeEach(() => {
    mockState = { careerId: "career-1" };
    invoke.mockReset();
    fingeQueRola(true);
  });

  afterEach(restauraLayout);

  it("nao gasta a abertura na passagem descartada do StrictMode", async () => {
    pisoDeAbertura.mockClear();
    invoke.mockImplementation((command) =>
      Promise.resolve(command === "get_driver_world_rank" ? null : detail()),
    );

    render(
      <DriverDetailModalV2 driverId="D1" driverIds={["D1"]} onSelectDriver={vi.fn()} onClose={vi.fn()} />,
      { wrapper: StrictMode },
    );
    await screen.findByTestId("driver-detail-hero");

    // Em dev o StrictMode monta, desmonta e remonta o efeito. Enquanto a
    // bandeira da primeira carga era baixada ao PEDIR o piso, quem a gastava era
    // a passagem descartada — e a passagem que de fato desenha pedia o piso como
    // se fosse navegação entre pilotos, sem esperar nada.
    expect(pisoDeAbertura).toHaveBeenCalled();
    expect(pisoDeAbertura.mock.calls.every(([ehAbertura]) => ehAbertura === true)).toBe(true);
  });

  it("carrega titulo, personalidade e estado no cabecalho, e nao numeros de carreira", async () => {
    renderFicha();

    const header = await screen.findByTestId("driver-detail-hero");
    expect(header.querySelector('[data-personality="primary"]')).toHaveTextContent("Calculista");
    expect(within(header).getByTestId("driver-detail-motivation")).toHaveTextContent("72%");
    // A posição no campeonato saiu do cabeçalho: é da temporada corrente e já
    // abre em número grande na aba Temporada atual.
    expect(within(header).getByTestId("driver-detail-state")).not.toHaveTextContent("P2");
    // Título tem faixa própria; no meio dos chips cinzas ele saía com o peso
    // visual de uma licença.
    expect(within(header).getByTestId("driver-detail-titles")).toHaveTextContent("2");
    // As 120 corridas de carreira vivem no Histórico, onde há rank para dar
    // escala a elas — no cabeçalho seriam números sem referência, e zeros na
    // ficha de um estreante.
    expect(header).not.toHaveTextContent("120");
  });

  it("desenha a posicao no ranking mundial ao lado da estrela", async () => {
    renderFicha({}, detail(), { indice: 4210.4, posicao: 12, total: 240, delta: 3 });

    const mark = await screen.findByTestId("driver-detail-world-rank");
    expect(mark).toHaveTextContent("12º");
    // O índice vai arredondado: 4.210,4 e 4.210 dizem a mesma coisa no topo.
    expect(mark).toHaveTextContent("4.210");
    expect(mark).toHaveTextContent("▲3");
  });

  it("nao desenha a marca do ranking para quem esta fora dele", async () => {
    renderFicha({}, detail(), null);

    await screen.findByTestId("driver-detail-hero");
    expect(screen.queryByTestId("driver-detail-world-rank")).toBeNull();
  });

  it("some com a faixa de titulo quem nunca foi campeao", async () => {
    renderFicha({}, detail({ trajetoria: { ...detail().trajetoria, titulos: 0, foi_campeao: false } }));

    const header = await screen.findByTestId("driver-detail-hero");
    expect(within(header).queryByTestId("driver-detail-titles")).toBeNull();
  });

  it("mostra o que um estreante TEM em vez de um vazio", async () => {
    renderFicha(
      {},
      detail({
        stats_temporada: { corridas: 0, pontos: 0, vitorias: 0, podios: 0, poles: 0, dnfs: 0 },
        stats_carreira: { corridas: 0, pontos: 0, vitorias: 0, podios: 0, poles: 0, dnfs: 0 },
        resumo_atual: { posicao_campeonato: 1, pontos: 0, vitorias: 0, podios: 0 },
        contrato_mercado: {
          contrato: {
            equipe_nome: "Sunday Speed Club",
            papel: "Numero2",
            salario_anual: 120000,
            ano_inicio: 2026,
            ano_fim: 2027,
            anos_restantes: 2,
            status: "ativo",
          },
          mercado: null,
        },
      }),
    );

    await abrirTemporada();
    expect(await screen.findByTestId("driver-detail-rookie-banner")).toBeInTheDocument();
    // O contrato é o que ainda cabe aqui; traços e leitura técnica mudaram de
    // casa para a aba "Perfil", que é a que responde por um piloto sem temporada.
    expect(screen.getByText("Sunday Speed Club")).toBeInTheDocument();
    // Antes da primeira largada o grid inteiro está zerado e a ordem é desempate
    // alfabético: anunciar "Campeonato P1" para quem nunca correu é mentira.
    expect(screen.getByTestId("driver-detail-drawer")).not.toHaveTextContent("P1");
  });

  it("abre a temporada com a posicao e a distancia, e nao com quatro caixas", async () => {
    renderFicha(
      {},
      detail({
        resumo_atual: {
          veredito: "Em alta",
          tom: "success",
          posicao_campeonato: 3,
          gap_lider: 8,
          gap_proximo: 2,
          pontos: 15,
          vitorias: 0,
          podios: 1,
          media_recente: 3,
          tendencia: "->",
        },
      }),
    );

    await abrirTemporada();
    const faixa = await screen.findByTestId("driver-detail-verdict");
    // A posição aparece UMA vez, e grande — não mais repartida entre o card de
    // veredito e um MiniMetric "Campeonato".
    expect(screen.getByTestId("driver-detail-championship")).toHaveTextContent("P3");
    expect(faixa).toHaveTextContent("Em alta");

    // P3 a 8 pontos do líder e P3 a 80 são temporadas diferentes: é a distância
    // que diz qual das duas, e ela ocupa o lugar que os zeros ocupavam.
    const linha = screen.getByTestId("driver-detail-standings-line");
    expect(linha).toHaveTextContent("15 pontos");
    expect(linha).toHaveTextContent("8 pontos do líder");
    expect(linha).toHaveTextContent("+2 sobre o P4");
    expect(linha).toHaveTextContent("1 pódio");
    // Zero vitória não é notícia — a ausência já se lê na faixa de forma.
    expect(linha).not.toHaveTextContent("vitória");
  });

  it("chama de lider quem lidera, em vez de dizer que esta a zero do lider", async () => {
    renderFicha(
      {},
      detail({
        resumo_atual: { veredito: "Em alta", tom: "success", posicao_campeonato: 1, gap_lider: 0, gap_proximo: 12, pontos: 96 },
      }),
    );

    await abrirTemporada();
    const linha = await screen.findByTestId("driver-detail-standings-line");
    expect(linha).toHaveTextContent("líder do campeonato");
    expect(linha).not.toHaveTextContent("do líder");
    expect(linha).toHaveTextContent("+12 sobre o P2");
  });

  it("quebra a classificacao em linhas com ancora, e nao numa fita de texto", async () => {
    renderFicha(
      {},
      detail({
        resumo_atual: {
          veredito: "Bom",
          tom: "success",
          posicao_campeonato: 1,
          gap_lider: 0,
          gap_proximo: 8,
          pontos: 26,
          vitorias: 1,
          podios: 1,
          media_recente: 1,
          tendencia: "->",
        },
      }),
    );

    await abrirTemporada();
    const bloco = await screen.findByTestId("driver-detail-standings-line");
    // Quatro fatos, quatro linhas: pontos+situação, vantagem, conquistas, média.
    // Num `join(" · ")` os cinco pesavam igual e nenhum era achável de relance.
    expect(bloco.children).toHaveLength(4);
    // A posição virou o assunto da faixa, em corpo grande.
    expect(screen.getByTestId("driver-detail-championship")).toHaveTextContent("P1");
  });

  it("nao gasta uma linha para anunciar que nao houve vitoria nem podio", async () => {
    renderFicha(
      {},
      detail({
        resumo_atual: {
          veredito: "Regular",
          tom: "warning",
          posicao_campeonato: 9,
          gap_lider: 40,
          gap_proximo: 2,
          pontos: 6,
          vitorias: 0,
          podios: 0,
          media_recente: 9,
        },
      }),
    );

    await abrirTemporada();
    const bloco = await screen.findByTestId("driver-detail-standings-line");
    expect(bloco.children).toHaveLength(3);
    expect(bloco).not.toHaveTextContent("pódio");
  });

  it("desenha o delta contra o pacote dentro da propria faixa", async () => {
    renderFicha(
      {},
      detail({
        leitura_desempenho: {
          entregue_posicao: 2,
          esperado_posicao: 6,
          delta_posicao: 4,
          piloto_pontos: 96,
          companheiro_pontos: 48,
          leitura: "Entrega acima do pacote atual.",
        },
      }),
    );

    await abrirTemporada();
    // "P2 é bom?" só se responde contra o que o pacote prometia — a resposta
    // agora mora ao lado do P2, no espaço que a faixa deixava em branco.
    const faixa = await screen.findByTestId("driver-detail-verdict");
    const delta = within(faixa).getByTestId("driver-detail-performance");
    expect(delta).toHaveTextContent("+4");
    expect(delta).toHaveTextContent("P2 entregue");
    expect(delta).toHaveTextContent("P6 esperado");
    expect(delta).toHaveTextContent("Entrega acima do pacote atual.");
  });

  it("cala o delta quando o pacote nao prometeu nada", async () => {
    renderFicha(
      {},
      detail({ leitura_desempenho: { piloto_pontos: 96, companheiro_pontos: 48, leitura: "-" } }),
    );

    await abrirTemporada();
    await screen.findByTestId("driver-detail-verdict");
    // Sem `esperado_posicao` um "+0" pendurado na faixa afirmaria que o piloto
    // entregou o previsto quando ninguém previu nada.
    expect(screen.queryByTestId("driver-detail-performance")).toBeNull();
  });

  // O zero é o CENTRO da barra. Enquanto ele era a borda e o preenchimento media
  // a fatia do par, qualquer x × 0 dava "100% contra 0%": vencer por 2 e vencer
  // por 15 desenhavam idêntico. Ancorada no meio, a barra cresce com a margem.
  it.each([
    { meus: 2, dele: 0, fill: "5", side: "piloto", ponta: "esquerda" },
    { meus: 15, dele: 0, fill: "38", side: "piloto", ponta: "esquerda" },
    { meus: 20, dele: 0, fill: "50", side: "piloto", ponta: "esquerda" },
    { meus: 40, dele: 0, fill: "50", side: "piloto", ponta: "esquerda" },
    { meus: 0, dele: 15, fill: "38", side: "companheiro", ponta: "direita" },
  ])("cresce do meio com a margem ($meus x $dele)", async ({ meus, dele, fill, side, ponta }) => {
    renderFicha(
      {},
      detail({
        leitura_desempenho: {
          piloto_pontos: meus,
          companheiro_pontos: dele,
          companheiro_nome: "Romain Fournier",
        },
      }),
    );

    await abrirTemporada();
    const barra = await screen.findByTestId("driver-detail-duel-bar");
    expect(barra).toHaveAttribute("data-fill", fill);
    expect(barra).toHaveAttribute("data-side", side);
    // A direção é a PONTA do próprio preenchimento. Como chevron solto ao lado da
    // barra ela boiava no trilho vazio, e na margem saturada escapava para fora.
    expect(within(barra).getByTestId("driver-detail-duel-fill")).toHaveAttribute(
      "data-tip",
      ponta,
    );
    // A margem é lida junto com o desenho, pendurada sobre a ponta — a frase que
    // ficava embaixo virou `aria-label`, para quem não vê o desenho.
    expect(within(barra).getByTestId("driver-detail-duel-margin")).toHaveTextContent(
      `+${Math.abs(meus - dele)}`,
    );
    expect(barra).toHaveAttribute("aria-label", expect.stringContaining("pontos"));
  });

  it("diz em que altura do campeonato o duelo interno acontece", async () => {
    renderFicha(
      {},
      detail({
        leitura_desempenho: {
          piloto_pontos: 18,
          companheiro_pontos: 1,
          companheiro_nome: "Rasmus Thomsen",
          entregue_posicao: 2,
          companheiro_posicao: 14,
          companheiro_nacionalidade: "Dinamarca",
        },
      }),
    );

    await abrirTemporada();
    const bloco = await screen.findByTestId("driver-detail-teammate");
    // 18 × 1 na briga do título e 18 × 1 no fundo do grid desenham a mesma barra
    // e são duas temporadas diferentes.
    expect(within(bloco).getByTestId("driver-detail-duel-pos-piloto")).toHaveTextContent("P2");
    expect(within(bloco).getByTestId("driver-detail-duel-pos-companheiro")).toHaveTextContent(
      "P14",
    );
    // Uma bandeira por lado — a do piloto aberto e a do companheiro.
    expect(within(bloco).getAllByRole("img", { name: /Brasil|Dinamarca/ })).toHaveLength(2);
  });

  it("nao anuncia posicao no duelo interno antes da primeira largada", async () => {
    renderFicha(
      {},
      detail({
        stats_temporada: { corridas: 0, pontos: 0, vitorias: 0, podios: 0, poles: 0, dnfs: 0 },
        leitura_desempenho: {
          piloto_pontos: 0,
          companheiro_pontos: 0,
          companheiro_nome: "Rasmus Thomsen",
          entregue_posicao: 1,
          companheiro_posicao: 2,
        },
      }),
    );

    await abrirTemporada();
    const bloco = await screen.findByTestId("driver-detail-teammate");
    // Com o grid inteiro zerado a ordem é desempate alfabético — mesmo motivo
    // pelo qual a faixa lá em cima também cala a posição.
    expect(within(bloco).queryByTestId("driver-detail-duel-pos-piloto")).toBeNull();
    expect(within(bloco).queryByTestId("driver-detail-duel-pos-companheiro")).toBeNull();
  });

  it("nao inventa vencedor no duelo interno quando ninguem pontuou", async () => {
    renderFicha(
      {},
      detail({
        leitura_desempenho: { piloto_pontos: 0, companheiro_pontos: 0, companheiro_nome: "X" },
      }),
    );

    await abrirTemporada();
    const bloco = await screen.findByTestId("driver-detail-teammate");
    const barra = within(bloco).getByTestId("driver-detail-duel-bar");
    expect(barra).toHaveAttribute("data-side", "empate");
    expect(barra).toHaveAttribute("data-fill", "0");
    expect(barra).toHaveAttribute("aria-label", "Empatados");
    // Sem margem o rótulo é "0" sobre o zero — nem "+0", que sugeriria vantagem.
    expect(within(barra).getByTestId("driver-detail-duel-margin")).toHaveTextContent("0");
    // E sem vantagem não há direção: uma ponta apontando para algum lado no
    // empate seria a única informação errada da barra.
    expect(within(barra).queryByTestId("driver-detail-duel-fill")).toBeNull();
    // A marca do zero fica mesmo sem preenchimento: é ela que faz a barra
    // significar margem em vez de território.
    expect(within(barra).getByTestId("driver-detail-duel-zero")).toBeInTheDocument();
  });

  // Nada do perfil sai das corridas do ano: traços e níveis técnicos saem dos
  // atributos do piloto e dizem o mesmo em janeiro e em dezembro. Encostados nos
  // números da temporada eles se liam como veredito de forma.
  it("tira o perfil da aba de temporada", async () => {
    renderFicha();

    await abrirTemporada();
    await screen.findByTestId("driver-detail-verdict");
    expect(screen.queryByTestId("driver-detail-profile-strip")).toBeNull();
    expect(screen.queryByTestId("driver-detail-technical")).toBeNull();
  });

  // A fita comanda o realce de tudo o que vem depois: embaixo do que comanda,
  // ela obrigaria a olhar para cima e para baixo ao mesmo tempo.
  it("abre a aba de perfil pelos tracos", async () => {
    renderFicha();

    await abrirPerfil();
    const strip = await screen.findByTestId("driver-detail-profile-strip");
    const blocos = [...strip.parentElement.children].map((node) =>
      node.getAttribute("data-testid"),
    );
    expect(blocos[0]).toBe("driver-detail-profile-strip");
    expect(blocos).toContain("driver-detail-technical");
  });

  it("agrupa a leitura tecnica por onde o eixo se manifesta", async () => {
    renderFicha();

    await abrirPerfil();
    const tecnica = await screen.findByTestId("driver-detail-technical");
    // Doze eixos numa fita única viram um paredão: cada coluna responde a uma
    // pergunta inteira, e a ordem acompanha o fim de semana.
    const grupos = [...tecnica.querySelectorAll("[data-technical-group]")].map((node) =>
      node.getAttribute("data-technical-group"),
    );
    expect(grupos).toEqual(["volta_seca", "corrida", "condicoes"]);
    expect(tecnica.querySelector("[data-technical='ritmo']")).toHaveTextContent("Elite");
    // Estilo saiu da leitura técnica: vizinho de eixo com nota, o marcador no
    // meio da régua era lido como nota média.
    expect(tecnica.querySelector("[data-technical='agressividade']")).toBeNull();
  });

  // Agressividade não tem lado bom: um piloto agressivo ganha na largada e paga
  // em pneu e em incidente. O eixo É o par de polos — duas palavras e um
  // marcador, e nem nome de eixo nem faixa do meio repetindo a mesma coisa.
  it("reduz o estilo a dois polos e um marcador", async () => {
    renderFicha();

    await abrirPerfil();
    const estilo = await screen.findByTestId("driver-detail-style");
    const agressividade = estilo.querySelector("[data-technical='agressividade']");
    expect(agressividade).toHaveTextContent("Calculista");
    expect(agressividade).toHaveTextContent("Agressivo");
    // O nome do eixo e a faixa do meio saem da tela: eram um terceiro e um quarto
    // rótulo dizendo o que os dois polos e a posição já dizem.
    expect(agressividade).not.toHaveTextContent("Agressividade");
    expect(agressividade.querySelector("[data-technical-marker='estilo']")).toHaveStyle({
      left: "40%",
    });
    // Estilo não ganha barra preenchida em lugar nenhum: preencher diz "mais é
    // melhor", que é exatamente o julgamento que este bloco existe para não fazer.
    expect(agressividade.querySelector("div[style*='width']")).toBeNull();
    // Duas marcas soltas obrigam a medir com o olho; o vão entre elas desenha a
    // distância até o grid como comprimento — 40 contra mediana 62.
    expect(agressividade.querySelector("[data-technical-vao]")).toHaveStyle({
      left: "40%",
      width: "22%",
    });
  });

  // Confiança é o terceiro do trio que sai no roster de IA do iRacing
  // (aggression, smoothness, optimism), e "Fraco em confiança" seria um defeito
  // onde só existe um jeito de correr.
  it("le a confianca como polo e nao como nota", async () => {
    renderFicha();

    await abrirPerfil();
    const estilo = await screen.findByTestId("driver-detail-style");
    const confianca = estilo.querySelector("[data-technical='confianca']");
    expect(confianca).toHaveTextContent("Cauteloso");
    expect(confianca).toHaveTextContent("Confiante");
    expect(screen.getByTestId("driver-detail-technical").querySelector("[data-technical='confianca']")).toBeNull();
  });

  // A régua é o que separa "Forte" de "Muito forte" sem obrigar a decorar a
  // escala de cor.
  it("desenha a regua de cada eixo", async () => {
    renderFicha();

    await abrirPerfil();
    const tecnica = await screen.findByTestId("driver-detail-technical");
    const ritmo = tecnica.querySelector("[data-technical='ritmo'] div[style*='width']");
    expect(ritmo).toHaveStyle({ width: "92%" });
  });

  // Mentalidade era um dos dois atributos que a ficha inteira nunca mostrava — só
  // vazava como chip em Traços quando era extremo — e decide corrida: é ela que
  // resolve clutch/choke sob pressão de campeonato e de duelo. Fica na CORRIDA,
  // que é onde a pressão se manifesta.
  it("mostra a mentalidade, que a ficha nunca mostrou", async () => {
    renderFicha();

    await abrirPerfil();
    const tecnica = await screen.findByTestId("driver-detail-technical");
    const corrida = tecnica.querySelector("[data-technical-group='corrida']");
    expect(corrida.querySelector("[data-technical='mentalidade']")).toHaveTextContent("Forte");
  });

  // A régua se explica OLHANDO — a mediana é linha de referência, mais alta que a
  // régua e mais apagada que o dado. O hover só nomeia essa linha, e num painel
  // com a casca do tooltip do dossiê em vez do balão do sistema operacional.
  it("nomeia a linha da mediana no hover", async () => {
    renderFicha();

    await abrirPerfil();
    const tecnica = await screen.findByTestId("driver-detail-technical");
    fireEvent.mouseEnter(tecnica.querySelector("[data-technical='ritmo'] [data-technical-regua]"));

    const painel = await screen.findByTestId("driver-detail-regua-tooltip");
    expect(painel).toHaveTextContent("Mediana do grid");
    // Nada de valor bruto: "72 de racecraft" não é linguagem do jogo, e a régua
    // já mostra a magnitude sem precisar soletrá-la.
    expect(painel.textContent).not.toMatch(/\d/);
  });

  // Sem grid para comparar não há linha na régua, e sem linha não há o que
  // explicar: o eixo sai sem alvo de hover em vez de abrir um painel vazio.
  it("nao abre painel no eixo sem mediana", async () => {
    renderFicha();

    await abrirPerfil();
    const tecnica = await screen.findByTestId("driver-detail-technical");
    fireEvent.mouseEnter(
      tecnica.querySelector("[data-technical='classificacao'] [data-technical-regua]"),
    );

    expect(screen.queryByTestId("driver-detail-regua-tooltip")).toBeNull();
  });

  // "Instável" contra quem, e desde quando: as duas âncoras que a régua abriu.
  it("ancora o eixo na mediana do grid e no ano passado", async () => {
    renderFicha();

    await abrirPerfil();
    const tecnica = await screen.findByTestId("driver-detail-technical");
    const ritmo = tecnica.querySelector("[data-technical='ritmo']");
    expect(ritmo.querySelector("[data-technical-median]")).toHaveStyle({ left: "51%" });
    expect(ritmo.querySelector("[data-technical-delta='subiu']")).toHaveTextContent("+4");

    const classificacao = tecnica.querySelector("[data-technical='classificacao']");
    expect(classificacao.querySelector("[data-technical-delta='caiu']")).toHaveTextContent("-3");
    // Sem mediana no payload não se inventa um traço no meio da régua.
    expect(classificacao.querySelector("[data-technical-median]")).toBeNull();
  });

  // Payload antigo (save aberto por build anterior) não traz a régua: a linha
  // volta a ser rótulo e nível em vez de desenhar uma barra zerada, que se leria
  // como "este piloto é zero em ritmo".
  it("nao inventa regua quando o payload nao tem escala", async () => {
    renderFicha(
      {},
      detail({
        leitura_tecnica: {
          itens: [{ chave: "ritmo", grupo: "volta_seca", label: "Ritmo", nivel: "Elite", tom: "elite" }],
        },
      }),
    );

    await abrirPerfil();
    const tecnica = await screen.findByTestId("driver-detail-technical");
    expect(tecnica.querySelector("[data-technical='ritmo']")).toHaveTextContent("Elite");
    expect(tecnica.querySelector("[data-technical='ritmo'] div[style*='width']")).toBeNull();
  });

  // A tag JÁ nomeia o eixo em quase todo caso ("Bom Defensor" é defesa): o eixo
  // fica no `title`, e a pílula diz a palavra e mais nada.
  it("mantem os tracos como chips com o eixo so no title", async () => {
    renderFicha();

    await abrirPerfil();
    const perfil = await screen.findByTestId("driver-detail-profile-strip");
    const traco = within(perfil).getByText("Rapida").closest("[data-trait]");
    expect(traco).toHaveAttribute("data-trait", "strength");
    expect(traco).toHaveTextContent(/^Rapida$/);
    expect(traco).toHaveAttribute("data-tooltip", "Velocidade");
  });

  // O traço é o pico de um eixo, e a fita não dizia de qual. Passar o mouse
  // responde no lugar onde o dado já está desenhado: o eixo fica, o resto recua.
  it("acende o eixo de origem ao passar por cima do traco", async () => {
    renderFicha();

    await abrirPerfil();
    const perfil = await screen.findByTestId("driver-detail-profile-strip");
    const tecnica = await screen.findByTestId("driver-detail-technical");
    const traco = within(perfil).getByText("Rapida").closest("[data-trait]");
    // "Rapida" e uma tag de skill, e skill se chama "ritmo" na leitura tecnica.
    expect(traco).toHaveAttribute("data-trait-eixo", "ritmo");

    const ritmo = tecnica.querySelector("[data-technical='ritmo']");
    const mentalidade = tecnica.querySelector("[data-technical='mentalidade']");
    expect(ritmo.className).not.toMatch(/opacity-25/);
    expect(mentalidade.className).not.toMatch(/opacity-25/);

    fireEvent.mouseEnter(traco);
    // Os DOIS lados: o entorno recua E o alvo acende. Só apagar deixava o olho
    // procurando o buraco no escuro — treze reguas mudavam e a que interessa
    // continuava igual a si mesma.
    expect(ritmo.className).not.toMatch(/opacity-25/);
    expect(ritmo).toHaveAttribute("data-em-foco", "true");
    expect(ritmo.querySelector("div[style*='box-shadow']")).not.toBeNull();
    expect(mentalidade.className).toMatch(/opacity-25/);
    expect(mentalidade).not.toHaveAttribute("data-em-foco");
    // O estilo recua junto: o realce vale para a aba inteira, nao so para o
    // painel em que o eixo caiu.
    const estilo = await screen.findByTestId("driver-detail-style");
    expect(estilo.querySelector("[data-technical='confianca']").className).toMatch(/opacity-25/);

    fireEvent.mouseLeave(traco);
    expect(mentalidade.className).not.toMatch(/opacity-25/);
  });

  // Experiencia, desenvolvimento e midia nao tem regua na leitura tecnica: sem
  // alvo no arco e no estrelato, um terco dos tracos de um novato ("Calouro",
  // "Em Ascensao") teria hover morto.
  it("acende o arco quando o traco nao tem regua propria", async () => {
    renderFicha(
      {},
      detail({
        competitivo: {
          qualidades: [],
          defeitos: [
            { attribute_name: "experiencia", tag_text: "Calouro", level: "defeito_grave", color: "#f85149" },
          ],
          neutro: false,
        },
      }),
    );

    await abrirPerfil();
    const perfil = await screen.findByTestId("driver-detail-profile-strip");
    const arco = await screen.findByTestId("driver-detail-arc");
    const traco = within(perfil).getByText("Calouro").closest("[data-trait]");
    expect(traco).toHaveAttribute("data-trait-eixo", "arco:experiencia");

    fireEvent.mouseEnter(traco);
    expect(arco.querySelector("[data-arc='experiencia']").className).not.toMatch(/opacity-25/);
    expect(arco.querySelector("[data-arc='margem']").className).toMatch(/opacity-25/);
  });

  // A fita se lê como um degradê: do que ele tem de melhor ao que ele tem de
  // pior. Sem isso, o nível só existiria como cor, e duas qualidades de peso
  // muito diferente sairiam lado a lado na ordem em que o payload as listou.
  it("ordena os tracos do nivel mais alto ao mais baixo", async () => {
    renderFicha(
      {},
      detail({
        competitivo: {
          qualidades: [
            { attribute_name: "consistencia", tag_text: "Metronomo", level: "qualidade", color: "#3fb950" },
            { attribute_name: "skill", tag_text: "Fenomeno", level: "elite", color: "#bc8cff" },
          ],
          defeitos: [
            { attribute_name: "experiencia", tag_text: "Cru", level: "defeito_grave", color: "#f85149" },
            { attribute_name: "fitness", tag_text: "Sedentario", level: "defeito", color: "#db6d28" },
          ],
          neutro: false,
        },
      }),
    );

    await abrirPerfil();
    const perfil = await screen.findByTestId("driver-detail-profile-strip");
    const niveis = [...perfil.querySelectorAll("[data-trait-level]")].map((no) =>
      no.getAttribute("data-trait-level"),
    );
    expect(niveis).toEqual(["elite", "qualidade", "defeito", "defeito_grave"]);
  });

  // A única pergunta de contratação que a ficha não respondia: sobra estrada?
  it("diz onde o piloto esta na propria curva", async () => {
    renderFicha();

    await abrirPerfil();
    const arco = await screen.findByTestId("driver-detail-arc");
    expect(within(arco).getByTestId("driver-detail-arc-phase")).toHaveTextContent("Em ascensao");
    expect(arco.querySelector("[data-arc='margem']")).toHaveTextContent("Boa");
    expect(arco.querySelector("[data-arc='desenvolvimento']")).toHaveTextContent("Rapida");
    expect(arco.querySelector("[data-arc='experiencia']")).toHaveTextContent("Rodado");
  });

  // "Em ascensão" em corpo grande diz onde ele está e não quanto ainda falta. A
  // faixa das cinco fases responde as duas de uma vez.
  it("situa a fase atual na faixa das cinco", async () => {
    renderFicha();

    await abrirPerfil();
    const faixa = await screen.findByTestId("driver-detail-arc-track");
    const fases = [...faixa.querySelectorAll("[data-arc-phase]")].map((node) =>
      node.getAttribute("data-arc-phase"),
    );
    expect(fases).toEqual(["formacao", "ascensao", "auge", "plato", "crepusculo"]);
    expect(faixa.querySelectorAll("[data-arc-current]")).toHaveLength(1);
    expect(faixa.querySelector("[data-arc-current]")).toHaveAttribute("data-arc-phase", "ascensao");
  });

  // Sem `fase_chave` (payload antigo) a faixa some inteira — melhor nenhuma do
  // que uma com a fase errada acesa.
  it("some com a faixa quando o payload nao diz a fase", async () => {
    renderFicha({}, detail({ arco: { fase: "Em ascensao", tom_fase: "info", resumo: "" } }));

    await abrirPerfil();
    await screen.findByTestId("driver-detail-arc");
    expect(screen.queryByTestId("driver-detail-arc-track")).toBeNull();
  });

  // Teto 0.0 é teto NÃO DERIVADO (jogador e saves antigos), e não teto no chão.
  it("cala o teto quando ninguem mediu o teto", async () => {
    renderFicha(
      {},
      detail({
        arco: {
          idade: 34,
          fase: "No plato",
          tom_fase: "neutral",
          nivel_experiencia: "Veterano",
          nivel_desenvolvimento: "Lenta",
          resumo: "Nao cresce mais, mas ainda entrega.",
        },
      }),
    );

    await abrirPerfil();
    const arco = await screen.findByTestId("driver-detail-arc");
    expect(arco.querySelector("[data-arc='margem']")).toBeNull();
    expect(arco.querySelector("[data-arc='experiencia']")).toHaveTextContent("Veterano");
  });

  // Fama e carisma são traços da PESSOA — no Mercado eles se liam como cláusula.
  it("traz o estrelato do mercado para o perfil", async () => {
    renderFicha(
      {},
      detail({
        estrelato: {
          fama: 74,
          carisma: 61,
          nivel_fama: "Estrela",
          tom_fama: "success",
          nivel_carisma: "Magnetico",
          tom_carisma: "info",
          resumo: "O publico responde.",
        },
      }),
    );

    await abrirPerfil();
    expect(await screen.findByTestId("driver-detail-stardom")).toHaveTextContent("Estrela");

    fireEvent.click(screen.getByTestId("driver-detail-tab-mercado"));
    expect(screen.queryByTestId("driver-detail-stardom")).toBeNull();
  });

  it("nao desenha mais a fileira de trofeus", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    // Melhor temporada, maior sequência e status repetiam, em ouro, o que os
    // cards do dossiê já respondem logo abaixo — e empurravam o dossiê inteiro
    // para fora da tela.
    expect(document.querySelector("[data-highlight]")).toBeNull();
  });

  it("lista os anos de titulo com a equipe de cada um", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    const anos = await screen.findByTestId("driver-detail-title-years");
    expect(anos.querySelectorAll("[data-title-year]")).toHaveLength(2);
    expect(anos).toHaveTextContent("2023");
    expect(anos).toHaveTextContent("2021");
    expect(within(anos).getAllByTestId("driver-detail-title-logo")).toHaveLength(2);
  });

  it("abre ano, equipe e categoria de cada conquista no hover dos cards", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");

    // O card conta QUANTAS; o painel conta quando, por quem e em que escada.
    const cartao = (id) => document.querySelector(`[data-record="${id}"]`).closest("[data-has-detail]");

    fireEvent.mouseEnter(cartao("vitorias"));
    const vitorias = screen.getByTestId("dossier-detail-tooltip");
    expect(vitorias).toHaveTextContent("2021");
    expect(vitorias).toHaveTextContent("Aures Racing");
    expect(vitorias).toHaveTextContent("GT4");
    expect(vitorias).toHaveTextContent("2023");
    expect(vitorias).toHaveTextContent("Ferrari");
    // Nada de colocacao no campeonato: "P1" numa lista de vitorias diz o obvio, e
    // numa de podios ("P10") se le como a chegada — que seria mentira.
    expect(vitorias).not.toHaveTextContent(/\bP\d/);
    fireEvent.mouseLeave(cartao("vitorias"));

    fireEvent.mouseEnter(cartao("podios"));
    const podios = screen.getByTestId("dossier-detail-tooltip");
    expect(podios).toHaveTextContent("11");
    expect(podios).not.toHaveTextContent(/\bP\d/);
    fireEvent.mouseLeave(cartao("podios"));

    // Os titulos abrem a temporada inteira de cada um — o card mostra a logo e o
    // ano, o painel diz em que categoria e com que campanha.
    fireEvent.mouseEnter(cartao("titulos"));
    const titulos = screen.getByTestId("dossier-detail-tooltip");
    expect(titulos).toHaveTextContent("240 pts · 4V · 9P");
    // Aqui toda linha e P1 por definicao — repetir a colocacao nao diz nada.
    expect(titulos).not.toHaveTextContent(/\bP\d/);
    fireEvent.mouseLeave(cartao("titulos"));

    // E corridas reaproveita a lista de temporadas, que responde a mesma coisa.
    fireEvent.mouseEnter(cartao("corridas"));
    expect(screen.getByTestId("dossier-detail-tooltip")).toHaveTextContent("P11");
    fireEvent.mouseLeave(cartao("corridas"));
  });

  it("deixa as equipes campeas fluirem lado a lado", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    const anos = await screen.findByTestId("driver-detail-title-years");
    // Uma linha por equipe crescia noventa pixels num tricampeao, e como os
    // quatro cards dividem a linha do grid o vazio sobrava embaixo de Corridas,
    // Vitorias e Podios. Com wrap, tres titulos cabem numa linha so.
    expect(anos.className).toContain("flex-wrap");
    expect(anos.className).not.toContain("flex-col");
  });

  it("agrupa os titulos por equipe em vez de estourar o card", async () => {
    const dinastia = [
      ...[2025, 2024, 2023, 2022, 2021, 2020, 2019, 2018, 2017].map((ano) => ({
        ano,
        categoria: "gt3",
        equipe: "McLaren",
        equipe_cor: "#ff8000",
      })),
      { ano: 2014, categoria: "gt4", equipe: "Aures Racing", equipe_cor: "#3fb950" },
      { ano: 2011, categoria: "gt4", equipe: "Acura", equipe_cor: "#1f6feb" },
      { ano: 2008, categoria: "bmw_m2", equipe: "Ferrari", equipe_cor: "#dc0000" },
    ];
    renderFicha(
      {},
      detail({
        trajetoria: { ...detail().trajetoria, titulos: 12, titulos_detalhe: dinastia },
      }),
    );

    await screen.findByTestId("driver-detail-hero");
    const anos = await screen.findByTestId("driver-detail-title-years");
    // Doze títulos viravam doze chips e quatro fileiras; por equipe são quatro
    // linhas, e as nove taças da mesma equipe dividem uma logo só.
    expect(anos.querySelectorAll("[data-title-team]")).toHaveLength(4);
    // Nove anos seguidos viram UM intervalo: nove rotulos custavam duas linhas e
    // obrigavam a somar de cabeca para descobrir que era uma dinastia.
    const mclaren = anos.querySelector('[data-title-team="McLaren"]');
    expect(mclaren.querySelectorAll("[data-title-year]")).toHaveLength(1);
    expect(mclaren).toHaveTextContent("2017 ~ 2025");
    // Ordem: a dinastia mais recente primeiro.
    expect(anos.querySelectorAll("[data-title-team]")[0]).toHaveAttribute(
      "data-title-team",
      "McLaren",
    );
    expect(anos.querySelectorAll("[data-title-year]")[0]).toHaveTextContent("2025");
  });

  it("nao vira intervalo com dois anos seguidos", async () => {
    const bi = [2025, 2024].map((ano) => ({
      ano,
      categoria: "gt3",
      equipe: "McLaren",
      equipe_cor: "#ff8000",
    }));
    renderFicha(
      {},
      detail({ trajetoria: { ...detail().trajetoria, titulos: 2, titulos_detalhe: bi } }),
    );

    await screen.findByTestId("driver-detail-hero");
    const anos = await screen.findByTestId("driver-detail-title-years");
    // "2024 ~ 2025" e mais largo que "2024 2025" e ainda esconde que sao dois.
    expect(anos.querySelectorAll("[data-title-year]")).toHaveLength(2);
    expect(anos).not.toHaveTextContent("~");
  });

  it("resume as equipes que passam do teto do card", async () => {
    const espalhado = [2025, 2023, 2021, 2019, 2017, 2015].map((ano, index) => ({
      ano,
      categoria: "gt3",
      equipe: `Equipe ${index}`,
      equipe_cor: "#1f6feb",
    }));
    renderFicha(
      {},
      detail({
        trajetoria: { ...detail().trajetoria, titulos: 6, titulos_detalhe: espalhado },
      }),
    );

    await screen.findByTestId("driver-detail-hero");
    const anos = await screen.findByTestId("driver-detail-title-years");
    // Seis equipes distintas: quatro linhas e o resto vira uma frase, senão o
    // card volta a crescer sem teto.
    expect(anos.querySelectorAll("[data-title-team]")).toHaveLength(4);
    expect(anos).toHaveTextContent("2 títulos");
  });

  it("mostra so o total quando o piloto nao tem arquivo de titulos", async () => {
    renderFicha({}, detail({ trajetoria: { ...detail().trajetoria, titulos_detalhe: [] } }));

    await screen.findByTestId("driver-detail-hero");
    // Piloto histórico pré-gerado tem o total mas não tem temporada arquivada:
    // a lista some, o card continua dizendo que ele é campeão.
    expect(screen.queryByTestId("driver-detail-title-years")).toBeNull();
    expect(document.querySelector('[data-record="titulos"]')).toHaveTextContent("2");
  });

  it("desenha a barra de posicao com o rank por extenso", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    expect(screen.getByText("3º de 240")).toBeInTheDocument();
    expect(screen.getByText("40º de 240")).toBeInTheDocument();
  });

  it("leva o card de recorde ao ranking mundial da metrica e fecha a ficha", async () => {
    const onOpenRanking = vi.fn();
    const onClose = vi.fn();
    renderFicha({ onOpenRanking, onClose });

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
    fireEvent.click(screen.getByTestId("driver-detail-record-link-vitorias"));

    // Sem categoria: no recorte mundial o destino é o mundo inteiro.
    expect(onOpenRanking).toHaveBeenCalledWith({
      metric: "vitorias",
      driverId: "D1",
      category: null,
    });
    // A tela de destino ocupa o lugar da ficha — deixar o modal aberto por cima
    // esconderia o ranking que o clique pediu.
    expect(onClose).toHaveBeenCalled();
  });

  it("leva a categoria junto quando o recorte e o grid atual", async () => {
    const onOpenRanking = vi.fn();
    renderFicha(
      { onOpenRanking },
      detail({ rankings_grid: { corridas: 4, vitorias: 2, podios: 3, titulos: 1, total: 24 } }),
    );

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
    fireEvent.click(screen.getByTestId("driver-detail-rank-scope-grid"));
    fireEvent.click(screen.getByTestId("driver-detail-record-link-podios"));

    // A tela de destino tem que responder a MESMA pergunta que a origem: quem
    // leu "3º de 24 no grid" não pode cair na lista dos 610 do mundo.
    expect(onOpenRanking).toHaveBeenCalledWith({
      metric: "podios",
      driverId: "D1",
      category: "gt3",
    });
  });

  it("nao torna o card de recorde clicavel sem destino", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    expect(screen.queryByTestId("driver-detail-record-link-vitorias")).toBeNull();
  });

  it("alterna os numeros de carreira entre o mundo e o grid atual", async () => {
    renderFicha(
      {},
      detail({ rankings_grid: { corridas: 4, vitorias: 2, podios: 3, titulos: 1, total: 24 } }),
    );

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    // ACIMA dos cards: o denominador precisa ser lido antes do "3º de 240", não
    // depois. `DOCUMENT_POSITION_FOLLOWING` = o card vem depois do seletor.
    const seletor = screen.getByTestId("driver-detail-rank-scope-toggle");
    const primeiroCard = document.querySelector('[data-record="corridas"]');
    expect(seletor.compareDocumentPosition(primeiroCard) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    // O mundo é o padrão.
    expect(screen.getByText("3º de 240")).toBeInTheDocument();
    expect(screen.getByTestId("driver-detail-rank-scope-mundo")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    // Com o seletor na tela a frase de recorte sai: ela repetiria o botão aceso
    // e o "de 240" que já está em cada card.
    expect(screen.queryByTestId("driver-detail-rank-scope")).toBeNull();

    fireEvent.click(screen.getByTestId("driver-detail-rank-scope-grid"));

    expect(screen.getByText("2º de 24")).toBeInTheDocument();
    expect(screen.queryByText("3º de 240")).toBeNull();
    expect(screen.getByTestId("driver-detail-rank-scope-grid")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("esconde o seletor de escopo para quem nao tem grid", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    // Sem `rankings_grid` no payload não há segundo recorte — e um botão morto
    // seria pior que nenhum.
    expect(screen.queryByTestId("driver-detail-rank-scope-toggle")).toBeNull();
    expect(screen.getByTestId("driver-detail-rank-scope")).toHaveTextContent("do mundo");
  });

  it("esconde a barra quando o payload nao traz o total", async () => {
    renderFicha({}, detail({ rankings_carreira: { vitorias: 3 } }));

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    expect(screen.queryByText(/de 240/)).not.toBeInTheDocument();
    // Preso ao card, e nao ao texto solto: a curva de campeonato escreve a
    // posicao final em cada ponto, e "3º" passou a existir tambem la.
    expect(document.querySelector('[data-record="vitorias"]')).toHaveTextContent("3º");
  });

  it("mostra a queda e a confiabilidade no dossie", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const dossie = screen.getByTestId("driver-detail-career-dossier");
    const queda = dossie.querySelector('[data-group="queda"]');
    // O jejum carrega o periodo: "24" e um numero, "24 (2018-2021)" e uma queda.
    expect(queda).toHaveTextContent("24 (2018–2021)");
    // O jejum de PODIOS e mais fundo que o de vitorias, e e a marca que serve
    // para quem nunca venceu.
    expect(queda).toHaveTextContent("31 (2018–2022)");
    // A pior temporada leva o resultado colado: sem ele, "2019, GT4" seria
    // indistinguivel de uma linha qualquer da escada de categorias.
    expect(queda).toHaveTextContent("2019, GT4 · P11");

    const confiabilidade = dossie.querySelector('[data-group="confiabilidade"]');
    expect(confiabilidade).toHaveTextContent("7.5%");
    expect(confiabilidade).toHaveTextContent("18");
  });

  it("agrupa o dossie em tres linhas tematicas de tres colunas", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grupos = [...screen.getByTestId("driver-detail-career-dossier").querySelectorAll("[data-group]")]
      .map((node) => node.dataset.group);
    expect(grupos).toEqual([
      // quem ele é
      "presenca",
      "mobilidade",
      "primeiros",
      // o que entrega
      "auge",
      "sabado",
      "duelos",
      // o que custa
      "queda",
      "confiabilidade",
      "lesoes",
    ]);
    // Em tres colunas, cada card da terceira linha cai embaixo do seu par da
    // segunda: auge sobre queda, sabado sobre confiabilidade.
    expect(grupos.indexOf("queda")).toBe(grupos.indexOf("auge") + 3);
    expect(grupos.indexOf("confiabilidade")).toBe(grupos.indexOf("sabado") + 3);
  });

  it("auge e queda sao espelhos, linha por linha", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const dossie = screen.getByTestId("driver-detail-career-dossier");
    const auge = dossie.querySelector('[data-group="auge"]');
    const queda = dossie.querySelector('[data-group="queda"]');
    // Melhor e pior temporada levam o resultado colado. "Melhor campeonato P1"
    // era uma linha inteira repetindo a colocacao da linha de cima.
    expect(auge).toHaveTextContent("2023, GT3 · P1");
    expect(queda).toHaveTextContent("2019, GT4 · P11");
    expect(auge).not.toHaveTextContent("Melhor campeonato");
    // Sequencia e jejum, de vitorias e de podios, com o periodo nos dois lados.
    expect(auge).toHaveTextContent("3 (2023)");
    expect(auge).toHaveTextContent("9 (2022–2023)");
    expect(queda).toHaveTextContent("24 (2018–2021)");
    expect(queda).toHaveTextContent("31 (2018–2022)");
    expect(auge.querySelectorAll("[class*=border-b]")).toHaveLength(
      queda.querySelectorAll("[class*=border-b]").length,
    );
  });

  it("abre o detalhe das equipes ao passar o mouse na linha", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const linha = screen.getByText("Equipes defendidas").closest("[data-has-detail]");
    expect(screen.queryByTestId("dossier-detail-tooltip")).not.toBeInTheDocument();

    fireEvent.mouseEnter(linha);
    const painel = screen.getByTestId("dossier-detail-tooltip");
    expect(painel).toHaveTextContent("Aures Racing");
    expect(painel).toHaveTextContent("Ferrari");
    expect(painel).toHaveTextContent("2019-2021");

    // Sem a bolinha fechada o painel é volátil: sai o mouse, some na hora.
    fireEvent.mouseLeave(linha);
    expect(screen.queryByTestId("dossier-detail-tooltip")).not.toBeInTheDocument();
  });

  it("a bolinha prende o painel, que so solta 5s depois de o mouse ir embora", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      renderFicha();

      await screen.findByTestId("driver-detail-hero");
      fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
      const linha = screen.getByText("Equipes defendidas").closest("[data-has-detail]");

      fireEvent.mouseEnter(linha);
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "false");

      // Segurando o mouse na linha a bolinha fecha o circulo e o painel prende.
      act(() => vi.advanceTimersByTime(700));
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "true");

      // Preso, ele sobrevive a saida do mouse — que e o ponto: a lista rolavel
      // so e alcancavel assim.
      fireEvent.mouseLeave(linha);
      act(() => vi.advanceTimersByTime(4000));
      expect(screen.getByTestId("dossier-detail-tooltip")).toBeInTheDocument();

      // Voltar para o proprio painel zera a contagem dos 5s.
      fireEvent.mouseEnter(screen.getByTestId("dossier-detail-tooltip"));
      act(() => vi.advanceTimersByTime(4000));
      expect(screen.getByTestId("dossier-detail-tooltip")).toBeInTheDocument();

      fireEvent.mouseLeave(screen.getByTestId("dossier-detail-tooltip"));
      act(() => vi.advanceTimersByTime(5000));
      expect(screen.queryByTestId("dossier-detail-tooltip")).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("o botao do meio prende na hora e solta na hora", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      renderFicha();

      await screen.findByTestId("driver-detail-hero");
      fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
      const linha = screen.getByText("Equipes defendidas").closest("[data-has-detail]");

      fireEvent.mouseEnter(linha);
      fireEvent.mouseDown(linha, { button: 1 });
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "true");

      // E solta sem esperar os 5s, para tirar o painel da frente.
      fireEvent.mouseDown(linha, { button: 1 });
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "false");

      // Solto, ele nao volta a prender sozinho com o mouse parado onde esta.
      act(() => vi.advanceTimersByTime(2000));
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "false");

      fireEvent.mouseLeave(linha);
      expect(screen.queryByTestId("dossier-detail-tooltip")).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("o painel que cabe na tela nao oferece prender", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    fingeQueRola(false);
    try {
      renderFicha();

      await screen.findByTestId("driver-detail-hero");
      fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
      const linha = screen.getByText("Equipes defendidas").closest("[data-has-detail]");

      fireEvent.mouseEnter(linha);
      const painel = screen.getByTestId("dossier-detail-tooltip");
      // Sem barra de rolagem nao ha o que alcancar: nem bolinha, nem legenda.
      expect(within(painel).queryByTestId("dossier-detail-bolinha")).not.toBeInTheDocument();
      expect(painel).not.toHaveTextContent("prender");

      act(() => vi.advanceTimersByTime(700));
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "false");

      fireEvent.mouseLeave(linha);
      expect(screen.queryByTestId("dossier-detail-tooltip")).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("o botao do meio prende ate o painel que nao rola", async () => {
    fingeQueRola(false);
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
    const linha = screen.getByText("Equipes defendidas").closest("[data-has-detail]");

    fireEvent.mouseEnter(linha);
    // A bolinha nao aparece — mas o gesto deliberado vale em qualquer painel.
    fireEvent.mouseDown(linha, { button: 1 });
    const painel = screen.getByTestId("dossier-detail-tooltip");
    expect(painel).toHaveAttribute("data-preso", "true");
    // E preso ele ganha o rodape com a saida, senao ficaria sem porta.
    expect(within(painel).getByTestId("dossier-detail-soltar")).toBeInTheDocument();

    fireEvent.mouseLeave(linha);
    expect(screen.getByTestId("dossier-detail-tooltip")).toBeInTheDocument();
  });

  it("a bolinha so corre com o cursor parado", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      renderFicha();

      await screen.findByTestId("driver-detail-hero");
      fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
      const linha = screen.getByText("Equipes defendidas").closest("[data-has-detail]");

      fireEvent.mouseEnter(linha);
      act(() => vi.advanceTimersByTime(500));

      // Andar sobre a linha reinicia o anel: e o que impede atravessar o dossie
      // devagar e prender cada linha do caminho.
      fireEvent.mouseMove(linha, { clientX: 10, clientY: 10 });
      fireEvent.mouseMove(linha, { clientX: 40, clientY: 10 });
      act(() => vi.advanceTimersByTime(500));
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "false");

      act(() => vi.advanceTimersByTime(300));
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "true");
    } finally {
      vi.useRealTimers();
    }
  });

  it("so um painel fica preso por vez", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      renderFicha();

      await screen.findByTestId("driver-detail-hero");
      fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
      const equipes = screen.getByText("Equipes defendidas").closest("[data-has-detail]");
      const promocoes = screen.getByText("Promoções").closest("[data-has-detail]");

      fireEvent.mouseEnter(equipes);
      act(() => vi.advanceTimersByTime(700));
      fireEvent.mouseLeave(equipes);

      // O painel preso cobre as linhas vizinhas: dois presos ao mesmo tempo e
      // uma pilha, nao uma leitura.
      fireEvent.mouseEnter(promocoes);
      act(() => vi.advanceTimersByTime(700));
      const abertos = screen.getAllByTestId("dossier-detail-tooltip");
      expect(abertos).toHaveLength(1);
      expect(abertos[0]).toHaveTextContent("GT4 → GT3");
    } finally {
      vi.useRealTimers();
    }
  });

  it("o x e o Esc soltam o painel preso, e o Esc nao fecha a ficha por baixo", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const onClose = vi.fn();
    try {
      renderFicha({ onClose });

      await screen.findByTestId("driver-detail-hero");
      fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
      const linha = screen.getByText("Equipes defendidas").closest("[data-has-detail]");

      fireEvent.mouseEnter(linha);
      act(() => vi.advanceTimersByTime(700));
      fireEvent.click(screen.getByTestId("dossier-detail-soltar"));
      expect(screen.queryByTestId("dossier-detail-tooltip")).not.toBeInTheDocument();

      fireEvent.mouseEnter(linha);
      act(() => vi.advanceTimersByTime(700));
      fireEvent.keyDown(window, { key: "Escape" });
      expect(screen.queryByTestId("dossier-detail-tooltip")).not.toBeInTheDocument();
      expect(onClose).not.toHaveBeenCalled();

      // Sem painel preso o Esc volta a ser da ficha.
      fireEvent.keyDown(window, { key: "Escape" });
      expect(onClose).toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it("mostra a logo da equipe e o ano, sem o dia", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Primeira vitória").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    expect(within(painel).getByTestId("dossier-detail-logo")).toBeInTheDocument();
    // O dia exato é precisão que ninguém pediu, e rouba a linha da rodada e da
    // pista, que dizem muito mais.
    expect(painel).toHaveTextContent("2019 · Rodada 6");
    expect(painel).not.toHaveTextContent("mai");
  });

  it("diz de qual categoria para qual na promocao", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Promoções").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    // O backend manda as CHAVES; o rotulo e montado aqui.
    expect(painel).toHaveTextContent("GT4 → GT3");
    // Promocao nao tem equipe por natureza: aqui o X seria um buraco em toda
    // linha anunciando uma ausencia que nao significa nada.
    expect(within(painel).queryByTestId("dossier-detail-sem-equipe")).not.toBeInTheDocument();
  });

  it("mostra a data real da primeira vitoria, e nao so o numero da corrida", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Primeira vitória").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    expect(painel).toHaveTextContent("Rodada 6");
    expect(painel).toHaveTextContent("Circuito de Navarra");
    expect(painel).toHaveTextContent("Aures Racing");
    // A data ISO vira data local, com o dia — o ano sozinho ja estava no card.
    expect(painel).toHaveTextContent("2019");
  });

  it("mostra quem sao os companheiros, com equipe de hoje, idade e nacionalidade", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Companheiros enfrentados").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    expect(painel).toHaveTextContent("Igor Petrov");
    expect(painel).toHaveTextContent("Vector Racing");
    expect(painel).toHaveTextContent("29 anos");

    // A bandeira vai como ARTE: o Windows nao tem glifo de bandeira nacional, e
    // o emoji do backend caia para as duas letras cruas ("RU") na ficha.
    expect(within(painel).getByAltText("🇷🇺 Russo")).toBeInTheDocument();
    expect(painel).toHaveTextContent("Russo");
    expect(painel).not.toHaveTextContent("🇷🇺");

    // Companheiro sem equipe hoje ocupa o mesmo slot do logo com um X, senao a
    // linha ficava desalinhada do resto da lista.
    expect(within(painel).getByTestId("dossier-detail-sem-equipe")).toBeInTheDocument();
  });

  it("mostra a lesao com a data e a equipe daquele dia", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Leves").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    expect(painel).toHaveTextContent("Dor no pescoço");
    expect(painel).toHaveTextContent("Rodada 4");
    expect(painel).toHaveTextContent("Ferrari");
    // Cada gravidade abre SO as suas: "Leves 1" listando a moderada junto era a
    // linha mentindo sobre o proprio numero.
    expect(painel).not.toHaveTextContent("Fratura no punho");
    fireEvent.mouseLeave(screen.getByText("Leves").closest("[data-has-detail]"));

    fireEvent.mouseEnter(screen.getByText("Moderadas").closest("[data-has-detail]"));
    const moderadas = screen.getByTestId("dossier-detail-tooltip");
    expect(moderadas).toHaveTextContent("Fratura no punho");
    expect(moderadas).not.toHaveTextContent("Dor no pescoço");
  });

  it("clicar na equipe do painel abre o Atlas e fecha a ficha", async () => {
    const onOpenTeam = vi.fn();
    const onClose = vi.fn();
    renderFicha({ onOpenTeam, onClose });

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Equipes defendidas").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    // Duas equipes na lista, mas so a que ainda existe no mundo vira porta — e
    // sao DOIS alvos por equipe: o logo e o nome.
    const portas = within(painel).getAllByTestId("dossier-detail-abrir-equipe");
    expect(portas).toHaveLength(2);

    fireEvent.click(portas[0]);
    expect(onOpenTeam).toHaveBeenCalledWith({
      id: "T1",
      nome: "Aures Racing",
      cor_primaria: "#3fb950",
    });
    // As duas telas ocupam o mesmo espaco: a ficha sai da frente do Atlas.
    expect(onClose).toHaveBeenCalled();
  });

  it("o tempo de carreira nomeia as duas pontas, e nao lista as equipes do meio", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Tempo de carreira").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    // Sem os rotulos, duas linhas passavam por uma lista de equipes cortada.
    expect(painel).toHaveTextContent("Equipe de estreia");
    expect(painel).toHaveTextContent("Equipe atual");
  });

  it("o ano parado diz de qual equipe ele saiu e quando voltou", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Anos desempregado").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    expect(painel).toHaveTextContent("Aures Racing");
    expect(painel).toHaveTextContent("→ 2022");
  });

  it("a temporada sem podio mostra a melhor chegada do ano", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Temporadas sem pódio").closest("[data-has-detail]"));
    // Sem podio, a melhor chegada e o unico resultado que da tamanho ao ano.
    expect(screen.getByTestId("dossier-detail-tooltip")).toHaveTextContent("melhor: P5");
  });

  it("a taxa de abandono e o rival mais duro abrem o proprio detalhe", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Taxa de abandono").closest("[data-has-detail]"));
    // Numerador e denominador a vista: "4,3%" sozinho nao diz se sao 3 em 70.
    expect(screen.getByTestId("dossier-detail-tooltip")).toHaveTextContent("3/70 · 4.3%");
    fireEvent.mouseLeave(screen.getByText("Taxa de abandono").closest("[data-has-detail]"));

    fireEvent.mouseEnter(screen.getByText("Mais duro").closest("[data-has-detail]"));
    expect(screen.getByTestId("dossier-detail-tooltip")).toHaveTextContent("388 x 412");
  });

  it("o toggle de recordes liga a posicao no grid e no mundo", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    // Desligado por padrão: a posição é a pergunta SEGUINTE, e ligada sempre
    // dobraria a altura de nove cards.
    expect(screen.queryAllByTestId("dossier-rank")).toHaveLength(0);

    fireEvent.click(screen.getByTestId("driver-detail-records-toggle"));
    const marcas = screen.getAllByTestId("dossier-rank");
    // Poles tem as duas posições; taxa de abandono não tem grid (aposentado,
    // sem pelotão de domingo) e mostra só a do mundo.
    expect(marcas[0]).toHaveTextContent("2º");
    expect(marcas[0]).toHaveTextContent("41º");
    expect(marcas.some((marca) => marca.textContent.trim() === "7º")).toBe(true);

    // Os denominadores vão UMA vez, na legenda — repetidos em vinte linhas eles
    // quebravam cada uma em duas e dobravam a altura dos nove cards.
    const escopo = screen.getByTestId("driver-detail-records-scope");
    expect(escopo).toHaveTextContent("Grid de 24");
    expect(escopo).toHaveTextContent("mundo de 610");
    // O número exato de cada linha, que varia, fica no title do próprio ordinal.
    expect(within(marcas[0]).getByText("2º")).toHaveAttribute("data-tooltip", "2º de 24 no grid");

    fireEvent.click(screen.getByTestId("driver-detail-records-toggle"));
    expect(screen.queryAllByTestId("dossier-rank")).toHaveLength(0);
    expect(screen.queryByTestId("driver-detail-records-scope")).toBeNull();
  });

  it("nao esvazia a ficha ao trocar de piloto", async () => {
    const primeiro = detail();
    const tela = renderFicha();
    await screen.findByTestId("driver-detail-hero");
    expect(screen.getByText(primeiro.nome)).toBeInTheDocument();

    // O proximo piloto demora a responder: e nessa janela que o painel de carga
    // aparecia, piscando entre uma ficha e outra.
    let entregar;
    invoke.mockImplementation((command) => {
      if (command === "get_driver_world_rank") return Promise.resolve(null);
      return new Promise((resolve) => {
        entregar = resolve;
      });
    });

    await act(async () => {
      tela.rerender(
        <DriverDetailModalV2
          driverId="D2"
          driverIds={["D0", "D1", "D2"]}
          onSelectDriver={vi.fn()}
          onClose={vi.fn()}
        />,
      );
    });

    // A ficha anterior segue inteira no lugar — sem painel de carga no meio.
    expect(screen.queryByTestId("driver-detail-loading")).toBeNull();
    expect(screen.getByText(primeiro.nome)).toBeInTheDocument();

    await act(async () => {
      entregar(detail({ id: "D2", nome: "Outro Piloto" }));
    });
    expect(await screen.findByText("Outro Piloto")).toBeInTheDocument();
    expect(screen.queryByText(primeiro.nome)).toBeNull();
  });

  it("so busca os recordes quando o toggle liga", async () => {
    // O payload da ficha nao traz mais o mapa: monta-lo exige varrer o mundo
    // inteiro (503ms num save de 27 mil resultados) e era cobrado de toda
    // abertura e de toda troca de piloto para alimentar um botao desligado.
    const payload = detail();
    delete payload.trajetoria.historico.recordes;
    const ranks = {
      poles: { grid: 2, grid_total: 24, mundo: 41, mundo_total: 610 },
    };
    invoke.mockImplementation((command) => {
      if (command === "get_driver_world_rank") return Promise.resolve(null);
      if (command === "get_driver_dossier_ranks") return Promise.resolve(ranks);
      return Promise.resolve(payload);
    });

    render(
      <DriverDetailModalV2 driverId="D1" driverIds={["D1"]} onSelectDriver={vi.fn()} onClose={vi.fn()} />,
    );
    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const buscou = () => invoke.mock.calls.some(([cmd]) => cmd === "get_driver_dossier_ranks");
    expect(buscou()).toBe(false);
    // O botao aparece mesmo sem mapa nenhum em maos — quem monta o mapa e o clique.
    fireEvent.click(screen.getByTestId("driver-detail-records-toggle"));
    expect(buscou()).toBe(true);

    const marcas = await screen.findAllByTestId("dossier-rank");
    expect(marcas[0]).toHaveTextContent("2º");
    expect(marcas[0]).toHaveTextContent("41º");
    expect(screen.getByTestId("driver-detail-records-scope")).toHaveTextContent("Grid de 24");

    // Desligar e ligar de novo nao repete a busca: o mapa ja esta em maos.
    const antes = invoke.mock.calls.filter(([cmd]) => cmd === "get_driver_dossier_ranks").length;
    fireEvent.click(screen.getByTestId("driver-detail-records-toggle"));
    fireEvent.click(screen.getByTestId("driver-detail-records-toggle"));
    expect(invoke.mock.calls.filter(([cmd]) => cmd === "get_driver_dossier_ranks")).toHaveLength(antes);
  });

  it("nao abre painel na linha que o backend nao detalhou", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    // Ele nunca foi rebaixado, entao o backend nem manda a chave — e sem linhas
    // o wrapper nao vira alvo de hover.
    expect(screen.getByText("Rebaixamentos").closest("[data-has-detail]")).toBeNull();
  });

  it("mostra o sabado, que a ficha inteira ignorava, com a media do mundo", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const sabado = screen
      .getByTestId("driver-detail-career-dossier")
      .querySelector('[data-group="sabado"]');
    expect(sabado).toHaveTextContent("18");
    // Largar na frente e converter sao habilidades diferentes.
    expect(sabado).toHaveTextContent("11");
    expect(sabado).toHaveTextContent("P4.2");
    // "P4.2" sozinho nao diz se e frente ou fundo; a media do mundo diz.
    expect(sabado).toHaveTextContent("média P10.5");
  });

  it("mostra o confronto com o companheiro de equipe", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const duelos = screen
      .getByTestId("driver-detail-career-dossier")
      .querySelector('[data-group="duelos"]');
    expect(duelos).toHaveTextContent("7 de 9");
    expect(duelos).toHaveTextContent("Igor Petrov 1-2");
  });

  it("nao inventa confronto para quem nunca dividiu box", async () => {
    const base = detail();
    renderFicha(
      {},
      detail({
        trajetoria: {
          ...base.trajetoria,
          historico: {
            ...base.trajetoria.historico,
            duelos: { companheiros: 0, temporadas: 0, temporadas_vencidas: 0, rival_mais_duro: null },
          },
        },
      }),
    );

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const duelos = screen
      .getByTestId("driver-detail-career-dossier")
      .querySelector('[data-group="duelos"]');
    // "0 de 0" leria como derrota; sem companheiro a comparacao nao existe.
    expect(duelos).not.toHaveTextContent("de 0");
    expect(duelos).toHaveTextContent("-");
  });

  it("lista os primeiros marcos do mais alto para o mais baixo", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const marcos = screen
      .getByTestId("driver-detail-career-dossier")
      .querySelector('[data-group="primeiros"]');
    const linhas = [...marcos.querySelectorAll("span")].map((n) => n.textContent);
    const ordem = ["Primeiro título", "Primeira vitória", "Primeiro pódio", "Primeiro DNF"];
    const posicoes = ordem.map((rotulo) => linhas.indexOf(rotulo));
    expect(posicoes.every((p) => p >= 0)).toBe(true);
    expect([...posicoes].sort((a, b) => a - b)).toEqual(posicoes);
    // O titulo e o unico marco que nao cabe num numero de corrida.
    expect(marcos).toHaveTextContent("2021, GT4");
  });

  it("diz Nunca, e nao um traco, para quem jamais foi campeao", async () => {
    const base = detail();
    renderFicha(
      {},
      detail({
        trajetoria: {
          ...base.trajetoria,
          historico: {
            ...base.trajetoria.historico,
            primeiros_marcos: { ...base.trajetoria.historico.primeiros_marcos, primeiro_titulo: null },
          },
        },
      }),
    );

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const marcos = screen
      .getByTestId("driver-detail-career-dossier")
      .querySelector('[data-group="primeiros"]');
    // Os vizinhos dizem "Nunca"; um "-" solto no meio da coluna sairia como
    // dado faltando, e nao como marca que nao aconteceu.
    expect(marcos).toHaveTextContent("Nunca");
    expect(marcos).not.toHaveTextContent("2021, GT4");
  });

  it("nao inventa taxa de abandono para quem nunca largou", async () => {
    renderFicha(
      {},
      detail({
        trajetoria: {
          ...detail().trajetoria,
          historico: {
            ...detail().trajetoria.historico,
            confiabilidade: { abandonos: 0, corridas: 0, taxa_abandono: null, maior_sequencia_chegadas: 0 },
          },
        },
      }),
    );

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const confiabilidade = screen
      .getByTestId("driver-detail-career-dossier")
      .querySelector('[data-group="confiabilidade"]');
    // "0%" se leria como "nunca abandona" — um estreante sem largada nenhuma nao
    // e o piloto mais confiavel do grid.
    expect(confiabilidade).toHaveTextContent("-");
    expect(confiabilidade).not.toHaveTextContent("0%");
  });

  // A curva de campeonato tomou o lugar da escada de categorias: ela desenha os
  // mesmos anos e as mesmas categorias — como coluna de fundo — e ainda responde
  // ONDE ele terminou cada um deles.
  it("pinta a escada como coluna de fundo da curva de campeonato", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    const colunas = [...grafico.querySelectorAll('[data-coluna="categoria"]')].map(
      (node) => node.dataset.categoria,
    );
    // Duas colunas, e a troca de escada cai exatamente onde a carreira mudou.
    expect(colunas).toEqual(["gt4", "gt3"]);
    // A escada saiu de cena: a mesma faixa de anos duas vezes na tela era a
    // segunda dizendo menos que a primeira.
    expect(screen.queryByTestId("driver-detail-category-ladder")).toBeNull();
  });

  // O que o gráfico mede não é a altura da linha branca — é a DISTANCIA dela
  // ate a expectativa. "P5" sozinho nao diz se ele tirou leite de pedra ou
  // desperdicou o melhor carro do grid.
  it("desenha o resultado contra o esperado, com a faixa entre os dois", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    expect(grafico.querySelector('[data-serie="posicao"]')).not.toBeNull();
    expect(grafico.querySelector('[data-serie="esperado"]')).not.toBeNull();
    // A faixa e o objeto que carrega a leitura — sem ela sao duas linhas soltas.
    expect(grafico.querySelector('[data-faixa="diferenca"]')).not.toBeNull();
    // Os dois titulos ganham marca propria — P1 no eixo e uma altura, nao um fato.
    expect(grafico.querySelectorAll("[data-titulo]")).toHaveLength(2);
  });

  // O fundo do grid saiu: gastava metade do quadro numa sombra para responder
  // "de quantos carros era este campeonato", que cabe no balao. E a faixa
  // amarela do podio saiu junto — era mais uma marca disputando o topo do eixo.
  it("nao desenha mais o fundo do grid nem a faixa do podio", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    expect(grafico.querySelector('[data-serie="chao"]')).toBeNull();
    expect(grafico.querySelector('[data-area="fora-do-grid"]')).toBeNull();
    expect(grafico.querySelector('[data-faixa="podio"]')).toBeNull();
  });

  // A bolinha marca o COMECO da temporada, e nao o meio nem o fim dela.
  //
  // No fim, ela caia exatamente sobre a borda esquerda da coluna SEGUINTE — e ali
  // lia como sendo do ano seguinte: a posicao de 2018 aparecia em cima da coluna
  // da categoria de 2019, encostada no rotulo errado. No comeco, tudo que
  // descreve o ano fica a direita do ponto.
  //
  // Sao duas reguas: a da MOLDURA (centro da coluna: rotulo, fita, alvo de
  // hover) e a da SERIE (comeco do ano). Elas nunca coincidem.
  it("poe a bolinha no comeco do ano, inclusive na temporada em curso", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    // Seis temporadas no eixo: cada coluna vale um sexto do plot.
    const passo = 606 / 6;
    const marcadores = [...grafico.querySelectorAll("circle")]
      .filter((no) => no.getAttribute("r") !== "8")
      .map((no) => Number(no.getAttribute("cx")));

    // A estreia abre na propria borda esquerda do plot.
    expect(Math.min(...marcadores)).toBeCloseTo(62, 5);
    // A temporada em curso nao e excecao: comeco da coluna, como as outras.
    const parcial = grafico.querySelector("[data-parcial]");
    expect(Number(parcial.getAttribute("cx"))).toBeCloseTo(62 + 5 * passo, 5);
    // O rotulo do ano segue no centro da coluna: ele nomeia a FAIXA, nao o ponto.
    const anos = [...grafico.querySelectorAll("text")].filter((no) => no.textContent === "2024");
    expect(Number(anos[0].getAttribute("x"))).toBeCloseTo(62 + 5.5 * passo, 5);
  });

  // ...e a coluna do ultimo ano nao fica vazia por causa disso. O traco que sai
  // de um ponto atravessa a coluna do proprio ano ate o ponto do ano seguinte, e
  // a ultima temporada nao tem ano seguinte: sem o fecho, o marcador ficaria
  // sozinho na borda esquerda de uma coluna vazia.
  it("fecha as duas linhas na borda do plot", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    const passo = 606 / 6;

    // A posicao fecha no trecho FANTASMA — a temporada em curso e a ultima do
    // eixo, e quem continua dali e ela.
    const series = [
      grafico.querySelector('[data-serie="posicao"][data-futuro]'),
      grafico.querySelector('[data-serie="esperado"]'),
    ];
    series.forEach((serie) => {
      const vertices = serie
        .getAttribute("points")
        .split(" ")
        .map((par) => par.split(",").map(Number));
      expect(vertices.at(-1)[0]).toBeCloseTo(668, 5);
      expect(vertices.at(-2)[0]).toBeCloseTo(62 + 5 * passo, 5);
      // Reta: nao ha medida seguinte para inclinar coisa alguma.
      expect(vertices.at(-1)[1]).toBeCloseTo(vertices.at(-2)[1], 5);
    });

    // A faixa entre as duas fecha junto — senao a distancia que o grafico existe
    // para mostrar morreria na borda esquerda da ultima coluna.
    const faixa = grafico.querySelector('[data-faixa="diferenca"]').getAttribute("d");
    expect(faixa.startsWith("M62,")).toBe(true);
    expect(faixa.split("L668,")).toHaveLength(3);
  });

  // A coluna da estreia continua cheia, mas por outra peca: quando o ano seguinte
  // e fora do grid, quem atravessa a coluna do primeiro ano e o trecho CHEIO da
  // ponte, que so vira tracejado onde a hachura comeca.
  it("cobre a coluna da estreia com o trecho cheio da ponte", async () => {
    const base = detail();
    const ilhada = curvaCampeonato().map((ponto, indice) =>
      indice === 1
        ? {
            ...ponto,
            categoria: "",
            equipe_nome: null,
            equipe_cor: null,
            posicao: null,
            esperado: null,
            corridas: 0,
          }
        : ponto,
    );
    renderFicha({}, detail({ trajetoria: { ...base.trajetoria, curva_campeonato: ilhada } }));

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    const passo = 606 / 6;
    const cheio = [...grafico.querySelectorAll('[data-ponte="contratado"]')].find(
      (no) => no.getAttribute("stroke") === "#f0f6fc",
    );
    expect(Number(cheio.getAttribute("x1"))).toBeCloseTo(62, 5);
    expect(Number(cheio.getAttribute("x2"))).toBeCloseTo(62 + passo, 5);

    // E dali em diante e tracejado, exatamente sobre a hachura.
    const vao = [...grafico.querySelectorAll('[data-ponte="sem-contrato"]')].find(
      (no) => no.getAttribute("stroke") === "#f0f6fc",
    );
    expect(Number(vao.getAttribute("x1"))).toBeCloseTo(62 + passo, 5);
    expect(Number(vao.getAttribute("x2"))).toBeCloseTo(62 + 2 * passo, 5);
  });

  // O espelho do caso acima: voltou ao grid este ano depois de um fora. O ponto
  // fica SOLTO, e sem o fecho na borda do plot a coluna do ano em curso nasceria
  // vazia, com um marcador na borda esquerda e nada mais.
  it("fecha a coluna do ultimo ano mesmo quando ele fica ilhado", async () => {
    const base = detail();
    const ilhado = curvaCampeonato().map((ponto, indice) =>
      indice === 4
        ? {
            ...ponto,
            categoria: "",
            equipe_nome: null,
            equipe_cor: null,
            posicao: null,
            esperado: null,
            corridas: 0,
          }
        : ponto,
    );
    renderFicha({}, detail({ trajetoria: { ...base.trajetoria, curva_campeonato: ilhado } }));

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    const fecho = [...grafico.querySelectorAll('[data-serie="posicao"]')]
      .map((no) => no.getAttribute("points").split(" "))
      .find((vertices) => vertices.at(-1).startsWith("668,"));
    expect(fecho).toHaveLength(2);
    expect(fecho[0].split(",")[0]).toBe(String(62 + 5 * (606 / 6)));
  });

  // Num grid de MX-5 identicos "o que o carro dava" continua tendo um numero, mas
  // deixa de medir MAQUINA. O tracejado e a ressalva desenhada: a referencia
  // existe, e vale menos ali.
  it("traceja a linha do carro nos anos de monomarca", async () => {
    const base = detail();
    const daRookie = curvaCampeonato().map((ponto, indice) =>
      indice < 2
        ? { ...ponto, categoria: "toyota_rookie", monomarca: true }
        : { ...ponto, monomarca: false },
    );
    renderFicha({}, detail({ trajetoria: { ...base.trajetoria, curva_campeonato: daRookie } }));

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    const passo = 606 / 6;
    const trechos = [...grafico.querySelectorAll('[data-serie="esperado"]')].map((no) => {
      const xs = no.getAttribute("points").split(" ").map((par) => Number(par.split(",")[0]));
      return {
        traco: no.getAttribute("stroke-dasharray"),
        cor: no.getAttribute("stroke"),
        mono: no.hasAttribute("data-monomarca"),
        de: (xs[0] - 62) / passo,
        ate: (xs.at(-1) - 62) / passo,
      };
    });

    // Dois pedacos, partidos so onde o TRACO muda — nao onde a equipe muda. Cada
    // pedaco avanca um ponto para alcancar onde seus segmentos chegam, e por isso
    // um termina onde o proximo comeca.
    expect(trechos).toEqual([
      // A emenda pertence ao ano de SAIDA: o segmento que sai da Rookie de 2020
      // ocupa a coluna de 2020, entao ele ainda e tracejado — a ressalva vale ate
      // o fim do ano em que o carro nao importava.
      { traco: "4 4", cor: "#000000", mono: true, de: 0, ate: 2 },
      // ...e o ultimo pedaco fecha na borda do plot, um passo depois do ultimo
      // ponto.
      { traco: null, cor: "#000000", mono: false, de: 2, ate: 6 },
    ]);

    // E o tracejado ganha chave, senao e uma marca sem vocabulario.
    expect(grafico.querySelector('[data-chave="monomarca"]')).toHaveTextContent("Monomarca");
  });

  // A referencia e a linha mais ESCURA do quadro, contra a branca que e a mais
  // clara. Uma cor por equipe punha oito cores saturadas atravessando um grafico
  // de duas linhas, e a que devia recuar virava a mais forte dele.
  it("desenha a expectativa em preto e o resultado em branco", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    const cores = [...grafico.querySelectorAll('[data-serie="esperado"]')].map((no) =>
      no.getAttribute("stroke"),
    );
    // Uma cor so para a serie inteira, apesar das tres equipes da fixture.
    expect(new Set(cores)).toEqual(new Set(["#000000"]));
    expect(grafico.querySelector('[data-serie="posicao"]')).toHaveAttribute("stroke", "#f0f6fc");

    // O marcador acompanha, com contorno mais CLARO que o miolo: um disco preto
    // com anel da cor do cartao e um buraco, e some dentro da propria linha.
    const marcador = grafico.querySelector('[data-marcador="esperado"]');
    expect(marcador).toHaveAttribute("fill", "#000000");
    expect(marcador).toHaveAttribute("stroke", "#30363d");

    // A faixa entre as duas NAO segue ate o preto: ela e area, e preto a 10%
    // sobre fundo escuro escurece o que devia destacar.
    expect(grafico.querySelector('[data-faixa="diferenca"]')).toHaveAttribute("fill", "#8b949e");
  });

  // A carreira que nunca passou por monomarca nao carrega a legenda: chave de uma
  // marca que nao esta no desenho e ruido.
  it("nao mostra a chave do monomarca em quem nunca correu numa", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    expect(grafico.querySelector('[data-chave="monomarca"]')).toBeNull();
    expect(grafico.querySelector("[data-monomarca]")).toBeNull();
  });

  // O ponto da temporada em curso e o MENOS confirmado da curva, e era o maior:
  // vazado e ainda com metade de raio a mais, virava uma bola gigante flutuando
  // depois da regua do HOJE com mais peso que os campeonatos ganhos.
  it("nao infla o marcador da temporada em curso", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    expect(grafico.querySelector("[data-parcial]")).toHaveAttribute("r", "3");
    // Vazado ele continua: e o que o distingue de um campeonato terminado.
    expect(grafico.querySelector("[data-parcial]")).toHaveAttribute("fill", "#0f1c2b");
  });

  // A distancia dita em numero, para quem prefere ler a ver. O sinal e
  // invertido em relacao a posicao: bater a expectativa e o numero DIMINUIR.
  it("o balao diz quantas posicoes ele ficou acima ou abaixo do carro", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    const alvos = grafico.querySelectorAll('[data-alvo="temporada"]');

    // 2019: terminou em 9º com um carro de 11º — duas acima.
    fireEvent.mouseEnter(alvos[0]);
    expect(await screen.findByTestId("driver-detail-championship-tooltip")).toHaveTextContent(
      "2 posições acima do que o carro dava",
    );

    // 2022: 12º com um carro de 8º — quatro abaixo.
    fireEvent.mouseEnter(alvos[3]);
    expect(screen.getByTestId("driver-detail-championship-tooltip")).toHaveTextContent(
      "4 posições abaixo do que o carro dava",
    );
  });

  // A temporada em curso e o unico ponto parcial: o campeonato dela ainda esta
  // sendo disputado, e imprimir a posicao de hoje como resultado seria dar um
  // campeonato por encerrado.
  it("trata a temporada em curso como parcial", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    expect(grafico.querySelector('[data-serie="posicao"][data-futuro]')).not.toBeNull();
    expect(grafico.querySelector('[data-marca="hoje"]')).not.toBeNull();
    expect(grafico.querySelector("[data-parcial]")).not.toBeNull();
    // O rotulo do ano em curso escreve mais fraco, como a linha que chega nele:
    // o numero existe, e nao e resultado ainda.
    const rotulos = [...grafico.querySelectorAll('[data-rotulo="posicao"]')];
    expect(rotulos.at(-1)).toHaveTextContent("3º");
    expect(rotulos.at(-1)).toHaveAttribute("opacity", "0.55");
    expect(rotulos.at(-2).getAttribute("opacity")).toBe("1");
  });

  // A curva mostrava a FORMA da trajetoria e escondia os numeros dela: da para
  // ver que um ano foi pior que o anterior sem descobrir se foi 17º ou 22º, que e
  // a diferenca entre um ano ruim e um ano de fundo de grid. O hover respondia —
  // e hover e uma resposta que exige a pergunta.
  it("escreve a posicao final em cada ponto", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    const rotulos = [...grafico.querySelectorAll('[data-rotulo="posicao"]')];
    expect(rotulos.map((no) => no.textContent)).toEqual(["9º", "4º", "1º", "12º", "1º", "3º"]);

    // O ano do titulo escreve na cor do titulo, como o marcador.
    expect(rotulos[2]).toHaveAttribute("fill", "#d4a72c");
    expect(rotulos[0]).toHaveAttribute("fill", "#f0f6fc");

    // Halo da cor do cartao por baixo: vinte rotulos sobre duas linhas cruzam
    // alguma coisa por definicao.
    expect(rotulos[0]).toHaveAttribute("stroke", "#0f1c2b");
  });

  // O rotulo foge da expectativa, que e a outra linha do quadro: nos anos em que
  // ele ficou ABAIXO do carro ela passa por cima do ponto, e o numero desce.
  it("poe o numero do lado oposto a linha do carro", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    const rotulos = [...grafico.querySelectorAll('[data-rotulo="posicao"]')];
    const marcadores = [...grafico.querySelectorAll('[data-serie="posicao"]')];
    expect(marcadores.length).toBeGreaterThan(0);

    // 2019: terminou em 9º com um carro de 11º — a expectativa esta ABAIXO, e o
    // numero sobe.
    expect(Number(rotulos[0].getAttribute("y"))).toBeLessThan(
      Number(grafico.querySelectorAll('[data-marcador="esperado"]')[0].getAttribute("cy")),
    );
    // 2022: 12º com um carro de 8º — a expectativa esta ACIMA, e o numero desce
    // para nao cair em cima dela.
    const marcador2022 = grafico.querySelectorAll('[data-marcador="esperado"]')[3];
    expect(Number(rotulos[3].getAttribute("y"))).toBeGreaterThan(
      Number(marcador2022.getAttribute("cy")),
    );
  });

  // O denominador anda colado no numero: "3º" sozinho nao diz se foram doze ou
  // trinta carros na disputa.
  it("o balao da temporada traz a posicao com o tamanho do grid", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    const alvos = grafico.querySelectorAll('[data-alvo="temporada"]');
    fireEvent.mouseEnter(alvos[3]);

    const balao = await screen.findByTestId("driver-detail-championship-tooltip");
    expect(balao).toHaveTextContent("12º de 18");
    expect(balao).toHaveTextContent("Aures Racing");
    // E nada da contagem de corridas, vitorias e podios: o balao responde a
    // pergunta do grafico — onde ele terminou contra onde o carro terminaria —,
    // e esses tres numeros estao na tabela e na aba de estatisticas.
    expect(balao).not.toHaveTextContent("14C");
    expect(balao.querySelector("[data-testid$='-logo']")).not.toBeNull();
  });

  // Abaixo de tres temporadas fechadas o grafico nao tem trajetoria a mostrar:
  // dois pontos numa moldura dimensionada para uma carreira inteira leem-se como
  // "faltou informacao", quando a informacao esta toda ali.
  it("abre na tabela quando a carreira ainda e curta", async () => {
    const base = detail();
    renderFicha(
      {},
      detail({
        trajetoria: {
          ...base.trajetoria,
          // Tres pontos, mas so DOIS fechados: a temporada em curso nao e
          // historico.
          curva_campeonato: curvaCampeonato().slice(3),
        },
      }),
    );

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    expect(await screen.findByTestId("driver-detail-championship-table")).toBeInTheDocument();
    // A escolha automatica e um PADRAO, nao uma trava.
    fireEvent.click(screen.getByTestId("driver-detail-championship-toggle"));
    expect(screen.queryByTestId("driver-detail-championship-table")).toBeNull();
    expect(
      screen
        .getByTestId("driver-detail-championship-curve")
        .querySelector('[data-serie="posicao"]'),
    ).not.toBeNull();
  });

  // Com tres anos fechados ja ha uma VIRADA a ler — subiu e caiu, caiu e subiu,
  // ou seguiu na mesma —, e essa e a menor forma que a curva sabe desenhar. Dois
  // pontos so sabem dizer "melhorou" ou "piorou", e para isso a tabela basta.
  it("abre no grafico assim que ha tres temporadas fechadas", async () => {
    const base = detail();
    renderFicha(
      {},
      detail({
        trajetoria: {
          ...base.trajetoria,
          curva_campeonato: curvaCampeonato().slice(2),
        },
      }),
    );

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = await screen.findByTestId("driver-detail-championship-curve");
    expect(screen.queryByTestId("driver-detail-championship-table")).toBeNull();
    expect(grafico.querySelector('[data-serie="posicao"]')).not.toBeNull();
    expect(screen.getByTestId("driver-detail-championship-toggle")).toHaveTextContent("Ver tabela");
  });

  // Ano fora do grid nao e lacuna de dado — e o que aconteceu com ele. Ocupa
  // espaco no grafico, hachurado e nomeado no lugar, em vez de virar um vao mudo
  // que se le como bug.
  it("hachura o ano em que ele ficou sem equipe", async () => {
    const base = detail();
    const comBuraco = curvaCampeonato().map((ponto, indice) =>
      indice === 3
        ? {
            ...ponto,
            categoria: "",
            equipe_nome: null,
            equipe_cor: null,
            posicao: null,
            corridas: 0,
          }
        : ponto,
    );
    renderFicha({}, detail({ trajetoria: { ...base.trajetoria, curva_campeonato: comBuraco } }));

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    expect(grafico.querySelector('[data-faixa="sem-contrato"]')).not.toBeNull();
    expect(grafico.querySelector('[data-fita="sem-equipe"]')).not.toBeNull();
    // A linha ATRAVESSA o vão em tracejado em vez de sumir por um ano e voltar
    // do outro lado: a carreira continua, o que não houve foi campeonato.
    const pontes = grafico.querySelectorAll('[data-ponte="sem-contrato"]');
    // UMA ponte só: nesta fixture a expectativa do carro sobrevive ao ano sem
    // equipe, então a laranja atravessa cheia e não tem vão a costurar. Ponte
    // por cima de linha desenhada seria o mesmo trecho duas vezes.
    expect(pontes).toHaveLength(1);
    expect(pontes[0]).toHaveAttribute("stroke-dasharray", "4 4");
    expect(pontes[0]).toHaveAttribute("stroke", "#f0f6fc");
  });

  // As DUAS séries atravessam o vão. Interromper só a laranja deixava a branca
  // cruzando sozinha, e do outro lado as duas recomeçavam sem que nada dissesse
  // o que houve com a de baixo.
  it("costura as duas linhas quando o ano fora do grid nao tem nem resultado nem expectativa", async () => {
    const base = detail();
    const comBuraco = curvaCampeonato().map((ponto, indice) =>
      indice === 3
        ? {
            ...ponto,
            categoria: "",
            equipe_nome: null,
            equipe_cor: null,
            posicao: null,
            esperado: null,
            corridas: 0,
          }
        : ponto,
    );
    renderFicha({}, detail({ trajetoria: { ...base.trajetoria, curva_campeonato: comBuraco } }));

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grafico = screen.getByTestId("driver-detail-championship-curve");
    const cores = [...grafico.querySelectorAll('[data-ponte="sem-contrato"]')].map((ponte) =>
      ponte.getAttribute("stroke"),
    );
    expect(cores).toEqual(["#000000", "#f0f6fc"]);
    // A laranja vem antes: onde as duas se cruzam, quem fica por cima é o
    // resultado, como nas séries.
  });

  it("desenha o calendario inteiro das duas temporadas, separadas", async () => {
    renderFicha();
    await abrirTemporada();

    const strip = await screen.findByTestId("driver-detail-form-strip");
    const grupos = strip.querySelectorAll("[data-season]");
    expect(grupos).toHaveLength(2);
    // A temporada fechada vem inteira (4 corridas), não recortada em 5 últimas.
    expect(grupos[0]).toHaveAttribute("data-season", "2025");
    expect(grupos[0].querySelectorAll("[data-round]")).toHaveLength(4);
    // A atual vem marcada e à direita da divisa.
    expect(grupos[1]).toHaveAttribute("data-current", "true");
    expect(grupos[1]).toHaveTextContent("2026");
    expect(grupos[1].querySelectorAll("[data-round]")).toHaveLength(2);
    // Média por temporada, e não uma só cruzando os dois campeonatos.
    expect(grupos[0]).toHaveTextContent("P4.0");
    expect(grupos[0]).toHaveTextContent("1 DNF");
  });

  it("marca o abandono em vez de desenhar uma coluna", async () => {
    renderFicha();
    await abrirTemporada();

    const strip = await screen.findByTestId("driver-detail-form-strip");
    const passada = strip.querySelector('[data-season="2025"]');
    expect(passada.querySelector('[data-round="3"]')).toHaveAttribute("data-dnf", "true");
    expect(passada.querySelector('[data-round="2"]')).toHaveTextContent("P2");
  });

  it("cai numa faixa unica quando o payload nao traz temporadas", async () => {
    renderFicha(
      {},
      detail({
        forma: {
          tendencia: "->",
          momento: "forte",
          ultimas_10: [
            { rodada: 1, chegada: 3, dnf: false },
            { rodada: 2, chegada: 5, dnf: false },
          ],
        },
      }),
    );
    await abrirTemporada();

    const strip = await screen.findByTestId("driver-detail-form-strip");
    expect(strip.querySelectorAll("[data-season]")).toHaveLength(0);
    expect(strip.querySelectorAll("[data-round]")).toHaveLength(2);
  });

  it("mostra so o historico para piloto aposentado", async () => {
    renderFicha({}, detail({ status: "aposentado" }));

    await screen.findByTestId("driver-detail-hero");
    expect(screen.getByTestId("driver-detail-tab-historico")).toBeInTheDocument();
    expect(screen.queryByTestId("driver-detail-tab-temporada")).not.toBeInTheDocument();
    expect(screen.queryByTestId("driver-detail-tab-mercado")).not.toBeInTheDocument();
  });

  it("abre a aba de habilidade so para o jogador", async () => {
    renderFicha({}, detail({ is_jogador: true }));

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-habilidade"));
    expect(screen.getByText("dossie-habilidade")).toBeInTheDocument();
    // Jogador não se favorita.
    expect(screen.queryByTestId("driver-detail-favorite")).not.toBeInTheDocument();
  });

  // ── Rivais ────────────────────────────────────────────────────────────────
  //
  // A aba mostrava três números numa escala invisível e o valor cru do enum do
  // motor. O que ela precisa provar agora é que o CONFRONTO manda: placar,
  // último encontro e nível nomeado.

  it("abre o confronto direto do rival principal com placar e ultimo encontro", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    const hero = await screen.findByTestId("driver-detail-rival-hero");
    expect(within(hero).getByText("Tiago Sousa")).toBeInTheDocument();
    // 13 a 10 em 23: o placar e o total sao coisas diferentes, e a diferenca
    // (as duas corridas sem duelo) e informacao, nao arredondamento.
    const duelo = within(hero).getByTestId("driver-detail-duel");
    expect(within(duelo).getByText("13")).toBeInTheDocument();
    expect(within(duelo).getByText("10")).toBeInTheDocument();
    expect(within(duelo).getByText("23 corridas juntos")).toBeInTheDocument();
  });

  // A faixa é o que responde "em QUAIS corridas" — o placar sozinho só diz
  // quanto. O ano vai no lugar do número da temporada porque "T27" não é uma
  // data que alguém reconheça.
  it("desenha a faixa do confronto agrupada por ano e comeca pelo ultimo encontro", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    const faixa = await screen.findByTestId("driver-detail-duel-timeline");
    const anos = within(faixa)
      .getAllByText(/^20\d\d$/)
      .map((no) => no.textContent);
    expect(anos).toEqual(["2026", "2027"]);
    // Uma marca por corrida, com a cor dizendo quem ganhou o dia.
    expect(faixa.querySelectorAll("[data-duel-mark]")).toHaveLength(3);
    expect(faixa.querySelectorAll('[data-duel-mark="piloto"]')).toHaveLength(2);

    // Em repouso a legenda mostra o ENCONTRO MAIS RECENTE: quem não passar o
    // mouse em nada leva daqui a última vez que os dois se cruzaram.
    expect(within(faixa).getByText("Interlagos · 2027")).toBeInTheDocument();
    expect(within(faixa).getByTestId("driver-detail-duel-winner")).toHaveTextContent("Ana");
  });

  // Marcas de altura igual desenhavam a mesma coisa para "ele me ganhou por tres
  // decimos" e "ele me tomou uma volta".
  it("da altura as barras conforme o tempo entre os dois, e lado conforme quem venceu", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    const faixa = await screen.findByTestId("driver-detail-duel-timeline");
    const barras = [...faixa.querySelectorAll('[data-duel-band="gap"] rect')];
    const altura = (indice) => Number(barras[indice].getAttribute("height"));
    const topo = (indice) => Number(barras[indice].getAttribute("y"));

    // 0.4s, 12s e 3s: a ordem das alturas e a ordem dos tempos.
    expect(altura(1)).toBeGreaterThan(altura(2));
    expect(altura(2)).toBeGreaterThan(altura(0));
    // Mas a raiz impede que a corrida de 12s achate a de 0.4s contra a linha:
    // trinta vezes mais tempo nao vira trinta vezes mais barra.
    expect(altura(1) / altura(0)).toBeLessThan(30);

    // Quem venceu cresce para CIMA da linha central, quem perdeu para baixo.
    expect(topo(0)).toBeLessThan(20);
    expect(topo(2)).toBeLessThan(20);
    expect(topo(1)).toBe(20);
  });

  it("mostra o tempo da corrida em foco ao lado do nome de quem venceu", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    const faixa = await screen.findByTestId("driver-detail-duel-timeline");
    // Em repouso, o ultimo encontro: Interlagos, 3s de vantagem para a Ana. O
    // espaco entre os dois e margem e nao texto, entao a asserção olha as duas
    // partes.
    const legenda = within(faixa).getByTestId("driver-detail-duel-winner");
    expect(legenda).toHaveTextContent("Ana");
    expect(legenda).toHaveTextContent("3.0s");
  });

  it("troca a legenda da faixa para a corrida sob o ponteiro", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    const faixa = await screen.findByTestId("driver-detail-duel-timeline");
    fireEvent.mouseEnter(faixa.querySelectorAll("[data-duel-mark]")[1]);

    expect(within(faixa).getByText("Spa · 2026")).toBeInTheDocument();
    // Só o nome de quem venceu, e não as duas colocações: a pergunta é quem
    // ganhou o dia, e comparar P6 com P1 é o caminho longo para a resposta.
    expect(within(faixa).getByTestId("driver-detail-duel-winner")).toHaveTextContent("Tiago");
  });

  it("marca como sem duelo a corrida em que alguem abandonou", async () => {
    renderFicha(
      {},
      detail({
        rivais: {
          itens: [
            rival({
              encontros: [
                { ano: 2027, season_number: 3, rodada: 8, pista: "Sebring", vencedor: "nenhum" },
              ],
            }),
          ],
        },
      }),
    );
    await abrirRivais();

    const faixa = await screen.findByTestId("driver-detail-duel-timeline");
    expect(within(faixa).getByTestId("driver-detail-duel-winner")).toHaveTextContent("sem duelo");
    expect(faixa.querySelectorAll('[data-duel-mark="nenhum"]')).toHaveLength(1);
  });

  it("nomeia a faixa de intensidade em vez de mostrar o numero cru", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival({ nivel_chave: "forte", intensidade: 66 })] } }));
    await abrirRivais();

    const hero = await screen.findByTestId("driver-detail-rival-hero");
    expect(within(hero).getByText("Rivalidade forte")).toBeInTheDocument();
    expect(within(hero).queryByText("66")).not.toBeInTheDocument();
  });

  it("vai para a ficha do rival por um alvo proprio, e nao pelo card inteiro", async () => {
    const onSelectDriver = vi.fn();
    renderFicha({ onSelectDriver }, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    // O card inteiro nao navega mais: o clique grande e o gesto de abrir a
    // rivalidade, e sequestra-lo para trocar de piloto era o que impedia ver o
    // confronto de qualquer rival que nao fosse o primeiro.
    fireEvent.click(await screen.findByTestId("driver-detail-rival-hero"));
    expect(onSelectDriver).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId("driver-detail-rival-open"));
    expect(onSelectDriver).toHaveBeenCalledWith("D9");
  });

  // Num grid fechado todo mundo divide o mesmo numero de corridas, e um piloto
  // de meio de pelotao perde para os tres de cima quase na mesma proporcao: o
  // placar de carreira e justamente o numero que NAO separa esses rivais.
  it("separa rivais de placar parecido pelo recorte recente, pela sequencia e pelo sabado", async () => {
    const corrida = (ano, rodada, vencedor) => ({
      ano,
      season_number: ano === 2025 ? 2 : 3,
      rodada,
      pista: "Spa",
      vencedor,
    });
    const encontros = [
      // Doze antigas de mao unica para o rival...
      ...Array.from({ length: 12 }, (_, indice) => corrida(2025, indice + 1, "rival")),
      // ...e as dez recentes quase empatadas, terminando em quatro seguidas para
      // o dono da ficha.
      ...["rival", "rival", "rival", "piloto", "rival", "piloto", "piloto"].map(
        (vencedor, indice) => corrida(2026, indice + 1, vencedor),
      ),
      // Abandono no meio da sequencia: nao decide nada e nao a quebra.
      corrida(2026, 8, "nenhum"),
      corrida(2026, 9, "piloto"),
      corrida(2026, 10, "piloto"),
    ];
    renderFicha(
      {},
      detail({
        rivais: { itens: [rival({ encontros, confrontos: 22, vitorias: 5, derrotas: 16 })] },
      }),
    );
    await abrirRivais();

    const fatos = await screen.findByTestId("driver-detail-duel-facts");
    const fato = (chave) => fatos.querySelector(`[data-duel-fact="${chave}"]`);

    // Na vida ele apanha de 5 a 16; nas ultimas dez esta 4 a 5. Sao dois
    // retratos diferentes do mesmo par de pessoas, e so o recorte mostra o
    // segundo. O par vem na ordem do placar grande: rival a esquerda, dono da
    // ficha a direita.
    expect(fato("recente")).toHaveTextContent("Últimas 10");
    expect(fato("recente")).toHaveTextContent("4–5");
    // Quatro seguidas, com o abandono atravessado no meio contando como
    // transparente em vez de quebrar a serie.
    expect(fato("sequencia")).toHaveTextContent("4 p/ Ana");
    // O sabado e outro esporte: da para perder o domingo a vida inteira e ainda
    // assim largar na frente.
    expect(fato("quali")).toHaveTextContent("14–9");
  });

  it("mede a distancia do confronto, e nao so a contagem", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    const fatos = await screen.findByTestId("driver-detail-duel-facts");
    // O gap diz se a derrota e fotografia ou surra: 13–10 pode ser meio segundo
    // ou meia volta, e o placar nao distingue.
    expect(fatos.querySelector('[data-duel-fact="gap"]')).toHaveTextContent("2.4s");
  });

  // A unica comparacao da ficha sem o carro no meio.
  it("destaca o periodo de box dividido e junta anos seguidos num intervalo", async () => {
    renderFicha(
      {},
      detail({
        rivais: {
          itens: [
            rival({
              companheirismo: {
                equipe: "Aures Racing",
                anos: [2024, 2025, 2026, 2028],
                vitorias: 21,
                derrotas: 15,
              },
            }),
          ],
        },
      }),
    );
    await abrirRivais();

    const dupla = await screen.findByTestId("driver-detail-duel-teammate");
    // Tres anos seguidos sao um periodo, e lista-los item a item contaria como
    // tres fatos o que e um so.
    expect(dupla).toHaveTextContent("Dividiram box na Aures Racing em 2024–2026, 2028.");
    expect(dupla).toHaveTextContent("Mesmo carro: 21 a 15 para Ana sobre Tiago.");
  });

  it("cala o box dividido para quem nunca foi companheiro", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    await screen.findByTestId("driver-detail-rival-hero");
    expect(screen.queryByTestId("driver-detail-duel-teammate")).toBeNull();
  });

  it("cala o recorte recente quando ele seria a carreira inteira de novo", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    const fatos = await screen.findByTestId("driver-detail-duel-facts");
    // Tres encontros no total: "ultimas 10" seria o mesmo numero com outro nome.
    expect(fatos.querySelector('[data-duel-fact="recente"]')).toBeNull();
    expect(fatos.querySelector('[data-duel-fact="quali"]')).not.toBeNull();
  });

  it("resume a linha fechada pelo saldo, que ja chega comparavel", async () => {
    renderFicha(
      {},
      detail({
        rivais: {
          itens: [
            rival(),
            rival({ driver_id: "D8", nome: "Eli Green", confrontos: 45, vitorias: 12, derrotas: 32 }),
          ],
        },
      }),
    );
    await abrirRivais();

    const linha = await screen.findByTestId("driver-detail-rival-row");
    // "12–32" e "12–33" obrigam a subtrair para comparar duas linhas.
    expect(linha).toHaveTextContent("−20");
    expect(linha).not.toHaveTextContent("12–32");
  });

  it("rebaixa os rivais seguintes a linha, com o principal em destaque", async () => {
    renderFicha(
      {},
      detail({
        rivais: { itens: [rival(), rival({ driver_id: "D8", nome: "Eli Green", confrontos: 4 })] },
      }),
    );
    await abrirRivais();

    await screen.findByTestId("driver-detail-rival-hero");
    const linhas = screen.getAllByTestId("driver-detail-rival-row");
    expect(linhas).toHaveLength(1);
    expect(linhas[0]).toHaveTextContent("Eli Green");
  });

  // Era isto que estava quebrado: tocar num rival secundario trocava de piloto,
  // entao a unica rivalidade que dava para ler era a primeira.
  it("abre a rivalidade da linha tocada e fecha a que estava aberta", async () => {
    const onSelectDriver = vi.fn();
    renderFicha(
      { onSelectDriver },
      detail({
        rivais: {
          itens: [
            rival(),
            rival({
              driver_id: "D8",
              nome: "Eli Green",
              confrontos: 4,
              vitorias: 1,
              derrotas: 3,
              encontros: [
                { ano: 2027, season_number: 3, rodada: 9, pista: "Suzuka", vencedor: "rival" },
              ],
            }),
          ],
        },
      }),
    );
    await abrirRivais();

    fireEvent.click(await screen.findByTestId("driver-detail-rival-row"));

    // A lista nao reordena: o card aberto nasce onde estava a linha tocada, e o
    // que estava aberto vira linha no lugar dele.
    expect(await screen.findByTestId("driver-detail-rival-hero")).toHaveTextContent("Eli Green");
    expect(screen.getByTestId("driver-detail-rival-row")).toHaveTextContent("Tiago Sousa");
    // Abrir uma rivalidade nao e navegar.
    expect(onSelectDriver).not.toHaveBeenCalled();
    // E o confronto mostrado passa a ser o do rival aberto.
    expect(screen.getByTestId("driver-detail-duel-timeline")).toHaveTextContent("Suzuka · 2027");
  });

  it("ensina como nasce uma rivalidade quando o piloto nao tem nenhuma", async () => {
    renderFicha({}, detail({ rivais: { itens: [] } }));
    await abrirRivais();

    const vazio = await screen.findByTestId("driver-detail-rivals-empty");
    expect(vazio).toHaveTextContent("Ainda ninguém");
    expect(vazio).toHaveTextContent("dividir o mesmo box");
  });

  it("avisa quando os dois nao dividem mais o grid", async () => {
    renderFicha(
      {},
      detail({
        rivais: { itens: [rival({ mesma_categoria: false, categoria_atual: "F3" })] },
      }),
    );
    await abrirRivais();

    expect(await screen.findByTestId("driver-detail-rival-hero")).toHaveTextContent(
      "Corre na F3 — não dividem mais o grid.",
    );
  });

  it("navega para o piloto vizinho pela calha de setas", async () => {
    const onSelectDriver = vi.fn();
    renderFicha({ onSelectDriver });

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-step-down"));
    expect(onSelectDriver).toHaveBeenCalledWith("D2");
  });

  // ─────────────────────────────── Mercado ───────────────────────────────

  it("decompoe a chance de troca nas forcas que a compoem", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: {
            valor_mercado: 3341311,
            salario_estimado: 1200000,
            chance_transferencia: 57,
            forcas_transferencia: {
              contrato: 54,
              motivacao: 0,
              mercado: 3,
              anos_restantes: 0,
            },
          },
        },
      }),
    );

    await abrirMercado();
    const medidor = await screen.findByTestId("driver-detail-transfer-meter");
    expect(within(medidor).getByTestId("driver-detail-transfer-chance")).toHaveTextContent("57%");

    // A barra é a chance inteira: as parcelas fecham no total, então o segmento
    // do contrato ocupa 54/57 dela, e não 54% de uma escala 0-100.
    expect(medidor.querySelector("[data-forca='contrato']")).toHaveStyle({
      width: `${(54 / 57) * 100}%`,
    });
    // Força zerada não vira um fiapo de 0px na barra — só sobra na legenda.
    expect(medidor.querySelector("[data-forca='motivacao']")).toBeNull();
    expect(medidor.querySelector("[data-forca-key='motivacao']")).toHaveTextContent("Desmotivação");

    // A barra e as legendas dizem quem está puxando. O parágrafo que narrava a
    // força dominante saiu: repetia em prosa o desenho logo acima.
    expect(medidor).not.toHaveTextContent(/O contrato acaba nesta janela/);
  });

  it("explica cada forca da chance de troca no hover", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato({ anos_restantes: 3, ano_fim: 2029 }),
          mercado: {
            valor_mercado: 3341311,
            salario_estimado: 1200000,
            chance_transferencia: 19,
            forcas_transferencia: {
              contrato: 14,
              motivacao: 0,
              mercado: 5,
              anos_restantes: 3,
            },
          },
        },
      }),
    );

    await abrirMercado();
    const medidor = await screen.findByTestId("driver-detail-transfer-meter");

    // "Assédio" era jargão de imprensa esportiva: quem não conhece a mecânica
    // lia a palavra e não sabia de onde vinha o número.
    const cobica = medidor.querySelector("[data-forca-key='mercado']");
    expect(cobica).toHaveTextContent("Interesse de fora");
    expect(cobica).toHaveAttribute("data-tooltip", expect.stringContaining("Talento cobiçado"));
    expect(medidor.querySelector("[data-forca-key='contrato']")).toHaveAttribute(
      "data-tooltip",
      expect.stringContaining("O prazo em si"),
    );
    // Força zerada continua explicada: é ela que responde "por que 0?".
    expect(medidor.querySelector("[data-forca-key='motivacao']")).toHaveAttribute(
      "data-tooltip",
      expect.stringContaining("insatisfeito"),
    );
  });

  it("mostra a desmotivacao mandando pelo tamanho do segmento", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato({ anos_restantes: 2, ano_fim: 2028 }),
          mercado: {
            valor_mercado: 800000,
            salario_estimado: 240000,
            chance_transferencia: 48,
            forcas_transferencia: {
              contrato: 14,
              motivacao: 31,
              mercado: 3,
              anos_restantes: 2,
            },
          },
        },
      }),
    );

    await abrirMercado();
    const medidor = await screen.findByTestId("driver-detail-transfer-meter");
    // Quem manda se lê no desenho: o segmento da desmotivação é o maior.
    expect(medidor.querySelector("[data-forca='motivacao']")).toHaveStyle({
      width: `${(31 / 48) * 100}%`,
    });
  });

  it("nomeia o contrato que acaba agora e guarda a vigencia no balao", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato({ ano_inicio: 2024, ano_fim: 2026, anos_restantes: 0 }),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 60 },
        },
      }),
    );

    await abrirMercado();
    // "0 ano" virava conta de cabeça; o prazo por extenso responde direto, e a
    // régua de temporadas saiu para o gráfico, que já desenha os anos assinados.
    const prazo = (await screen.findByTestId("driver-detail-situation")).querySelector(
      "[data-prazo]",
    );
    expect(prazo).toHaveAttribute("data-prazo", "agora");
    expect(prazo).toHaveTextContent("Expira nesta janela");

    // Os anos moram na régua, nomeados um a um — e todos cumpridos, porque não
    // resta nenhum.
    const regua = screen.getByTestId("driver-detail-contract-ruler");
    expect([...regua.querySelectorAll("[data-temporada]")].map((no) => no.dataset.temporada)).toEqual(
      ["2024", "2025", "2026"],
    );
    expect(regua.querySelectorAll("[data-cumprida]")).toHaveLength(3);
    expect(regua).toHaveTextContent("2024");
    expect(regua).toHaveTextContent("2026");
  });

  it("separa na regua o ano que ainda falta do que ja foi cumprido", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato({ ano_inicio: 2026, ano_fim: 2028, anos_restantes: 1 }),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 30 },
        },
      }),
    );

    await abrirMercado();
    const prazo = (await screen.findByTestId("driver-detail-situation")).querySelector(
      "[data-prazo]",
    );
    expect(prazo).toHaveAttribute("data-prazo", "ultimo");

    // Dois cumpridos, um por vir — e o que falta é tracejado, não é uma barra
    // vazia: o vocabulário é o mesmo do futuro contratado no gráfico.
    const regua = screen.getByTestId("driver-detail-contract-ruler");
    expect(regua.querySelectorAll("[data-temporada]")).toHaveLength(3);
    expect(regua.querySelectorAll("[data-cumprida]")).toHaveLength(2);
    expect(regua.querySelector("[data-temporada='2028']")).not.toHaveAttribute("data-cumprida");
    expect(regua.querySelector("[data-temporada='2028']").style.backgroundImage).toContain(
      "repeating-linear-gradient",
    );
  });

  it("compara o que ele ganha com o que o mercado pagaria, na direcao certa", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato({ salario_anual: 960534 }),
          mercado: {
            valor_mercado: 3341311,
            salario_estimado: 1300000,
            chance_transferencia: 57,
          },
        },
      }),
    );

    await abrirMercado();
    const card = await screen.findByTestId("driver-detail-situation");
    expect(card.querySelector("[data-selo]")).toHaveAttribute("data-selo", "pechincha");

    // As duas barras dividem a escala: quem vale mais é a maior.
    const pago = card.querySelector("[data-barra='pago'] span span");
    const mercado = card.querySelector("[data-barra='mercado'] span span");
    expect(mercado).toHaveStyle({ width: "100%" });
    expect(pago).toHaveStyle({ width: `${(960534 / 1300000) * 100}%` });

    // A frase diz a conta na direção em que ela é verdadeira: 960534/1300000 é
    // 26% a menos do que ele vale — e não os "+35%" da razão invertida.
    expect(card).toHaveTextContent("26% a menos");
    expect(card).not.toHaveTextContent("35%");
  });

  it("nao inventa selo de preco para piloto sem contrato", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: null,
          mercado: {
            valor_mercado: 500000,
            salario_estimado: 180000,
            chance_transferencia: 100,
          },
        },
      }),
    );

    await abrirMercado();
    const card = await screen.findByTestId("driver-detail-situation");
    expect(card.querySelector("[data-selo]")).toBeNull();
    // Sem contrato não há barra do pago nem prazo — e nada disso vira zero.
    expect(card.querySelector("[data-barra='pago']")).toBeNull();
    expect(card.querySelector("[data-prazo]")).toBeNull();
    expect(screen.getByTestId("driver-detail-transfer-meter")).toHaveTextContent(
      /Sem contrato ativo/,
    );
  });

  it("da regua ao valor de mercado com a posicao no grid da categoria", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: {
            valor_mercado: 900000,
            salario_estimado: 300000,
            chance_transferencia: 40,
            posicao_valor: 3,
            total_valor: 24,
            categoria_valor: "gt3",
          },
        },
      }),
    );

    await abrirMercado();
    const regua = await screen.findByTestId("driver-detail-market-rank");
    expect(regua).toHaveTextContent("3º de 24");
    // A barra é a fatia do pelotão que está atrás dele: 3º de 24 são 22 carros.
    expect(regua.querySelector("[data-preenchimento='posto']")).toHaveStyle({
      width: `${((24 - 3 + 1) / 24) * 100}%`,
    });
  });

  it("nao desenha a regua de valor para quem nao tem assento no grid", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 40 },
        },
      }),
    );

    await abrirMercado();
    await screen.findByTestId("driver-detail-situation");
    expect(screen.queryByTestId("driver-detail-market-rank")).toBeNull();
  });

  it("mede a tendencia do valor contra a ultima temporada avaliada", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 40 },
          curva: [
            { season_number: 1, ano: 2024, categoria: "gt3", valor_mercado: 600000, atual: false },
            { season_number: 2, ano: 2025, categoria: "gt3", valor_mercado: 750000, atual: false },
            { season_number: 3, ano: 2026, categoria: "gt3", valor_mercado: 900000, atual: true },
            // Ano já contratado não tem avaliação e não pode virar base.
            { season_number: 4, ano: 2027, categoria: "gt3", futuro: true },
          ],
        },
      }),
    );

    await abrirMercado();
    const card = await screen.findByTestId("driver-detail-situation");
    const chip = card.querySelector("[data-tendencia]");
    expect(chip).toHaveAttribute("data-tendencia", "alta");
    expect(chip).toHaveTextContent("+20%");
    expect(chip).toHaveAttribute("data-tooltip", expect.stringContaining("2025"));
  });

  it("desenha a carreira em dinheiro com as duas series no mesmo eixo", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 3341311, salario_estimado: 1300000, chance_transferencia: 57 },
          curva: curva(),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    expect(grafico.querySelector("[data-serie='mercado']")).toBeInTheDocument();
    expect(grafico).toHaveTextContent("2022");
    expect(grafico).toHaveTextContent("2026");
    // Rótulo direto só na ponta — um número por ponto viraria ruído.
    expect(grafico.querySelectorAll("[data-rotulo='ponta']")).toHaveLength(2);
  });

  it("nao liga a linha do salario por cima de uma temporada sem contrato", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 40 },
          // 2024 sem contrato: o piloto ficou sem equipe naquele ano.
          curva: curva().map((ponto) =>
            ponto.ano === 2024
              ? { ...ponto, salario_contrato: null, equipe_nome: null }
              : ponto,
          ),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    // Duas linhas separadas, e não uma atravessando o buraco: ligar os lados
    // inventaria um salário que não houve.
    expect(grafico.querySelectorAll("[data-serie='pago']")).toHaveLength(2);
    // E o vão não fica mudo: vira faixa marcada e nomeada no próprio gráfico —
    // um buraco sem explicação lê-se como bug.
    const faixas = grafico.querySelectorAll("[data-faixa='sem-contrato']");
    expect(faixas).toHaveLength(1);
    expect(faixas[0]).toHaveTextContent("Sem contrato");
    const ponte = grafico.querySelector("[data-ponte='sem-contrato']");
    expect(ponte.getAttribute("stroke-dasharray")).toBeTruthy();
    // E o pontilhado cobre a hachura EXATAMENTE: fora dela houve contrato, e ali
    // a ponte volta a ser traço cheio.
    const faixa = grafico.querySelector("[data-faixa='sem-contrato'] rect");
    const inicioDaFaixa = Number(faixa.getAttribute("x"));
    expect(Number(ponte.getAttribute("x1"))).toBeCloseTo(inicioDaFaixa, 5);
    expect(Number(ponte.getAttribute("x2"))).toBeCloseTo(
      inicioDaFaixa + Number(faixa.getAttribute("width")),
      5,
    );
  });

  // Dois pontos numa moldura dimensionada para uma carreira inteira leem-se como
  // "faltou informação", quando a informação está toda ali.
  it("abre na tabela quando o piloto ainda nao tem tres temporadas cumpridas", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato({ anos_restantes: 2, ano_fim: 2030 }),
          mercado: { valor_mercado: 40000, salario_estimado: 13000, chance_transferencia: 19 },
          curva: [
            { season_number: 1, ano: 2026, categoria: "mazdacup", equipe_nome: "Kitsune", salario_contrato: 12000, salario_mercado: 12400, atual: false },
            { season_number: 2, ano: 2027, categoria: "mazdacup", equipe_nome: "Kitsune", salario_contrato: 12000, salario_mercado: 12600, atual: false },
            { season_number: 3, ano: 2028, categoria: "mazdacup", equipe_nome: "Kitsune", salario_contrato: 13000, salario_mercado: 13000, atual: true },
            // Nem a temporada em curso nem as assinadas contam como histórico:
            // um rookie com contrato longo continua sendo um rookie.
            { season_number: 4, ano: 2029, categoria: "mazdacup", equipe_nome: "Kitsune", salario_contrato: 13000, salario_mercado: null, atual: false, futuro: true },
            { season_number: 5, ano: 2030, categoria: "mazdacup", equipe_nome: "Kitsune", salario_contrato: 13000, salario_mercado: null, atual: false, futuro: true },
          ],
        },
      }),
    );

    await abrirMercado();
    expect(await screen.findByTestId("driver-detail-curve-table")).toBeInTheDocument();
    const alternar = screen.getByTestId("driver-detail-curve-toggle");
    expect(alternar).toHaveTextContent("Ver gráfico");

    // Padrão, não trava: o desenho continua alcançável para quem quiser vê-lo.
    fireEvent.click(alternar);
    expect(screen.queryByTestId("driver-detail-curve-table")).toBeNull();
    expect(
      screen.getByTestId("driver-detail-market-curve").querySelector("[data-serie='pago']"),
    ).toBeInTheDocument();
  });

  it("abre no grafico assim que ha tres temporadas cumpridas", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 3341311, salario_estimado: 1300000, chance_transferencia: 57 },
          // Quatro anteriores mais a em curso.
          curva: curva(),
        },
      }),
    );

    await abrirMercado();
    expect(screen.queryByTestId("driver-detail-curve-table")).toBeNull();
    expect(screen.getByTestId("driver-detail-curve-toggle")).toHaveTextContent("Ver tabela");
  });

  // A escada 1-3-10 é cega para uma carreira curta: entre $12k e $25k não há
  // potência de 3 nem de 10, e o eixo saía sem marca nenhuma — a escala se
  // auto-ajusta, então um degrau de $1k desenhava um abismo sem régua que o
  // desmentisse.
  it("da marcas ao eixo mesmo quando a carreira cabe dentro de uma decada", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato({ salario_anual: 12000 }),
          mercado: { valor_mercado: 40000, salario_estimado: 13000, chance_transferencia: 19 },
          // Cinco temporadas cumpridas para a ficha abrir no gráfico: a régua é
          // sobre a FAIXA de valores caber numa década, não sobre o tamanho da
          // carreira.
          curva: [
            { season_number: 1, ano: 2024, categoria: "mazdacup", equipe_nome: "Kitsune", salario_contrato: 12000, salario_mercado: 12100, atual: false },
            { season_number: 2, ano: 2025, categoria: "mazdacup", equipe_nome: "Kitsune", salario_contrato: 12000, salario_mercado: 12300, atual: false },
            { season_number: 3, ano: 2026, categoria: "mazdacup", equipe_nome: "Kitsune", salario_contrato: 12000, salario_mercado: 12400, atual: false },
            { season_number: 4, ano: 2027, categoria: "mazdacup", equipe_nome: "Kitsune", salario_contrato: 24000, salario_mercado: 12600, atual: false },
            { season_number: 5, ano: 2028, categoria: "mazdacup", equipe_nome: "Kitsune", salario_contrato: 12000, salario_mercado: 13000, atual: true },
          ],
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    const marcas = [...grafico.querySelectorAll("text")]
      .map((no) => no.textContent)
      .filter((texto) => /^\$/.test(texto));
    // Pelo menos três alturas nomeadas, e nenhuma repetida: o formato compacto
    // arredonda, e duas linhas escritas "$13k" são piores do que nenhuma.
    expect(marcas.length).toBeGreaterThanOrEqual(3);
    expect(new Set(marcas).size).toBe(marcas.length);
  });

  // Um piloto de base é quase todo futuro: sem os anos já assinados a curva dele
  // são três pontos num quadro dimensionado para uma carreira inteira.
  it("estende a linha do salario pelos anos ja contratados", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato({ anos_restantes: 2, ano_fim: 2028 }),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 19 },
          curva: [
            ...curva(),
            { season_number: 6, ano: 2027, categoria: "gt3", equipe_nome: "Arclight", salario_contrato: 960534, salario_mercado: null, atual: false, futuro: true },
            { season_number: 7, ano: 2028, categoria: "gt3", equipe_nome: "Arclight", salario_contrato: 960534, salario_mercado: null, atual: false, futuro: true },
          ],
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    expect(grafico).toHaveTextContent("2028");

    // A azul continua, mas em dois pesos: cumprido e assinado não são a mesma
    // certeza. E ela continua LIGADA — o corte compartilha o ponto de hoje.
    const pagas = grafico.querySelectorAll("[data-serie='pago']");
    expect(pagas).toHaveLength(2);
    const cheia = grafico.querySelector("[data-serie='pago']:not([data-futuro])");
    const fantasma = grafico.querySelector("[data-serie='pago'][data-futuro]");
    expect(cheia.getAttribute("points").split(" ").at(-1)).toBe(
      fantasma.getAttribute("points").split(" ")[0],
    );

    // A laranja NÃO avança: valor de mercado futuro dependeria de quem ele vai
    // ser, e inventar isso é o que a curva inteira existe para não fazer.
    const mercados = grafico.querySelectorAll("[data-serie='mercado']");
    expect(mercados).toHaveLength(1);
    // Cinco temporadas com valor de mercado, uma por vértice. A estreia abre na
    // propria borda esquerda do plot, e não há vértice de FECHO: a laranja para
    // antes da última coluna do eixo, que é de um ano ainda por correr.
    const vertices = mercados[0].getAttribute("points").split(" ");
    expect(vertices).toHaveLength(5);
    expect(Number(vertices[0].split(",")[0])).toBeCloseTo(62, 5);
    expect(Number(vertices.at(-1).split(",")[0])).toBeLessThan(668);

    // A régua do presente explica o corte, e a legenda explica o traço fraco.
    expect(grafico.querySelector("[data-marca='hoje']")).toHaveTextContent("Hoje");
    expect(grafico.querySelector("[data-chave='futuro']")).toHaveTextContent("Já contratado");
  });

  it("nao promete futuro nem regua de hoje quando a curva acaba no presente", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 40 },
          curva: curva(),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    expect(grafico.querySelector("[data-marca='hoje']")).toBeNull();
    expect(grafico.querySelector("[data-chave='futuro']")).toBeNull();
    expect(grafico.querySelector("[data-serie='pago'][data-futuro]")).toBeNull();
  });

  it("nomeia a temporada futura como ainda nao corrida, e nao como arquivo perdido", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato({ anos_restantes: 1, ano_fim: 2027 }),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 19 },
          curva: [
            ...curva(),
            { season_number: 6, ano: 2027, categoria: "gt3", equipe_nome: "Arclight", salario_contrato: 960534, salario_mercado: null, atual: false, futuro: true },
          ],
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");

    // A nota de arquivo perdido conta lacunas do passado. 2027 não é lacuna.
    expect(grafico).not.toHaveTextContent(/o arquivo não permite reconstruir/);

    fireEvent.click(screen.getByTestId("driver-detail-curve-toggle"));
    const tabela = await screen.findByTestId("driver-detail-curve-table");
    expect(tabela).toHaveTextContent("Ainda não corrida");
    expect(tabela).not.toHaveTextContent("Sem arquivo");
  });

  // Dois anos seguidos fora do grid são UM período, não duas listras coladas.
  it("junta temporadas seguidas sem contrato numa faixa so", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 40 },
          curva: curva().map((ponto) =>
            ponto.ano === 2023 || ponto.ano === 2024
              ? { ...ponto, salario_contrato: null, equipe_nome: null }
              : ponto,
          ),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    expect(grafico.querySelectorAll("[data-faixa='sem-contrato']")).toHaveLength(1);
  });

  // Um ano contratado espremido entre dois vãos: a coluna inteira é dele. Era o
  // caso que a regra antiga — faixa de marcador a marcador — apagava, cobrindo as
  // duas metades da coluna com as duas hachuras vizinhas. Medindo por coluna,
  // sobra por construção.
  it("devolve a coluna ao ano contratado espremido entre dois vaos", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 40 },
          curva: curva().map((ponto) =>
            ponto.ano === 2023 || ponto.ano === 2025
              ? { ...ponto, salario_contrato: null, equipe_nome: null }
              : ponto,
          ),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    const faixas = [...grafico.querySelectorAll("[data-faixa='sem-contrato'] rect")].filter(
      (rect) => rect.getAttribute("fill") !== "none",
    );
    expect(faixas).toHaveLength(2);
    const fimDaPrimeira =
      Number(faixas[0].getAttribute("x")) + Number(faixas[0].getAttribute("width"));
    const inicioDaSegunda = Number(faixas[1].getAttribute("x"));
    // A folga entre elas é a coluna inteira de 2024, não uma sobra de desenho:
    // uma temporada vale um passo do eixo.
    const passo = 606 / 5;
    expect(inicioDaSegunda - fimDaPrimeira).toBeCloseTo(passo, 5);
    // E nessa coluna devolvida a linha do salário é CHEIA: 2024 teve contrato, e
    // pontilhar ano pago era o que estava errado.
    const cheios = grafico.querySelectorAll("[data-ponte='contratado']");
    expect(cheios.length).toBeGreaterThan(0);
    cheios.forEach((trecho) => expect(trecho.getAttribute("stroke-dasharray")).toBeNull());
  });

  // A hachura e a coluna de categoria falam dos MESMOS anos, então têm que medir
  // do mesmo jeito. Indo de marcador a marcador, a faixa acabava no meio da
  // coluna do ano em que o contrato voltou, meio passo depois de onde a coluna
  // daquele ano começa — e o desalinhamento aparecia no gráfico como duas marcas
  // discordando sobre onde um ano termina.
  it("alinha a faixa sem contrato com a coluna do ano na categoria", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 40 },
          // 2024 sem contrato, mas com categoria: o vão é só do dinheiro.
          curva: curva().map((ponto) =>
            ponto.ano === 2024
              ? { ...ponto, salario_contrato: null, equipe_nome: null }
              : ponto,
          ),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    const faixa = grafico.querySelector("[data-faixa='sem-contrato'] rect");
    const passo = 606 / 5;
    expect(Number(faixa.getAttribute("width"))).toBeCloseTo(passo, 5);

    // E a borda esquerda cai exatamente onde a coluna da gt4 termina — as duas
    // marcas usam a mesma régua, sem folga entre elas.
    const gt4 = grafico.querySelector("[data-coluna='categoria'] rect");
    const emenda = Number(gt4.getAttribute("x")) + Number(gt4.getAttribute("width"));
    expect(Number(faixa.getAttribute("x"))).toBeCloseTo(emenda, 5);
  });

  // Sem a coluna o gráfico fala de dinheiro no vácuo: $300k na escada de entrada
  // e $300k na categoria de cima são carreiras opostas.
  it("mostra a categoria de cada trecho e onde ele trocou de equipe", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 3341311, salario_estimado: 1300000, chance_transferencia: 57 },
          curva: curva(),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    // curva(): 2022-2023 na gt4 e 2024-2026 na gt3 — duas colunas, dois rótulos.
    const colunas = grafico.querySelectorAll("[data-coluna='categoria']");
    expect(colunas).toHaveLength(2);
    expect(colunas[0].getAttribute("data-categoria")).toBe("gt4");
    expect(colunas[1].getAttribute("data-categoria")).toBe("gt3");
    // O nome do degrau vai escrito na própria coluna — é isso que aposenta a
    // legenda de categorias, e por isso ele não pode depender dela.
    colunas.forEach((coluna) => expect(coluna.querySelector("text")).not.toBeNull());
    expect(grafico).toHaveTextContent(/gt4/i);
    expect(grafico).toHaveTextContent(/gt3/i);
    // E uma única troca: Sunday Speed Club -> Arclight, em 2024.
    expect(grafico.querySelectorAll("[data-marca='troca-equipe']")).toHaveLength(1);

    // Passar pela emenda diz de onde para onde — a régua sozinha conta que
    // houve mudança, não qual foi.
    fireEvent.mouseEnter(grafico.querySelector("[data-alvo='troca']"));
    const balao = screen.getByTestId("driver-detail-curve-troca-tooltip");
    expect(balao).toHaveTextContent("Sunday Speed Club");
    expect(balao).toHaveTextContent("Arclight");
    expect(balao).toHaveTextContent("2024");
    // Verbo em vez de seta, e o salário dos dois lados: $42k na Sunday Speed
    // Club contra $310k na Arclight.
    expect(balao).toHaveTextContent("Saiu");
    expect(balao).toHaveTextContent("Assinou");
    expect(balao).toHaveTextContent("+$268,000 no salário");
  });

  // Deitado, o rótulo só cabia na coluna larga, e o degrau de uma temporada só
  // caía numa legenda — o jogador tinha de procurar a cor numa lista para saber
  // em que categoria ele corria. De pé, o nome corre na altura do plot, que é a
  // mesma para toda coluna, e a largura deixa de decidir quem é nomeado.
  it("escreve o nome de toda categoria de pe na coluna, inclusive na estreita", async () => {
    // Vinte temporadas com uma passagem de um ano só pela Production no meio: a
    // coluna dela vale um vinte avos do eixo, e é ali que o rótulo deitado
    // morria e a categoria caía para a legenda.
    const longa = Array.from({ length: 20 }, (_, indice) => ({
      season_number: indice + 1,
      ano: 2006 + indice,
      categoria: indice === 9 ? "production_challenger" : indice < 9 ? "gt4" : "gt3",
      equipe_nome: "Arclight",
      equipe_cor: "#dc0000",
      salario_contrato: 80000 + indice * 60000,
      salario_mercado: 90000 + indice * 65000,
      atual: indice === 19,
    }));

    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 3341311, salario_estimado: 1300000, chance_transferencia: 57 },
          curva: longa,
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    // Três colunas — gt4, a passagem de um ano pela Production, gt3 — e as três
    // nomeadas de pé. É essa garantia que aposenta a legenda de categorias.
    const colunas = grafico.querySelectorAll("[data-coluna='categoria']");
    expect(colunas).toHaveLength(3);
    colunas.forEach((coluna) => {
      const rotulo = coluna.querySelector("[data-rotulo='categoria']");
      expect(rotulo.textContent.trim()).not.toBe("");
      expect(rotulo.getAttribute("transform")).toMatch(/^rotate\(-90 /);
    });

    // O rótulo da coluna estreita não vaza para fora dela: o recuo encolhe junto
    // com a largura, senão o nome do degrau de um ano nasceria em cima do
    // vizinho.
    const estreita = colunas[1].querySelector("rect");
    const eixo = Number(colunas[1].querySelector("[data-rotulo='categoria']").getAttribute("x"));
    expect(eixo).toBeGreaterThanOrEqual(Number(estreita.getAttribute("x")));
    expect(eixo).toBeLessThanOrEqual(
      Number(estreita.getAttribute("x")) + Number(estreita.getAttribute("width")),
    );
  });

  // A marca de troca conta que houve mudança e cala sobre entre quem — e a
  // resposta atrás de um hover é resposta que quase ninguém encontra. Com um
  // chip por vínculo, a emenda se lê parada: chip, régua, chip.
  it("mostra a logo da equipe de cada trecho ao redor da emenda", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 3341311, salario_estimado: 1300000, chance_transferencia: 57 },
          curva: curva(),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    const fita = grafico.querySelectorAll("[data-fita='equipe']");
    expect(fita).toHaveLength(2);
    expect(fita[0].getAttribute("data-equipe")).toBe("Sunday Speed Club");
    expect(fita[1].getAttribute("data-equipe")).toBe("Arclight");
    // Arte de verdade, e não o monograma de reserva: as duas equipes do caso
    // têm logo no acervo.
    fita.forEach((trecho) => expect(trecho.querySelector("image")).not.toBeNull());

    // A arte fica DENTRO do chip do próprio vínculo, e não pendurada nele:
    // solta, ela desgrudava a casa do período em que ele correu por ela.
    fita.forEach((trecho) => {
      const chip = trecho.querySelector("rect");
      const topo = Number(chip.getAttribute("y"));
      const base = topo + Number(chip.getAttribute("height"));
      const arte = trecho.querySelector("image");
      const y = Number(arte.getAttribute("y"));
      expect(y).toBeGreaterThanOrEqual(topo);
      expect(y + Number(arte.getAttribute("height"))).toBeLessThanOrEqual(base);
    });

    // A emenda fica ENTRE os dois chips: o da esquerda termina antes da régua e
    // o da direita começa depois. Sem isso a fita seria só duas logos soltas no
    // rodapé, sem dizer qual veio antes.
    const regua = grafico.querySelector("[data-marca='troca-equipe']");
    const emenda = Number(regua.getAttribute("x1"));
    const chip = (trecho) => trecho.querySelector("rect");
    const esquerdo = chip(fita[0]);
    const direito = chip(fita[1]);
    expect(
      Number(esquerdo.getAttribute("x")) + Number(esquerdo.getAttribute("width")),
    ).toBeLessThanOrEqual(emenda);
    expect(Number(direito.getAttribute("x"))).toBeGreaterThanOrEqual(emenda);

    // E cada chip veste a cor da própria casa: dois cinzas idênticos deixariam
    // a fita dizendo só "houve alguém aqui", com a identidade toda por conta de
    // uma arte de 18 unidades de largura.
    expect(esquerdo.getAttribute("fill")).not.toBe(direito.getAttribute("fill"));
    expect(esquerdo.getAttribute("fill")).toMatch(/^#[0-9a-f]{6}$/i);
    expect(esquerdo.getAttribute("fill")).not.toBe("#16232f");
  });

  // Save antigo e equipe dissolvida não trazem cor. O chip cai no neutro em vez
  // de virar um retângulo transparente no meio de vizinhos pintados.
  it("cai no chip neutro quando a equipe nao tem cor", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 40 },
          curva: curva().map(({ equipe_cor: _cor, ...ponto }) => ponto),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    grafico.querySelectorAll("[data-fita='equipe'] rect").forEach((chip) => {
      expect(chip.getAttribute("fill")).toBe("#16232f");
    });
  });

  // A regressão com que a fita nasceu: o piloto que trocou de casa sete vezes
  // via três logos, porque o trecho de uma temporada só não alcançava a largura
  // mínima e era descartado. A arte encolhe até caber — sumir apagava justamente
  // as trocas seguidas, que são a parte da carreira que a fita existe para
  // contar.
  it("mostra a logo de todo trecho, inclusive o de uma temporada so", async () => {
    const casas = [
      "Sunday Speed Club",
      "Arclight",
      "Silver Peak Performance",
      "Heart of Racing",
      "North Sea Motorsport",
      "Aures Racing",
      "Aichi Works",
      "Formosa Corsa",
    ];
    // Vinte temporadas: sete trocas em sete anos seguidos e o resto na última
    // casa — o mesmo desenho da carreira que quebrou.
    const longa = Array.from({ length: 20 }, (_, indice) => ({
      season_number: indice + 1,
      ano: 2010 + indice,
      categoria: indice < 8 ? "gt4" : "gt3",
      equipe_nome: casas[Math.min(indice, casas.length - 1)],
      salario_contrato: 50000 + indice * 40000,
      salario_mercado: 60000 + indice * 45000,
      atual: indice === 19,
    }));

    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 3341311, salario_estimado: 1300000, chance_transferencia: 57 },
          curva: longa,
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    expect(grafico.querySelectorAll("[data-marca='troca-equipe']")).toHaveLength(7);
    const fita = grafico.querySelectorAll("[data-fita='equipe']");
    expect(fita).toHaveLength(8);
    fita.forEach((trecho) => expect(trecho.querySelector("image")).not.toBeNull());
  });

  // Seis temporadas na mesma casa e uma só não podem virar a mesma marca. A logo
  // centrada dizia POR QUEM ele correu e calava sobre POR QUANTO TEMPO — a
  // duração ficava escondida na distância até a marca vizinha. O chip é a régua:
  // o comprimento dele é o período.
  it("estica o chip do vinculo pelo periodo inteiro na equipe", async () => {
    const casas = ["Arclight", "Sunday Speed Club"];
    // Dez temporadas: uma na primeira casa e nove na segunda.
    const longa = Array.from({ length: 10 }, (_, indice) => ({
      season_number: indice + 1,
      ano: 2010 + indice,
      categoria: "gt3",
      equipe_nome: casas[indice === 0 ? 0 : 1],
      salario_contrato: 100000 + indice * 50000,
      salario_mercado: 120000 + indice * 55000,
      atual: indice === 9,
    }));

    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 3341311, salario_estimado: 1300000, chance_transferencia: 57 },
          curva: longa,
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    const chips = [...grafico.querySelectorAll("[data-fita='equipe'] rect")];
    expect(chips).toHaveLength(2);

    // Nove temporadas contra uma: o chip longo vale nove passos do eixo e o
    // curto vale um. A folga entra uma vez em cada ponta dos dois.
    //
    // A estreia deixou de ser cortada ao meio quando cada temporada passou a
    // valer uma coluna INTEIRA: antes o primeiro ponto caía em cima da borda do
    // plot, e o chip dele nascia com meio passo — largura em que a arte da
    // primeira casa da carreira mal cabia.
    const passo = 606 / 10;
    const folga = 2;
    expect(Number(chips[0].getAttribute("width"))).toBeCloseTo(passo - folga * 2, 5);
    expect(Number(chips[1].getAttribute("width"))).toBeCloseTo(passo * 9 - folga * 2, 5);

    // E eles não se encostam: a folga entre os dois É a troca de equipe, agora
    // que nenhuma marca própria pousa ali.
    const fimDoPrimeiro =
      Number(chips[0].getAttribute("x")) + Number(chips[0].getAttribute("width"));
    expect(Number(chips[1].getAttribute("x")) - fimDoPrimeiro).toBeCloseTo(folga * 2, 5);

    // Os dois na mesma pista e com a mesma altura: chip mais alto que o vizinho
    // leria como "esta casa importou mais", que não é o que a fita mede.
    expect(chips[0].getAttribute("height")).toBe(chips[1].getAttribute("height"));
    expect(chips[0].getAttribute("y")).toBe(chips[1].getAttribute("y"));
  });

  // Voltar pela mesma equipe depois de um ano fora do grid não é troca — mas o
  // ano de fora PARTE o chip, porque a pergunta que o chip responde é "por quem
  // ele correu neste ano", e ali a resposta é ninguém. Um chip atravessando o
  // vão contaria uma continuidade que não houve e ainda tomaria o lugar do
  // tracejado que diz o que aconteceu.
  it("parte o chip no ano sem equipe sem contar uma troca", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 40 },
          curva: curva().map((ponto) =>
            ponto.ano === 2023
              ? { ...ponto, salario_contrato: null, equipe_nome: null }
              : { ...ponto, equipe_nome: "Arclight" },
          ),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    const fita = grafico.querySelectorAll("[data-fita='equipe']");
    expect(fita).toHaveLength(2);
    fita.forEach((trecho) => expect(trecho.getAttribute("data-equipe")).toBe("Arclight"));

    // No lugar do vão, um traço — e não um chip vazio, que teria o peso de uma
    // casa para dizer que não houve casa nenhuma.
    const lacuna = grafico.querySelectorAll("[data-fita='sem-equipe']");
    expect(lacuna).toHaveLength(1);
    expect(lacuna[0].tagName.toLowerCase()).toBe("line");
    expect(lacuna[0].getAttribute("stroke-dasharray")).toBe("3 3");

    // E nenhuma régua de troca: sair e voltar para a mesma casa não é mudar de
    // casa, por mais que a fita mostre dois chips.
    expect(grafico.querySelectorAll("[data-marca='troca-equipe']")).toHaveLength(0);
  });

  // A trilha e os anos ficam fora da faixa de alvos mas dentro do SVG: descer o
  // cursor da curva até a emenda nunca disparava a saída, e os dois balões
  // ficavam abertos ao mesmo tempo.
  it("nao deixa dois baloes abertos ao passar da curva para a emenda", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 3341311, salario_estimado: 1300000, chance_transferencia: 57 },
          curva: curva(),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    const temporada = grafico.querySelectorAll("[data-alvo='temporada']")[2];
    fireEvent.mouseEnter(temporada);
    expect(screen.getByTestId("driver-detail-curve-tooltip")).toBeInTheDocument();

    fireEvent.mouseLeave(temporada);
    fireEvent.mouseEnter(grafico.querySelector("[data-alvo='troca']"));
    expect(screen.queryByTestId("driver-detail-curve-tooltip")).toBeNull();
    expect(screen.getByTestId("driver-detail-curve-troca-tooltip")).toBeInTheDocument();
  });

  // Ano fora do grid não é troca de equipe, e voltar pela mesma equipe tampouco.
  it("nao conta o ano sem equipe como troca", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 40 },
          curva: curva().map((ponto) =>
            ponto.ano === 2023
              ? { ...ponto, salario_contrato: null, equipe_nome: null }
              : { ...ponto, equipe_nome: "Arclight" },
          ),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    expect(grafico.querySelectorAll("[data-marca='troca-equipe']")).toHaveLength(0);
  });

  it("nao desenha curva com uma temporada so", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 40 },
          curva: [curva()[0]],
        },
      }),
    );

    await abrirMercado();
    await screen.findByTestId("driver-detail-situation");
    expect(screen.queryByTestId("driver-detail-market-curve")).toBeNull();
  });

  it("oferece o mesmo dado em tabela, sem depender de cor nem de hover", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 3341311, salario_estimado: 1300000, chance_transferencia: 57 },
          curva: curva(),
        },
      }),
    );

    await abrirMercado();
    fireEvent.click(await screen.findByTestId("driver-detail-curve-toggle"));

    const tabela = screen.getByTestId("driver-detail-curve-table");
    expect(tabela).toHaveTextContent("Arclight");
    expect(tabela).toHaveTextContent("$960,534");
    expect(tabela).toHaveTextContent("$1,300,000");
  });

  // O save real trouxe dez temporadas com arquivo enxuto, e a curva desenhava
  // uma reta chapada no piso — um piloto de $1,4M "valendo" $39k por uma década.
  // Aquilo não era medição, era o default preenchendo o buraco.
  it("parte a linha de mercado nas temporadas sem arquivo em vez de chapar no piso", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 3341311, salario_estimado: 1300000, chance_transferencia: 57 },
          curva: curva().map((ponto) =>
            ponto.ano === 2024 ? { ...ponto, salario_mercado: null } : ponto,
          ),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    // Dois trechos de linha laranja, e nenhum ponto inventado no meio.
    expect(grafico.querySelectorAll("[data-serie='mercado']")).toHaveLength(2);
    // O buraco é contado em vez de sumir em silêncio — sem isso lê-se como bug.
    expect(grafico).toHaveTextContent(/Em 1 temporada o arquivo não permite reconstruir o valor/);
  });

  it("nao desenha a faixa de diferenca onde falta um dos dois lados", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 40 },
          curva: curva().map((ponto) =>
            ponto.ano === 2023 ? { ...ponto, salario_mercado: null } : ponto,
          ),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    // A faixa mede a distância entre as duas linhas; sem uma delas não há
    // distância, e preencher até o nada pintaria um bloco sem significado.
    grafico.querySelectorAll("path").forEach((faixa) => {
      expect(faixa.getAttribute("d")).not.toContain("NaN");
    });
  });

  it("afasta os rotulos da ponta quando as duas series terminam juntas", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 40 },
          // Fim empatado: com posição fixa os dois números se sobrepunham.
          curva: curva().map((ponto) =>
            ponto.atual ? { ...ponto, salario_contrato: 990000, salario_mercado: 1000000 } : ponto,
          ),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    const alturas = [...grafico.querySelectorAll("[data-rotulo='ponta']")].map((no) =>
      Number(no.getAttribute("y")),
    );
    expect(alturas).toHaveLength(2);
    expect(Math.abs(alturas[0] - alturas[1])).toBeGreaterThan(12);
  });

  it("abre um balao com equipe, logo e os dois valores ao passar o mouse", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 3341311, salario_estimado: 1300000, chance_transferencia: 57 },
          curva: curva(),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    expect(screen.queryByTestId("driver-detail-curve-tooltip")).toBeNull();

    // O alvo é a fatia inteira da temporada, não o ponto — 2024 é o índice 2.
    fireEvent.mouseEnter(grafico.querySelectorAll("[data-alvo='temporada']")[2]);

    const balao = screen.getByTestId("driver-detail-curve-tooltip");
    expect(balao).toHaveTextContent("Arclight");
    expect(within(balao).getByTestId("driver-detail-curve-tooltip-logo")).toBeInTheDocument();
    expect(balao).toHaveTextContent("$310,000");
    expect(balao).toHaveTextContent("$420,000");
    expect(balao).toHaveTextContent("+$110,000 de diferença");
    // Se o balão capturasse ponteiro, ele se fecharia ao aparecer sob o cursor.
    expect(balao.className).toContain("pointer-events-none");
  });

  it("nao inventa diferenca no balao de uma temporada sem arquivo", async () => {
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: { valor_mercado: 900000, salario_estimado: 300000, chance_transferencia: 40 },
          curva: curva().map((ponto) =>
            ponto.ano === 2024 ? { ...ponto, salario_mercado: null } : ponto,
          ),
        },
      }),
    );

    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    fireEvent.mouseEnter(grafico.querySelectorAll("[data-alvo='temporada']")[2]);

    const balao = screen.getByTestId("driver-detail-curve-tooltip");
    expect(balao).toHaveTextContent("Sem arquivo");
    expect(balao).not.toHaveTextContent(/de diferença/);
  });

  it("DUMP", async () => {
    const { writeFileSync } = await import("node:fs");
    const anos = [];
    const equipePorAno = (ano) => {
      if (ano <= 2016) return "Sunday Speed Club";
      if (ano <= 2020) return "Aures Racing";
      if (ano <= 2023) return "Arclight";
      return "Kitsune";
    };
    for (let ano = 2013; ano <= 2026; ano += 1) {
      const i = ano - 2013;
      const semEquipe = ano === 2014 || ano === 2017 || ano === 2019;
      anos.push({
        season_number: i + 1,
        ano,
        categoria: ano <= 2015 ? "mazdacup" : ano <= 2020 ? "gt4" : "gt3",
        equipe_nome: semEquipe ? null : equipePorAno(ano),
        salario_contrato: semEquipe ? null : Math.round(90000 * 1.35 ** i),
        salario_mercado: Math.round(85000 * 1.22 ** i),
        atual: ano === 2026,
      });
    }
    renderFicha(
      {},
      detail({
        contrato_mercado: {
          contrato: contrato(),
          mercado: {
            valor_mercado: 3341311,
            salario_estimado: 1300000,
            chance_transferencia: 57,
            forcas_transferencia: { contrato: 54, motivacao: 0, mercado: 3, anos_restantes: 0 },
          },
          curva: anos,
        },
      }),
    );
    await abrirMercado();
    const grafico = await screen.findByTestId("driver-detail-market-curve");
    fireEvent.mouseEnter(grafico.querySelectorAll("[data-alvo='troca']")[2]);
    writeFileSync(
      "C:/dev/Loop/__curva_preview.html",
      `<style>
        body{background:#0a1420;margin:0;padding:28px;font-family:system-ui,sans-serif;color:#c9d1d9}
        .wrap{max-width:1000px;margin:0 auto}
        svg{width:100%;display:block}
        .font-mono{font-family:ui-monospace,Menlo,monospace}
        .tabular-nums{font-variant-numeric:tabular-nums}.font-semibold{font-weight:600}
        [class*="text-[9px]"]{font-size:9px}[class*="text-[10px]"]{font-size:10px}
        [class*="text-[11px]"]{font-size:11px}[class*="text-xs"]{font-size:12px}
        [class*="fill-[#6e7681]"]{fill:#6e7681}[class*="fill-[#db6d28]"]{fill:#db6d28}
        [class*="fill-[#388bfd]"]{fill:#388bfd}[class*="fill-[#c9d1d9]"]{fill:#c9d1d9}
        [class*="fill-[#8b949e]"]{fill:#8b949e}[class*="text-[8px]"]{font-size:8px}
        [class*="uppercase"]{text-transform:uppercase}[class*="tracking-["]{letter-spacing:.1em}
        .h-2\\.5{height:10px}.w-px{width:1px}.bg-\\[\\#8b949e\\]{background:#8b949e}
        .h-2{height:8px}.w-2{width:8px}.mt-2{margin-top:8px}.pt-2{padding-top:8px}
        .h-1\\.5{height:6px}.w-1\\.5{width:6px}.rotate-45{transform:rotate(45deg)}
        .bg-\\[\\#e6edf3\\]{background:#e6edf3}
        .truncate{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.min-w-0{min-width:0}
        .w-11{width:44px}.opacity-60{opacity:.6}[class*="text-[9px]"]{font-size:9px}
        .justify-between{justify-content:space-between}.mt-1\\.5{margin-top:6px}
        [data-testid="driver-detail-curve-troca-tooltip"]{transform:translate(-50%,-112%);border:1px solid rgba(255,255,255,.1);background:#0b1622;padding:8px 12px;box-shadow:0 12px 32px rgba(0,0,0,.5);z-index:10;width:max-content;max-width:220px}
        .flex-wrap{flex-wrap:wrap}.gap-x-3{column-gap:12px}.gap-y-1{row-gap:4px}.ml-auto{margin-left:auto}
        [class*="border-white"]{border-top:1px solid rgba(255,255,255,.06)}
        .text-text-secondary{color:#8b949e}.text-text-muted{color:#6e7681}.text-text-primary{color:#e6edf3}
        .flex{display:flex}.items-baseline{align-items:baseline}.items-center{align-items:center}
        .justify-between{justify-content:space-between}.flex-wrap{flex-wrap:wrap}.relative{position:relative}
        .absolute{position:absolute}.gap-3{gap:12px}.gap-2{gap:8px}.gap-4{gap:16px}.mt-2{margin-top:8px}
        .mt-1{margin-top:4px}.mt-1\\.5{margin-top:6px}.h-2{height:8px}.w-2{width:8px}
        .rounded-sm{border-radius:2px}.rounded-lg{border-radius:8px}.flex-1{flex:1}.gap-1\\.5{gap:6px}
        .space-y-1 > * + *{margin-top:4px}.w-max{width:max-content}.pt-1\\.5{padding-top:6px}
        button{background:none;border:1px solid rgba(255,255,255,.15);border-radius:999px;color:#8b949e;padding:2px 8px;font-size:10px}
        [data-testid="driver-detail-curve-tooltip"]{border:1px solid rgba(255,255,255,.1);background:#0b1622;padding:8px 12px;box-shadow:0 12px 32px rgba(0,0,0,.5);z-index:10;max-width:240px}
        [data-testid="driver-detail-market-curve"]{background:#0f1c2b;border-radius:12px;padding:14px 16px}
        img{max-width:100%}
      </style><div class="wrap">${grafico.outerHTML}</div>`,
      "utf8",
    );
  });

  it("segura a ficha atras do aviso de lesao ate a confirmacao", async () => {
    renderFicha(
      {},
      detail({
        saude: {
          lesao_ativa: { nome: "Fratura no pulso", tipo: "moderada", corrida_ocorrida_id: "R1", corridas_total: 3, corridas_restantes: 2 },
        },
      }),
    );

    expect(await screen.findByTestId("driver-detail-injury")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "OK" }));
    expect(screen.queryByTestId("driver-detail-injury")).not.toBeInTheDocument();
  });
});
