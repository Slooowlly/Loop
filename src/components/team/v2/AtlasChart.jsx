import { Fragment, useMemo } from "react";
import { useTranslation } from "react-i18next";

import TeamLogoMark from "../TeamLogoMark";
import { teamHighlight, teamTrackToTeamRow } from "../worldTeamChartGeometry";
import { useElementSize } from "./useElementSize";
import {
  LABEL_HEIGHT,
  LABEL_FONT_SIZE,
  LABEL_MAX_WIDTH,
  YEAR_HEADER_HEIGHT,
  bandAccent,
  bandStartDividerX,
  bandUnavailableBox,
  buildAtlasGeometry,
  buildEntryLabels,
  assignCorridorLanes,
  buildTrackSegments,
  crossingIsLong,
  familyPreSeriesBox,
  firstSeriesYear,
  isLivePayload,
  labelConnector,
  liveColumnBox,
  trackVertices,
} from "./atlasV2Geometry";

// Verde do estado "acontecendo agora" — o mesmo do selo no card lateral.
const LIVE_ACCENT = "#4ade80";

// Travessia LONGA — a que percorre boa parte da altura do gráfico — fica sólida só
// nas duas pontas, saindo da divisão antiga e chegando na nova, e some no meio do
// caminho. O miolo dessa vertical passa por cima de campeonatos que não têm nada a
// ver com aquela troca; as travessias curtas não têm miolo a esconder e seguem
// inteiras. Ver `crossingIsLong` — o critério é distância, não faixas puladas.
//
// PARA VOLTAR ATRÁS: troque para `false`. O corredor volta a ser um traço de
// opacidade uniforme, exatamente como antes; nada mais depende desta constante.
const FADE_LONG_CORRIDORS = true;
// Onde o traço já está atenuado, em fração do percurso vertical. Fora desta janela
// ele é sólido: é o que mantém legível de onde a equipe saiu e onde ela chegou.
const CORRIDOR_FADE_START = 0.18;
const CORRIDOR_FADE_END = 0.82;
// Piso do miolo. Já foi 0.05, e 0.05 não é atenuar: multiplicado pela opacidade do
// traço isso dava ~0.02 na tela, ou seja, a travessia sumia. Quem sobe ou cai
// muitas posições é justamente quem tem a travessia mais longa — e era a história
// mais interessante do gráfico que desaparecia.
//
// O miolo continua mais fraco que as pontas: ele passa por cima de campeonatos que
// não têm nada a ver com aquela troca, e não pode competir com as linhas de dados.
// Fraco, porém, é diferente de invisível.
const CORRIDOR_FADE_OPACITY = 0.5;

// Id de gradiente por travessia. O id vai dentro de um `url(#...)`, então tudo que
// não for letra, número ou hífen sai fora — nome de equipe não entra aqui, mas o
// id dela vem do banco e não é garantido ser seguro.
function corridorGradientId(teamId, lineKey, index) {
  return `corridor-${String(teamId).replace(/[^\w-]/g, "")}-${lineKey}-${index}`;
}

// UMA intensidade só. As duas áreas hachuradas são adjacentes (a da família termina
// onde a da categoria começa) e dizem a mesma coisa para quem lê uma linha: aqui não
// houve corrida. Com tons diferentes o encontro delas parecia duas peças sobrepostas
// em vez de hierarquia. Onde a família começou continua marcado pela divisa vertical,
// que é o lugar certo para essa informação.
const HATCH_OUTSIDE = "repeating-linear-gradient(135deg,rgba(148,163,184,0.14) 0 6px,rgba(148,163,184,0.03) 6px 12px)";
// Largura mínima da hachura para caber a identificação textual sem apertar.
const HATCH_LABEL_MIN_WIDTH = 150;
// Fundo sob a hachura — o que de fato separa "sem corridas" de "com corridas". A
// hachura sozinha era leve demais para marcar a divisa; escurecendo a área abaixo
// dela, a mudança se lê antes de qualquer linha. Vai DEPOIS na lista de camadas
// para ficar embaixo, senão apaga a própria hachura que deveria emoldurar.
const OUTSIDE_FILL = "rgba(1,4,10,0.82)";
const OUTSIDE_BACKGROUND = `${HATCH_OUTSIDE}, linear-gradient(${OUTSIDE_FILL},${OUTSIDE_FILL})`;

