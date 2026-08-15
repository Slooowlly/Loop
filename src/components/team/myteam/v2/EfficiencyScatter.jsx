import { ListOrdered, Minus, Ruler, TrendingDown, TrendingUp } from "lucide-react";
import { useTranslation } from "react-i18next";

import Tooltip from "../../../ui/Tooltip";
import TeamLogoMark, { HALO_FILTER, getTeamLogoSrc } from "../../TeamLogoMark";
import useElementSize from "../../v2/useElementSize";
import { formatMoney, formatMoneyCompact } from "../../../../utils/formatters";
import { chartView, efficiencyScatter, readableOn } from "./gridMetrics";

// Altura fixa: a largura estica com a coluna, a altura não.
const CHART_HEIGHT = 224;

// As linhas de apoio vão em rgba LITERAL, e não em `stroke="currentColor"` com uma
// classe `text-white/6`. O Tailwind só gera a classe que encontra no código-fonte, e
// uma opacidade nova que ainda não foi gerada não "quase funciona": o `currentColor`
// cai na cor de texto herdada e a grade discreta vira um feixe de linhas brancas
// cruzando o gráfico. Medido aqui: `text-white/6` e `text-white/8` resolviam para
// #e6edf3. Para grade, o literal é mais barato do que a dependência.
const GRADE = "rgba(255,255,255,0.06)";
const EIXO = "rgba(255,255,255,0.12)";

