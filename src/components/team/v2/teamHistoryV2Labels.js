// Rótulos e dicas do dossiê de equipe v2: o que vira TEXTO na tela.
//
// Extraído de `TeamHistoryDrawerV2.jsx` em 11/08/2026, na mesma leva que criou
// [teamHistoryV2Logic.js] — a vistoria de 10/08 marcou o arquivo como [Alta] (4.400
// linhas, 63 funções internas atrás de um único export) e apontou o caminho que o
// próprio v2 já tinha aberto com `atlasV2Geometry.js` e `gridMetrics.js`.
//
// A divisão entre os três módulos é por PERGUNTA:
//   • `teamHistoryV2Logic` decide CONTEÚDO — ordem, corte e cor derivada de dado;
//   • este decide o TEXTO — o que a dica diz, em que ordem e em quantas linhas;
//   • o que ficou no componente decide DESENHO — SVG, JSX e estado de realce.
//
// O `t` entra por parâmetro em vez de vir de um `useTranslation` interno: é o que
// mantém estas funções puras e testáveis fora do React, e é como o componente já as
// chamava.

import { MEDAL_COLORS } from "./teamHistoryV2Logic";

/// Uma dica no formato que o balão do app desenha.
///
/// O par header/meta em cima, as linhas de contagem embaixo. O `texto` é a mesma
/// informação achatada em `\n`: é o nome acessível do elemento, para quem lê por
/// leitor de tela — e por isso ele usa `textoAcessivel` quando a linha tem os dois
/// (na tela o quadradinho colorido já diz a colocação; no leitor, não existe cor).
export function montarDica(header, meta, linhas) {
  return {
    header,
    meta,
    linhas,
    texto: [
      header,
      meta,
      ...(linhas.length ? ["", ...linhas.map((linha) => linha.textoAcessivel ?? linha.texto)] : []),
    ].join("\n"),
  };
}

/// O par header/meta das temporadas fora do recorte vem colado num só valor de
/// i18n, separado por "\n" — herança de quando o balão era o do sistema. Separar
/// aqui evita duplicar a chave só para mudar quem desenha.
export function dicaDeTexto(texto) {
  const [header, ...resto] = String(texto).split("\n");
  return montarDica(header, resto.join(" ").trim(), []);
}

/// Tooltip da coluna de trajetória, em linhas.
///
/// A versão anterior era uma frase única com tudo separado por "·", incluindo as
/// colocações zeradas ("0× 2º") — ilegível justamente onde o jogador para o mouse
/// para entender a barra. Aqui cada coisa tem sua linha, e só aparece o que
/// aconteceu: a lista de colocações espelha os blocos desenhados, de cima para
/// baixo, na mesma ordem — com a mesma cor do bloco ao lado do texto.
export function seasonTooltip(t, { row, races, topFive, steps, dnfs }) {
  const base = "myTeamTab.history.records.seasonTooltip";
  const header = row.category ? `${row.year} · ${row.category}` : String(row.year);
  const hasPosition = row.position && row.position !== "—";
  const meta = hasPosition
    ? t(`${base}.meta`, { position: row.position, races, topFive })
    : t(`${base}.metaNoPosition`, { races, topFive });
  const linhas = steps.length
    ? steps.map((step) => ({
        id: step.id,
        color: step.color,
        // Na tela, só a contagem: o quadradinho ao lado JÁ é a colocação, na
        // mesma cor do bloco da barra e da legenda embaixo do gráfico. Repetir
        // "1º" ao lado do ouro é dizer duas vezes a mesma coisa num balão que
        // tem quatro linhas.
        //
        // `value` e não `count`: `count` é palavra reservada do i18next e ligaria
        // a máquina de plural, mandando procurar chaves `..._one`/`..._other`.
        texto: t(`${base}.countShort`, { value: step.count }),
        // Para o leitor de tela a cor não existe — ali a colocação continua
        // escrita por extenso.
        textoAcessivel: t(`${base}.count`, {
          value: step.count,
          label: t(`myTeamTab.history.records.medals.${step.id}`),
        }),
      }))
    : [{ id: "empty", color: null, texto: t(`${base}.empty`) }];
  // O abandono entra por último e SÓ quando existe. Ele guarda o rótulo "DNF"
  // porque não é uma colocação: as linhas de cima contam onde a equipe terminou,
  // esta conta o fim de semana em que ela não terminou — e a unidade é CARRO,
  // não corrida (os dois carros podem abandonar no mesmo domingo).
  if (dnfs > 0) {
    linhas.push({
      id: "dnf",
      color: MEDAL_COLORS.dnf,
      texto: t(`${base}.count`, { value: dnfs, label: t("myTeamTab.history.records.medals.dnf") }),
    });
  }
  return montarDica(header, meta, linhas);
}

/// Rótulo de uma faixa da distribuição de resultados: "12 (34%)".
export function rotuloFaixa(t, faixa) {
  return t("myTeamTab.history.sport.spreadValue", { value: faixa.value, percent: faixa.percent });
}

/// Idade do último encontro em linguagem de calendário. A fonte é em SEMANAS
/// porque é assim que o mundo do Loop marca o tempo (`week_of_year`), e a escada
/// sobe conforme a distância: semanas viram meses, meses viram anos. `null` só
/// acontece em payload antigo, e aí o card cala em vez de inventar "há 0 semanas".
export function formatMeetingAge(t, weeksAgo) {
  if (weeksAgo == null) return t("myTeamTab.history.identity.rivalAgeUnknown");
  if (weeksAgo <= 1) return t("myTeamTab.history.identity.rivalAgeNow");
  if (weeksAgo < 9) return t("myTeamTab.history.identity.rivalAgeWeeks", { count: weeksAgo });
  if (weeksAgo < 52) {
    return t("myTeamTab.history.identity.rivalAgeMonths", { count: Math.round(weeksAgo / 4.33) });
  }
  return t("myTeamTab.history.identity.rivalAgeYears", { count: Math.floor(weeksAgo / 52) });
}

/// Chave do elo entre a campanha e a fita: ano + rodada. As duas desenham as
/// MESMAS corridas — a campanha somadas contra o grid, a fita uma a uma — e a
/// rodada é o que elas têm em comum. O ano entra junto porque a fita atravessa
/// temporadas e a campanha é de uma só; sem ele, a rodada 3 do ano passado
/// acenderia a rodada 3 deste.
export function chaveDaRodada(year, round) {
  const ano = Number(year);
  const rodada = Number(round);
  if (!Number.isFinite(ano) || !Number.isFinite(rodada)) return null;
  return `${ano}-${rodada}`;
}

// Chip de posição sobre o ponto da curva de campeonato. Largura estimada pelo
// número de caracteres — "P1" e "P12" não podem dividir a mesma caixa fixa.
export const CHIP_HEIGHT = 17;
export const CHIP_GAP = 13;

export function chipWidth(texto) {
  return 12 + String(texto).length * 6.4;
}

// Abaixo desta distância entre temporadas os chips começam a se encostar, e a
// etiqueta que devia acelerar a leitura vira uma tarja. Aí só os títulos e a
// última temporada fechada continuam rotulados.
export const CHIP_MIN_STEP = 52;
