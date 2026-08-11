import {
  CHIP_NEUTRO,
  CURVA_GEO,
  ancoraDoBalao,
  blocosSemVinculo,
  caminhoDaFaixa,
  colunasDeCategoria,
  faixasDeDiferenca,
  faixasSemVinculo,
  geometriaDaCurva,
  lacunasDeEquipe,
  luminanciaRelativa,
  misturarCores,
  monogramaDaEquipe,
  opacidadeDaColuna,
  partirNoPresente,
  partirPorEstilo,
  pontesSemVinculo,
  segmentosContinuos,
  segmentosDaPonte,
  tomDoChip,
  trechosDeCategoria,
  trechosDeEquipe,
  trocasDeEquipe,
  verticesDaSerie,
} from "./curvaDeCarreira.jsx";

// ---------------------------------------------------------------------------
// A moldura compartilhada pelas duas curvas da ficha (mercado e campeonato).
//
// Tudo aqui é determinístico e sem React: são as contas que decidem onde a
// coluna de 2014 cai, onde a série pousa o ponto do ano e onde o desenho se
// PARTE — no ano sem medida, no ano sem equipe, na virada de estilo. Errar
// qualquer uma delas não quebra nada: só desloca meia coluna na tela, que é
// justamente o tipo de defeito que ninguém acha lendo o código.
// ---------------------------------------------------------------------------

function ponto(ano, extra = {}) {
  return { ano, categoria: "gt3", equipe_nome: "Arclight", equipe_cor: "#58a6ff", ...extra };
}

const lerValor = (p) => p.valor;

describe("geometriaDaCurva", () => {
  it("divide a largura util em uma coluna inteira por temporada", () => {
    const geo = geometriaDaCurva([ponto(2020), ponto(2021), ponto(2022)]);
    const larguraPlot = CURVA_GEO.w - CURVA_GEO.padE - CURVA_GEO.padD;

    expect(geo.larguraPlot).toBe(larguraPlot);
    expect(geo.alturaPlot).toBe(CURVA_GEO.h - CURVA_GEO.padT - CURVA_GEO.padB);
    expect(geo.passo).toBe(larguraPlot / 3);
    expect(geo.ultimo).toBe(2);
  });

  it("poe o rotulo no centro da coluna e o ponto da serie na abertura dela", () => {
    const geo = geometriaDaCurva([ponto(2020), ponto(2021)]);

    expect(geo.xSerie(0)).toBe(CURVA_GEO.padE);
    expect(geo.x(0)).toBe(CURVA_GEO.padE + geo.passo / 2);
    expect(geo.x(0) - geo.xSerie(0)).toBe(geo.passo / 2);
    // A ultima coluna abre na metade do plot e o fecho mora na borda direita.
    expect(geo.xSerie(1)).toBe(CURVA_GEO.padE + geo.passo);
    expect(geo.fimDoPlot).toBe(CURVA_GEO.padE + geo.larguraPlot);
  });

  it("nao divide por zero quando a carreira ainda nao tem temporada", () => {
    const geo = geometriaDaCurva([]);

    expect(Number.isFinite(geo.passo)).toBe(true);
    expect(geo.ultimo).toBe(0);
  });
});

describe("partirNoPresente", () => {
  const trecho = { inicio: 0, itens: [ponto(2020), ponto(2021), ponto(2022)] };

  it("devolve o trecho inteiro como cumprido quando nao ha futuro no eixo", () => {
    expect(partirNoPresente(trecho, -1)).toEqual([{ ...trecho, futuro: false }]);
  });

  it("marca o trecho inteiro como futuro quando ele comeca depois do corte", () => {
    const [parte] = partirNoPresente({ ...trecho, inicio: 4 }, 3);

    expect(parte.futuro).toBe(true);
    expect(parte.itens).toHaveLength(3);
  });

  it("repete o ponto do corte nos dois lados para costurar o traco ao fantasma", () => {
    const [cumprido, aberto] = partirNoPresente(trecho, 2);

    expect(cumprido.futuro).toBe(false);
    expect(cumprido.itens).toHaveLength(3);
    // Quem fecha a serie na borda do plot e a parte em aberto, nunca a cumprida.
    expect(cumprido.fecho).toBe(false);
    expect(aberto.inicio).toBe(2);
    expect(aberto.futuro).toBe(true);
    expect(aberto.itens[0]).toBe(cumprido.itens[cumprido.itens.length - 1]);
  });
});