// Título da faixa: 13px quando cabe na coluna de etiquetas, menor quando não cabe.
// Nome de categoria não se abrevia — "BMW Production" cortado em "BMW Producti…"
// não identifica campeonato nenhum. O 0.72 é a largura média do caractere em bold
// maiúsculo com tracking 0.1em, por pixel de fonte.
const BAND_TITLE_FONT_SIZE = 13;
const BAND_TITLE_MIN_FONT_SIZE = 8.5;
function bandTitleFontSize(label, columnWidth) {
  const chars = String(label ?? "").length;
  if (!chars || !Number.isFinite(columnWidth) || columnWidth <= 0) return BAND_TITLE_FONT_SIZE;
  const fitted = columnWidth / (chars * 0.72);
  return Math.max(Math.min(BAND_TITLE_FONT_SIZE, fitted), BAND_TITLE_MIN_FONT_SIZE);
}

// Card do gráfico histórico. Ocupa a coluna da esquerda do layout e desenha tudo
// dentro da própria área medida — nenhuma linha atravessa para a coluna de rankings,
// porque não existe geometria fora deste retângulo.
//
// Todo X vem de `geometry.getBoundaryX` / `geometry.getCenterX`. Cabeçalho, grade,
// hachuras, linhas, pontos e etiquetas compartilham a mesma régua.
export function AtlasChart({
  payload,
  years,
  tracks,
  vertical,
  focusedTeamId,
  pinnedTeamId,
  onFocus,
  onTeamClick,
  onTeamDoubleClick,
}) {
  const { t } = useTranslation();
  const [plotRef, plotSize] = useElementSize();
  // A altura vem pronta na régua vertical compartilhada; daqui só sai a largura.
  const geometry = useMemo(
    () => buildAtlasGeometry(payload, years, plotSize, vertical),
    [payload, years, plotSize, vertical],
  );
  const labels = useMemo(
    () => buildEntryLabels(tracks, geometry, years, payload),
    [tracks, geometry, years, payload],
  );
  const bandByKey = useMemo(
    () => new Map((payload?.bands ?? []).map((band) => [band.key, band])),
    [payload],
  );

  // Traçados e vértices resolvidos uma vez só: as linhas e os pontos são pintados
  // em camadas separadas (com os conectores no meio), então percorrem esta lista
  // duas vezes em vez de recalcular a geometria em cada passada.
  // O ano em curso. Ele normalmente NÃO é uma coluna: a diagonal que sobe até ele
  // ocupa a última célula (é a convenção do gráfico inteiro — o traço dentro da
  // célula do ano N é a transição de N para N+1) e o ponto de agora cai na borda
  // direita, encostado no rabicho que leva ao card. Ver `axisEndYear`.
  const liveYear = isLivePayload(payload) ? payload.current_year : null;

  const lines = useMemo(
    () =>
      // `assignCorridorLanes` só sabe distribuir as verticais depois de ver todas as
      // equipes, então ele fecha a lista inteira de uma vez.
      assignCorridorLanes(
        tracks
          .flatMap((track) =>
            ["regular", "special"].map((lineKey) => {
              const points = (track.points ?? []).filter((point) => point.slot === lineKey);
              if (!points.length) return null;
              const vertices = trackVertices({ points }, geometry, years);
              if (!vertices.length) return null;
              const { segments, crossings } = buildTrackSegments(vertices, liveYear);
              return { track: { ...track, points }, lineKey, vertices, segments, crossings };
            }),
          )
          .filter(Boolean),
      ),
    [tracks, geometry, years, liveYear],
  );

  // Conectores só depois de TODAS as etiquetas colocadas — quem não saiu do lugar
  // ideal não ganha traço, e quem ficou sem lugar não é desenhado de forma alguma.
  const connectors = useMemo(
    () =>
      labels
        .filter(isLabelOnScreen(focusedTeamId, pinnedTeamId))
        .map((label) => ({ label, connector: labelConnector(label) }))
        .filter((entry) => entry.connector),
    [labels, focusedTeamId, pinnedTeamId],
  );

  const familyFirstYear = firstSeriesYear(payload);
  const preSeries = familyPreSeriesBox(geometry, years, familyFirstYear);
  const liveColumn = liveColumnBox(payload, geometry, years);

  // O título de cada faixa é centrado sobre a coluna de etiquetas dela, não sobre a
  // borda do gráfico: ele encabeça aquela lista de equipes, e a lista muda de x
  // conforme o ano de estreia. Só as fundadoras contam, porque são as que estão
  // sempre na tela — usar uma etiqueta que só aparece no hover moveria o título ao
  // passar o mouse. Sem nenhuma etiqueta colocada não há coluna, e o título volta
  // para a borda esquerda.
  const titleCenterByBand = useMemo(() => {
    const bounds = new Map();
    labels.forEach((label) => {
      if (label.unresolved || !label.isFounder) return;
      const current = bounds.get(label.band_key);
      const left = label.right - label.width;
      if (!current) {
        bounds.set(label.band_key, { left, right: label.right });
        return;
      }
      current.left = Math.min(current.left, left);
      current.right = Math.max(current.right, label.right);
    });
    return new Map(
      [...bounds].map(([key, box]) => [key, { center: (box.left + box.right) / 2, width: box.right - box.left }]),
    );
  }, [labels]);

  // O card do gráfico é desenhado como DOIS itens do grid do pai: a faixa dos anos
  // na linha 1 e a área de plotagem na linha 2 — a mesma linha da coluna de
  // rankings. É o CSS grid que garante que os dois comecem no mesmo Y; não há
  // nenhuma constante somada para compensar o cabeçalho, e não pode haver: foi
  // exatamente esse tipo de ajuste manual que produziu o descolamento de 34px.
  // As bordas são divididas entre as duas peças para continuarem lendo como um card.
  return (
    <>
      {/* Faixa dos anos: cada rótulo no CENTRO da sua célula, posicionado pela mesma
          régua dos pontos — é o que garante que redimensionar a janela não desloque
          um em relação ao outro. */}
      <div
        data-testid="atlas-v2-chart"
        className="relative overflow-hidden rounded-t-2xl border border-b-0 border-[#2a3f5c] bg-[linear-gradient(180deg,#0d1a2c_0%,#0a1421_100%)]"
        style={{ gridColumn: 1, gridRow: 1, height: YEAR_HEADER_HEIGHT }}
      >
        {years.map((year, index) => (
          <div
            key={year}
            data-testid={`atlas-v2-year-${year}`}
            // O ano em curso é o único que ainda pode mudar de conteúdo: ganha a
            // cor do estado vivo já na régua, antes de o olho descer para a coluna.
            className="absolute inset-y-0 grid place-items-center text-center font-mono text-[12px] font-bold tracking-tight text-slate-300"
            style={{
              left: geometry.getBoundaryX(index),
              width: geometry.yearWidth,
              color: year === liveYear ? LIVE_ACCENT : undefined,
            }}
          >
            {year}
          </div>
        ))}
        {years.slice(1).map((year, index) => (
          <div
            key={`divider-${year}`}
            aria-hidden="true"
            className="absolute inset-y-0 w-px bg-white/[0.06]"
            style={{ left: geometry.getBoundaryX(index + 1) }}
          />
        ))}
      </div>

      <div
        ref={plotRef}
        data-testid="atlas-v2-plot"
        className="relative min-h-0 overflow-hidden rounded-b-2xl border border-t-0 border-[#2a3f5c] bg-[linear-gradient(180deg,#0b1626_0%,#081120_100%)]"
        style={{ gridColumn: 1, gridRow: 2 }}
        onMouseLeave={() => onFocus(null)}
      >
        {/* Brilho central discreto — dá profundidade sem chapar o fundo. */}
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-0"
          style={{ background: "radial-gradient(120% 80% at 55% 45%, rgba(56,110,180,0.14) 0%, rgba(8,17,32,0) 72%)" }}
        />

        {/* Grade: uma linha em cada LIMITE interno de ano. */}
        <div aria-hidden="true" data-testid="atlas-v2-year-grid" className="pointer-events-none absolute inset-0">
          {years.slice(1).map((year, index) => (
            <div
              key={year}
              className="absolute inset-y-0 w-px border-l border-dashed border-white/[0.07]"
              style={{ left: geometry.getBoundaryX(index + 1) }}
            />
          ))}
        </div>

        {/* Cabeçalho de cada faixa: mesma tinta do cabeçalho do card lateral, com a
            linha de base na cor do campeonato. É o topo do "card" do lado do
            gráfico, e é ele que marca onde uma divisão começa.

            Pintado ANTES das hachuras de propósito. A tinta atravessa a largura
            inteira, então nas colunas em que a série ainda não existia ela ficaria
            por cima do "fora da série" — e a faixa aparecia como um retângulo claro
            recortado dentro da hachura. Aqui embaixo, a hachura sempre vence. */}
        {(payload?.bands ?? []).map((band) => {
          const box = geometry.bands[band.key];
          if (!box) return null;
          const accent = bandAccent(band);
          return (
            <div
              key={`band-header-${band.key}`}
              aria-hidden="true"
              data-testid={`atlas-v2-band-header-${band.key}`}
              className="pointer-events-none absolute inset-x-0"
              style={{
                top: box.headerTop ?? box.top,
                height: box.headerHeight,
                background: `linear-gradient(180deg, color-mix(in srgb, ${accent} 11%, transparent) 0%, color-mix(in srgb, ${accent} 2%, transparent) 100%)`,
                borderBottom: `1px solid color-mix(in srgb, ${accent} 32%, transparent)`,
              }}
            />
          );
        })}

        {/* (A) Antes de a família existir: altura inteira do gráfico. */}
        {preSeries ? (
          <div
            aria-hidden="true"
            data-testid="atlas-v2-pre-series"
            className="pointer-events-none absolute inset-y-0"
            style={{
              left: preSeries.left,
              width: preSeries.width,
              background: OUTSIDE_BACKGROUND,
            }}
          />
        ) : null}

        {/* A divisa "a partir daqui esta série existe": atravessa o gráfico inteiro
            e é o marco mais forte do eixo, acima dos degraus por categoria. */}
        {preSeries ? (
          <div
            aria-hidden="true"
            data-testid="atlas-v2-series-boundary"
            className="pointer-events-none absolute inset-y-0 w-px bg-[rgba(255,153,45,0.34)]"
            style={{ left: preSeries.left + preSeries.width }}
          />
        ) : null}

        {/* Identificação discreta dentro da hachura larga: sem ela, entender a área
            depende de ir até a legenda. Só aparece quando há espaço de sobra. Vai
            no RODAPÉ da hachura porque o topo dela agora é o título da primeira
            faixa — e o título do campeonato ganha o lugar de honra. */}
        {preSeries && preSeries.width >= HATCH_LABEL_MIN_WIDTH ? (
          <span
            aria-hidden="true"
            data-testid="atlas-v2-pre-series-label"
            className="pointer-events-none absolute select-none text-[9px] font-bold uppercase tracking-[0.16em] text-[rgba(150,174,197,0.44)]"
            style={{ left: preSeries.left + 14, bottom: 8 }}
          >
            {t("globalTeams.legendOutside")}
          </span>
        ) : null}

        {/* (C) A coluna da temporada em curso. Não é hachura de "não houve corrida"
            — houve, e está acontecendo — então usa tinta verde levíssima e uma
            divisa própria em vez do padrão cinza de fora-da-série. */}
        {liveColumn ? (
          <>
            <div
              aria-hidden="true"
              data-testid="atlas-v2-live-column"
              className="pointer-events-none absolute inset-y-0"
              style={{
                left: liveColumn.left,
                width: liveColumn.width,
                background: `linear-gradient(180deg, ${LIVE_ACCENT}0f 0%, ${LIVE_ACCENT}05 100%)`,
              }}
            />
            <div
              aria-hidden="true"
              data-testid="atlas-v2-live-divider"
              className="pointer-events-none absolute inset-y-0 w-px"
              style={{ left: liveColumn.left, background: `${LIVE_ACCENT}59` }}
            />
            <span
              aria-hidden="true"
              data-testid="atlas-v2-live-column-label"
              className="pointer-events-none absolute select-none whitespace-nowrap text-[9px] font-bold uppercase tracking-[0.16em]"
              style={{ left: liveColumn.left + 8, bottom: 8, color: `${LIVE_ACCENT}b3` }}
            >
              {t("globalTeams.liveBadge")}
            </span>
          </>
        ) : null}

        {(payload?.bands ?? []).map((band, index) => {
          const box = geometry.bands[band.key];
          if (!box) return null;
          // (B) Categoria que nasceu depois da família: só na altura dela.
          const unavailable = bandUnavailableBox(band, geometry, years, familyFirstYear);
          const dividerX = bandStartDividerX(band, geometry, years);
          const titleColumn = titleCenterByBand.get(band.key);
          return (
            <div key={band.key} aria-hidden="true" className="pointer-events-none">
              {/* Separação entre campeonatos.
                  Uma régua de 2px no meio do vão se perdia no meio de sessenta
                  linhas coloridas — era só mais um traço horizontal. O que separa
                  agora é PROFUNDIDADE: o vão inteiro vira um sulco escuro, e o
                  campeonato debaixo abre com a mesma faixa tingida que o card
                  lateral dele usa. As duas colunas passam a ter a mesma moldura,
                  e a divisão se lê sem depender de enxergar uma linha fina. */}
              {index > 0 ? (
                <div
                  data-testid={`atlas-v2-band-rule-${band.key}`}
                  className="absolute inset-x-0"
                  style={{
                    top: box.top - geometry.bandGap,
                    height: geometry.bandGap,
                    background: "rgba(2,7,14,0.72)",
                    boxShadow: "inset 0 1px 0 rgba(0,0,0,0.5), inset 0 -1px 0 rgba(151,178,203,0.10)",
                  }}
                />
              ) : null}
              {unavailable ? (
                <div
                  data-testid={`atlas-v2-unavailable-${band.key}`}
                  className="absolute"
                  style={{
                    left: unavailable.left,
                    top: unavailable.top,
                    width: unavailable.width,
                    height: unavailable.height,
                    background: OUTSIDE_BACKGROUND,
                  }}
                />
              ) : null}
              {/* Divisa de início da CATEGORIA, exatamente onde a hachura dela
                  termina. É o marco mais forte DENTRO da faixa: sem ele, os três
                  campeonatos pareciam começar todos na divisa da família. */}
              {unavailable && Number.isFinite(dividerX) && dividerX !== null ? (
                <div
                  data-testid={`atlas-v2-start-divider-${band.key}`}
                  className="absolute bg-[rgba(255,153,45,0.62)]"
                  style={{ left: dividerX, top: box.top, height: box.height, width: 1.25 }}
                />
              ) : null}
              {/* O nome do campeonato ocupa a faixa vazia que o gráfico já reserva
                  acima da primeira posição — a mesma altura do cabeçalho do card
                  lateral. Sem ele, a única forma de saber que faixa é qual era
                  atravessar a tela até a coluna da direita. Aqui é só o nome: o
                  troféu vive no cabeçalho do card lateral, e repeti-lo dos dois
                  lados do vão não acrescentava nada — só empurrava o texto para
                  fora da coluna que ele encabeça. */}
              <div
                data-testid={`atlas-v2-band-title-${band.key}`}
                className="absolute flex items-center justify-center"
                style={{
                  // Centrado na coluna de etiquetas quando ela existe; encostado na
                  // borda esquerda quando não há etiqueta alguma para encabeçar. A
                  // largura é a da própria coluna: o título não pode transbordar
                  // para cima das linhas nem para fora do gráfico.
                  left: titleColumn?.center ?? geometry.getBoundaryX(0) + 14,
                  transform: titleColumn ? "translateX(-50%)" : undefined,
                  width: titleColumn?.width,
                  top: box.headerTop ?? box.top,
                  height: box.headerHeight,
                }}
              >
                <span
                  className="whitespace-nowrap font-bold uppercase tracking-[0.1em]"
                  style={{ color: bandAccent(band), fontSize: bandTitleFontSize(band.label, titleColumn?.width) }}
                >
                  {band.label}
                </span>
              </div>
              {/* A extensão da categoria NÃO repete o texto: ela é a continuação da
                  mesma área contínua de "fora da série", já identificada uma vez à
                  esquerda. Repetir o rótulo não explicava a diferença entre as duas
                  camadas — só carregava o gráfico. A distinção fica na intensidade
                  da hachura e na divisa de início da categoria. */}
            </div>
          );
        })}

        {/* overflow visible: sem isso o marcador e a ponta arredondada da linha na
            última coluna são cortados pela viewport do próprio SVG. A geometria dos
            anos não muda — só a área permitida para pintura. */}
        <svg
          className="absolute inset-0"
          width={geometry.plotWidth}
          height={geometry.bodyHeight}
          style={{ overflow: "visible" }}
          aria-hidden="true"
        >
          {/* Ordem de pintura: linhas → conectores → pontos. O conector precisa ficar
              ACIMA das linhas (senão some no meio delas) e ABAIXO dos pontos, para
              não competir com o marcador de estreia. */}
          {lines.map(({ track, lineKey, segments, crossings }) => {
            const { isFocused, isDimmed } = teamHighlight(track.team_id, focusedTeamId, pinnedTeamId);
            const width = isFocused ? 3 : 2;
            const suffix = (index) => (index === 0 ? "" : `-${index}`);
            return (
            <g
              key={`line-${track.team_id}-${lineKey}`}
              opacity={isDimmed ? 0.16 : 1}
              className="cursor-pointer"
              onMouseEnter={() => onFocus(track.team_id)}
              onClick={() => onTeamClick(teamTrackToTeamRow(track, track.points?.[0]?.band_key, bandByKey))}
            >
              {/* Um traçado por DIVISÃO. A linha nunca sai da faixa em que a equipe
                  correu — quem liga uma divisão à outra é a travessia, abaixo. */}
              {segments.map((segment, index) => (
                <Fragment key={`segment-${segment.bandKey}-${index}`}>
                  {segment.path ? (
                    <path
                      data-testid={`atlas-v2-track-${track.team_id}-${lineKey}${suffix(index)}`}
                      d={segment.path}
                      fill="none"
                      stroke={track.cor_primaria}
                      strokeWidth={width}
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      opacity={isFocused ? 1 : 0.88}
                      pointerEvents="stroke"
                    />
                  ) : null}
                  {/* Temporada em curso: mesma cor, mesma espessura, traço
                      interrompido. A posição ainda pode mudar até a última etapa, e
                      uma linha cheia prometeria um resultado que ninguém tem. */}
                  {segment.livePath ? (
                    <path
                      data-testid={`atlas-v2-track-live-${track.team_id}-${lineKey}${suffix(index)}`}
                      d={segment.livePath}
                      fill="none"
                      stroke={track.cor_primaria}
                      strokeWidth={width}
                      strokeDasharray="5 4"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      opacity={isFocused ? 1 : 0.8}
                      pointerEvents="stroke"
                    />
                  ) : null}
                </Fragment>
              ))}

              {/* Promoção e rebaixamento, pelo corredor.
                  A diagonal direta cruzava os campeonatos do meio no mesmo ângulo
                  das linhas de dados e virava mais um fio no novelo. Aqui a
                  travessia é um Z: reta na altura antiga, vertical no corredor,
                  reta na altura nova. Vertical não se parece com dado nenhum —
                  ninguém muda de posição dentro de uma temporada —, então ela lê
                  como passagem. Fina e apagada por padrão, cheia no foco: é um
                  vínculo entre duas temporadas, não uma temporada. */}
              {crossings.map((crossing, index) => {
                if (!crossing.path) return null;
                // O gradiente é vertical e vai do y de saída ao y de chegada. As
                // pernas horizontais do Z ficam justamente nas duas pontas dessa
                // faixa, então elas saem sólidas de graça — o que apaga é só o
                // miolo da vertical.
                const fades = FADE_LONG_CORRIDORS && crossingIsLong(crossing, geometry.bodyHeight);
                const gradientId = fades ? corridorGradientId(track.team_id, lineKey, index) : null;
                return (
                  <Fragment key={`crossing-${index}`}>
                    {gradientId ? (
                      <defs>
                        <linearGradient
                          id={gradientId}
                          gradientUnits="userSpaceOnUse"
                          x1="0"
                          y1={crossing.from.y}
                          x2="0"
                          y2={crossing.to.y}
                        >
                          <stop offset="0%" stopColor={track.cor_primaria} stopOpacity="1" />
                          <stop
                            offset={`${CORRIDOR_FADE_START * 100}%`}
                            stopColor={track.cor_primaria}
                            stopOpacity={CORRIDOR_FADE_OPACITY}
                          />
                          <stop
                            offset={`${CORRIDOR_FADE_END * 100}%`}
                            stopColor={track.cor_primaria}
                            stopOpacity={CORRIDOR_FADE_OPACITY}
                          />
                          <stop offset="100%" stopColor={track.cor_primaria} stopOpacity="1" />
                        </linearGradient>
                      </defs>
                    ) : null}
                    <path
                      data-testid={`atlas-v2-crossing-${track.team_id}-${lineKey}-${index}`}
                      d={crossing.path}
                      fill="none"
                      stroke={gradientId ? `url(#${gradientId})` : track.cor_primaria}
                      strokeWidth={isFocused ? 2 : 1.3}
                      // Pontilhada em repouso, tracejada quando é a temporada em
                      // curso. O ponto fino de "1 4" ficava perto do nada somado à
                      // atenuação do miolo; "1.5 3.5" continua lendo como ponto e
                      // sobrevive à redução.
                      strokeDasharray={crossing.isLive ? "5 4" : "1.5 3.5"}
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      // 0.45 em repouso era pouco para um traço que já é fino e
                      // pontilhado. Segue abaixo das linhas de dados (0.88): a
                      // travessia é um vínculo entre temporadas, não uma temporada.
                      opacity={isFocused ? 0.95 : 0.6}
                      pointerEvents="stroke"
                    />
                  </Fragment>
                );
              })}
            </g>
            );
          })}

          {connectors.map(({ label, connector }) => (
            <g key={`connector-${label.key}`} opacity={teamHighlight(label.team_id, focusedTeamId, pinnedTeamId).isDimmed ? 0.15 : 1}>
              <line
                data-testid={`atlas-v2-label-connector-${label.team_id}`}
                x1={connector.x1}
                y1={connector.y1}
                x2={connector.x2}
                y2={connector.y2}
                stroke={label.cor}
                strokeWidth="1.3"
                strokeOpacity="0.78"
                strokeLinecap="round"
                fill="none"
              />
              <circle cx={connector.x1} cy={connector.y1} r="1.8" fill={label.cor} fillOpacity="0.85" />
            </g>
          ))}

          {lines.map(({ track, lineKey, vertices }) => {
            const { isFocused, isDimmed } = teamHighlight(track.team_id, focusedTeamId, pinnedTeamId);
            return (
            <g
              key={`points-${track.team_id}-${lineKey}`}
              data-testid={`atlas-v2-points-${track.team_id}-${lineKey}`}
              opacity={isDimmed ? 0.16 : 1}
              className="cursor-pointer"
              onMouseEnter={() => onFocus(track.team_id)}
              onClick={() => onTeamClick(teamTrackToTeamRow(track, track.points?.[0]?.band_key, bandByKey))}
            >
              {/* O vértice de fechamento entra no traçado, mas NÃO vira marcador:
                  ele não é uma temporada a mais. */}
              {vertices.map((vertex, index) =>
                vertex.isClosing ? null : (
                  // O primeiro ponto é a estreia no campeonato: anel vazado, como
                  // na legenda. Os demais são pontos anuais cheios.
                  <circle
                    key={`${vertex.point.year}-${vertex.point.position}`}
                    data-testid={index === 0 ? `atlas-v2-entry-point-${track.team_id}-${lineKey}` : undefined}
                    cx={vertex.x}
                    cy={vertex.y}
                    r={index === 0 ? 4 : 3.2}
                    fill={index === 0 ? "#081120" : track.cor_primaria}
                    stroke={index === 0 ? track.cor_display : "none"}
                    strokeWidth={index === 0 ? 1.5 : 0}
                    opacity={isFocused ? 1 : 0.92}
                    style={{ color: track.cor_primaria, filter: "drop-shadow(0 0 3px currentColor)" }}
                  />
                ),
              )}
            </g>
            );
          })}
        </svg>

        {/* Fundadoras ficam sempre na tela; quem entrou no meio do caminho só
            aparece quando a própria linha está em destaque (hover ou equipe fixada).
            Sem isso o gráfico virava um paredão de chips no miolo. */}
        {labels.filter(isLabelOnScreen(focusedTeamId, pinnedTeamId)).map((label) => (
          <EntryLabel
            key={label.key}
            label={label}
            track={tracks.find((track) => track.team_id === label.team_id)}
            bandByKey={bandByKey}
            focusedTeamId={focusedTeamId}
            pinnedTeamId={pinnedTeamId}
            onFocus={onFocus}
            onTeamClick={onTeamClick}
            onTeamDoubleClick={onTeamDoubleClick}
          />
        ))}
      </div>
    </>
  );
}

