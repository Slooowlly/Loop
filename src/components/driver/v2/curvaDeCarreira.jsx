import { useTranslation } from "react-i18next";

import TeamLogoMark, { getTeamLogoSrc, HALO_FILTER } from "../../team/TeamLogoMark";
import { getCategoryColor } from "../../../utils/categoryColors";
import { formatCategoryLabel } from "../detalhes/formatadores.js";

// A MOLDURA de uma curva de carreira — o que duas séries diferentes têm em comum
// quando o eixo X é a carreira inteira, uma temporada por coluna.
//
// Nasceu dentro da curva de mercado e saiu de lá quando a curva de campeonato
// passou a desenhar exatamente o mesmo quadro: colunas de categoria ao fundo,
// hachura no ano fora do grid, réguas nas trocas de casa, fita de equipes no
// rodapé, alvos de hover por temporada. O que muda entre os dois gráficos é a
// SÉRIE — dinheiro num, posição no outro — e é só ela que cada um escreve.
//
// Duas cópias desse quadro divergiriam na primeira vez que alguém mexesse num
// espaçamento: os dois cartões vivem no mesmo modal, a uma aba de distância, e
// o desalinhamento apareceria lado a lado.

// O rodapé carrega duas coisas: os anos e a fita de equipes. A CATEGORIA saiu
// dele — virou coluna atrás do gráfico, onde ela é contexto da série em vez de
// mais uma tira disputando leitura com o desenho.
export const CURVA_GEO = { w: 680, h: 220, padE: 62, padD: 12, padT: 14, padB: 56 };
export const CURVA_Y_ANOS = 184;

// A fita de equipes: um chip por vínculo, com a logo centrada e o fundo puxando
// para a cor da casa. A versão anterior gastava um losango mais uma linha de
// legenda só para dizer que houve troca — aqui a troca é a folga entre dois
// chips, e a identidade da casa é a cor mais a arte.
export const CURVA_FITA = { y: 192, altura: 20, folga: 2, raio: 5 };

// O tamanho da logo dentro do chip, e onde ela para de caber e sobra o chip
// vazio — que ainda diz "aqui houve uma casa" e responde no balão. O piso é
// baixo de propósito: a primeira e a última temporada ocupam MEIA coluna, e um
// piso confortável apagava justo a estreia.
export const CURVA_CHIP = { logo: 18, logoMin: 9 };

// A coluna da categoria, atrás do plot: tinte baixo mais um friso saturado na
// base. O tinte diz "estes anos são o mesmo degrau" sem competir com as séries;
// o friso é o que deixa a cor IDENTIFICÁVEL, porque a 6% de opacidade o amarelo
// da Rookie e o vermelho da Cup quase empatam sobre o #0f1c2b.
export const CURVA_COLUNA = {
  friso: 2.5,
  opacidadeDoFriso: 0.92,
  rotuloDePe: 6,
  recuoDoRotulo: 10,
};

// A régua da troca de equipe: desce do topo do plot até o chip do rodapé.
//
// É ela que amarra causa e efeito — a linha dá um degrau PORQUE ele mudou de
// casa, e as duas coisas viviam em andares separados do cartão. Atravessa a
// faixa dos anos de propósito: o que ela liga é o salto no gráfico ao chip lá
// embaixo, e parar na base do plot deixaria a ligação pela metade.
//
// Se distingue da régua do "hoje" por onde termina (aquela para na base) e por
// cair numa EMENDA entre dois anos, nunca em cima de um.
export const CURVA_REGUA_TROCA = { cor: "rgba(255,255,255,0.26)", traco: "4 3" };

// Id de `<pattern>` é global ao documento — nome longo de propósito, para não
// colidir com a hachura de outro gráfico que apareça no mesmo modal. Os dois
// gráficos da ficha compartilham a MESMA hachura justamente por isso: dois
// patterns com o mesmo desenho e ids diferentes seriam duas definições do mesmo
// vocabulário, e uma delas ficaria para trás.
export const HACHURA_SEM_VINCULO = "curva-carreira-sem-vinculo";

// Quantas temporadas cumpridas o piloto precisa ter para a curva abrir como
// desenho. Abaixo disso a ficha abre na tabela: não é o gráfico que fica ruim
// com poucos pontos, é que ele PROMETE trajetória — e um par de números não tem
// trajetória para mostrar, que é justamente o que a tabela faz melhor.
//
// Três, e não quatro. Com três anos já há uma virada a ler — subiu e caiu, caiu
// e subiu, ou seguiu na mesma — e é essa a menor forma que a curva sabe
// desenhar. Dois pontos só sabem dizer "melhorou" ou "piorou", e para isso a
// tabela basta. A régua também mudou desde que este número foi escolhido: com o
// ponto no começo da coluna e o fecho na borda, três temporadas ocupam o quadro
// inteiro em vez de se amontoarem à esquerda.
export const MINIMO_PARA_O_GRAFICO = 3;

// As DUAS réguas do eixo X, idênticas nos dois gráficos. Saem daqui e não de
// cada curva porque são elas que fazem a coluna de 2014 cair no mesmo pixel nos
// dois cartões.
//
// Cada temporada vale uma coluna INTEIRA. O eixo é categórico — uma temporada é
// um período, não um instante — e as duas réguas são as duas leituras que esse
// período pede:
//
// - `x` é o CENTRO da coluna: onde mora o que descreve o ano como um todo. O
//   rótulo do ano, a coluna de categoria, o chip da equipe, o alvo de hover.
// - `xSerie` é o COMEÇO da coluna: onde a série pousa o ponto daquele ano.
//
// A série já pousou nos três lugares possíveis, e os dois primeiros erraram pelo
// mesmo motivo. No CENTRO, o ponto ficava no meio do ano e o traço até o vizinho
// atravessava a emenda, cobrindo meia coluna de cada um. No FIM, o ponto caía
// exatamente sobre a borda esquerda da coluna SEGUINTE — e ali ele lê como sendo
// do ano seguinte: a posição de 2018 aparecia em cima da coluna roxa da Mazda
// Production de 2019, encostada no rótulo errado.
//
// No COMEÇO, tudo que descreve o ano fica à DIREITA do ponto: a coluna de
// categoria abre ali, o rótulo do ano está logo abaixo, o chip da equipe começa
// no mesmo x. E o traço que sai do ponto atravessa a coluna do próprio ano até
// encontrar o ponto do ano seguinte — o segmento É o ano, do valor com que ele
// abriu ao valor com que o próximo abriu.
//
// Isso deixa o último ano sem traço à direita, e é o que `verticesDaSerie`
// resolve fechando a série na borda do plot.
export function geometriaDaCurva(pontos) {
  const { w, h, padE, padD, padT, padB } = CURVA_GEO;
  const larguraPlot = w - padE - padD;
  const alturaPlot = h - padT - padB;
  const colunas = Math.max(1, pontos.length);
  const passo = larguraPlot / colunas;

  return {
    w,
    h,
    padE,
    padD,
    padT,
    padB,
    larguraPlot,
    alturaPlot,
    passo,
    x: (indice) => padE + (indice + 0.5) * passo,
    xSerie: (indice) => padE + indice * passo,
    // A borda direita do plot, onde a última temporada fecha. Sai da geometria
    // para que a curva não tenha de remontar `padE + larguraPlot` na mão.
    fimDoPlot: padE + larguraPlot,
    ultimo: Math.max(0, pontos.length - 1),
  };
}

