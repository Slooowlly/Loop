import { useId } from "react";
import { useTranslation } from "react-i18next";
import { Waypoints } from "lucide-react";

import GlassCard from "../../../ui/GlassCard";
import useElementSize from "../../v2/useElementSize";
import { formatMoney } from "../../../../utils/formatters";
import { EXPENSE_LINES, INCOME_LINES } from "../teamMetrics";
import { ribbonPath, roundMoneyFlow } from "./gridMetrics";

const TONES = {
  income: "var(--flow-income)",
  coverage: "var(--flow-coverage)",
  expense: "var(--flow-expense)",
  balance: "var(--flow-balance)",
};

// Para onde foi o dinheiro DESTA rodada — em Sankey, como o fluxo da carreira no
// dossiê de equipe.
//
// As colunas que estavam aqui antes mediam cada saída contra a receita, mas não
// mostravam que é o MESMO dinheiro atravessando: entrou por seis portas, virou um
// bolo só, saiu por cinco. É a largura relativa das fitas que conta a história —
// "operação é metade do tronco" diz mais do que "$21,266" numa lista.
//
// Quando a rodada gasta mais do que arrecada, a diferença aparece como uma fita
// vermelha entrando à esquerda: o dinheiro veio do caixa, e o desenho não finge que
// ele nasceu ali.
function RoundMoneyFlow({ latest, teamColor }) {
  const { t } = useTranslation();
  const uid = useId().replace(/:/g, "");
  const [measureRef, size] = useElementSize({ width: 600, height: 200 });
  const flow = roundMoneyFlow({
    latest,
    incomeLines: INCOME_LINES,
    expenseLines: EXPENSE_LINES,
    width: size.width,
  });

  return (
    <GlassCard hover={false} className="rounded-[24px] p-5" data-testid="my-team-v2-round-flow">
      <div ref={measureRef}>
        {/* Sem título: o desenho se explica — dinheiro entra à esquerda, sai à direita.
            O texto do título sobrevive só como `aria-label` do SVG, para quem lê por
            leitor de tela e não tem a imagem. */}
        <div className="flex flex-wrap items-baseline justify-end gap-3">
          {flow.hasData ? (
            <p className="font-mono text-xs text-text-secondary">
              {t("myTeamTabV2.flow.totalIncome", { value: formatMoney(flow.incomeTotal) })}
            </p>
          ) : null}
        </div>

        {!flow.hasData ? (
          <div className="mt-4 rounded-2xl border border-white/8 bg-black/10 px-4 py-6 text-center">
            <Waypoints size={26} strokeWidth={1.5} aria-hidden="true" className="mx-auto text-text-muted" />
            <p className="mt-2.5 text-xs leading-5 text-text-secondary">{t("myTeamTabV2.flow.empty")}</p>
          </div>
        ) : (
          <svg
            viewBox={`0 0 ${flow.width} ${flow.height}`}
            width="100%"
            height={flow.height}
            className="mt-3 block"
            role="img"
            aria-label={t("myTeamTabV2.flow.title")}
            style={{
              "--flow-income": "#3fb950",
              "--flow-coverage": "#f85149",
              "--flow-expense": "#d29922",
              "--flow-balance": "#58a6ff",
              "--flow-trunk": teamColor || "#58a6ff",
            }}
          >
            <defs>
              {/* Cada fita degrada da cor do NÓ para a cor da equipe no tronco. É o que
                  faz o meio do desenho virar uma massa só — a receita da rodada — em
                  vez de dez fios coloridos atravessando o card. */}
              {flow.left.map((node) => (
                <linearGradient
                  key={`gl-${node.key}`}
                  id={`${uid}-l-${node.key}`}
                  x1={node.x}
                  x2={flow.trunkX}
                  y1="0"
                  y2="0"
                  gradientUnits="userSpaceOnUse"
                >
                  {/* A fita do veredito não desbota: ela tem de puxar o olho antes
                      de qualquer linha de custo. */}
                  <stop offset="0%" stopColor={TONES[node.tone]} stopOpacity={node.verdict ? 0.95 : 0.62} />
                  <stop offset="100%" stopColor="var(--flow-trunk)" stopOpacity={node.verdict ? 0.6 : 0.4} />
                </linearGradient>
              ))}
              {flow.right.map((node) => (
                <linearGradient
                  key={`gr-${node.key}`}
                  id={`${uid}-r-${node.key}`}
                  x1={flow.trunkX + flow.trunkWidth}
                  x2={node.x + 9}
                  y1="0"
                  y2="0"
                  gradientUnits="userSpaceOnUse"
                >
                  <stop offset="0%" stopColor="var(--flow-trunk)" stopOpacity={node.verdict ? 0.6 : 0.4} />
                  <stop offset="100%" stopColor={TONES[node.tone]} stopOpacity={node.verdict ? 0.95 : 0.62} />
                </linearGradient>
              ))}
            </defs>

            {flow.left.map((node) => (
              <g key={`l-${node.key}`}>
                <path
                  d={ribbonPath(node.x + node.pill, node.top, node.bottom, flow.trunkX, node.anchorTop, node.anchorBottom)}
                  fill={`url(#${uid}-l-${node.key})`}
                />
                <rect
                  x={node.x}
                  y={node.top}
                  width={node.pill}
                  height={node.bottom - node.top}
                  rx={node.pill / 2}
                  fill={TONES[node.tone]}
                />
                <FlowLabel
                  x={node.x + node.pill + 8}
                  y={node.top - 7}
                  anchor="start"
                  node={node}
                  label={labelFor(t, node)}
                />
              </g>
            ))}

            {flow.right.map((node) => (
              <g key={`r-${node.key}`}>
                <path
                  d={ribbonPath(
                    flow.trunkX + flow.trunkWidth,
                    node.anchorTop,
                    node.anchorBottom,
                    node.x,
                    node.top,
                    node.bottom,
                  )}
                  fill={`url(#${uid}-r-${node.key})`}
                />
                <rect
                  x={node.x}
                  y={node.top}
                  width={node.pill}
                  height={node.bottom - node.top}
                  rx={node.pill / 2}
                  fill={TONES[node.tone]}
                />
                <FlowLabel x={node.x - 8} y={node.top - 7} anchor="end" node={node} label={labelFor(t, node)} />
              </g>
            ))}

            {/* O tronco vai POR CIMA das fitas: é ele que fecha a conta, e as pontas
                não devem vazar por dentro dele. */}
            <rect
              x={flow.trunkX}
              y={flow.trunkTop}
              width={flow.trunkWidth}
              height={flow.body}
              rx="5"
              fill="var(--flow-trunk)"
              fillOpacity="0.85"
            />
          </svg>
        )}
      </div>
    </GlassCard>
  );
}

