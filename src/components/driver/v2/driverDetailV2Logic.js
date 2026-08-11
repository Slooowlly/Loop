// Lógica pura da ficha do piloto v2: cores derivadas de resultado, ordenação de traços,
// agrupamento de títulos, leitura do confronto com o rival e a escala do gráfico de salário.
//
// Extraído de `DriverDetailModalV2.jsx` em 11/08/2026. O arquivo tinha 5.505 linhas, ~91
// funções internas e 27 hooks num export só — a vistoria de 10/08 marcou como [Alta] e
// observou o nó: o teste de 4.482 linhas protege o comportamento e ao mesmo tempo trava
// qualquer refatoração barata, porque tudo passa pelo render do modal inteiro.
//
// O corte é o mesmo que o projeto já fez em `atlasV2Geometry.js`, `gridMetrics.js` e
// `teamHistoryV2Logic.js`: sai o que decide CONTEÚDO (ordem, corte, cor derivada de dado),
// fica o que decide DESENHO (JSX, geometria de SVG, estado de realce).

import { formatMoneyCompact } from "../../../utils/formatters";
import { comprimeSequenciasDeAnos } from "../../../utils/sequenciaDeAnos";

/// A paleta de medalha da ficha. As mesmas três cores do dossiê de equipe: um pódio não muda
/// de significado entre as duas telas.
export const MEDAL_COLORS = {
  first: "#f2c46d",
  second: "#c2ccd8",
  third: "#c07f4a",
  nearMiss: "#46586d",
  dnf: "#ef4444",
};

export const DUEL_WIN_COLOR = "#3fb950";
export const DUEL_LOSS_COLOR = "#f85149";

/// Os mesmos tons de `technicalToneClass`, em hex — o que se pinta com `style` (barra,
/// marcador, faixa de fase, fecho de balão) não pode sair de uma classe do Tailwind.
///
/// Mora aqui, e não no modal, porque a curva de mercado saiu para um módulo irmão e continua
/// pintando o mesmo verde e o mesmo amarelo do card ao lado: duas tabelas divergiriam na
/// primeira vez que alguém mexesse num tom.
export const TONE_HEX = {
  danger: "#f85149",
  warning: "#d29922",
  neutral: "#8b949e",
  info: "#58a6ff",
  success: "#3fb950",
  elite: "#bc8cff",
};

/// A cor da marca de uma corrida no histórico.
///
/// O abandono tem cor PRÓPRIA e não é "posição ruim": terminar em 20º e não terminar são
/// coisas diferentes, e a fita existe para deixar isso visível de relance.
export function finishColor(dnf, finish) {
  if (dnf) return MEDAL_COLORS.dnf;
  if (finish === 1) return MEDAL_COLORS.first;
  if (finish === 2) return MEDAL_COLORS.second;
  if (finish === 3) return MEDAL_COLORS.third;
  return MEDAL_COLORS.nearMiss;
}

/// Um valor de 0 a 100 para a régua de atributo, ou `null` quando não é número.
///
/// O grampo existe porque o backend pode entregar acima de 100 depois de um bônus somado, e
/// uma barra de 130% vaza do painel. `null` e não 0: barra vazia e barra ausente contam
/// coisas diferentes.
export function naRegua(valor) {
  const numero = Number(valor);
  return Number.isFinite(numero) ? Math.max(0, Math.min(numero, 100)) : null;
}

export const TRAIT_LEVEL_ORDER = ["elite", "qualidade_alta", "qualidade", "defeito", "defeito_grave"];

/// Ordena os traços dentro do próprio grupo, do mais forte para o mais fraco.
///
/// Nível desconhecido (payload antigo) vai para o FIM do próprio grupo, nunca para a frente:
/// o backend já entrega qualidades e defeitos separados, e é essa separação — não o nível —
/// que decide de que lado da fita o traço cai. O índice original desempata para que a ordem
/// fique estável entre renders.
export function ordenarPorNivel(tags, tone) {
  return (tags ?? [])
    .map((tag, indice) => ({ tag, tone, indice }))
    .sort((a, b) => {
      const nivelA = TRAIT_LEVEL_ORDER.indexOf(a.tag.level);
      const nivelB = TRAIT_LEVEL_ORDER.indexOf(b.tag.level);
      const rankA = nivelA < 0 ? TRAIT_LEVEL_ORDER.length : nivelA;
      const rankB = nivelB < 0 ? TRAIT_LEVEL_ORDER.length : nivelB;
      return rankA - rankB || a.indice - b.indice;
    });
}