// GRÁFICO 4 — quem transforma dinheiro em resultado.
//
// É literalmente o que a seção se chama no v1: "comparativo de gestão e
// performance". A tabela ordenada nunca faz esse julgamento — mostra caixa e pontos
// em colunas separadas e deixa a conta para o jogador. Aqui a reta é a conversão
// MÉDIA de caixa em pontos do grid, e a posição relativa a ela é o veredito: acima,
// você pontua mais do que o seu dinheiro explica; abaixo, menos.
//
// A primeira versão desenhava só a reta e seis bolinhas, e o desvio — que É o
// assunto — era um fio pontilhado de 1px atrás do ponto do jogador. Dava para
// trocar o cenário inteiro (equipe dominante por equipe em crise) e o desenho sair
// igual. O que faz o gráfico falar agora:
//
//   - o desvio de TODAS as equipes vira um traço vertical verde/vermelho até a
//     reta, então o julgamento de gestão é do grid inteiro, não só do jogador;
//   - a reta ganha uma FAIXA de tolerância (o desvio médio do grid), porque com
//     seis pontos "acima da reta" por meio ponto não é notícia;
//   - a escala ganha linhas e marcas, senão não há régua para estimar distância.
//
// O gráfico ocupa DUAS TERÇAS da faixa, não a largura inteira: seis pontos de 5px
// espalhados por 1600px eram ar, e a frase que dava o veredito ficava no menor tipo
// da tela. A terça restante é a leitura escrita — o ranking de conversão que o
// desenho deixava implícito.
function EfficiencyScatter({ teams, playerTeamId }) {
  const { t } = useTranslation();
  const [measureRef, size] = useElementSize({ width: 520, height: CHART_HEIGHT });
  const view = chartView(size.width, CHART_HEIGHT, { left: 54, bottom: 34, top: 22 });
  const scatter = efficiencyScatter(teams, playerTeamId, view);

  const player = scatter.player;
  // A sua marca é desenhada por ÚLTIMO: com logos de 30px em vez de bolinhas de 5px,
  // duas equipes de caixa parecido se sobrepõem, e a que some não pode ser a sua.
  const marks = [...(scatter.points ?? [])].sort((a, b) => Number(a.isPlayer) - Number(b.isPlayer));

  return (
    <div data-testid="my-team-v2-scatter" className="grid gap-6 lg:grid-cols-[1.55fr_0.45fr]">
      {/* O ref de medição fica no MESMO nó em todos os estados — inclusive no vazio,
          que é o estado da primeira renderização, antes de `get_teams_standings`
          responder. Um ref que troca de elemento (ou que só monta depois) deixa o
          ResizeObserver preso no nó antigo, e o gráfico passa a desenhar com a
          largura de fallback dentro de uma coluna muito mais larga. */}
      <div ref={measureRef} className="min-w-0">
        {!scatter.hasData ? (
          <p className="border-y border-white/[0.08] px-4 py-8 text-center text-[11px] leading-5 text-text-secondary">
            {scatter.reason === "noPoints" ? t("myTeamTabV2.scatter.emptyNoPoints") : t("myTeamTabV2.scatter.empty")}
          </p>
        ) : (
          <>
            <div className="flex flex-wrap items-baseline justify-between gap-3">
              <p className="text-[10px] uppercase tracking-[0.22em] text-text-muted">{t("myTeamTabV2.scatter.title")}</p>
              <div className="flex items-center gap-3">
                <LegendChip className="bg-status-green" label={t("myTeamTabV2.scatter.legendAbove")} />
                <LegendChip className="bg-status-red" label={t("myTeamTabV2.scatter.legendBelow")} />
                <LegendChip className="bg-white/25" label={t("myTeamTabV2.scatter.legendBand")} />
              </div>
            </div>

            <svg
              viewBox={`0 0 ${view.width} ${view.height}`}
              width="100%"
              height={view.height}
              className="mt-3 block"
              role="img"
              aria-label={t("myTeamTabV2.scatter.title")}
            >
              {scatter.yTicks.map((tick) => (
                <g key={`y-${tick.value}`}>
                  <line x1={view.left} y1={tick.y} x2={view.right} y2={tick.y} stroke={GRADE} />
                  <text x={view.left - 8} y={tick.y + 3.5} textAnchor="end" className="fill-text-muted text-[10px]">
                    {tick.value}
                  </text>
                </g>
              ))}

              {/* Faixa primeiro, no fundo: ela é o "esperado", o pano contra o qual
                  tudo o mais é desvio. */}
              <polygon points={scatter.band} className="fill-white/[0.05]" />
              <line
                x1={scatter.trend.x1}
                y1={scatter.trend.y1}
                x2={scatter.trend.x2}
                y2={scatter.trend.y2}
                className="stroke-text-muted"
                strokeWidth="1.5"
                strokeDasharray="5 4"
              />

              {/* O traço do desvio, de todo mundo. É o que transforma "seis pontos
                  perto de uma reta" em "quem rende e quem queima dinheiro". */}
              {scatter.points.map((point) => (
                <line
                  key={`${point.id}-residual`}
                  x1={point.x}
                  y1={point.y}
                  x2={point.x}
                  y2={point.expectedY}
                  className={RESIDUAL_STROKE[point.verdict]}
                  strokeWidth={point.isPlayer ? 3 : 2}
                  strokeLinecap="round"
                  strokeOpacity={point.isPlayer ? 0.95 : 0.5}
                />
              ))}

              {/* O rótulo ao lado da marca é o CAIXA, não a sigla. A logo já diz de
                  quem é o ponto, e repetir "TST" ao lado dela gastava o único texto
                  do gráfico para não dizer nada. O eixo horizontal É o caixa, mas ler
                  posição contra três marcas de escala é estimativa: o valor escrito
                  transforma o eixo em número exato sem custar mais um elemento. */}
              {marks.map((point) => (
                <text
                  key={`${point.id}-label`}
                  x={point.x + (point.isPlayer ? 26 : 20)}
                  y={point.y + 4}
                  className={point.isPlayer ? "fill-text-primary text-[11px] font-semibold" : "fill-text-secondary text-[10px]"}
                >
                  {formatMoneyCompact(point.cash)}
                </text>
              ))}

              {marks.map((point) => (
                <Tooltip
                  key={point.id}
                  texto={t("myTeamTabV2.scatter.pointTooltip", {
                    team: point.name,
                    cash: formatMoney(point.cash),
                    points: point.pontos,
                    residual: formatSigned(point.residual),
                  })}
                >
                  <g data-testid={point.isPlayer ? "scatter-player-point" : undefined}>
                    <TeamMark point={point} />
                  </g>
                </Tooltip>
              ))}

              <line x1={view.left} y1={view.bottom} x2={view.right} y2={view.bottom} stroke={EIXO} />
              <line x1={view.left} y1={view.top} x2={view.left} y2={view.bottom} stroke={EIXO} />

              <text x={view.left - 8} y={view.top - 8} textAnchor="end" className="fill-text-muted text-[9px] uppercase tracking-[0.12em]">
                {t("myTeamTabV2.scatter.axisPoints")}
              </text>
              {scatter.xTicks.map((tick) => (
                <text
                  key={`x-${tick.x}`}
                  x={tick.x}
                  y={view.height - 8}
                  textAnchor={tick.anchor}
                  className="fill-text-muted text-[10px]"
                >
                  {formatMoneyCompact(tick.value)}
                </text>
              ))}
            </svg>
          </>
        )}
      </div>

      {!scatter.hasData || !player ? null : <ScatterReading t={t} scatter={scatter} player={player} />}
    </div>
  );
}

