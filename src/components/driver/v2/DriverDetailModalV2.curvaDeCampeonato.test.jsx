import { fireEvent, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import {
  abrirTemporada,
  contrato,
  curva,
  curvaCampeonato,
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

describe("DriverDetailModalV2 — curva de campeonato", () => {
  beforeEach(() => {
    mockState = { careerId: "career-1" };
    invoke.mockReset();
    fingeQueRola(true);
  });

  afterEach(restauraLayout);

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
});