/// O primeiro nome, para as frases que falam do piloto em tom direto.
export function primeiroNome(nome) {
  return String(nome || "").trim().split(/\s+/)[0] || "";
}

/// Os títulos agrupados por equipe, cada grupo com os anos em ordem decrescente e os blocos
/// de anos consecutivos comprimidos ("2019–2021" em vez de três chips).
///
/// A ordem entre grupos é pelo título MAIS RECENTE de cada um: a leitura da linha do tempo
/// pede que a equipe do último título abra a lista.
export function groupTitlesByTeam(titles) {
  if (!Array.isArray(titles) || !titles.length) return [];
  const groups = new Map();
  for (const entry of titles) {
    const key = entry?.equipe ?? "";
    if (!groups.has(key)) {
      groups.set(key, { key: key || "sem-equipe", equipe: entry?.equipe ?? null, anos: [] });
    }
    const group = groups.get(key);
    if (Number.isFinite(entry?.ano)) group.anos.push(entry.ano);
  }
  return [...groups.values()]
    .map((group) => ({
      ...group,
      anos: [...group.anos].sort((a, b) => b - a),
      blocos: comprimeSequenciasDeAnos([...group.anos].sort((a, b) => b - a)),
    }))
    .sort((left, right) => (right.anos[0] ?? 0) - (left.anos[0] ?? 0));
}

/// Anos como texto, com os consecutivos comprimidos em intervalo: "2018, 2020–2023".
export function listaDeAnos(anos) {
  const ordenados = [...new Set(anos ?? [])].sort((a, b) => a - b);
  const blocos = [];
  ordenados.forEach((ano) => {
    const ultimo = blocos[blocos.length - 1];
    if (ultimo && ano === ultimo.fim + 1) {
      ultimo.fim = ano;
      return;
    }
    blocos.push({ inicio: ano, fim: ano });
  });
  return blocos
    .map(({ inicio, fim }) => (inicio === fim ? `${inicio}` : `${inicio}–${fim}`))
    .join(", ");
}

/// A sequência CORRENTE de vitórias no confronto, lida de trás para frente.
///
/// Corrida que não decidiu nada (abandono de um dos dois) é TRANSPARENTE em vez de quebrar a
/// série: uma sequência de cinco vitórias com um motor quebrado no meio continua sendo
/// domínio, e chamá-la de "duas e depois duas" seria deixar o azar reescrever a história.
export function sequenciaAtual(encontros) {
  let vencedor = null;
  let total = 0;
  for (let indice = (encontros?.length ?? 0) - 1; indice >= 0; indice -= 1) {
    const atual = encontros[indice].vencedor;
    if (atual !== "piloto" && atual !== "rival") continue;
    if (!vencedor) {
      vencedor = atual;
      total = 1;
      continue;
    }
    if (atual !== vencedor) break;
    total += 1;
  }
  return vencedor ? { vencedor, total } : null;
}

/// Uma coluna por encontro da faixa de confronto, com o índice original preservado — é ele
/// que a legenda usa para saber qual marca está sob o ponteiro.
export function colunasDoConfronto(encontros) {
  return (encontros ?? []).map((encontro, indice) => ({
    indice,
    vencedor: encontro.vencedor,
    gap: Number.isFinite(encontro.gap) ? encontro.gap : null,
  }));
}

/// Os encontros quebrados por temporada. Eles já chegam em ordem cronológica do backend;
/// aqui só ganham a quebra e o índice original.
export function agrupaPorTemporada(encontros) {
  const temporadas = [];
  (encontros ?? []).forEach((encontro, indice) => {
    const ultima = temporadas[temporadas.length - 1];
    const corrida = { ...encontro, indice };
    if (ultima && ultima.season_number === encontro.season_number) {
      ultima.corridas.push(corrida);
      return;
    }
    temporadas.push({
      season_number: encontro.season_number,
      ano: encontro.ano,
      corridas: [corrida],
    });
  });
  return temporadas;
}

