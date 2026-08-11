import { fireEvent, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import {
  abrirMercado,
  contrato,
  curva,
  detail,
  fingeQueRola,
  renderFicha,
  restauraLayout,
} from "./driverDetailV2TestKit.jsx";

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

describe("DriverDetailModalV2 — mercado", () => {
  beforeEach(() => {
    mockState = { careerId: "career-1" };
    invoke.mockReset();
    fingeQueRola(true);
  });

  afterEach(restauraLayout);

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