// A marca da equipe no gráfico: a logo, e não uma bolinha colorida. Seis cores
// arbitrárias exigiam uma legenda — e a legenda era a sigla ao lado, que gastava o
// texto do ponto. A logo identifica sozinha e libera o rótulo para o caixa.
//
// Quando o catálogo não tem a arte, o fallback continua sendo o círculo na cor da
// equipe: um retângulo vazio no lugar de uma logo lê como imagem quebrada.
function TeamMark({ point }) {
  const logo = getTeamLogoSrc(point.fullName);
  const cor = point.color ?? "#8b949e";

  if (!logo) {
    return (
      <>
        {point.isPlayer ? (
          <circle cx={point.x} cy={point.y} r={12} fill="none" stroke={cor} strokeOpacity={0.35} strokeWidth={1.5} />
        ) : null}
        <circle
          cx={point.x}
          cy={point.y}
          r={point.isPlayer ? 7 : 5}
          fill={cor}
          stroke={point.isPlayer ? "#e6edf3" : "none"}
          strokeWidth={point.isPlayer ? 2 : 0}
        />
      </>
    );
  }

  // A sua equipe NÃO ganha placa atrás da logo. A primeira versão desenhava um
  // retângulo na cor do time ali, e o efeito era o oposto do pretendido: a arte
  // passava a parecer emoldurada por engano, como se a imagem tivesse falhado e
  // sobrado a caixa. A ênfase já existe em três lugares — a logo é maior, o traço do
  // desvio é o mais grosso do gráfico e o rótulo de caixa é o único em branco forte.
  // Um quarto sinal não estava somando; estava chamando atenção para si mesmo.
  const largura = point.isPlayer ? 42 : 30;
  const altura = point.isPlayer ? 28 : 20;
  return (
    <>
      {/* O halo segue o alfa da arte: várias das 102 logos são desenhadas em preto e
          sumiriam sobre o painel escuro sem o contorno. */}
      <image
        href={logo}
        x={point.x - largura / 2}
        y={point.y - altura / 2}
        width={largura}
        height={altura}
        preserveAspectRatio="xMidYMid meet"
        style={{ filter: HALO_FILTER }}
      />
    </>
  );
}

const RESIDUAL_STROKE = {
  above: "stroke-status-green",
  below: "stroke-status-red",
  onPar: "stroke-text-muted",
};

// Só a cor: a placa de 44px que embalava esta seta virou ícone solto na cor do
// veredito, pelo mesmo motivo dos medidores — a placa pesava mais que o número que
// ela acompanha.
const VERDICT_TILE = {
  above: "text-status-green",
  below: "text-status-red",
  onPar: "text-text-secondary",
};
const VERDICT_ICON = { above: TrendingUp, below: TrendingDown, onPar: Minus };