/// A cor do saldo do confronto.
///
/// Saldo zero não é vitória nem derrota, e pintá-lo de verde ou vermelho seria dar lado a um
/// empate — daí o `undefined`, que deixa a cor herdada do texto.
export function corDoSaldo(saldo) {
  if (saldo > 0) return DUEL_WIN_COLOR;
  if (saldo < 0) return DUEL_LOSS_COLOR;
  return undefined;
}

/// O saldo como texto, com sinal explícito no positivo.
///
/// "+8" e "8" contam coisas diferentes numa coluna cujo vizinho é negativo. E o sinal de
/// menos é o TIPOGRÁFICO (−), não o hífen: numa coluna monoespaçada o hífen fica alto e curto
/// demais e se lê como travessão de intervalo.
export function formataSaldo(saldo) {
  if (saldo > 0) return `+${saldo}`;
  if (saldo < 0) return `−${Math.abs(saldo)}`;
  return "0";
}

/// A tendência do valor de mercado entre os dois últimos anos MEDIDOS, ou `null`.
///
/// Ponto futuro não conta: ele é projeção, e comparar projeção com medição faria a seta
/// contar o que o modelo acha em vez do que aconteceu.
export function tendenciaDeValor(curva) {
  if (!Array.isArray(curva)) return null;
  const medidos = curva.filter((ponto) => !ponto.futuro && Number.isFinite(ponto.valor_mercado));
  if (medidos.length < 2) return null;

  const atual = medidos[medidos.length - 1];
  const anterior = medidos[medidos.length - 2];
  if (!(anterior.valor_mercado > 0)) return null;

  const variacao = atual.valor_mercado / anterior.valor_mercado - 1;
  // Abaixo de 1% a seta viraria ruído: não foi o piloto que mudou, foi o arredondamento.
  if (Math.abs(variacao) < 0.01) return null;
  return { variacao, ano: anterior.ano, base: anterior.valor_mercado };
}

/// Abaixo deste vão em décadas a escala log vira régua linear: com meia década de amplitude a
/// curva log é indistinguível de uma reta e a marcação em potências de 10 dá uma marca só.
const VAO_LINEAR = 0.5;

/// A escala do gráfico de salário: as marcas do eixo e a fração de altura de um valor.
export function escalaLog(valores) {
  const piso = Math.max(1, Math.min(...valores) / 1.2);
  const teto = Math.max(...valores) * 1.2;
  const lo = Math.log10(piso);
  const hi = Math.log10(teto);
  const vao = Math.max(hi - lo, 0.0001);

  return {
    marcas: vao < VAO_LINEAR ? marcasLineares(piso, teto) : marcasEmDecadas(piso, teto),
    fracao: (valor) => (Math.log10(Math.max(valor, piso)) - lo) / vao,
  };
}

function marcasEmDecadas(piso, teto) {
  const marcas = [];
  for (let expoente = Math.floor(Math.log10(piso)); expoente <= Math.ceil(Math.log10(teto)); expoente += 1) {
    for (const passo of [1, 3]) {
      const marca = passo * 10 ** expoente;
      if (marca >= piso && marca <= teto) marcas.push(marca);
    }
  }
  return marcas;
}

/// Três marcas é o alvo: menos que isso não é régua, mais polui um gráfico de 150px de
/// altura. O 2,5 fica de fora da escada de propósito — com valores na casa dos milhares ele
/// produz passos que o formato compacto arredonda para o mesmo rótulo ($12,5k e $13k viram os
/// dois "$13k"), e duas linhas com a mesma etiqueta são piores do que nenhuma.
function marcasLineares(piso, teto) {
  const bruto = (teto - piso) / 3;
  const magnitude = 10 ** Math.floor(Math.log10(bruto));
  const passo = ([1, 2, 5, 10].find((m) => m * magnitude >= bruto) ?? 10) * magnitude;

  const marcas = [];
  for (let valor = Math.ceil(piso / passo) * passo; valor <= teto; valor += passo) {
    marcas.push(valor);
  }
  // Rede de segurança do arredondamento: em faixas de centenas o passo pode cair abaixo da
  // resolução do rótulo compacto. Sobrevive uma marca por texto.
  const vistos = new Set();
  return marcas.filter((marca) => {
    const rotulo = formatMoneyCompact(marca);
    if (vistos.has(rotulo)) return false;
    vistos.add(rotulo);
    return true;
  });
}
