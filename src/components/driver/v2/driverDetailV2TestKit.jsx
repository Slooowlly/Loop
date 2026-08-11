import { fireEvent, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import { DriverDetailModalV2 } from "./DriverDetailModalV2";

// Os dados e os atalhos que os testes da ficha v2 compartilham.
//
// Saiu de `DriverDetailModalV2.test.jsx` em 11/08/2026, quando o arquivo foi
// fatiado por seção do modal — eram 4.482 linhas num describe só, 16% da suíte de
// UI, e rodar um caso custava o arquivo inteiro. Os `vi.mock` NÃO moram aqui:
// eles valem por arquivo de teste, e cada fatia declara os seus.
//
// Nada aqui afirma nada. Um helper que precisasse de `expect` viraria teste, e
// estaria no arquivo errado.

export function detail(overrides = {}) {
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
export function renderFicha(props = {}, payload = detail(), worldRank = null) {
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
export async function abrirTemporada() {
  await screen.findByTestId("driver-detail-hero");
  fireEvent.click(screen.getByTestId("driver-detail-tab-temporada"));
}

export async function abrirPerfil() {
  await screen.findByTestId("driver-detail-hero");
  fireEvent.click(screen.getByTestId("driver-detail-tab-perfil"));
}

export async function abrirRivais() {
  await screen.findByTestId("driver-detail-hero");
  fireEvent.click(screen.getByTestId("driver-detail-tab-rivais"));
}

export async function abrirMercado() {
  await screen.findByTestId("driver-detail-hero");
  fireEvent.click(screen.getByTestId("driver-detail-tab-mercado"));
}

// Cinco temporadas subindo a escada: salário atrás do que o mercado pagaria.
// A carreira em posicao de campeonato: seis temporadas, dois titulos (2021 na
// gt4, 2023 na gt3) e a de 2024 ainda em disputa. O grid muda de tamanho junto
// com a categoria — 24 na escada de entrada, 18 na gt3 — que e justamente o que
// o chao do grafico existe para mostrar.
export function curvaCampeonato() {
  return [
    { season_number: 1, ano: 2019, categoria: "gt4", equipe_nome: "Thunderline Academy", equipe_cor: "#3fb950", posicao: 9, grid: 24, esperado: 11, pontos: 88, vitorias: 0, podios: 1, corridas: 12, titulo: false, atual: false },
    { season_number: 2, ano: 2020, categoria: "gt4", equipe_nome: "Thunderline Academy", equipe_cor: "#3fb950", posicao: 4, grid: 24, esperado: 6, pontos: 160, vitorias: 1, podios: 5, corridas: 12, titulo: false, atual: false },
    { season_number: 3, ano: 2021, categoria: "gt4", equipe_nome: "Aures Racing", equipe_cor: "#3fb950", posicao: 1, grid: 22, esperado: 3, pontos: 240, vitorias: 6, podios: 10, corridas: 12, titulo: true, atual: false },
    { season_number: 4, ano: 2022, categoria: "gt3", equipe_nome: "Aures Racing", equipe_cor: "#3fb950", posicao: 12, grid: 18, esperado: 8, pontos: 54, vitorias: 0, podios: 0, corridas: 14, titulo: false, atual: false },
    { season_number: 5, ano: 2023, categoria: "gt3", equipe_nome: "Ferrari", equipe_cor: "#dc0000", posicao: 1, grid: 18, esperado: 2, pontos: 310, vitorias: 7, podios: 12, corridas: 14, titulo: true, atual: false },
    { season_number: 6, ano: 2024, categoria: "gt3", equipe_nome: "Ferrari", equipe_cor: "#dc0000", posicao: 3, grid: 18, esperado: 4, pontos: 96, vitorias: 2, podios: 4, corridas: 6, titulo: false, atual: true },
  ];
}

export function curva() {
  return [
    { season_number: 1, ano: 2022, categoria: "gt4", equipe_nome: "Sunday Speed Club", equipe_cor: "#3fb950", salario_contrato: 42000, salario_mercado: 60000, atual: false },
    { season_number: 2, ano: 2023, categoria: "gt4", equipe_nome: "Sunday Speed Club", equipe_cor: "#3fb950", salario_contrato: 42000, salario_mercado: 110000, atual: false },
    { season_number: 3, ano: 2024, categoria: "gt3", equipe_nome: "Arclight", equipe_cor: "#dc0000", salario_contrato: 310000, salario_mercado: 420000, atual: false },
    { season_number: 4, ano: 2025, categoria: "gt3", equipe_nome: "Arclight", equipe_cor: "#dc0000", salario_contrato: 960534, salario_mercado: 980000, atual: false },
    { season_number: 5, ano: 2026, categoria: "gt3", equipe_nome: "Arclight", equipe_cor: "#dc0000", salario_contrato: 960534, salario_mercado: 1300000, atual: true },
  ];
}

export function contrato(overrides = {}) {
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
export function rival(overrides = {}) {
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
export function fingeQueRola(rola) {
  Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
    configurable: true,
    get: () => (rola ? 640 : 320),
  });
  Object.defineProperty(HTMLElement.prototype, "clientHeight", {
    configurable: true,
    get: () => 320,
  });
}

export function restauraLayout() {
  delete HTMLElement.prototype.scrollHeight;
  delete HTMLElement.prototype.clientHeight;
}
