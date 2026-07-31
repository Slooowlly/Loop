import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import GlobalTeamsTabV2 from "./GlobalTeamsTabV2";
import { ATLAS_CATEGORY_LOGOS } from "../../../components/team/v2/atlasCategoryLogos";
import { LOGO_FRAME_HEIGHT, LOGO_FRAME_WIDTH } from "../../../components/team/v2/atlasLogoNormalization";

let mockState = {};

vi.mock("../../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("../../../components/team/TeamLogoMark", () => ({
  default: ({ teamName, testId = "standings-team-logo" }) => (
    <span data-testid={testId}>{teamName} logo</span>
  ),
}));

vi.mock("../../../components/team/history", () => ({
  TeamHistoryDrawer: ({ team, onClose }) => (
    <aside data-testid="team-history-drawer">
      <h4>{team.nome}</h4>
      <button type="button" onClick={onClose}>Fechar dossie</button>
    </aside>
  ),
}));

// ---------------------------------------------------------------------------
// Geometria derivada deste fixture (usada nas asserções abaixo):
//   firstSeriesYear = primeiro ano com ponto de verdade            = 2020
//   displayStart    = ano fixado da familia Mazda                  = 2014
//   displayEnd      = ultima temporada disputada                   = 2025
//   years           = [2014..2025] → 12 colunas, sem cauda futura
//
// Em jsdom o getBoundingClientRect devolve zero, então useElementSize mantém o
// fallback de 960x520. A timeline usa a largura INTEIRA: 960 / 12 = 80px por ano,
// e os limites caem em múltiplos de 80 a partir de zero.
// ---------------------------------------------------------------------------
const payload = {
  selected_family: "mazda",
  min_year: 2000,
  max_year: 2025,
  current_year: 2025,
  window_start: 2000,
  window_end: 2025,
  window_size: 26,
  families: [
    { id: "mazda", label: "Mazda", bands: [] },
    { id: "toyota", label: "Toyota", bands: [] },
  ],
  bands: [
    {
      key: "production_mazda",
      label: "Mazda Production",
      category: "production_challenger",
      class_name: "mazda",
      starts_year: 2018,
      is_special: false,
      rows: [
        {
          team_id: "T001",
          nome: "Kestrel",
          nome_curto: "KST",
          cor_primaria: "#ff6b6b",
          cor_secundaria: "#70141d",
          base_position: 1,
          titles: [{ band_key: "production_mazda", band_label: "Mazda Production", count: 8 }],
          is_reigning_champion: true,
          points: [
            { year: 2024, slot: "regular", position: 2, points: 100, wins: 3, titles: 0 },
            { year: 2025, slot: "regular", position: 1, points: 120, wins: 6, titles: 1 },
          ],
        },
        {
          team_id: "T002",
          nome: "Aperture",
          nome_curto: "APT",
          cor_primaria: "#38bdf8",
          cor_secundaria: "#0b2545",
          base_position: 2,
          titles: [],
          is_reigning_champion: false,
          points: [
            // Fundadora: e o primeiro ponto de toda a familia, em 2020.
            { year: 2020, slot: "regular", position: 1, points: 95, wins: 4, titles: 1 },
            { year: 2024, slot: "regular", position: 1, points: 110, wins: 5, titles: 1 },
            { year: 2025, slot: "regular", position: 2, points: 105, wins: 2, titles: 0 },
          ],
        },
        {
          team_id: "T003",
          nome: "Velocity Prep Motorsport",
          nome_curto: "VPM",
          cor_primaria: "#c084fc",
          cor_secundaria: "#2a0f45",
          base_position: 3,
          titles: [],
          is_reigning_champion: false,
          points: [
            // Segunda fundadora da Production, para a coluna de estreia ter grupo.
            { year: 2020, slot: "regular", position: 4, points: 40, wins: 0, titles: 0 },
            { year: 2025, slot: "regular", position: 3, points: 80, wins: 0, titles: 0 },
          ],
        },
      ],
    },
    {
      key: "mazda_rookie",
      label: "Mazda Rookie",
      category: "mazda_rookie",
      class_name: null,
      starts_year: 2020,
      is_special: false,
      rows: [
        {
          team_id: "T004",
          nome: "Amateur Hour Racing",
          nome_curto: "AHR",
          cor_primaria: "#e5e7eb",
          cor_secundaria: "#111827",
          base_position: 1,
          titles: [],
          is_reigning_champion: false,
          points: [
            { year: 2024, slot: "regular", position: 1, points: 90, wins: 3, titles: 0 },
            { year: 2025, slot: "regular", position: 1, points: 95, wins: 4, titles: 0 },
          ],
        },
        {
          team_id: "T005",
          nome: "Grid Start Racing School",
          nome_curto: "GSR",
          cor_primaria: "#f2c46d",
          cor_secundaria: "#3a2610",
          base_position: 2,
          // Bicampea da Production, mas correndo na Rookie: nao pode exibir trofeu.
          titles: [{ band_key: "production_mazda", band_label: "Mazda Production", count: 2 }],
          is_reigning_champion: false,
          points: [
            { year: 2024, slot: "regular", position: 3, points: 60, wins: 0, titles: 0 },
            { year: 2025, slot: "regular", position: 2, points: 70, wins: 1, titles: 0 },
          ],
        },
      ],
    },
  ],
};

async function renderAtlas(props = {}) {
  const utils = render(<GlobalTeamsTabV2 onBack={vi.fn()} {...props} />);
  await screen.findByTestId("atlas-v2-chart");
  return utils;
}