// Corta um trecho contínuo no presente, para que a parte já cumprida e a parte
// ainda em aberto possam ser desenhadas com pesos diferentes.
//
// O ponto de hoje entra nos DOIS lados de propósito: é ele que costura o traço
// cheio ao fantasma. Sem essa sobreposição de um ponto a linha ganharia um vão
// em branco de um passo inteiro entre o que aconteceu e o que ainda está em
// jogo — e vão em branco, nestes gráficos, já quer dizer outra coisa.
// `inicioDoFuturo` é o índice do PRIMEIRO ano ainda em aberto, e não o do último
// já cumprido: com o ponto no começo da coluna, o traço que sai do ano N cobre a
// coluna de N, então é a partir do ponto do ano em aberto que o desenho perde
// certeza. Cortar um índice antes esmaeceria a coluna de um ano já fechado.
export function partirNoPresente(trecho, inicioDoFuturo) {
  const fim = trecho.inicio + trecho.itens.length - 1;
  if (inicioDoFuturo < 0 || fim < inicioDoFuturo) return [{ ...trecho, futuro: false }];
  if (trecho.inicio >= inicioDoFuturo) return [{ ...trecho, futuro: true }];

  const corte = inicioDoFuturo - trecho.inicio;
  return [
    // A parte cumprida não fecha na borda do plot mesmo quando alcança a última
    // temporada: quem fecha é a parte em aberto, que continua dali.
    { inicio: trecho.inicio, itens: trecho.itens.slice(0, corte + 1), futuro: false, fecho: false },
    { inicio: inicioDoFuturo, itens: trecho.itens.slice(corte), futuro: true },
  ];
}

// Corta um trecho contínuo onde o ESTILO muda — outra cor, outro traço —, para
// que uma série possa mudar de aparência ano a ano sem se partir em duas.
//
// Existe porque uma linha pode dizer coisas diferentes em anos diferentes. "O
// que o carro dava" é uma medida forte na GT3 e fraca numa monomarca, onde o
// grid inteiro corre no mesmo carro — e é de um CARRO específico, o da equipe
// daquele ano, que ela fala. Apagar a linha na monomarca perderia o número;
// deixá-la cheia afirmaria uma precisão que ela não tem; pintá-la de uma cor só
// calaria sobre de quem era o carro.
//
// Cada segmento herda o estilo do ponto em que TERMINA, e a razão é geométrica:
// com o marcador no fim da coluna do ano, o traço entre dois pontos ocupa
// exatamente a coluna do ano SEGUINTE. O segmento é aquele ano, então é dele que
// tira cor e traço — a virada da Rookie para a GT4 é sólida porque pertence ao
// ano de GT4, e o degrau da troca de casa já sai na cor da casa nova.
//
// Por isso cada parte se estende um ponto para TRÁS: ela precisa do ponto de
// onde seus segmentos saem. É a mesma sobreposição do corte no presente — sem
// ela a linha ganharia um vão de um passo inteiro em cada virada, e vão em
// branco já quer dizer outra coisa neste gráfico.
export function partirPorEstilo(trecho, estiloDoAno) {
  const estilos = trecho.itens.map((ponto) => estiloDoAno(ponto));
  const chave = (estilo) => `${estilo.cor ?? ""}|${estilo.tracejada ? 1 : 0}`;
  const corridas = [];
  estilos.forEach((estilo, indice) => {
    const ultima = corridas[corridas.length - 1];
    if (ultima && chave(ultima.estilo) === chave(estilo)) {
      ultima.fim = indice;
      return;
    }
    corridas.push({ inicio: indice, fim: indice, estilo });
  });

  return corridas
    .map((corrida, ordem) => {
      const ultima = ordem === corridas.length - 1;
      return {
        inicio: trecho.inicio + corrida.inicio,
        itens: trecho.itens.slice(corrida.inicio, (ultima ? corrida.fim : corrida.fim + 1) + 1),
        ...corrida.estilo,
        futuro: trecho.futuro,
        // Só a última parte fecha na borda do plot. Sem isso a parte anterior,
        // que avança um ponto e também alcança o fim, desenharia o mesmo fecho
        // por baixo com outro traço.
        fecho: ultima && trecho.fecho !== false,
      };
    })
    // Parte de um ponto só sobrevive apenas quando FECHA a série, onde a borda
    // do plot lhe dá para onde correr. Fora dali ela não tem segmento nenhum: o
    // único traço que sairia dela pertence ao ano seguinte, que é de outro
    // estilo e já o desenhou.
    .filter((parte) => parte.itens.length > 1 || parte.fecho);
}

// Trechos em que a série existe sem buracos. Temporada sem medida é buraco de
// verdade — ligar os dois lados inventaria um valor que não houve.
export function segmentosContinuos(pontos, ler) {
  const trechos = [];
  let atual = null;
  pontos.forEach((ponto, indice) => {
    if (Number.isFinite(ler(ponto))) {
      if (!atual) {
        atual = { inicio: indice, itens: [] };
        trechos.push(atual);
      }
      atual.itens.push(ponto);
    } else {
      atual = null;
    }
  });
  // Um ponto solto não é linha... exceto na ÚLTIMA temporada do eixo, que tem a
  // borda do plot para onde correr. É o caso do piloto que voltou ao grid este
  // ano depois de um fora: sem esta exceção a coluna dele nasceria vazia, com um
  // marcador na borda esquerda e nada mais.
  return trechos.filter(
    (trecho) =>
      trecho.itens.length > 1 || trecho.inicio + trecho.itens.length - 1 === pontos.length - 1,
  );
}