// Uma etiqueta está na tela se tem lugar E (é fundadora OU a equipe dela está em
// destaque). Mesma regra vale para o conector — o traço não pode sobreviver ao chip.
function isLabelOnScreen(focusedTeamId, pinnedTeamId) {
  return (label) => {
    if (label.unresolved) return false;
    if (label.isFounder) return true;
    return label.team_id === focusedTeamId || label.team_id === pinnedTeamId;
  };
}

// Etiqueta da equipe no ponto de estreia, encostada à esquerda dele. Quando a rotina
// de anticolisão precisou deslocá-la, um conector fino liga a etiqueta ao ponto real.
function EntryLabel({ label, track, bandByKey, focusedTeamId, pinnedTeamId, onFocus, onTeamClick, onTeamDoubleClick }) {
  if (!track) return null;
  const { isDimmed } = teamHighlight(label.team_id, focusedTeamId, pinnedTeamId);

  return (
    <button
        type="button"
        data-testid={`atlas-v2-entry-label-${label.team_id}`}
        onMouseEnter={() => onFocus(label.team_id)}
        onFocus={() => onFocus(label.team_id)}
        onClick={() => onTeamClick(teamTrackToTeamRow(track, label.band_key, bandByKey))}
        onDoubleClick={() => onTeamDoubleClick(teamTrackToTeamRow(track, label.band_key, bandByKey))}
        className="absolute z-20 flex cursor-pointer items-center rounded-md border bg-[#081120]/95 text-left font-semibold leading-none shadow-[0_6px_16px_rgba(0,0,0,0.35)]"
        style={{
          // A etiqueta termina LABEL_GAP antes do ponto e cresce para a esquerda.
          left: label.right,
          top: label.renderY - LABEL_HEIGHT / 2,
          height: LABEL_HEIGHT,
          // Largura vem do GRUPO de estreia: todas as etiquetas do mesmo
          // campeonato e ano têm a mesma, formando uma coluna. É a mesma largura
          // que a busca por colisão usou, então o desenho e a caixa não divergem.
          width: label.width,
          maxWidth: LABEL_MAX_WIDTH,
          // A fonte é o que cede quando a faixa pré-série é estreita — o nome da
          // equipe sai inteiro em qualquer escala. Ver LABEL_FONT_SCALES.
          fontSize: LABEL_FONT_SIZE * (label.fontScale ?? 1),
          gap: 7,
          paddingLeft: 9,
          paddingRight: 9,
          transform: "translateX(-100%)",
          color: label.cor,
          borderColor: `${label.cor}66`,
          opacity: isDimmed ? 0.18 : 1,
        }}
      >
      {/* A caixa tem a largura REAL do brasão `xs` (36px) reduzida por escala: numa
          caixa menor que ele, o logo transbordava por cima do nome — era metade do
          aperto visual da etiqueta. */}
      <span className="grid h-5 w-7 shrink-0 place-items-center">
        <span className="origin-center scale-[0.78]">
          <TeamLogoMark teamName={track.nome} color={label.cor} size="xs" testId="atlas-v2-entry-logo" />
        </span>
      </span>
      <span className="min-w-0 whitespace-nowrap">{track.nome}</span>
    </button>
  );
}

export default AtlasChart;