describe("GlobalTeamsTabV2", () => {
  beforeEach(() => {
    mockState = { careerId: "career-1" };
    invoke.mockReset();
    invoke.mockResolvedValue(payload);
    // A dica de primeira vez persiste no localStorage: sem limpar, o primeiro
    // teste que a dispensa apagaria a dica de todos os seguintes.
    localStorage.clear();
  });

  it("monta cabecalho, grafico, rankings, legenda e rodape dentro de uma moldura unica", async () => {
    await renderAtlas();

    const shell = screen.getByTestId("atlas-v2-shell");
    expect(shell).toBeInTheDocument();
    expect(within(shell).getByTestId("atlas-v2-chart")).toBeInTheDocument();
    expect(within(shell).getByTestId("atlas-v2-rankings")).toBeInTheDocument();
    expect(within(shell).getByTestId("atlas-v2-legend")).toBeInTheDocument();
    expect(screen.getByText(/Mazda: janela 2014-2025/i)).toBeInTheDocument();
    expect(screen.getByText(/Temporada atual/i)).toBeInTheDocument();
  });

  it("mostra tres anos pre-serie e nenhum ano futuro, e anuncia esse intervalo", async () => {
    await renderAtlas();

    // A serie estreia em 2020; o eixo abre em 2014 (ano fixado da familia) para dar
    // lugar real ao periodo pre-serie. O titulo acompanha o intervalo EXIBIDO.
    expect(screen.getByText(/Mazda: janela 2014-2025/i)).toBeInTheDocument();
    expect(screen.getByTestId("atlas-v2-year-2014")).toBeInTheDocument();
    expect(screen.getByTestId("atlas-v2-year-2025")).toBeInTheDocument();
    expect(screen.queryByTestId("atlas-v2-year-2013")).not.toBeInTheDocument();
    expect(screen.queryByTestId("atlas-v2-year-2026")).not.toBeInTheDocument();
  });

  it("hachura a altura inteira do grafico nos anos anteriores a serie", async () => {
    await renderAtlas();

    const hatch = screen.getByTestId("atlas-v2-pre-series");

    // 6 colunas de 80px a partir de zero: 2014..2019.
    expect(parseFloat(hatch.style.left)).toBe(0);
    expect(parseFloat(hatch.style.width)).toBeCloseTo(480, 1);
    // inset-y-0 = altura inteira, atravessando os tres campeonatos.
    expect(hatch.className).toContain("inset-y-0");
  });

  it("alinha cabecalho, grade e pontos na mesma regua", async () => {
    await renderAtlas();

    // Celula do ano: abre no limite e mede exatamente uma coluna.
    const year2020 = screen.getByTestId("atlas-v2-year-2020");
    expect(parseFloat(year2020.style.left)).toBeCloseTo(480, 1);
    expect(parseFloat(year2020.style.width)).toBeCloseTo(80, 1);

    // O primeiro ponto da fundadora cai na divisa 2019|2020 — o mesmo X onde a
    // hachura pre-serie termina e onde a celula de 2020 comeca.
    const founder = screen.getByTestId("atlas-v2-track-T002-regular").getAttribute("d");
    expect(founder.startsWith("M 480 ")).toBe(true);
  });

  it("monta a moldura direto no body, fora de qualquer ancestral animado", async () => {
    await renderAtlas();

    // O `.tab-pane-fade` do Dashboard anima transform, e um ancestral com transform
    // vira o bloco de contencao de `position: fixed` — a moldura abriria dentro do
    // main, com padding e max-width, e so pularia para a viewport no fim da animacao.
    const layer = screen.getByTestId("atlas-v2-layer");
    expect(layer.parentElement).toBe(document.body);
    // No body, a moldura disputa z-index com o backdrop do TeamHistoryOverlay (80)
    // e com o TeamHistoryDrawer (90/91) — precisa ficar exatamente entre os dois.
    expect(layer.className).toContain("z-[85]");
  });

  it("poe plotagem e rankings na MESMA linha do grid, sem recuo manual", async () => {
    await renderAtlas();

    const plot = screen.getByTestId("atlas-v2-plot");
    const rankings = screen.getByTestId("atlas-v2-rankings");
    const yearHeader = screen.getByTestId("atlas-v2-chart");

    // A faixa dos anos ocupa a linha 1; plotagem e rankings dividem a linha 2. E o
    // grid que lhes da a mesma origem vertical.
    expect(yearHeader.style.gridRow).toBe("1");
    expect(plot.style.gridRow).toBe("2");
    expect(rankings.style.gridRow).toBe("2");
    expect(plot.style.gridColumn).toBe("1");
    expect(rankings.style.gridColumn).toBe("2");

    // Nenhum dos dois pode compensar a barra dos anos por conta propria: um recuo
    // manual aqui foi o que produziu o descolamento de 34px. Pior: os cards sao
    // absolutos, entao um padding-top nem sequer os moveria.
    expect(rankings.style.paddingTop).toBe("");
    expect(rankings.style.marginTop).toBe("");
    expect(plot.style.paddingTop).toBe("");
    expect(plot.style.marginTop).toBe("");
  });

  it("alinha o centro da linha do card com o Y da equipe no grafico", async () => {
    await renderAtlas();

    // T001 termina 2025 em P1 na Production. O terminal da linha e o centro da
    // linha do card tem de cair no MESMO Y — e o que a regua vertical unica
    // garante. Antes, a altura do cabecalho do card nao entrava na conta do lado
    // do grafico e o desalinhamento crescia a cada card.
    const chart = screen.getByTestId("atlas-v2-chart");
    const card = screen.getByTestId("atlas-v2-ranking-production_mazda");
    const row = screen.getByTestId("atlas-v2-ranking-row-T001").parentElement;

    // Y do terminal da linha, no espaco do grafico.
    const d = screen.getByTestId("atlas-v2-track-T001-regular").getAttribute("d");
    const terminalY = Number(d.split("L").pop().trim().split(" ")[1]);

    // Y do centro da linha do card, no mesmo espaco: topo do card + topo da linha
    // + metade da altura da linha.
    const centroNoCard =
      parseFloat(card.style.top) + parseFloat(row.style.top) + parseFloat(row.style.height) / 2;

    expect(centroNoCard).toBeCloseTo(terminalY, 1);
    expect(chart).toBeInTheDocument();
  });

  it("liga a linha ao card com um rabicho no vao entre as colunas", async () => {
    await renderAtlas();

    const connector = screen.getByTestId("atlas-v2-row-connector-T001");
    expect(connector.style.right).toBe("100%");
    expect(parseFloat(connector.style.width)).toBeGreaterThan(0);
  });

  it("mantem os rankings FORA do grafico, em coluna propria", async () => {
    await renderAtlas();

    const chart = screen.getByTestId("atlas-v2-chart");
    const rankings = screen.getByTestId("atlas-v2-rankings");

    // O ponto central do redesenho: no v1 as tabelas eram filhas absolutas do
    // grafico; aqui elas nao podem estar contidas nele de forma alguma.
    expect(chart.contains(rankings)).toBe(false);
    expect(rankings.contains(chart)).toBe(false);
    expect(rankings.className).not.toContain("absolute");
  });

  it("da um card independente por campeonato, com o ano de referencia no titulo", async () => {
    await renderAtlas();

    const production = screen.getByTestId("atlas-v2-ranking-production_mazda");
    const rookie = screen.getByTestId("atlas-v2-ranking-mazda_rookie");

    // O ano de referencia faz parte do proprio titulo, sem selo separado.
    expect(within(production).getByText("Mazda Production 2025")).toBeInTheDocument();
    expect(within(rookie).getByText("Mazda Rookie 2025")).toBeInTheDocument();
    expect(production.contains(rookie)).toBe(false);
  });

  it("mostra so os trofeus da categoria em que a equipe esta", async () => {
    await renderAtlas();

    // T001 tem 8 titulos na propria Production → trofeu com a contagem.
    const champion = screen.getByTestId("atlas-v2-ranking-row-T001");
    expect(within(champion).getByTitle(/8 t.tulos na Mazda Production/i)).toHaveTextContent("8");

    // T005 corre na Rookie e seus 2 titulos sao da Production: nada ao lado do
    // nome. Se for promovida, o trofeu aparece la — e some se voltar.
    const foraDaDivisao = screen.getByTestId("atlas-v2-ranking-row-T005");
    expect(within(foraDaDivisao).queryByTitle(/t.tulos? na/i)).not.toBeInTheDocument();

    // T002 nunca ganhou nada: coluna vazia, sem traco nem seta.
    const semTitulo = screen.getByTestId("atlas-v2-ranking-row-T002");
    expect(within(semTitulo).queryByTitle(/t.tulos? na/i)).not.toBeInTheDocument();

    // As setas de variacao sairam de vez — nenhuma linha as exibe.
    expect(screen.queryByTitle(/Subida de posi..o/i)).not.toBeInTheDocument();
    expect(screen.queryByTitle(/Queda de posi..o/i)).not.toBeInTheDocument();
    expect(screen.queryByTitle(/Sem mudan.a/i)).not.toBeInTheDocument();
  });

  it("desenha as linhas dentro da largura medida da plotagem, com um ponto por ano", async () => {
    await renderAtlas();

    const path = screen.getByTestId("atlas-v2-track-T001-regular");
    const d = path.getAttribute("d");

    // Timeline de 960px em 12 colunas → 80px por ano, limites a partir de zero.
    // T001 abre em 2024 (indice 10 → 800), segue para 2025 (indice 11 → 880) e
    // fecha no FIM de 2025, que e a borda direita do grafico.
    const xs = [...d.matchAll(/[ML] (\d+(?:\.\d+)?)/g)].map((match) => Number(match[1]));
    expect(xs).toEqual([800, 880, 960]);
    expect(Math.max(...xs)).toBe(960);

    // Um circulo por temporada — o vertice de fechamento nao vira ponto. Os pontos
    // vivem numa camada propria, acima dos conectores e abaixo das etiquetas.
    const points = screen.getByTestId("atlas-v2-points-T001-regular");
    expect(points.querySelectorAll("circle")).toHaveLength(2);
    // O primeiro e o marcador de estreia (anel vazado).
    expect(screen.getByTestId("atlas-v2-entry-point-T001-regular")).toHaveAttribute("stroke-width", "1.5");
  });

  it("alinha numa coluna as etiquetas fundadoras do mesmo campeonato", async () => {
    await renderAtlas();

    // T002 e T003 estreiam juntas na Production em 2020: mesma borda direita e
    // mesma largura, formando um bloco alinhado.
    const fundadoras = ["T002", "T003"].map((id) => screen.getByTestId(`atlas-v2-entry-label-${id}`));

    expect(fundadoras[0].style.width).toBe(fundadoras[1].style.width);
    expect(fundadoras[0].style.left).toBe(fundadoras[1].style.left);
    expect(fundadoras[0].style.top).not.toBe(fundadoras[1].style.top);
  });

  it("so mostra a etiqueta de quem entrou no meio do caminho no hover da linha", async () => {
    await renderAtlas();

    // Fundadora: sempre na tela.
    expect(screen.getByTestId("atlas-v2-entry-label-T002")).toBeInTheDocument();
    // T001 estreou em 2024, ja com a serie em andamento: fica escondida.
    expect(screen.queryByTestId("atlas-v2-entry-label-T001")).not.toBeInTheDocument();

    fireEvent.mouseEnter(screen.getByTestId("atlas-v2-track-T001-regular"));
    expect(screen.getByTestId("atlas-v2-entry-label-T001")).toBeInTheDocument();

    // E sai de cena quando o destaque passa para outra equipe.
    fireEvent.mouseEnter(screen.getByTestId("atlas-v2-track-T005-regular"));
    expect(screen.queryByTestId("atlas-v2-entry-label-T001")).not.toBeInTheDocument();
  });

  it("mostra a etiqueta da equipe fixada em analise sem precisar de hover", async () => {
    await renderAtlas({ pinnedTeamId: "T001" });

    expect(screen.getByTestId("atlas-v2-entry-label-T001")).toBeInTheDocument();
  });

  it("mantem visivel a formacao inaugural de CADA categoria, nao so da familia", async () => {
    await renderAtlas();

    // A Rookie so comeca em 2024, quatro anos depois da familia. T004 e T005 sao a
    // formacao inaugural DELA — sao fundadoras e ficam na tela, mesmo estreando
    // muito depois do primeiro ano da familia.
    expect(screen.getByTestId("atlas-v2-entry-label-T004")).toBeInTheDocument();
    expect(screen.getByTestId("atlas-v2-entry-label-T005")).toBeInTheDocument();

    // Ja T001 chegou a Production em 2024, com a categoria rodando desde 2020.
    expect(screen.queryByTestId("atlas-v2-entry-label-T001")).not.toBeInTheDocument();
  });

  it("nomeia cada campeonato dentro do proprio grafico, na cor do degrau", async () => {
    await renderAtlas();

    // O nome fica na faixa vazia acima da primeira posicao — sem ele, saber que
    // faixa e qual exigia atravessar a tela ate a coluna da direita.
    const production = screen.getByTestId("atlas-v2-band-title-production_mazda");
    const rookie = screen.getByTestId("atlas-v2-band-title-mazda_rookie");
    expect(production).toHaveTextContent("Mazda Production");
    expect(rookie).toHaveTextContent("Mazda Rookie");

    // A cor vem do degrau da escada, e e a MESMA no card lateral do campeonato.
    const corDoTitulo = (node) => node.querySelector("span:last-child").style.color;
    expect(corDoTitulo(production)).not.toBe(corDoTitulo(rookie));
    expect(screen.getByTestId("atlas-v2-ranking-title-mazda_rookie").style.color).toBe(corDoTitulo(rookie));
  });

  it("centraliza o titulo sobre a coluna de etiquetas da propria faixa", async () => {
    await renderAtlas();

    // Production estreia em 2020 e Rookie em 2024: as duas colunas de etiquetas
    // ficam em x diferentes, e cada titulo acompanha a SUA. Alinhar os dois pela
    // borda do grafico deixaria o titulo longe da lista que ele nomeia.
    const centroDoTitulo = (bandKey) => {
      const node = screen.getByTestId(`atlas-v2-band-title-${bandKey}`);
      // Ancorado pelo centro: left e o centro e o transform desloca meia largura.
      expect(node.style.transform).toBe("translateX(-50%)");
      return Number.parseFloat(node.style.left);
    };
    // Ja a etiqueta e ancorada pela DIREITA (left + translateX(-100%)), entao o
    // centro visivel dela e left - width / 2.
    const centroDaEtiqueta = (teamId) => {
      const chip = screen.getByTestId(`atlas-v2-entry-label-${teamId}`);
      return Number.parseFloat(chip.style.left) - Number.parseFloat(chip.style.width) / 2;
    };

    expect(centroDoTitulo("production_mazda")).toBeCloseTo(centroDaEtiqueta("T002"), 0);
    expect(centroDoTitulo("mazda_rookie")).toBeCloseTo(centroDaEtiqueta("T004"), 0);
    expect(centroDoTitulo("production_mazda")).not.toBeCloseTo(centroDoTitulo("mazda_rookie"), 0);

    // E o titulo nao pode ser mais largo que a coluna: fora dela ele vazaria por
    // cima das linhas do grafico. Nome longo diminui a FONTE, nunca abrevia — nome
    // de categoria cortado ("BMW Producti...") nao identifica campeonato nenhum.
    const titulo = screen.getByTestId("atlas-v2-band-title-production_mazda");
    const chip = screen.getByTestId("atlas-v2-entry-label-T002");
    expect(Number.parseFloat(titulo.style.width)).toBeLessThanOrEqual(Number.parseFloat(chip.style.width));
    const textoDoTitulo = titulo.querySelector("span");
    expect(textoDoTitulo.className).toContain("whitespace-nowrap");
    expect(textoDoTitulo.className).not.toContain("truncate");
    expect(Number.parseFloat(textoDoTitulo.style.fontSize)).toBeLessThanOrEqual(13);
  });

  it("abre o salao dos campeoes pelo trofeu da categoria", async () => {
    const champions = {
      band_key: "mazda_rookie",
      band_label: "Mazda Rookie",
      dynasties: [
        { team_id: "T004", nome: "Kestrel", cor_primaria: "#ff4d4d", titles: 2, last_year: 2025 },
        { team_id: "T005", nome: "Aperture", cor_primaria: "#38bdf8", titles: 1, last_year: 2024 },
      ],
      seasons: [
        {
          year: 2025,
          team_id: "T004",
          nome: "Kestrel",
          cor_primaria: "#ff4d4d",
          wins: 9,
          drivers: [
            { driver_id: "D1", nome: "Lucien Moreau", is_season_champion: true },
            { driver_id: "D2", nome: "Rui Okafor", is_season_champion: false },
          ],
        },
        // Campea de construtores sem o campeao de pilotos: a marca nao aparece.
        { year: 2024, team_id: "T005", nome: "Aperture", cor_primaria: "#38bdf8", wins: 6, drivers: [] },
      ],
    };
    await renderAtlas();

    invoke.mockImplementation((command) =>
      command === "get_band_champions" ? Promise.resolve(champions) : Promise.resolve(payload),
    );
    fireEvent.click(screen.getByTestId("atlas-v2-champions-open-mazda_rookie"));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("get_band_champions", {
        careerId: "career-1",
        bandKey: "mazda_rookie",
      }),
    );

    const painel = await screen.findByTestId("atlas-v2-champions");
    expect(within(painel).getByTestId("atlas-v2-champion-2025")).toHaveTextContent("Lucien Moreau");
    // A estrela distingue o campeao de pilotos do resto da dupla.
    expect(within(screen.getByTestId("atlas-v2-champion-2025")).getByTitle(/campe/i)).toBeInTheDocument();
    // Sem dupla registrada, a celula do piloto fica com o travessao.
    expect(screen.getByTestId("atlas-v2-champion-2024")).toHaveTextContent("—");
    // E o podio de dinastias resume quem manda na categoria.
    expect(within(painel).getByTestId("atlas-v2-champions-dynasties")).toHaveTextContent("2");
  });

  it("fecha o salao dos campeoes com Escape", async () => {
    await renderAtlas();
    invoke.mockResolvedValue({ band_key: "mazda_rookie", band_label: "Mazda Rookie", dynasties: [], seasons: [] });

    fireEvent.click(screen.getByTestId("atlas-v2-champions-open-mazda_rookie"));
    await screen.findByTestId("atlas-v2-champions");

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(screen.queryByTestId("atlas-v2-champions")).not.toBeInTheDocument());
  });

  it("recarrega o payload ao trocar a familia", async () => {
    await renderAtlas();

    invoke.mockClear();
    fireEvent.click(screen.getByRole("button", { name: /Toyota/i }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("get_global_team_history", {
        careerId: "career-1",
        family: "toyota",
        startYear: 2000,
        windowSize: 32,
      }),
    );
  });

  it("abre o dossie ao clicar numa linha do ranking e fecha sem derrubar o atlas", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      await renderAtlas();

      fireEvent.click(screen.getByTestId("atlas-v2-ranking-row-T001"));
      vi.advanceTimersByTime(250);

      const drawer = await screen.findByTestId("team-history-drawer");
      expect(within(drawer).getByText("Kestrel")).toBeInTheDocument();

      fireEvent.click(within(drawer).getByRole("button", { name: /Fechar dossie/i }));
      await waitFor(() => expect(screen.queryByTestId("team-history-drawer")).not.toBeInTheDocument());
      expect(screen.getByTestId("atlas-v2-chart")).toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("separa campeonatos com sulco e cabecalho tingido, nao com uma regua fina", async () => {
    await renderAtlas();

    // O sulco ocupa o VAO INTEIRO entre as duas faixas — uma regua de 2px no meio
    // dele se perdia entre as linhas coloridas.
    const trench = screen.getByTestId("atlas-v2-band-rule-mazda_rookie");
    expect(parseFloat(trench.style.height)).toBe(12);
    // E cada campeonato abre com a faixa tingida na cor dele, igual ao cabecalho do
    // card lateral: e a mesma moldura dos dois lados do vao.
    const header = screen.getByTestId("atlas-v2-band-header-mazda_rookie");
    expect(header.style.background).toContain("color-mix");
    expect(header.style.borderBottom).toContain("color-mix");
    // A primeira faixa nao tem sulco acima: nao ha o que separar do topo do card.
    expect(screen.queryByTestId("atlas-v2-band-rule-production_mazda")).not.toBeInTheDocument();
  });

  it("anuncia o salao dos campeoes no cabecalho inteiro, nao so no brasao", async () => {
    await renderAtlas();

    // O alvo de clique e a faixa toda: 22px de simbolo nao davam nem alvo nem sinal,
    // e a tela dos campeoes ficava sem como ser descoberta.
    const header = screen.getByTestId("atlas-v2-champions-open-mazda_rookie");
    expect(header.tagName).toBe("BUTTON");
    expect(within(header).getByTestId("atlas-v2-ranking-title-mazda_rookie")).toBeInTheDocument();
    expect(within(header).getByTestId("atlas-v2-champions-chevron-mazda_rookie")).toBeInTheDocument();

    fireEvent.click(header);
    expect(await screen.findByTestId("atlas-v2-champions")).toBeInTheDocument();
  });

  it("guarda o chevron para o cabecalho, sem repeti-lo em cada linha", async () => {
    await renderAtlas();

    // Um chevron por linha era dez chevrons por card: repetido em toda linha ele
    // vira ruido e para de significar "continua para la". O sinal fica so no
    // cabecalho, e o realce no hover e o que anuncia o clique na linha.
    const row = screen.getByTestId("atlas-v2-ranking-row-T001");
    expect(within(row).queryByTestId("atlas-v2-row-chevron")).not.toBeInTheDocument();
    expect(row.className).toContain("cursor-pointer");
  });

  it("abre o cabecalho com o brasao da categoria, nao com um trofeu generico", async () => {
    await renderAtlas();

    // O trofeu dizia so "campeonato", que e o que o titulo ja diz. O brasao e
    // reconhecimento imediato da divisao, antes mesmo da leitura do nome.
    const header = screen.getByTestId("atlas-v2-champions-open-mazda_rookie");
    const logo = within(header).getByTestId("atlas-v2-category-logo-mazda_rookie");
    expect(logo.tagName).toBe("IMG");
    expect(logo.getAttribute("src")).toContain("MX5%20ROOKIE");
    // Decorativo: quem nomeia o destino e o aria-label do proprio cabecalho.
    expect(logo.getAttribute("alt")).toBe("");

    // Moldura de tamanho FIXO, igual em todo card: as marcas vao de quadradas a
    // muito largas, e altura fixa com largura livre fazia cada brasao sair de um
    // tamanho. A imagem se ajusta dentro da caixa, nunca a caixa a imagem.
    const frame = within(header).getByTestId("atlas-v2-category-logo-frame-mazda_rookie");
    expect(frame.style.width).toBe(`${LOGO_FRAME_WIDTH}px`);
    expect(frame.style.height).toBe(`${LOGO_FRAME_HEIGHT}px`);

    // E a mesma caixa em todos: dois cards com marcas de formatos diferentes tem
    // de reservar exatamente o mesmo espaco.
    const outro = screen.getByTestId("atlas-v2-category-logo-frame-production_challenger");
    expect(outro.style.width).toBe(frame.style.width);
    expect(outro.style.height).toBe(frame.style.height);

    // Sem medida (jsdom nunca carrega a imagem), o encaixe simples segura a barra:
    // pior que a normalizacao, mas nunca transborda.
    expect(logo.className).toContain("object-contain");
    expect(logo.className).toContain("max-h-full");
    expect(logo.className).toContain("max-w-full");

    // E fica INVISIVEL enquanto nao ha resposta. O encaixe simples e maior que o
    // normalizado, entao mostra-lo durante a espera dava um salto de tamanho na
    // troca de familia: aparecia o brasao grande por um frame e ele encolhia.
    expect(logo.className).toContain("invisible");
  });

  it("anuncia a saida com rotulo visivel, nao com um icone mudo", async () => {
    // O Atlas cobre a viewport inteira e nao tem outra saida na tela. Um icone de
    // 28px com o rotulo escondido no title e a diferenca entre sair daqui e ficar
    // preso — o rotulo tem de ser legivel em repouso, sem hover.
    const onBack = vi.fn();
    await renderAtlas({ onBack });

    const voltar = screen.getByTestId("atlas-v2-back");
    expect(voltar).toHaveTextContent(/voltar/i);

    fireEvent.click(voltar);
    expect(onBack).toHaveBeenCalled();
  });

  it("fecha o Atlas no Esc, mas so quando ele e a camada de cima", async () => {
    const onBack = vi.fn();
    await renderAtlas({ onBack });

    // Com o salao dos campeoes aberto, o Esc e DELE: fecha o painel e o Atlas fica.
    fireEvent.click(screen.getByTestId("atlas-v2-champions-open-mazda_rookie"));
    expect(await screen.findByTestId("atlas-v2-champions")).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onBack).not.toHaveBeenCalled();

    // Fechado o painel, o Esc volta a ser do Atlas.
    await waitFor(() => expect(screen.queryByTestId("atlas-v2-champions")).not.toBeInTheDocument());
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it("nao fecha o Atlas no Esc com o dossie aberto", async () => {
    const onBack = vi.fn();
    await renderAtlas({ onBack });

    fireEvent.click(screen.getByTestId("atlas-v2-ranking-row-T001"));
    expect(await screen.findByTestId("team-history-drawer")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(onBack).not.toHaveBeenCalled();
  });

  it("nao calibra tamanho de brasao na mao, marca por marca", async () => {
    await renderAtlas();

    // Alturas fixas por categoria acertavam um arquivo e erravam o proximo. O
    // tamanho e medido em runtime a partir do conteudo visivel — o mapa so aponta
    // o caminho do arquivo, e arquivo novo entra sabendo se comportar.
    for (const entrada of Object.values(ATLAS_CATEGORY_LOGOS)) {
      expect(typeof entrada).toBe("string");
    }
  });

  it("acende a luz de descoberta SO no card da categoria atual do jogador", async () => {
    // O convite tem de ser dirigido: uma luz em cada card nao aponta nada. O card
    // escolhido e o unico que o jogador tem motivo proprio para abrir.
    mockState = { careerId: "career-1", playerTeam: { id: 1, categoria: "production_challenger" } };
    await renderAtlas();

    expect(screen.getByTestId("atlas-v2-player-glow-production_mazda")).toBeInTheDocument();
    expect(screen.queryByTestId("atlas-v2-player-glow-mazda_rookie")).not.toBeInTheDocument();
  });

  it("nao acende luz nenhuma sem categoria do jogador", async () => {
    mockState = { careerId: "career-1" };
    await renderAtlas();

    expect(screen.getByTestId("atlas-v2-ranking-production_mazda")).toBeInTheDocument();
    expect(screen.queryByTestId("atlas-v2-player-glow-production_mazda")).not.toBeInTheDocument();
  });

  it("apaga a luz no primeiro clique dentro do card, e nao acende de novo", async () => {
    mockState = { careerId: "career-1", playerTeam: { id: 1, categoria: "production_challenger" } };
    const { unmount } = await renderAtlas();
    expect(screen.getByTestId("atlas-v2-player-glow-production_mazda")).toBeInTheDocument();

    // Abrir o dossie por uma linha do card ja prova que a porta foi achada — e o
    // clique tem de passar mesmo assim.
    fireEvent.click(screen.getByTestId("atlas-v2-ranking-row-T001"));
    expect(screen.queryByTestId("atlas-v2-player-glow-production_mazda")).not.toBeInTheDocument();
    expect(await screen.findByTestId("team-history-drawer")).toBeInTheDocument();

    // Convite aceito nao se repete: o registro e por instalacao, entao sobrevive a
    // fechar e reabrir a tela.
    unmount();
    await renderAtlas();
    expect(screen.queryByTestId("atlas-v2-player-glow-production_mazda")).not.toBeInTheDocument();
  });

  it("apaga a luz tambem quando o clique abre o salao dos campeoes", async () => {
    mockState = { careerId: "career-1", playerTeam: { id: 1, categoria: "production_challenger" } };
    await renderAtlas();
    expect(screen.getByTestId("atlas-v2-player-glow-production_mazda")).toBeInTheDocument();

    // O cabecalho leva a outro destino, mas prova a mesma coisa: o card foi aberto.
    fireEvent.click(screen.getByTestId("atlas-v2-champions-open-production_mazda"));
    expect(screen.queryByTestId("atlas-v2-player-glow-production_mazda")).not.toBeInTheDocument();
  });

  it("realca a linha da equipe apontada no ranking", async () => {
    await renderAtlas();

    fireEvent.mouseEnter(screen.getByTestId("atlas-v2-ranking-row-T001"));

    expect(screen.getByTestId("atlas-v2-track-T001-regular")).toHaveAttribute("stroke-width", "3");
    expect(screen.getByTestId("atlas-v2-track-T002-regular").closest("g")).toHaveAttribute("opacity", "0.16");
  });
});

// ---------------------------------------------------------------------------
// Temporada em andamento
//
// O Atlas passa a mostrar o AGORA: a coluna do ano corrente e provisoria e precisa
// se distinguir das temporadas ja decididas em toda parte — regua, plotagem, linha
// e tabela lateral. Fixture = o payload padrao com uma coluna viva de 2026 na
// Rookie, onde uma equipe subiu, uma caiu e uma estreia.
// ---------------------------------------------------------------------------
const payloadVivo = {
  ...payload,
  max_year: 2026,
  current_year: 2026,
  in_progress: true,
  last_completed_year: 2025,
  window_end: 2026,
  bands: payload.bands.map((band) =>
    band.key !== "mazda_rookie"
      ? band
      : {
          ...band,
          rows: [
            ...band.rows.map((row) => ({
              ...row,
              points: [
                ...row.points,
                // T004 era 1o em 2025 e caiu para 2o; T005 era 2o e assumiu a ponta.
                {
                  year: 2026,
                  slot: "regular",
                  position: row.team_id === "T004" ? 2 : 1,
                  points: row.team_id === "T004" ? 68 : 102,
                  wins: row.team_id === "T004" ? 0 : 3,
                  titles: 0,
                },
              ],
            })),
            {
              team_id: "T006",
              nome: "Overland",
              nome_curto: "OVL",
              cor_primaria: "#84cc16",
              cor_secundaria: "#1a2e05",
              base_position: 3,
              titles: [],
              is_reigning_champion: false,
              points: [{ year: 2026, slot: "regular", position: 3, points: 11, wins: 0, titles: 0 }],
            },
          ],
        },
  ),
};

describe("GlobalTeamsTabV2 com temporada em andamento", () => {
  beforeEach(() => {
    mockState = { careerId: "career-1" };
    invoke.mockReset();
    invoke.mockResolvedValue(payloadVivo);
  });

  it("nao gasta uma coluna com a temporada em andamento", async () => {
    await renderAtlas();

    // O eixo para na ultima temporada DECIDIDA. Uma coluna para 2026 so teria a
    // linha chegando e ficando parada ate a borda — um vao morto do tamanho de um
    // ano entre o ultimo resultado e o card lateral.
    expect(screen.getByTestId("atlas-v2-year-2025")).toBeInTheDocument();
    expect(screen.queryByTestId("atlas-v2-year-2026")).not.toBeInTheDocument();
    expect(screen.queryByTestId("atlas-v2-live-column")).not.toBeInTheDocument();
    // O intervalo anunciado inclui o ano em curso: ele nao tem coluna, mas esta
    // desenhado na borda.
    expect(screen.getByText(/Mazda: janela 2014-2026/i)).toBeInTheDocument();
  });

  it("poe o ponto de agora na borda direita, encostado no rabicho do card", async () => {
    await renderAtlas();

    // 2014..2025 = 12 colunas em 960px. O ponto de 2026 cai no limite seguinte,
    // que a regua trava na largura total — exatamente onde o rabicho comeca.
    const live = screen.getByTestId("atlas-v2-track-live-T004-regular");
    const finalX = live.getAttribute("d").trim().split(/\s+/).slice(-2)[0];
    expect(parseFloat(finalX)).toBeCloseTo(960, 1);
    expect(screen.getByTestId("atlas-v2-row-connector-T004")).toBeInTheDocument();
  });

  it("tracejada a chegada ate a coluna viva e mantem cheio o passado decidido", async () => {
    await renderAtlas();

    const live = screen.getByTestId("atlas-v2-track-live-T004-regular");
    expect(live).toHaveAttribute("stroke-dasharray");
    // O trecho decidido continua existindo e continua cheio.
    expect(screen.getByTestId("atlas-v2-track-T004-regular")).not.toHaveAttribute("stroke-dasharray");
  });

  it("equipe que so existe na temporada em curso nao ganha linha nenhuma, so o ponto", async () => {
    await renderAtlas();

    // Um unico ponto nao e tracado: sem ano anterior nao ha trecho a desenhar, nem
    // cheio nem tracejado. O que marca a presenca dela e o anel de estreia, na
    // borda direita, ligado ao card pelo rabicho.
    expect(screen.queryByTestId("atlas-v2-track-T006-regular")).not.toBeInTheDocument();
    expect(screen.queryByTestId("atlas-v2-track-live-T006-regular")).not.toBeInTheDocument();
    expect(screen.getByTestId("atlas-v2-entry-point-T006-regular")).toBeInTheDocument();
  });

  it("a tabela lateral fala do agora: ano corrente, placar parcial e variacao", async () => {
    await renderAtlas();

    // Sem selo "em andamento" no cabecalho: repetido em todo card ele virava
    // adorno e ainda comia a largura do titulo. Quem diz que a temporada esta em
    // curso e o proprio dado — ano corrente, variacao e placar parcial.
    expect(screen.getByTestId("atlas-v2-ranking-title-mazda_rookie")).toHaveTextContent("Mazda Rookie 2026");
    expect(screen.queryByTestId("atlas-v2-live-badge-mazda_rookie")).not.toBeInTheDocument();

    const lider = screen.getByTestId("atlas-v2-ranking-row-T005");
    expect(within(lider).getByText("102")).toBeInTheDocument();
    // T005 subiu de 2o para 1o, T004 caiu de 1o para 2o.
    expect(screen.getByTestId("atlas-v2-delta-T005")).toHaveTextContent("▲");
    expect(screen.getByTestId("atlas-v2-delta-T004")).toHaveTextContent("▼");
    // Estreante nao tem variacao a mostrar.
    expect(screen.queryByTestId("atlas-v2-delta-T006")).not.toBeInTheDocument();
  });

  it("nao contamina as faixas que nao estao disputando o ano corrente", async () => {
    await renderAtlas();

    // A Production parou em 2025 no fixture: continua sendo campeonato decidido.
    expect(screen.getByTestId("atlas-v2-ranking-title-production_mazda")).toHaveTextContent("Mazda Production 2025");
    expect(screen.queryByTestId("atlas-v2-live-badge-production_mazda")).not.toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Promocao e rebaixamento
//
// T007 sobe da Rookie (2024) para a Production (2025). A travessia entre as duas
// faixas atravessaria a altura inteira do grafico, cortando os campeonatos do
// meio — e ela nem e uma temporada, e o instante entre duas.
// ---------------------------------------------------------------------------
const payloadPromovido = {
  ...payload,
  bands: payload.bands.map((band) => {
    if (band.key === "mazda_rookie") {
      return {
        ...band,
        rows: [
          ...band.rows,
          {
            team_id: "T007",
            nome: "Subiu Racing",
            nome_curto: "SUB",
            cor_primaria: "#84cc16",
            cor_secundaria: "#1a2e05",
            base_position: 3,
            titles: [],
            is_reigning_champion: false,
            points: [{ year: 2024, slot: "regular", position: 2, points: 80, wins: 2, titles: 0 }],
          },
        ],
      };
    }
    if (band.key === "production_mazda") {
      return {
        ...band,
        rows: [
          ...band.rows,
          {
            team_id: "T007",
            nome: "Subiu Racing",
            nome_curto: "SUB",
            cor_primaria: "#84cc16",
            cor_secundaria: "#1a2e05",
            base_position: 5,
            titles: [],
            is_reigning_champion: false,
            points: [{ year: 2025, slot: "regular", position: 5, points: 20, wins: 0, titles: 0 }],
          },
        ],
      };
    }
    return band;
  }),
};

describe("GlobalTeamsTabV2 com equipe promovida", () => {
  beforeEach(() => {
    mockState = { careerId: "career-1" };
    invoke.mockReset();
    invoke.mockResolvedValue(payloadPromovido);
  });

  it("liga as divisoes por um Z, sem nenhuma diagonal atravessando o grafico", async () => {
    await renderAtlas();

    const path = screen.getByTestId("atlas-v2-crossing-T007-regular-0").getAttribute("d");
    // M x1 y1 L xc y1 L xc y2 L x2 y2 — o unico trecho que muda de altura e a
    // vertical, e ela acontece com x constante.
    const nodes = path.split(/[ML]/).filter(Boolean).map((pair) => pair.trim().split(/\s+/).map(Number));
    expect(nodes).toHaveLength(4);
    const [start, corner, exit, end] = nodes;
    expect(corner[1]).toBe(start[1]);
    expect(corner[0]).toBe(exit[0]);
    expect(end[1]).toBe(exit[1]);
    // E sobe: a Production fica acima da Rookie, entao o y final e menor.
    expect(end[1]).toBeLessThan(start[1]);
  });

  it("travessia longa atenua o miolo sem deixar ele sumir", async () => {
    await renderAtlas();

    // A T007 sai do 2o da Rookie e chega no 5o da Production: percorre boa parte
    // da altura, entao o traco vira gradiente em vez da cor chapada.
    const crossing = screen.getByTestId("atlas-v2-crossing-T007-regular-0");
    expect(crossing.getAttribute("stroke")).toBe("url(#corridor-T007-regular-0)");

    // O gradiente vai do y de saida ao y de chegada — as pernas horizontais do Z
    // caem justamente nas duas pontas dessa faixa, e por isso saem solidas.
    // O atlas monta num portal para o body, entao a busca sai do document.
    const gradient = document.querySelector("#corridor-T007-regular-0");
    expect(gradient.getAttribute("gradientUnits")).toBe("userSpaceOnUse");
    const stops = [...gradient.querySelectorAll("stop")].map((stop) =>
      parseFloat(stop.getAttribute("stop-opacity")),
    );
    expect(stops[0]).toBe(1);
    expect(stops[stops.length - 1]).toBe(1);

    // O miolo e mais fraco que as pontas, mas tem PISO: quem sobe ou cai muitas
    // posicoes tem a travessia mais longa, e era justo a historia mais interessante
    // do grafico que sumia quando o miolo ia a 0.05.
    const miolo = Math.min(...stops);
    expect(miolo).toBeLessThan(1);
    expect(miolo).toBeGreaterThanOrEqual(0.35);

    // E o traco inteiro tem de estar acima do limiar do invisivel depois de
    // multiplicado pela opacidade do elemento — que e onde o 0.05 morria.
    const opacidadeDoTraco = parseFloat(crossing.getAttribute("opacity"));
    expect(miolo * opacidadeDoTraco).toBeGreaterThan(0.2);
  });

  it("a travessia e discreta por padrao e ganha corpo no foco", async () => {
    await renderAtlas();

    const before = screen.getByTestId("atlas-v2-crossing-T007-regular-0");
    const restingWidth = parseFloat(before.getAttribute("stroke-width"));
    const restingOpacity = parseFloat(before.getAttribute("opacity"));

    fireEvent.mouseEnter(screen.getByTestId("atlas-v2-ranking-row-T007"));
    const after = screen.getByTestId("atlas-v2-crossing-T007-regular-0");
    expect(parseFloat(after.getAttribute("stroke-width"))).toBeGreaterThan(restingWidth);
    expect(parseFloat(after.getAttribute("opacity"))).toBeGreaterThan(restingOpacity);
    // Pontilhada nos dois estados: e um vinculo entre temporadas, nao uma delas.
    expect(after).toHaveAttribute("stroke-dasharray");
  });
});