describe("partirPorEstilo", () => {
  const estiloDoAno = (p) => ({ cor: p.cor, tracejada: Boolean(p.tracejada) });

  it("junta anos seguidos de mesma cor e mesmo traco numa parte so", () => {
    const trecho = {
      inicio: 0,
      itens: [ponto(2020, { cor: "#fff" }), ponto(2021, { cor: "#fff" })],
    };

    expect(partirPorEstilo(trecho, estiloDoAno)).toHaveLength(1);
  });

  it("estende cada parte um ponto adiante para nao abrir vao na virada", () => {
    const trecho = {
      inicio: 0,
      itens: [
        ponto(2020, { cor: "#fff" }),
        ponto(2021, { cor: "#fff" }),
        ponto(2022, { cor: "#000" }),
      ],
    };
    const [primeira, segunda] = partirPorEstilo(trecho, estiloDoAno);

    expect(primeira.cor).toBe("#fff");
    expect(primeira.itens).toHaveLength(3);
    expect(primeira.fecho).toBe(false);
    expect(segunda.cor).toBe("#000");
    expect(segunda.inicio).toBe(2);
    // So a ultima parte fecha na borda do plot: duas fechariam o mesmo traco.
    expect(segunda.fecho).toBe(true);
  });

  it("descarta parte de um ponto so que nao fecha a serie", () => {
    const trecho = {
      inicio: 0,
      itens: [ponto(2020, { cor: "#fff" }), ponto(2021, { cor: "#000" })],
      fecho: false,
    };
    const partes = partirPorEstilo(trecho, estiloDoAno);

    expect(partes).toHaveLength(1);
    expect(partes[0].cor).toBe("#fff");
  });

  it("herda de quem partiu a marca de futuro", () => {
    const trecho = { inicio: 0, itens: [ponto(2020, { cor: "#fff" })], futuro: true };

    expect(partirPorEstilo(trecho, estiloDoAno)[0].futuro).toBe(true);
  });
});

describe("segmentosContinuos", () => {
  it("parte a serie no ano sem medida em vez de ligar os dois lados", () => {
    const pontos = [
      ponto(2020, { valor: 10 }),
      ponto(2021, {}),
      ponto(2022, { valor: 30 }),
      ponto(2023, { valor: 40 }),
    ];
    const trechos = segmentosContinuos(pontos, lerValor);

    expect(trechos).toHaveLength(1);
    expect(trechos[0].inicio).toBe(2);
    expect(trechos[0].itens).toHaveLength(2);
  });

  it("guarda o ponto solto da ultima temporada, que tem a borda para onde correr", () => {
    const pontos = [ponto(2020, { valor: 10 }), ponto(2021, {}), ponto(2022, { valor: 30 })];
    const trechos = segmentosContinuos(pontos, lerValor);

    expect(trechos).toHaveLength(1);
    expect(trechos[0].inicio).toBe(2);
    expect(trechos[0].itens).toHaveLength(1);
  });
});

describe("blocos e faixas sem vinculo", () => {
  const temVinculo = (p) => Boolean(p.equipe_nome);

  it("junta anos vizinhos sem equipe num bloco so", () => {
    const pontos = [
      ponto(2020),
      ponto(2021, { equipe_nome: null }),
      ponto(2022, { equipe_nome: null }),
      ponto(2023),
    ];

    expect(blocosSemVinculo(pontos, temVinculo)).toEqual([{ inicio: 1, fim: 2 }]);
  });

  it("separa dois foras distintos", () => {
    const pontos = [
      ponto(2020, { equipe_nome: null }),
      ponto(2021),
      ponto(2022, { equipe_nome: null }),
    ];

    expect(blocosSemVinculo(pontos, temVinculo)).toEqual([
      { inicio: 0, fim: 0 },
      { inicio: 2, fim: 2 },
    ]);
  });

  it("mede a faixa pela coluna do ano e corta na borda do plot", () => {
    const pontos = [ponto(2020, { equipe_nome: null }), ponto(2021), ponto(2022)];
    const geo = geometriaDaCurva(pontos);
    const [faixa] = faixasSemVinculo(pontos, geo, temVinculo);

    // A primeira temporada vale meia coluna: a faixa comeca na borda, nao antes.
    expect(faixa.esquerda).toBe(geo.padE);
    expect(faixa.direita).toBe(geo.padE + geo.passo);
  });
});