// O nó de veredito ganha duas linhas e corpo de manchete; as linhas do livro-caixa
// ficam numa linha só, discretas. A hierarquia é o ponto: o jogador tem de sair
// daqui sabendo se a rodada sobrou ou faltou, mesmo sem ler mais nada.
function FlowLabel({ x, y, anchor, node, label }) {
  if (node.verdict) {
    const tone = node.key === "coverage" ? "fill-status-red" : "fill-accent-hover";
    return (
      <g>
        {/* As duas linhas ficam ACIMA da banda — o respiro extra do `verdictGap`
            existe para elas. Escrever por baixo cairia em cima da própria fita. */}
        <text x={x} y={y - 19} textAnchor={anchor} className="fill-text-secondary text-[12px] uppercase tracking-[0.14em]">
          {label}
          <tspan className="fill-text-muted"> {Math.round(node.share)}%</tspan>
        </text>
        <text x={x} y={y + 2} textAnchor={anchor} className={`${tone} font-mono text-[20px]`}>
          {formatMoney(node.value)}
        </text>
      </g>
    );
  }
  return (
    <text x={x} y={y} textAnchor={anchor} className="fill-text-secondary text-[11px]">
      {label}
      <tspan className="fill-text-primary font-mono"> {formatMoney(node.value)}</tspan>
      <tspan className="fill-text-muted"> {Math.round(node.share)}%</tspan>
    </text>
  );
}

// Os dois nós sintéticos não são linhas do livro-caixa: "Do caixa" é o rombo que a
// equipe cobriu com dinheiro que já tinha, e "Sobra" é o que restou.
function labelFor(t, node) {
  if (node.key === "coverage") return t("myTeamTabV2.flow.coverage");
  if (node.key === "balance") return t("myTeamTabV2.flow.net");
  return t(`myTeamTab.finance.lines.${node.key}`);
}

export default RoundMoneyFlow;