// Blocos de temporadas seguidas sem vínculo, em índices do eixo.
//
// Anos vizinhos viram UM bloco, e não três listras coladas: o que o jogador
// precisa ler é "esse pedaço da carreira ele passou sem equipe", e três hachuras
// separadas por um fio de nada só sujam o desenho.
export function blocosSemVinculo(pontos, temVinculo) {
  const blocos = [];
  let atual = null;
  pontos.forEach((ponto, indice) => {
    if (temVinculo(ponto)) {
      atual = null;
      return;
    }
    if (atual && atual.fim === indice - 1) {
      atual.fim = indice;
      return;
    }
    atual = { inicio: indice, fim: indice };
    blocos.push(atual);
  });
  return blocos;
}

// Onde cada faixa começa e termina no eixo: nas COLUNAS dos anos sem vínculo,
// da borda esquerda do primeiro à borda direita do último.
//
// A régua é a mesma da coluna de categoria e da fita de equipes do rodapé, e
// isso é o ponto: uma faixa que fosse de PONTO A PONTO da série comeria meia
// coluna de cada lado, e acabaria no MEIO do ano em que o vínculo voltou — meio
// passo depois de onde o bloco daquele ano começa. Duas marcas falando do mesmo
// ano, cada uma parando num lugar.
//
// A borda do plot corta o que passar dela: a primeira e a última temporada valem
// meia coluna no desenho, exatamente como valem na fita de baixo.
export function faixasSemVinculo(pontos, { x, passo, padE, larguraPlot }, temVinculo) {
  return blocosSemVinculo(pontos, temVinculo).map((bloco) => ({
    ...bloco,
    esquerda: Math.max(padE, x(bloco.inicio) - passo / 2),
    direita: Math.min(padE + larguraPlot, x(bloco.fim) + passo / 2),
  }));
}

// Blocos de temporadas seguidas na mesma categoria — as colunas do fundo.
export function trechosDeCategoria(pontos) {
  const trechos = [];
  pontos.forEach((ponto, indice) => {
    const atual = trechos[trechos.length - 1];
    if (atual && atual.categoria === ponto.categoria) {
      atual.fim = indice;
      return;
    }
    trechos.push({ categoria: ponto.categoria, inicio: indice, fim: indice });
  });
  return trechos.filter((trecho) => trecho.categoria);
}

// As colunas de categoria já medidas no eixo.
//
// Com o rótulo de pé não há mais decisão de caber ou não caber: o nome corre na
// altura do plot, que é a mesma para toda coluna, e a largura deixou de ser o
// que decide se a categoria é nomeada. Foi essa dependência que mandava o degrau
// de uma temporada só para a legenda.
export function colunasDeCategoria(pontos, { x, passo, padE, larguraPlot }) {
  return trechosDeCategoria(pontos).map((trecho) => {
    const esquerda = Math.max(padE, x(trecho.inicio) - passo / 2);
    return {
      ...trecho,
      esquerda,
      largura: Math.min(padE + larguraPlot, x(trecho.fim) + passo / 2) - esquerda,
      label: formatCategoryLabel(trecho.categoria),
      cor: getCategoryColor(trecho.categoria),
    };
  });
}

// Amarelo a 8% pesa muito mais que roxo a 8%. Sem compensar, a categoria mais
// clara da carreira vira a mais importante do gráfico só por ser clara — e a
// escada tem tanto o #FFD400 da Rookie quanto o #8020D0 da Production. O alpha
// desce conforme a luminância da cor sobe, e duas colunas vizinhas chegam ao
// mesmo peso de fundo.
//
// Derivado da cor, e não tabelado por categoria: categoria nova entra na paleta
// sem precisar passar por aqui.
export function opacidadeDaColuna(cor) {
  return Math.min(0.086, Math.max(0.048, 0.09 - 0.056 * luminanciaRelativa(cor)));
}

