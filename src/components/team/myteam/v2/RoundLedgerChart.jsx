import { useTranslation } from "react-i18next";
import { ChartColumn } from "lucide-react";

import GarageSheet from "./GarageSheet";
import Tooltip from "../../../ui/Tooltip";
import useElementSize from "../../v2/useElementSize";
import { formatMoney, formatMoneyCompact, formatSignedMoney } from "../../../../utils/formatters";
import { formatOrdinal } from "../teamMetrics";
import { chartView, seasonLedger } from "./gridMetrics";

// Altura FIXA. A largura vem medida do card; a altura é decisão de layout, não
// consequência da proporção do viewBox.
const CHART_HEIGHT = 224;

// GRÁFICO 1 — o que cada rodada deixou.
//
// Antes isto era a curva do caixa absoluto ao longo da temporada, e ela é sempre
// plana: o saldo anda 3% por etapa, então a linha ia de canto a canto sem revelar
// nada. O sinal está no resultado POR RODADA — que varia 100% entre um fim de
// semana e o outro — e é o que estas colunas mostram.
//
// Colunas cheias são rodadas corridas; vazadas são as que faltam, extrapoladas pelo
// resultado médio; a verde do fim é o prêmio de construtores. A linha fina no topo é
// o caixa acumulado, numa tira própria porque a escala é outra.
function RoundLedgerChart({ report, team, season }) {
  const { t } = useTranslation();
  // O ref de medição fica num wrapper que existe SEMPRE, nunca dentro do bloco do
  // gráfico: o relatório financeiro chega por invoke depois da primeira renderização,
  // e um ref montado tarde nunca é medido (o efeito do useElementSize roda uma única
  // vez, na montagem). Foi assim que o SVG passou a desenhar com a largura de
  // fallback, e o preserveAspectRatio centralizou o desenho deixando margem morta.
  const [measureRef, size] = useElementSize({ width: 600, height: CHART_HEIGHT });
  // Folga em cima e embaixo para os DOIS rótulos de cada coluna: o resultado na ponta
  // livre da barra e o caixa acumulado do outro lado da linha do zero.
  const view = chartView(size.width, CHART_HEIGHT, { left: 14, right: 14, top: 22, bottom: 42 });
  const ledger = seasonLedger({ report, team, season, view });

  return (
    <GarageSheet className="px-4 py-3" testId="my-team-v2-round-ledger">
      <div ref={measureRef}>
        {/* Sem título: as colunas com o valor escrito já dizem o que são. O texto do
            título continua no `aria-label` do SVG, para leitor de tela. Fica só a
            legenda dos TIPOS de coluna, que a imagem não consegue dizer sozinha. */}
        <div className="flex flex-wrap items-baseline justify-end gap-3">
          {ledger.hasData ? (
            <p className="text-[10px] uppercase tracking-[0.16em] text-text-muted">{t("myTeamTabV2.ledger.legend")}</p>
          ) : null}
        </div>

        {!ledger.hasData ? (
          <div className="mt-3 border-y border-white/[0.08] px-4 py-8 text-center">
            <ChartColumn size={22} strokeWidth={1.5} aria-hidden="true" className="mx-auto text-text-muted" />
            <p className="mt-2.5 text-[11px] leading-5 text-text-secondary">{t("myTeamTabV2.ledger.empty")}</p>
          </div>
        ) : (
          <>
            <svg
              viewBox={`0 0 ${view.width} ${view.height}`}
              width="100%"
              height={view.height}
              className="mt-3 block"
              role="img"
              aria-label={t("myTeamTabV2.ledger.title")}
            >
              <line x1={view.left} y1={ledger.zeroY} x2={view.right} y2={ledger.zeroY} stroke="currentColor" className="text-white/15" />

              {/* Divisória entre o calendário e o encerramento: o prêmio não é uma
                  rodada, e encostá-lo nas outras colunas sugeria que fosse. */}
              {ledger.prizeDividerX === null ? null : (
                <line
                  x1={ledger.prizeDividerX}
                  y1={ledger.plotTop}
                  x2={ledger.prizeDividerX}
                  y2={ledger.plotBottom}
                  className="stroke-white/10"
                  strokeDasharray="3 4"
                />
              )}

              {ledger.columns.map((column) => (
                <Tooltip key={column.key} texto={tooltipFor(t, column)}>
                  <g data-testid={column.isBreach ? "ledger-breach-column" : undefined}>
                    <rect
                      x={column.x}
                      y={column.y}
                      width={column.width}
                      height={column.height}
                      rx="3"
                      className={barClass(column)}
                      strokeWidth={column.kind === "projected" ? 1.5 : 0}
                      strokeDasharray={column.kind === "projected" ? "4 3" : undefined}
                    />
                    {column.isBreach ? (
                      <rect
                        x={column.x - 2}
                        y={column.y - 2}
                        width={column.width + 4}
                        height={column.height + 4}
                        rx="4"
                        fill="none"
                        className="stroke-status-red"
                        strokeWidth="1.5"
                      />
                    ) : null}
                    {ledger.showValues ? (
                      <>
                        {/* Na ponta livre da barra, quanto a rodada rendeu ou custou. */}
                        <text
                          x={column.centerX}
                          y={column.deltaY}
                          textAnchor="middle"
                          className={`font-garage text-[10px] ${valueClass(column)}`}
                        >
                          {formatSignedMoney(column.net)}
                        </text>
                        {/* Do outro lado da linha do zero, o caixa DEPOIS dela. */}
                        <text
                          x={column.centerX}
                          y={column.cashY}
                          textAnchor="middle"
                          className="fill-text-muted font-garage text-[10px]"
                        >
                          {formatMoneyCompact(column.cash)}
                        </text>
                      </>
                    ) : null}
                  </g>
                </Tooltip>
              ))}

              {ledger.columns.map((column, index) =>
                column.kind === "prize" ? (
                  <text
                    key="prize-label"
                    x={column.centerX}
                    y={view.height - 8}
                    textAnchor="middle"
                    className="fill-status-green text-[10px]"
                  >
                    {t("myTeamTabV2.ledger.prize")}
                  </text>
                ) : index % ledger.labelEvery === 0 ? (
                  <text
                    key={column.key}
                    x={column.centerX}
                    y={view.height - 8}
                    textAnchor="middle"
                    className={`text-[10px] ${column.kind === "projected" ? "fill-text-muted/70" : "fill-text-muted"}`}
                  >
                    {t("myTeamTabV2.ledger.roundLabel", { round: column.round })}
                  </text>
                ) : null,
              )}
            </svg>

            <div className="mt-3 space-y-1.5 border-t border-white/[0.08] pt-3 text-center">
              <p className="text-[11px] leading-5 text-text-secondary" data-testid="ledger-reading">
                {ledger.breach
                  ? t("myTeamTabV2.ledger.readingBreach", {
                      round: ledger.breach.round,
                      reserve: formatMoney(ledger.reserve),
                    })
                  : t("myTeamTabV2.ledger.readingSafe", {
                      value: formatMoney(ledger.finalCash),
                      average: formatSignedMoney(ledger.avgNet),
                    })}
              </p>
              {/* O prêmio precisa dizer TRÊS coisas que a coluna verde sozinha não diz:
                  de onde vem o valor, quando ele cai no caixa, e que ele é estimativa
                  presa a uma posição que ainda pode mudar. */}
              {ledger.prize > 0 ? (
                <p className="text-[11px] leading-5 text-status-green/90" data-testid="ledger-prize-note">
                  {ledger.position > 0
                    ? t("myTeamTabV2.ledger.prizeNote", {
                        value: formatMoney(ledger.prize),
                        position: formatOrdinal(ledger.position),
                        grid: ledger.gridSize,
                      })
                    : t("myTeamTabV2.ledger.prizeNoteNoPosition", { value: formatMoney(ledger.prize) })}
                </p>
              ) : null}
            </div>
          </>
        )}
      </div>
    </GarageSheet>
  );
}

function barClass(column) {
  if (column.kind === "prize") return "fill-status-green";
  if (column.kind === "projected") return "fill-accent-primary/10 stroke-accent-primary/60";
  return column.net >= 0 ? "fill-status-green/70" : "fill-status-red/70";
}

function valueClass(column) {
  if (column.kind === "prize") return "fill-status-green";
  if (column.kind === "projected") return "fill-text-muted";
  return column.net >= 0 ? "fill-status-green" : "fill-status-red";
}

function tooltipFor(t, column) {
  if (column.kind === "prize") {
    return t("myTeamTabV2.ledger.tooltipPrize", { value: formatMoney(column.net) });
  }
  const key = column.kind === "projected" ? "tooltipProjected" : "tooltipReal";
  return t(`myTeamTabV2.ledger.${key}`, {
    round: column.round,
    value: formatSignedMoney(column.net),
    cash: formatMoney(column.cash),
  });
}

export default RoundLedgerChart;