// A leitura escrita segue a MESMA faixa de tolerância do desenho. Antes o número
// aparecia verde ou vermelho sempre, e um desvio de 2 pontos em 69 era anunciado
// como veredito de gestão — o painel prometia mais precisão do que o ajuste tem.
//
// Melhor e pior do grid vêm com a LOGO da equipe, e não com a sigla. "WEE +16" e
// "TST -19" empilhados eram duas siglas monoespaçadas que o jogador tinha de decifrar
// no mesmo painel onde o gráfico ao lado já mostra as equipes por logo — duas
// linguagens para os mesmos seis times.
function ScatterReading({ t, scatter, player }) {
  const verdict = player.verdict;
  const tone = verdict === "above" ? "text-status-green" : verdict === "below" ? "text-status-red" : "text-text-secondary";
  const Icone = VERDICT_ICON[verdict] ?? Minus;

  return (
    /* Painel de leitura: filete à esquerda no lugar da caixa de canto redondo com
       fundo próprio. Ele não é um cartão dentro do bloco, é a coluna de texto do
       gráfico ao lado. */
    <div className="flex flex-col justify-center gap-4 border-white/[0.08] lg:border-l lg:pl-5">
      <div className="flex items-start gap-3" data-testid="scatter-reading">
        <span className={`mt-0.5 shrink-0 ${VERDICT_TILE[verdict] ?? VERDICT_TILE.onPar}`}>
          <Icone size={18} strokeWidth={1.7} aria-hidden="true" />
        </span>
        <div className="min-w-0">
          <p className={`font-garage text-[26px] font-semibold leading-none tabular-nums ${tone}`}>{formatSigned(player.residual)}</p>
          <p className="mt-2 text-xs leading-5 text-text-secondary">
            {verdict === "above"
              ? t("myTeamTabV2.scatter.readingAbove")
              : verdict === "below"
                ? t("myTeamTabV2.scatter.readingBelow")
                : t("myTeamTabV2.scatter.readingOnPar")}
          </p>
        </div>
      </div>

      <div className="space-y-3 border-t border-white/[0.08] pt-3.5">
        {scatter.best ? (
          <TeamLine label={t("myTeamTabV2.scatter.bestLabel")} point={scatter.best} tone="text-status-green" />
        ) : null}
        {scatter.worst ? (
          <TeamLine label={t("myTeamTabV2.scatter.worstLabel")} point={scatter.worst} tone="text-status-red" />
        ) : null}

        <div className="space-y-2 border-t border-white/[0.08] pt-3">
          <RankLine
            icon={ListOrdered}
            label={t("myTeamTabV2.scatter.rankLabel")}
            value={t("myTeamTabV2.scatter.rankValue", { position: scatter.playerRank, grid: scatter.points.length })}
          />
          <RankLine
            icon={Ruler}
            label={t("myTeamTabV2.scatter.toleranceLabel")}
            value={`± ${Math.max(1, Math.round(scatter.tolerance))}`}
            tone="text-text-muted"
          />
        </div>
      </div>
    </div>
  );
}

// Melhor e pior do grid: rótulo em cima, logo + nome + desvio embaixo. Duas linhas
// porque a coluna é estreita — espremer logo, nome e número numa linha só truncaria
// o nome justamente nas equipes de nome longo, que são a maioria.
function TeamLine({ label, point, tone }) {
  return (
    <div>
      <p className="text-[10px] uppercase tracking-[0.14em] text-text-muted">{label}</p>
      <div className="mt-1.5 flex items-center gap-2.5">
        <TeamBadge point={point} />
        <span className="min-w-0 flex-1 truncate text-xs text-text-primary">{point.fullName || point.name}</span>
        <span className={`shrink-0 font-garage text-xs font-semibold tabular-nums ${tone}`}>{formatSigned(point.residual)}</span>
      </div>
    </div>
  );
}

// Sem arte no catálogo, a etiqueta carrega a SIGLA sobre a cor da equipe. Um
// retângulo de cor pura e mais nada lê como imagem que não carregou.
function TeamBadge({ point }) {
  if (getTeamLogoSrc(point.fullName)) {
    return <TeamLogoMark teamName={point.fullName} color={point.color} size="xs" testId="scatter-team-logo" />;
  }

  const cor = point.color ?? "#30363d";
  return (
    <span
      data-testid="scatter-team-logo"
      className="grid h-6 w-9 shrink-0 place-items-center rounded border border-white/10 font-garage text-[10px] font-semibold"
      style={{ backgroundColor: cor, color: readableOn(cor) }}
    >
      {point.name}
    </span>
  );
}

// O sinal é obrigatório: "12" sozinho não diz se sobrou ou faltou ponto, e é
// justamente o sinal que o gráfico inteiro está medindo.
function formatSigned(value) {
  const rounded = Math.round(value);
  return rounded >= 0 ? `+${rounded}` : `${rounded}`;
}

function LegendChip({ className, label }) {
  return (
    <span className="inline-flex items-center gap-1.5 text-[10px] uppercase tracking-[0.12em] text-text-muted">
      <span aria-hidden="true" className={`h-1.5 w-4 rounded-full ${className}`} />
      {label}
    </span>
  );
}

function RankLine({ icon: Icone, label, value, tone = "text-text-primary" }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="flex items-center gap-2 text-[11px] uppercase tracking-[0.14em] text-text-muted">
        <Icone size={14} strokeWidth={1.7} aria-hidden="true" />
        {label}
      </span>
      <span className={`font-garage text-xs tabular-nums ${tone}`}>{value}</span>
    </div>
  );
}

export default EfficiencyScatter;