export function luminanciaRelativa(hex) {
  const bruto = String(hex).replace("#", "");
  if (bruto.length !== 6) return 0.5;
  const canais = [0, 2, 4].map((inicio) => {
    const v = parseInt(bruto.slice(inicio, inicio + 2), 16) / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * canais[0] + 0.7152 * canais[1] + 0.0722 * canais[2];
}

// Os anos em que ele estreou por uma equipe diferente da anterior.
//
// A comparação pula as temporadas sem equipe em vez de zerar a memória: quem
// saiu da Arclight, passou um ano fora do grid e voltou pela Kitsune trocou de
// equipe uma vez, não duas — e não trocou ao ficar sem contrato.
//
// O valor dos dois lados vem junto porque é a pergunta seguinte à troca, e a
// resposta já está na mão: sem ela o balão diz que ele mudou de casa e cala
// justamente sobre o que o gráfico inteiro trata. Qual valor é esse, quem lê a
// série decide — salário numa curva, posição na outra.
export function trocasDeEquipe(pontos, ler) {
  const trocas = [];
  let ultima = null;
  pontos.forEach((ponto, indice) => {
    const equipe = ponto.equipe_nome || null;
    if (!equipe) return;
    if (ultima !== null && equipe !== ultima.nome) {
      trocas.push({
        indice,
        de: ultima.nome,
        para: equipe,
        ano: ponto.ano,
        valorDe: ultima.valor,
        valorPara: ler(ponto),
      });
    }
    ultima = { nome: equipe, valor: ler(ponto) };
  });
  return trocas;
}

// Por quem ele correu em cada trecho. O ano sem equipe PARTE o trecho, ao
// contrário da contagem de trocas logo acima — e a diferença é de propósito.
//
// A contagem responde "quantas vezes ele mudou de casa", e voltar para a mesma
// equipe depois de um ano fora não é mudar. O chip responde "por quem ele
// correu neste ano", e o ano fora do grid tem uma resposta própria: ninguém.
// Um chip atravessando o vão contaria uma continuidade que não houve, e ainda
// tomaria o lugar do tracejado que diz o que aconteceu ali.
export function trechosDeEquipe(pontos) {
  const trechos = [];
  pontos.forEach((ponto, indice) => {
    const equipe = ponto.equipe_nome || null;
    if (!equipe) return;
    const ultimo = trechos[trechos.length - 1];
    if (ultimo && ultimo.equipe === equipe && ultimo.fim === indice - 1) {
      ultimo.fim = indice;
      return;
    }
    trechos.push({ equipe, cor: ponto.equipe_cor ?? null, inicio: indice, fim: indice });
  });
  return trechos;
}

// O avesso do de cima: as corridas de temporadas seguidas SEM equipe.
export function lacunasDeEquipe(pontos) {
  const lacunas = [];
  pontos.forEach((ponto, indice) => {
    if (ponto.equipe_nome) return;
    const ultima = lacunas[lacunas.length - 1];
    if (ultima && ultima.fim === indice - 1) {
      ultima.fim = indice;
      return;
    }
    lacunas.push({ inicio: indice, fim: indice });
  });
  return lacunas;
}

// O chip vestindo a cor da casa. Puxa o neutro na direção da equipe em vez de
// pintar a cor cheia: a fita tem até oito chips em sequência, e oito retângulos
// saturados atrás de oito logos são oito fundos brigando com a arte que deveriam
// sustentar. O que se quer é reconhecer a casa de relance, não ler a cor.
//
// O peso da mistura cai conforme a cor clareia, pelo mesmo motivo da coluna de
// categoria: sem isso a equipe amarela vira um bloco luminoso no meio de
// vizinhos discretos, e a fita passa a dizer que aquela casa importou mais.
//
// Sem cor — save antigo, equipe dissolvida — o chip fica no neutro de sempre.
export const CHIP_NEUTRO = { preenchimento: "#16232f", contorno: "rgba(255,255,255,0.09)" };

export function tomDoChip(cor) {
  if (!/^#[0-9a-f]{6}$/i.test(String(cor ?? ""))) return CHIP_NEUTRO;
  const peso = Math.min(0.3, Math.max(0.12, 0.3 - 0.2 * luminanciaRelativa(cor)));
  return {
    preenchimento: misturarCores(CHIP_NEUTRO.preenchimento, cor, peso),
    contorno: misturarCores(CHIP_NEUTRO.preenchimento, cor, Math.min(0.62, peso * 2.2)),
  };
}

// Mistura em hex, e não uma cor com alfa por cima: o chip é desenhado sobre a
// hachura do ano sem vínculo em alguns casos, e um fundo translúcido deixaria a
// listra diagonal aparecer por dentro dele.
export function misturarCores(base, alvo, peso) {
  const canais = (hex) =>
    [1, 3, 5].map((inicio) => parseInt(String(hex).slice(inicio, inicio + 2), 16));
  const [rb, gb, bb] = canais(base);
  const [ra, ga, ba] = canais(alvo);
  const componente = (deBase, deAlvo) =>
    Math.round(deBase + (deAlvo - deBase) * peso)
      .toString(16)
      .padStart(2, "0");
  return `#${componente(rb, ra)}${componente(gb, ga)}${componente(bb, ba)}`;
}

// "Silver Peak Performance" vira SPP. Nome de uma palavra dá as três primeiras
// letras em vez da inicial solta: "A" não distingue nada num mundo com Arclight,
// Aures e Aichi.
export function monogramaDaEquipe(nome) {
  const palavras = String(nome ?? "").trim().split(/\s+/).filter(Boolean);
  if (!palavras.length) return "";
  if (palavras.length === 1) return palavras[0].slice(0, 3).toUpperCase();
  return palavras
    .slice(0, 3)
    .map((palavra) => palavra[0])
    .join("")
    .toUpperCase();
}

// A ponte que atravessa o ano fora do grid, ligando os dois pontos das margens.
//
// Só há ponte onde há as DUAS margens: um vão que começa na estreia ou que
// alcança a temporada corrente tem um lado só, e um traço saindo do nada para
// lugar nenhum não liga coisa alguma — ali a faixa hachurada basta.
//
// O traço fica CONTIDO na hachura, e não solto até o marcador. Onde a faixa
// recuou para devolver a coluna a um ano com vínculo, um pedaço de tracejado
// correndo fora dela ficava sem chão: o que ele atravessa é o vão, e fora do vão
// ele não tem o que dizer.
//
// A altura sai da reta cheia entre os dois pontos, interpolada no x em que o
// traço realmente começa e termina: cortar em x sem recalcular o y inclinaria a
// ponte e ela apontaria para fora dos marcadores que liga.
export function pontesSemVinculo(faixas, pontos, ler) {
  return faixas
    // ...e onde a série realmente se interrompe. O vão é definido pelo VÍNCULO,
    // e nem toda série morre com ele: a expectativa do carro pode existir num
    // ano em que ele não tem equipe. Ali a linha já atravessa cheia, e uma ponte
    // por cima seria o mesmo trecho desenhado duas vezes, um pontilhado
    // contradizendo o traço que ele cobre.
    .filter((faixa) =>
      pontos
        .slice(faixa.inicio, faixa.fim + 1)
        .every((ponto) => !Number.isFinite(ler(ponto))),
    )
    .map((faixa) => ({ ...faixa, de: faixa.inicio - 1, para: faixa.fim + 1 }))
    .filter(({ de, para }) => de >= 0 && para < pontos.length)
    .map((faixa) => ({
      ...faixa,
      valorDe: ler(pontos[faixa.de]),
      valorPara: ler(pontos[faixa.para]),
    }))
    .filter((ponte) => Number.isFinite(ponte.valorDe) && Number.isFinite(ponte.valorPara));
}

// A ponte partida em três pela hachura: cheia enquanto pisa em ano com vínculo,
// pontilhada só na travessia do vão.
//
// O corte é a borda da faixa e não a coluna do ano vazio, porque é a hachura que
// nomeia o período no desenho: traço mudando de estilo num x diferente de onde a
// hachura começa lê-se como desalinhamento, não como fronteira.
export function segmentosDaPonte(ponte, { x, y }) {
  const xDe = x(ponte.de);
  const xPara = x(ponte.para);
  const yDe = y(ponte.valorDe);
  const yPara = y(ponte.valorPara);
  const alturaEm = (px) =>
    xPara === xDe ? yDe : yDe + ((yPara - yDe) * (px - xDe)) / (xPara - xDe);
  const trecho = (x1, x2) => ({ x1, y1: alturaEm(x1), x2, y2: alturaEm(x2) });

  const inicio = Math.max(xDe, Math.min(ponte.esquerda, xPara));
  const fim = Math.min(xPara, Math.max(ponte.direita, xDe));
  return {
    // Trecho cheio de menos de meio pixel é sujeira de arredondamento, não linha.
    antes: inicio - xDe > 0.5 ? trecho(xDe, inicio) : null,
    vao: trecho(inicio, fim),
    depois: xPara - fim > 0.5 ? trecho(fim, xPara) : null,
  };
}

// A ponte desenhada, com a mesma gramática nos dois gráficos: cheia onde há
// medida, pontilhada onde não há.
//
// Recebe `x` e `y` soltos, e não a geometria inteira: o `x` é da moldura (a
// régua dos anos, igual nos dois gráficos) mas o `y` é a escala de CADA série —
// logarítmica em dólar, linear e invertida em posição.
export function PonteSobreOVao({ ponte, x, y, cor }) {
  const partes = segmentosDaPonte(ponte, { x, y });
  return (
    <g>
      {[partes.antes, partes.depois].map((parte, lado) =>
        parte ? (
          <line
            key={lado}
            data-ponte="contratado"
            {...parte}
            stroke={cor}
            strokeWidth="2"
            strokeLinecap="round"
          />
        ) : null,
      )}
      <line
        data-ponte="sem-contrato"
        {...partes.vao}
        stroke={cor}
        strokeWidth="2"
        strokeDasharray="4 4"
        opacity="0.8"
      />
    </g>
  );
}

// A faixa entre as duas séries — o objeto que carrega a leitura nos dois
// gráficos: a distância entre o que foi e o que deveria ter sido.
//
// Só existe onde há os DOIS lados para comparar; um ano sem um deles não tem
// diferença a preencher, e a faixa se parte ali junto com as linhas.
export function faixasDeDiferenca(pontos, lerTopo, lerBase) {
  return segmentosContinuos(pontos, (ponto) =>
    Number.isFinite(lerBase(ponto)) ? lerTopo(ponto) : NaN,
  ).map((trecho) => ({ inicio: trecho.inicio, trecho: trecho.itens }));
}

export function caminhoDaFaixa(itens, geo, y, inicio, lerTopo, lerBase) {
  const topo = itens.map((ponto, indice) => `${geo.xSerie(inicio + indice)},${y(lerTopo(ponto))}`);
  const base = itens
    .map((ponto, indice) => `${geo.xSerie(inicio + indice)},${y(lerBase(ponto))}`)
    .reverse();
  // A faixa fecha junto com as linhas que ela separa. Sem isso a distância entre
  // as duas séries morreria na borda esquerda da última coluna, e o ano que a
  // faixa mais precisa explicar — o que está acontecendo — ficaria em branco.
  if (fechaASerie({ inicio, itens }, geo)) {
    const ultimo = itens[itens.length - 1];
    topo.push(`${geo.fimDoPlot},${y(lerTopo(ultimo))}`);
    base.unshift(`${geo.fimDoPlot},${y(lerBase(ultimo))}`);
  }
  return `M${topo.join("L")}L${base.join("L")}Z`;
}

// O trecho vai até a última temporada do eixo, e portanto é ele quem fecha a
// série na borda direita do plot.
function fechaASerie(trecho, geo) {
  return trecho.inicio + trecho.itens.length - 1 === geo.ultimo;
}

// Os vértices de um trecho da série, com o FECHO da última temporada.
//
// O marcador de cada temporada mora no COMEÇO da coluna dela, e o traço que sai
// dele atravessa essa coluna até o marcador do ano seguinte — o segmento é o
// ano. A última temporada não tem ano seguinte, então não teria traço nenhum: o
// marcador ficaria sozinho na borda esquerda de uma coluna vazia, que é
// exatamente o que a régua anterior fazia com a coluna de estreia.
//
// O fecho é um vértice a mais na borda direita do plot, na altura do próprio
// último ponto. Reto de propósito: a temporada corre com o valor que tem, e não
// há medida seguinte para inclinar coisa alguma.
export function verticesDaSerie(trecho, geo, y, ler) {
  const vertices = trecho.itens.map(
    (ponto, indice) => `${geo.xSerie(trecho.inicio + indice)},${y(ler(ponto))}`,
  );
  // `fecho: false` é o corte no presente ou por estilo dizendo que outro trecho
  // continua dali: fechado duas vezes, o traço cheio ficaria empilhado sob o
  // fantasma.
  if (trecho.fecho !== false && fechaASerie(trecho, geo)) {
    vertices.push(`${geo.fimDoPlot},${y(ler(trecho.itens[trecho.itens.length - 1]))}`);
  }
  return vertices.join(" ");
}

// Onde o balão encosta, em porcentagem do quadro — o SVG escala com a largura do
// modal, então posição em pixels descolaria do ponto.
//
// O balão NÃO é colado no ponto, e isso é deliberado: ele tem umas cinco linhas
// num gráfico de 190px de altura, então pendurá-lo acima do ponto o joga para
// fora pelo topo e pendurá-lo abaixo o joga para fora pelo rodapé do card —
// medi as duas coisas acontecendo. Em vez disso ele se encosta na borda
// horizontal OPOSTA à metade em que os pontos estão, o que garante que nunca
// cubra o que se está lendo e nunca vaze. A linha do crosshair é quem faz a
// ligação com a temporada.
//
// `alturas` já vem em pixels do plot: quem chama sabe quais séries tem.
export function ancoraDoBalao(alturas, indice, { x, w, h }) {
  const topo = (alturas.length ? Math.min(...alturas) : h / 2) / h;
  const fracaoX = x(indice) / w;
  const pontoNoAlto = topo < 0.45;

  return {
    esquerda: `${fracaoX * 100}%`,
    vertical: pontoNoAlto ? { bottom: 0 } : { top: 0 },
    // Encostado numa borda lateral o balão ancora por ela; no miolo, centraliza.
    transform: `translateX(${fracaoX > 0.7 ? "-100%" : fracaoX < 0.3 ? "0%" : "-50%"})`,
  };
}

// ── As peças desenhadas ──

// A hachura do ano sem vínculo, declarada uma vez por gráfico.
export function HachuraDefs() {
  return (
    <defs>
      {/* Hachura, e não preenchimento chapado: um bloco sólido no meio do
          gráfico se lê como uma SÉRIE a mais. Listra diagonal é a convenção de
          "aqui não há medida", e some no fundo. */}
      <pattern
        id={HACHURA_SEM_VINCULO}
        width="7"
        height="7"
        patternUnits="userSpaceOnUse"
        patternTransform="rotate(45)"
      >
        <line x1="0" y1="0" x2="0" y2="7" stroke="rgba(255,255,255,0.13)" strokeWidth="1.5" />
      </pattern>
    </defs>
  );
}

// O fundo da cena: colunas de categoria, faixas de ano sem vínculo e as réguas
// das trocas de casa. Tudo que fica ATRÁS da série.
//
// `reguaDoEixo` entra ENTRE as colunas e a hachura, e não antes ou depois de
// tudo: a régua do Y é de cada gráfico (dólar num, posição no outro), mas o
// lugar dela na pilha é o mesmo — por cima do tinte da categoria, por baixo do
// território hachurado que nomeia o ano fora do grid.
export function MolduraDeFundo({
  geo,
  colunas,
  faixas,
  trocas,
  trocaEmFoco,
  rotuloSemVinculo,
  reguaDoEixo = null,
}) {
  const { padE, padT, alturaPlot, x, passo } = geo;

  return (
    <>
      {/* A coluna da categoria, ao fundo de tudo. Responde "de qual disputa
          estamos falando" no lugar onde ela está desenhada — um número na
          escada de entrada e o mesmo número na categoria de cima são carreiras
          opostas, e a resposta morava num rodapé separado do gráfico que ela
          qualifica.

          O nome vai escrito de pé na própria coluna, e é isso que aposenta a
          legenda de categorias. */}
      {colunas.map((coluna) => (
        <g key={`coluna-${coluna.inicio}`} data-coluna="categoria" data-categoria={coluna.categoria}>
          <rect
            x={coluna.esquerda}
            y={padT}
            width={coluna.largura}
            height={alturaPlot}
            fill={coluna.cor}
            opacity={opacidadeDaColuna(coluna.cor)}
          />
          {/* Sem o friso a coluna diz que houve uma troca de degrau e cala sobre
              QUAL degrau: a 6% de opacidade as cores da paleta convergem todas
              para o mesmo cinza-azulado. */}
          <rect
            data-coluna="friso"
            x={coluna.esquerda}
            y={padT + alturaPlot - CURVA_COLUNA.friso}
            width={coluna.largura}
            height={CURVA_COLUNA.friso}
            fill={coluna.cor}
            opacity={CURVA_COLUNA.opacidadeDoFriso}
          />
          {/* A divisa entre duas colunas. Dois tintes vizinhos de luminância
              parecida encostam sem costura visível, e a carreira inteira vira um
              fundo só. */}
          {coluna.esquerda > padE + 0.5 ? (
            <line
              x1={coluna.esquerda}
              x2={coluna.esquerda}
              y1={padT}
              y2={padT + alturaPlot}
              stroke="rgba(255,255,255,0.09)"
              strokeWidth="1"
            />
          ) : null}
          {/* O nome, SEMPRE de pé.

              Deitado ele só cabia nas colunas largas, e a coluna que não o
              comportava caía numa legenda lá embaixo — o jogador tinha de
              procurar a cor numa lista para descobrir em que categoria ele
              estava correndo, que é justo a pergunta que a coluna existe para
              responder. De pé, o rótulo não depende da largura: sobra altura de
              plot para o nome mais longo da escada, e a mesma marca serve a um
              degrau de uma temporada e a um de dez.

              Ancorado embaixo, junto ao friso da própria cor, e recuado da borda
              esquerda: é ali que ele se lê como a abertura da coluna, e não como
              uma anotação solta no meio do gráfico. O recuo encolhe junto com a
              coluna estreita, senão o rótulo sairia por fora dela. */}
          {(() => {
            const eixo = coluna.esquerda + Math.min(CURVA_COLUNA.recuoDoRotulo, coluna.largura / 2);
            const base = padT + alturaPlot - CURVA_COLUNA.rotuloDePe;
            return (
              <text
                data-rotulo="categoria"
                x={eixo}
                y={base}
                textAnchor="start"
                transform={`rotate(-90 ${eixo} ${base})`}
                fill={coluna.cor}
                opacity="0.9"
                className="text-[8.5px] font-bold uppercase tracking-[0.1em]"
              >
                {coluna.label}
              </text>
            );
          })()}
        </g>
      ))}

      {reguaDoEixo}

      {/* Ano sem equipe não é lacuna de dado — é o que aconteceu com ele. Então
          ocupa espaço no gráfico, hachurado e nomeado no lugar, em vez de virar
          um vão mudo que se lê como bug. Vai atrás das linhas: é fundo de cena,
          não protagonista. */}
      {faixas.map((faixa) => {
        const { esquerda, direita } = faixa;
        const largura = direita - esquerda;
        return (
          <g key={`sem-vinculo-${faixa.inicio}`} data-faixa="sem-contrato">
            <rect
              x={esquerda}
              y={padT}
              width={largura}
              height={alturaPlot}
              fill={`url(#${HACHURA_SEM_VINCULO})`}
            />
            <rect
              x={esquerda}
              y={padT}
              width={largura}
              height={alturaPlot}
              fill="none"
              stroke="rgba(255,255,255,0.09)"
              strokeWidth="1"
            />
            {/* De pé no meio da faixa e no fundo da cena, junto com a hachura: é
                rótulo de território, não anotação de um ponto. Horizontal não
                caberia na fatia de um ano só, e montado na ponte exigia um
                recorte que abria um buraco no meio do gráfico. Aqui as linhas
                passam por cima e ele não disputa leitura com o dado. */}
            <text
              x={esquerda + largura / 2}
              y={padT + alturaPlot / 2}
              textAnchor="middle"
              transform={`rotate(-90 ${esquerda + largura / 2} ${padT + alturaPlot / 2})`}
              className="fill-[#8b949e] text-[8px] uppercase tracking-[0.12em]"
            >
              {rotuloSemVinculo}
            </text>
          </g>
        );
      })}

      {/* A régua da troca de equipe, do topo do plot até o chip do rodapé. O
          losango que ela substitui vivia na tira de baixo, longe do degrau da
          linha que ele explica — e precisava de uma chave na legenda para dizer
          o que era. A régua não precisa: ela encosta na emenda de dois chips, e
          a emenda diz sozinha. */}
      {trocas.map((troca) => (
        <line
          key={`regua-troca-${troca.indice}`}
          data-marca="troca-equipe"
          x1={x(troca.indice) - passo / 2}
          x2={x(troca.indice) - passo / 2}
          y1={padT}
          y2={CURVA_FITA.y + CURVA_FITA.altura}
          stroke={trocaEmFoco === troca.indice ? "rgba(255,255,255,0.55)" : CURVA_REGUA_TROCA.cor}
          strokeWidth="1"
          strokeDasharray={CURVA_REGUA_TROCA.traco}
        />
      ))}
    </>
  );
}

// O rodapé: a régua dos anos, a fita de equipes e os alvos de hover da emenda.
export function MolduraDeRodape({ geo, pontos, trocas, emFoco, setTrocaEmFoco }) {
  const { padE, padT, alturaPlot, larguraPlot, x, passo } = geo;

  return (
    <>
      {pontos.map((ponto, indice) => (
        <text
          key={`ano-${ponto.season_number}`}
          x={x(indice)}
          y={CURVA_Y_ANOS}
          textAnchor="middle"
          className={`font-mono text-[9px] tabular-nums ${
            indice === emFoco ? "fill-[#c9d1d9]" : "fill-[#6e7681]"
          }`}
        >
          {ponto.ano}
        </text>
      ))}

      {/* O ano fora do grid: um traço, e nada mais.

          Um chip tracejado ali era um chip — a mesma caixa dos vínculos, com o
          mesmo peso, dizendo que houve alguma coisa. O que houve foi nada, e
          nada se desenha como linha, não como recipiente vazio. A hachura logo
          acima já nomeia o ano; a fita só precisa não fingir que a sequência de
          casas continua. */}
      {lacunasDeEquipe(pontos).map((lacuna) => {
        const esquerda = Math.max(padE, x(lacuna.inicio) - passo / 2) + CURVA_FITA.folga;
        const direita =
          Math.min(padE + larguraPlot, x(lacuna.fim) + passo / 2) - CURVA_FITA.folga;
        if (direita <= esquerda) return null;
        return (
          <line
            key={`sem-equipe-${lacuna.inicio}`}
            data-fita="sem-equipe"
            x1={esquerda}
            x2={direita}
            y1={CURVA_FITA.y + CURVA_FITA.altura / 2}
            y2={CURVA_FITA.y + CURVA_FITA.altura / 2}
            stroke="rgba(255,255,255,0.18)"
            strokeWidth="1"
            strokeDasharray="3 3"
          />
        );
      })}

      {/* A fita de equipes. Um chip por vínculo, com a logo centrada e nada mais
          escrito: a arte JÁ é o nome da casa, e o nome ao lado dela dizia a
          mesma coisa duas vezes ocupando a fita inteira. Quem não reconhecer a
          marca tem o balão, que é onde o nome por extenso sempre esteve.

          O chip cobre exatamente o período do vínculo, então o comprimento
          responde "por quanto tempo?". E a emenda entre dois chips é a troca de
          equipe: dois retângulos separados por uma folga dizem "aqui mudou" sem
          marca própria e sem legenda.

          O fundo puxa para a COR DA EQUIPE, não para a da categoria: a categoria
          já está na coluna atrás do gráfico, e repeti-la aqui poria a mesma
          informação duas vezes na mesma vertical. Puxa, e não copia — cor cheia
          atrás de uma logo é fundo brigando com arte, e a fita tem oito casas em
          sequência. */}
      <g pointerEvents="none">
        {trechosDeEquipe(pontos).map((trecho) => {
          const esquerda = Math.max(padE, x(trecho.inicio) - passo / 2) + CURVA_FITA.folga;
          const largura =
            Math.min(padE + larguraPlot, x(trecho.fim) + passo / 2) - CURVA_FITA.folga - esquerda;
          if (largura <= 0) return null;
          const meio = CURVA_FITA.y + CURVA_FITA.altura / 2;
          const arte = getTeamLogoSrc(trecho.equipe);
          const tom = tomDoChip(trecho.cor);
          // A arte encolhe até caber com um de respiro de cada lado: a primeira
          // e a última temporada valem meia coluna, e numa carreira de vinte
          // temporadas esse chip tem 12 de largura.
          const larguraLogo = Math.min(CURVA_CHIP.logo, largura - 2);
          const alturaLogo = (larguraLogo * 2) / 3;
          const centroLogo = esquerda + largura / 2;
          return (
            <g key={`equipe-${trecho.inicio}`} data-fita="equipe" data-equipe={trecho.equipe}>
              <title>{trecho.equipe}</title>
              <rect
                x={esquerda}
                y={CURVA_FITA.y}
                width={largura}
                height={CURVA_FITA.altura}
                rx={CURVA_FITA.raio}
                fill={tom.preenchimento}
                stroke={tom.contorno}
                strokeWidth="1"
              />
              {/* Chip estreito demais até para a arte fica só com o contorno: ele
                  ainda diz que ali houve uma casa e por quanto tempo, e o balão
                  diz qual foi. Sumir com tudo era perder a temporada inteira do
                  desenho. */}
              {larguraLogo < CURVA_CHIP.logoMin ? null : arte ? (
                <image
                  href={arte}
                  x={centroLogo - larguraLogo / 2}
                  y={meio - alturaLogo / 2}
                  width={larguraLogo}
                  height={alturaLogo}
                  preserveAspectRatio="xMidYMid meet"
                  // Metade das 102 artes é desenhada em preto: sobre o fundo do
                  // chip elas somem sem o halo.
                  style={{ filter: HALO_FILTER }}
                />
              ) : (
                // Sem arte, o monograma. Chip vazio ao lado de chip com logo
                // lê-se como falha de carregamento, não como "esta equipe não
                // tem logo".
                <text
                  x={centroLogo}
                  y={meio + 3}
                  textAnchor="middle"
                  className="fill-[#f0f6fc] text-[9px] font-semibold uppercase tracking-[0.08em]"
                >
                  {monogramaDaEquipe(trecho.equipe)}
                </text>
              )}
            </g>
          );
        })}
      </g>

      {/* O alvo do balão da troca. Fica CONTIDO no rodapé, do fim do plot até a
          base do chip, e não ao longo da régua inteira: a régua cruza a área de
          hover das temporadas, e um alvo de 18 unidades subindo por ela roubaria
          o balão da temporada numa faixa vertical do gráfico. */}
      {trocas.map((troca) => {
        const cx = x(troca.indice) - passo / 2;
        return (
          <rect
            key={`alvo-troca-${troca.indice}`}
            data-alvo="troca"
            x={cx - 9}
            y={padT + alturaPlot}
            width="18"
            height={CURVA_FITA.y + CURVA_FITA.altura - padT - alturaPlot}
            fill="transparent"
            onMouseEnter={() => setTrocaEmFoco(troca.indice)}
            onMouseLeave={() => setTrocaEmFoco(null)}
          />
        );
      })}
    </>
  );
}

// O crosshair e os alvos de hover: o ALVO é a fatia inteira da temporada, mas o
// crosshair cai em cima do marcador — ele existe para ligar o balão ao ponto, e
// no meio da coluna ele apontaria para um lugar onde não há dado.
export function AlvosDeTemporada({ geo, pontos, emFoco, setEmFoco }) {
  const { padT, alturaPlot, x, xSerie, passo } = geo;

  return (
    <>
      {emFoco !== null ? (
        <line
          x1={xSerie(emFoco)}
          x2={xSerie(emFoco)}
          y1={padT}
          y2={padT + alturaPlot}
          stroke="rgba(255,255,255,0.22)"
          strokeWidth="1"
        />
      ) : null}

      {pontos.map((ponto, indice) => (
        <rect
          key={`alvo-${ponto.season_number}`}
          data-alvo="temporada"
          x={x(indice) - passo / 2}
          y={padT}
          width={passo}
          height={alturaPlot}
          fill="transparent"
          onMouseEnter={() => setEmFoco(indice)}
          // Cada alvo apaga o que acendeu. Só o `onMouseLeave` do SVG não basta:
          // a fita e os anos ficam FORA da faixa de alvos mas DENTRO do SVG,
          // então descer o cursor até a emenda de troca nunca disparava a saída —
          // e o balão da temporada ficava grudado na tela ao lado do da troca.
          onMouseLeave={() => setEmFoco(null)}
        />
      ))}
    </>
  );
}

// O fio em volta da amostra de cor.
//
// Sem ele a chave de uma série ESCURA é um buraco: um quadrado preto sobre o
// #0f1c2b do cartão não tem borda que o distinga do fundo, e a legenda passa a
// ter um item invisível bem ao lado de um visível. O fio custa nada nas cores
// claras — some sob o próprio preenchimento — e é o que torna a chave achável
// nas escuras.
const FIO_DA_AMOSTRA = "h-2 w-2 shrink-0 rounded-sm ring-1 ring-inset ring-white/20";

export function SerieKey({ cor, label, tracejada = false }) {
  return (
    <span className="flex items-center gap-1.5 text-[11px] text-text-secondary">
      <span
        className={tracejada ? "h-0 w-3 shrink-0 border-t border-dashed" : FIO_DA_AMOSTRA}
        style={tracejada ? { borderColor: cor } : { backgroundColor: cor }}
      />
      {label}
    </span>
  );
}

export function LinhaDoBalao({ cor, label, valor }) {
  return (
    <div className="flex items-baseline justify-between gap-4 text-[11px]">
      <span className="flex items-center gap-1.5 text-text-secondary">
        <span className={FIO_DA_AMOSTRA} style={{ backgroundColor: cor }} />
        {label}
      </span>
      <span className="shrink-0 font-mono tabular-nums text-text-primary">{valor}</span>
    </div>
  );
}

// O balão da emenda: de qual equipe para qual equipe, com o ano da estreia pela
// nova. Empilhado em duas linhas em vez de lado a lado porque nome de equipe é
// longo — "Sunday Speed Club → Silver Peak Performance" na horizontal ou estoura
// o balão ou trunca os dois nomes ao mesmo tempo.
//
// O rodapé (o que mudou com a troca) vem pronto de quem chama: numa curva é
// dinheiro a mais, na outra são posições ganhas, e as duas leituras têm sinais
// opostos — subir de P8 para P3 é um número que DIMINUI.
export function TrocaTooltip({ troca, esquerda, topo, formatarValor, rodape = null }) {
  const { t } = useTranslation();

  return (
    <div
      className="pointer-events-none absolute z-10 w-max max-w-[250px] -translate-x-1/2 -translate-y-[112%] rounded-lg border border-white/10 bg-[#0b1622] px-3 py-2 shadow-xl shadow-black/50"
      style={{ left: esquerda, top: topo }}
      data-testid="driver-detail-curve-troca-tooltip"
    >
      {/* Título antes de tudo: dois logos empilhados com uma seta minúscula
          entre eles obrigavam o jogador a deduzir o que tinha acontecido. */}
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-[10px] font-semibold uppercase tracking-[0.08em] text-text-secondary">
          {t("driverDetail.market.curve.teamChange")}
        </span>
        <span className="shrink-0 font-mono text-[10px] tabular-nums text-text-muted">
          {troca.ano}
        </span>
      </div>

      <div className="mt-1.5 space-y-1">
        <LinhaDaTroca
          rotulo={t("driverDetail.market.curve.leftTeam")}
          equipe={troca.de}
          valor={troca.valorDe}
          formatarValor={formatarValor}
        />
        <LinhaDaTroca
          rotulo={t("driverDetail.market.curve.joinedTeam")}
          equipe={troca.para}
          valor={troca.valorPara}
          formatarValor={formatarValor}
          destaque
        />
      </div>

      {rodape}
    </div>
  );
}

// "Saiu" e "Assinou" no lugar da seta: verbo diz o que aconteceu, seta pede
// interpretação. O rótulo tem largura fixa para os dois logos alinharem em
// coluna, e a linha de saída fica apagada — é passado, e o destaque pertence à
// equipe com que ele assinou.
function LinhaDaTroca({ rotulo, equipe, valor, formatarValor, destaque = false }) {
  return (
    <div className={`flex items-center gap-1.5 ${destaque ? "" : "opacity-60"}`}>
      <span className="w-11 shrink-0 text-[9px] uppercase tracking-[0.06em] text-text-muted">
        {rotulo}
      </span>
      <TeamLogoMark teamName={equipe} size="xs" halo testId="driver-detail-curve-troca-logo" />
      <span className="min-w-0 flex-1 truncate text-[11px] text-text-primary">{equipe}</span>
      {Number.isFinite(valor) ? (
        <span className="shrink-0 font-mono text-[10px] tabular-nums text-text-secondary">
          {formatarValor(valor)}
        </span>
      ) : null}
    </div>
  );
}

// O fecho do balão da troca: centralizado porque é fecho, não mais um item da
// lista — alinhado à esquerda ele entrava na coluna dos rótulos acima e lia como
// a quarta linha do mesmo bloco.
export function FechoDoBalao({ cor, children }) {
  return (
    <p
      className="mt-1.5 border-t border-white/[0.08] pt-1.5 text-center text-[10px]"
      style={{ color: cor }}
    >
      {children}
    </p>
  );
}