describe("colunas de categoria", () => {
  it("junta temporadas seguidas do mesmo degrau", () => {
    const pontos = [
      ponto(2020, { categoria: "gt4" }),
      ponto(2021, { categoria: "gt4" }),
      ponto(2022, { categoria: "gt3" }),
    ];

    expect(trechosDeCategoria(pontos)).toEqual([
      { categoria: "gt4", inicio: 0, fim: 1 },
      { categoria: "gt3", inicio: 2, fim: 2 },
    ]);
  });

  it("ignora o ano sem categoria em vez de abrir uma coluna anonima", () => {
    expect(trechosDeCategoria([ponto(2020, { categoria: null })])).toEqual([]);
  });

  it("mede cada coluna na mesma regua da fita do rodape", () => {
    const pontos = [ponto(2020, { categoria: "gt4" }), ponto(2021, { categoria: "gt3" })];
    const geo = geometriaDaCurva(pontos);
    const colunas = colunasDeCategoria(pontos, geo);

    expect(colunas[0].esquerda).toBe(geo.padE);
    expect(colunas[0].largura).toBe(geo.passo);
    expect(colunas[0].esquerda + colunas[0].largura).toBe(colunas[1].esquerda);
    expect(colunas[1].esquerda + colunas[1].largura).toBe(geo.padE + geo.larguraPlot);
    expect(colunas[0].label).toBeTruthy();
    expect(colunas[0].cor).toMatch(/^#[0-9a-f]{6}$/i);
  });
});

describe("peso da cor", () => {
  it("le a luminancia relativa dos extremos", () => {
    expect(luminanciaRelativa("#ffffff")).toBeCloseTo(1, 5);
    expect(luminanciaRelativa("#000000")).toBeCloseTo(0, 5);
  });

  it("assume o meio do caminho quando o hex nao serve", () => {
    expect(luminanciaRelativa("nao-e-cor")).toBe(0.5);
    expect(luminanciaRelativa("#fff")).toBe(0.5);
  });

  it("baixa o alpha da coluna clara para ela nao pesar mais que a escura", () => {
    const clara = opacidadeDaColuna("#ffd400");
    const escura = opacidadeDaColuna("#8020d0");

    expect(clara).toBeLessThan(escura);
    expect(opacidadeDaColuna("#ffffff")).toBeCloseTo(0.048, 5);
    expect(opacidadeDaColuna("#000000")).toBeCloseTo(0.086, 5);
  });

  it("mistura em hex opaco, sem alfa por cima", () => {
    expect(misturarCores("#000000", "#ffffff", 0.5)).toBe("#808080");
    expect(misturarCores("#000000", "#ffffff", 0)).toBe("#000000");
    expect(misturarCores("#000000", "#ffffff", 1)).toBe("#ffffff");
  });

  it("deixa o chip no neutro quando a equipe nao tem cor utilizavel", () => {
    expect(tomDoChip(null)).toBe(CHIP_NEUTRO);
    expect(tomDoChip("#fff")).toBe(CHIP_NEUTRO);
  });

  it("puxa o neutro na direcao da casa sem vestir a cor cheia", () => {
    const tom = tomDoChip("#8020d0");

    expect(tom.preenchimento).toMatch(/^#[0-9a-f]{6}$/i);
    expect(tom.preenchimento).not.toBe("#8020d0");
    expect(tom.preenchimento).not.toBe(CHIP_NEUTRO.preenchimento);
    expect(tom.contorno).not.toBe(tom.preenchimento);
  });
});

describe("monogramaDaEquipe", () => {
  it("usa as iniciais quando o nome tem mais de uma palavra", () => {
    expect(monogramaDaEquipe("Silver Peak Performance")).toBe("SPP");
    expect(monogramaDaEquipe("Kitsune Racing Team Japan")).toBe("KRT");
  });

  it("da as tres primeiras letras do nome de uma palavra so", () => {
    expect(monogramaDaEquipe("Arclight")).toBe("ARC");
  });

  it("nao inventa marca para nome vazio", () => {
    expect(monogramaDaEquipe("")).toBe("");
    expect(monogramaDaEquipe(null)).toBe("");
  });
});

describe("trocas e trechos de equipe", () => {
  it("nao conta troca no ano fora do grid quando ele volta pela mesma casa", () => {
    const pontos = [
      ponto(2020, { valor: 10 }),
      ponto(2021, { equipe_nome: null, valor: 0 }),
      ponto(2022, { valor: 30 }),
    ];

    expect(trocasDeEquipe(pontos, lerValor)).toEqual([]);
  });

  it("carrega os dois valores da troca junto com os dois nomes", () => {
    const pontos = [
      ponto(2020, { valor: 10 }),
      ponto(2021, { equipe_nome: "Kitsune", valor: 30 }),
    ];

    expect(trocasDeEquipe(pontos, lerValor)).toEqual([
      { indice: 1, de: "Arclight", para: "Kitsune", ano: 2021, valorDe: 10, valorPara: 30 },
    ]);
  });

  it("parte o chip no ano sem equipe, ao contrario da contagem de trocas", () => {
    const pontos = [ponto(2020), ponto(2021, { equipe_nome: null }), ponto(2022)];

    expect(trechosDeEquipe(pontos)).toEqual([
      { equipe: "Arclight", cor: "#58a6ff", inicio: 0, fim: 0 },
      { equipe: "Arclight", cor: "#58a6ff", inicio: 2, fim: 2 },
    ]);
    expect(lacunasDeEquipe(pontos)).toEqual([{ inicio: 1, fim: 1 }]);
  });
});

describe("ponte sobre o vao", () => {
  const temVinculo = (p) => Boolean(p.equipe_nome);
  const pontos = [
    ponto(2020, { valor: 10 }),
    ponto(2021, { equipe_nome: null }),
    ponto(2022, { valor: 30 }),
  ];

  it("liga as duas margens do vao", () => {
    const geo = geometriaDaCurva(pontos);
    const faixas = faixasSemVinculo(pontos, geo, temVinculo);
    const [ponte] = pontesSemVinculo(faixas, pontos, lerValor);

    expect(ponte).toMatchObject({ de: 0, para: 2, valorDe: 10, valorPara: 30 });
  });

  it("nao constroi ponte de um lado so", () => {
    const abertos = [ponto(2020, { equipe_nome: null }), ponto(2021, { valor: 30 })];
    const geo = geometriaDaCurva(abertos);

    expect(pontesSemVinculo(faixasSemVinculo(abertos, geo, temVinculo), abertos, lerValor)).toEqual(
      [],
    );
  });

  it("cala onde a serie ja atravessa cheia", () => {
    const medidos = [
      ponto(2020, { valor: 10 }),
      ponto(2021, { equipe_nome: null, valor: 20 }),
      ponto(2022, { valor: 30 }),
    ];
    const geo = geometriaDaCurva(medidos);

    expect(pontesSemVinculo(faixasSemVinculo(medidos, geo, temVinculo), medidos, lerValor)).toEqual(
      [],
    );
  });

  it("corta o tracejado na borda da hachura e interpola a altura no corte", () => {
    const partes = segmentosDaPonte(
      { de: 0, para: 2, valorDe: 0, valorPara: 200, esquerda: 50, direita: 150 },
      { x: (indice) => indice * 100, y: (valor) => valor },
    );

    expect(partes.antes).toEqual({ x1: 0, y1: 0, x2: 50, y2: 50 });
    expect(partes.vao).toEqual({ x1: 50, y1: 50, x2: 150, y2: 150 });
    expect(partes.depois).toEqual({ x1: 150, y1: 150, x2: 200, y2: 200 });
  });

  it("descarta o pedaco cheio de menos de meio pixel", () => {
    const partes = segmentosDaPonte(
      { de: 0, para: 2, valorDe: 0, valorPara: 200, esquerda: 0, direita: 200 },
      { x: (indice) => indice * 100, y: (valor) => valor },
    );

    expect(partes.antes).toBeNull();
    expect(partes.depois).toBeNull();
  });
});

describe("faixa de diferenca entre as duas series", () => {
  const lerTopo = (p) => p.topo;
  const lerBase = (p) => p.base;

  it("so existe onde ha os dois lados para comparar", () => {
    const pontos = [
      ponto(2020, { topo: 10, base: 5 }),
      ponto(2021, { topo: 20 }),
      ponto(2022, { topo: 30, base: 15 }),
      ponto(2023, { topo: 40, base: 25 }),
    ];
    const faixas = faixasDeDiferenca(pontos, lerTopo, lerBase);

    expect(faixas).toHaveLength(1);
    expect(faixas[0].inicio).toBe(2);
    expect(faixas[0].trecho).toHaveLength(2);
  });

  it("fecha o poligono na borda do plot junto com as linhas", () => {
    const pontos = [ponto(2020, { topo: 10, base: 5 }), ponto(2021, { topo: 20, base: 15 })];
    const geo = geometriaDaCurva(pontos);
    const caminho = caminhoDaFaixa(pontos, geo, (valor) => valor, 0, lerTopo, lerBase);

    expect(caminho.startsWith("M")).toBe(true);
    expect(caminho.endsWith("Z")).toBe(true);
    expect(caminho).toContain(`${geo.fimDoPlot},20`);
    expect(caminho).toContain(`${geo.fimDoPlot},15`);
  });
});

describe("verticesDaSerie", () => {
  const pontos = [ponto(2020, { valor: 10 }), ponto(2021, { valor: 20 })];
  const geo = geometriaDaCurva(pontos);

  it("pousa cada ano na abertura da coluna e fecha na borda do plot", () => {
    const vertices = verticesDaSerie({ inicio: 0, itens: pontos }, geo, (v) => v, lerValor);

    expect(vertices).toBe(`${geo.xSerie(0)},10 ${geo.xSerie(1)},20 ${geo.fimDoPlot},20`);
  });

  it("nao fecha duas vezes quando outro trecho continua dali", () => {
    const vertices = verticesDaSerie(
      { inicio: 0, itens: pontos, fecho: false },
      geo,
      (v) => v,
      lerValor,
    );

    expect(vertices).toBe(`${geo.xSerie(0)},10 ${geo.xSerie(1)},20`);
  });

  it("nao fecha o trecho que morre antes da ultima temporada", () => {
    const tres = [...pontos, ponto(2022, { valor: 30 })];
    const geoTres = geometriaDaCurva(tres);
    const vertices = verticesDaSerie({ inicio: 0, itens: pontos }, geoTres, (v) => v, lerValor);

    expect(vertices).not.toContain(String(geoTres.fimDoPlot));
  });
});

describe("ancoraDoBalao", () => {
  const quadro = { x: (indice) => indice * 100, w: 680, h: 220 };

  it("encosta o balao na borda oposta a metade em que os pontos estao", () => {
    expect(ancoraDoBalao([20], 1, quadro).vertical).toEqual({ bottom: 0 });
    expect(ancoraDoBalao([200], 1, quadro).vertical).toEqual({ top: 0 });
  });

  it("assume o meio do quadro quando nenhuma serie tem altura", () => {
    expect(ancoraDoBalao([], 1, quadro).vertical).toEqual({ top: 0 });
  });

  it("ancora pela lateral nas pontas e centraliza no miolo", () => {
    expect(ancoraDoBalao([20], 0, quadro).transform).toBe("translateX(0%)");
    expect(ancoraDoBalao([20], 3, quadro).transform).toBe("translateX(-50%)");
    expect(ancoraDoBalao([20], 6, quadro).transform).toBe("translateX(-100%)");
  });

  it("posiciona em porcentagem do quadro, nao em pixels", () => {
    expect(ancoraDoBalao([20], 2, quadro).esquerda).toBe(`${(200 / 680) * 100}%`);
  });
});
