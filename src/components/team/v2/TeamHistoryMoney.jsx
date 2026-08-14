import { useId, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { operationHealthTone } from "../teamHistoryDossier";
import { formatMoney, formatMoneyCompact } from "../../../utils/formatters";
import { BlockLabel, InfoCard } from "./teamHistoryV2Primitives.jsx";

// Seção Gestão do dossiê de equipe v2: a curva de caixa ao longo dos anos e o
// fluxo de entrada e saída do dinheiro.
//
// Extraída de `TeamHistoryDrawerV2.jsx` em 11/08/2026. Os dois gráficos leem o
// MESMO `ledger` por ângulos opostos — a curva mostra o saldo no tempo, o fluxo
// mostra a composição dele — e por isso viajam juntos com a seção que os
// emoldura.

const CASH_WIDTH = 640;
const CASH_HEIGHT = 156;
const CASH_LEFT = 8;
const CASH_RIGHT = 632;
const CASH_TOP = 12;

// Fundo da área de dados. Abaixo dele sobra a faixa da régua do tempo, que os
// rótulos de temporada ocupam sem invadir o desenho.
const CASH_FLOOR = 122;
const CASH_AXIS = 138;

// Curva de caixa da carreira inteira, com a dívida pendurada abaixo da linha do
// zero. São duas séries de propósito: caixa e dívida coexistem (dá para ter $1M em
// caixa e $2M de passivo), e um único traço do líquido esconderia justamente a
// equipe que opera alavancada. A leitura fica imediata — o que está acima do zero
// é dinheiro, o que está abaixo é buraco.
function CashCurve({ ledger }) {
  const { t } = useTranslation();
  const uid = useId().replace(/:/g, "");
  const dados = useMemo(() => {
    const pontos = ledger?.cashCurve ?? [];
    // Dois pontos é o mínimo para existir uma curva. Com um, o desenho seria um
    // ponto solto anunciando uma trajetória que ainda não aconteceu.
    if (pontos.length < 2) return null;
    const teto = Math.max(0, ...pontos.map((p) => p.cashBalance));
    const piso = Math.max(0, ...pontos.map((p) => p.debtBalance));
    const amplitude = teto + piso;
    if (amplitude <= 0) return null;
    const passo = (CASH_RIGHT - CASH_LEFT) / (pontos.length - 1);
    const y = (valor) => CASH_TOP + ((teto - valor) / amplitude) * (CASH_FLOOR - CASH_TOP);
    const comXY = pontos.map((ponto, index) => ({
      ...ponto,
      x: CASH_LEFT + index * passo,
      yCaixa: y(ponto.cashBalance),
      yDivida: y(-ponto.debtBalance),
    }));
    // Uma guia por virada de temporada — é o que dá escala de tempo ao eixo sem
    // um rótulo por rodada. A primeira coluna também entra: sem ela a carreira
    // começaria sem ano.
    const viradas = comXY.filter(
      (ponto, index) => index === 0 || ponto.seasonNumber !== comXY[index - 1].seasonNumber,
    );
    return { pontos: comXY, viradas, teto, piso, zero: y(0), temDivida: piso > 0 };
  }, [ledger]);

  if (!dados) return null;
  const { pontos, viradas, teto, piso, zero, temDivida } = dados;
  const areaId = `${uid}-caixa`;
  const dividaId = `${uid}-divida`;
  const linha = pontos.map((p) => `${p.x},${p.yCaixa}`).join(" ");
  const areaCaixa = `M ${pontos[0].x},${zero} ${pontos
    .map((p) => `L ${p.x},${p.yCaixa}`)
    .join(" ")} L ${pontos[pontos.length - 1].x},${zero} Z`;
  const areaDivida = `M ${pontos[0].x},${zero} ${pontos
    .map((p) => `L ${p.x},${p.yDivida}`)
    .join(" ")} L ${pontos[pontos.length - 1].x},${zero} Z`;
  // Só as viradas que cabem ganham rótulo: numa carreira longa os anos colariam
  // um no outro e a régua viraria uma mancha.
  const espacoPorVirada = (CASH_RIGHT - CASH_LEFT) / Math.max(viradas.length, 1);
  const rotulaTodas = espacoPorVirada >= 42;

  return (
    <div className="mt-3">
      <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
        <BlockLabel>{t("myTeamTab.history.management.cashCurve")}</BlockLabel>
        <span className="font-mono text-[11px] text-text-secondary">
          {t("myTeamTab.history.management.cashCurveScale", {
            peak: formatMoneyCompact(teto),
            debt: formatMoneyCompact(piso),
          })}
        </span>
      </div>
      <div className="mt-2 rounded-xl border border-white/[0.06] bg-[#0b1524] px-3 py-2.5">
        <svg
          viewBox={`0 0 ${CASH_WIDTH} ${CASH_HEIGHT}`}
          className="h-auto w-full"
          data-testid="team-history-cash-curve"
        >
          <defs>
            <linearGradient id={areaId} x1="0" y1={CASH_TOP} x2="0" y2={zero} gradientUnits="userSpaceOnUse">
              <stop offset="0%" stopColor="var(--team)" stopOpacity="0.45" />
              <stop offset="100%" stopColor="var(--team)" stopOpacity="0.03" />
            </linearGradient>
            <linearGradient id={dividaId} x1="0" y1={zero} x2="0" y2={CASH_FLOOR} gradientUnits="userSpaceOnUse">
              <stop offset="0%" stopColor="var(--status-red)" stopOpacity="0.06" />
              <stop offset="100%" stopColor="var(--status-red)" stopOpacity="0.5" />
            </linearGradient>
          </defs>

          {/* Guia por temporada. É o que transforma uma linha contínua de rodadas
              em anos legíveis. */}
          {viradas.map((ponto) => (
            <line
              key={`guia-${ponto.seasonNumber}`}
              x1={ponto.x}
              y1={CASH_TOP}
              x2={ponto.x}
              y2={CASH_FLOOR}
              stroke="#ffffff"
              strokeOpacity="0.06"
              strokeDasharray="3 5"
            />
          ))}

          <path d={areaCaixa} fill={`url(#${areaId})`} />
          {temDivida ? <path d={areaDivida} fill={`url(#${dividaId})`} /> : null}

          {/* A linha do zero é a régua moral do gráfico: acima dela é caixa, abaixo
              é dívida. Fica mais forte que as guias porque é o que separa os dois
              lados da história. */}
          <line
            x1={CASH_LEFT}
            y1={zero}
            x2={CASH_RIGHT}
            y2={zero}
            stroke="#ffffff"
            strokeOpacity="0.22"
          />

          <polyline
            points={linha}
            fill="none"
            stroke="var(--team)"
            strokeWidth="2.2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          {temDivida ? (
            <polyline
              points={pontos.map((p) => `${p.x},${p.yDivida}`).join(" ")}
              fill="none"
              stroke="var(--status-red)"
              strokeWidth="1.6"
              strokeOpacity="0.8"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          ) : null}

          {/* Fechamento de temporada: é onde o prêmio de construtores cai no caixa,
              o degrau mais informativo do desenho. Marcado no ponto, não numa
              legenda que ninguém lê. */}
          {pontos
            .filter((ponto) => ponto.isSeasonClose)
            .map((ponto) => (
              <circle
                key={`fecha-${ponto.seasonNumber}-${ponto.round}`}
                cx={ponto.x}
                cy={ponto.yCaixa}
                r="3"
                fill="var(--team)"
                stroke="#0b1524"
                strokeWidth="1.6"
              />
            ))}

          {viradas.map((ponto, index) =>
            rotulaTodas || index === 0 || index === viradas.length - 1 ? (
              <text
                key={`ano-${ponto.seasonNumber}`}
                x={Math.min(Math.max(ponto.x, CASH_LEFT + 10), CASH_RIGHT - 10)}
                y={CASH_AXIS}
                textAnchor="middle"
                fontSize="10"
                fontWeight="500"
                fill="#8ea0b4"
              >
                {t("myTeamTab.history.management.seasonShort", { season: ponto.seasonNumber })}
              </text>
            ) : null,
          )}
        </svg>
      </div>
    </div>
  );
}

// O viewBox é largo de propósito: escalado para a largura do painel, ele fica perto
// de 1:1 e o texto do gráfico sai do mesmo tamanho do texto do resto do dossiê. Num
// viewBox estreito o mesmo `fontSize` chegaria esticado e o gráfico gritaria.
const FLOW_WIDTH = 1040;
const FLOW_PAD_Y = 24;

// Folga vertical entre dois nós do mesmo lado. É o que abre espaço para o rótulo de
// cada fita — sem ela, duas fitas finas vizinhas teriam os rótulos colados.
const FLOW_GAP = 30;
const FLOW_MIN_BAND = 3;
const FLOW_PILL_W = 5;

// As pontas encostam nas BORDAS do viewBox. Antes sobrava um vão morto de ~130
// unidades à direita — herança de reservar espaço para os rótulos, que na verdade
// flutuam ACIMA das fitas e não precisam de coluna própria. O desenho é a coisa mais
// larga da aba; deixá-lo parar no meio do caminho encolhia justamente as diferenças
// de largura que ele existe para mostrar.
const FLOW_LEFT_X = 0;
const FLOW_RIGHT_X = FLOW_WIDTH - FLOW_PILL_W;
const FLOW_TRUNK_W = 13;
const FLOW_TRUNK_X = (FLOW_WIDTH - FLOW_TRUNK_W) / 2;

// Fluxo de dinheiro da carreira inteira: as linhas de receita convergem no tronco e
// saem repartidas em custos e saldo.
//
// Um Sankey e não dois gráficos separados porque a pergunta é uma só — o dinheiro
// que entrou é o mesmo que saiu, e são as LARGURAS relativas que contam a história:
// a folha salarial como metade do tronco diz mais do que "$6,3M" numa lista.
//
// A conta fecha dos dois lados por construção. Quando a equipe gasta mais do que
// arrecada, a diferença entra como um nó próprio à esquerda — o dinheiro veio de
// algum lugar (reservas ou dívida nova), e o desenho não pode fingir que apareceu.
function MoneyFlow({ ledger }) {
  const { t } = useTranslation();
  const uid = useId().replace(/:/g, "");
  const dados = useMemo(() => {
    const receita = ledger?.incomeLines ?? [];
    const custos = ledger?.expenseLines ?? [];
    if (!receita.length && !custos.length) return null;
    const saldo = (ledger.incomeTotal ?? 0) - (ledger.expensesTotal ?? 0);
    const cobertura = Math.max(0, -saldo);
    const tronco = (ledger.incomeTotal ?? 0) + cobertura;
    if (tronco <= 0) return null;

    const esquerda = receita.map((line, index) => ({
      key: line.id,
      label: t(`myTeamTab.finance.lines.${line.id}`),
      value: line.value,
      hue: "var(--status-green)",
      fade: Math.max(0.3, 1 - index * 0.15),
    }));
    if (cobertura > 0) {
      esquerda.push({
        key: "coverage",
        label: t("myTeamTab.history.management.flowCoverage"),
        value: cobertura,
        hue: "var(--status-red)",
        fade: 1,
      });
    }
    const direita = custos.map((line, index) => ({
      key: line.id,
      label: t(`myTeamTab.finance.lines.${line.id}`),
      value: line.value,
      hue: "var(--status-yellow)",
      fade: Math.max(0.3, 1 - index * 0.15),
    }));
    if (saldo > 0) {
      direita.push({
        key: "balance",
        label: t("myTeamTab.history.management.flowBalance"),
        value: saldo,
        hue: "var(--status-green)",
        fade: 1,
      });
    }

    // A altura do desenho é derivada, não fixa: o lado com mais nós define quantas
    // folgas cabem, e o tronco fica com o que sobra. Assim o gráfico cresce com o
    // dado em vez de espremer oito fitas numa caixa de altura fixa.
    const folgas = Math.max(esquerda.length, direita.length) - 1;
    const corpo = Math.max(120, 26 * (folgas + 1));
    const altura = FLOW_PAD_Y * 2 + corpo + folgas * FLOW_GAP;
    const banda = (valor) => Math.max(FLOW_MIN_BAND, (valor / tronco) * corpo);

    const empilha = (nos) => {
      const total = nos.reduce((soma, no) => soma + banda(no.value), 0) + (nos.length - 1) * FLOW_GAP;
      let cursor = (altura - total) / 2;
      return nos.map((no) => {
        const h = banda(no.value);
        const topo = cursor;
        cursor += h + FLOW_GAP;
        return { ...no, topo, base: topo + h, share: (no.value / tronco) * 100 };
      });
    };

    const nosEsquerda = empilha(esquerda);
    const nosDireita = empilha(direita);
    // As fitas chegam ao tronco na MESMA ordem em que saem dos nós, empilhadas sem
    // folga: o tronco é contínuo, é o total.
    const troncoTopo = (altura - corpo) / 2;
    let cursorEsq = troncoTopo;
    const fitasEsquerda = nosEsquerda.map((no) => {
      const h = no.base - no.topo;
      const ancora = cursorEsq;
      cursorEsq += h;
      return { ...no, ancoraTopo: ancora, ancoraBase: ancora + h };
    });
    let cursorDir = troncoTopo;
    const fitasDireita = nosDireita.map((no) => {
      const h = no.base - no.topo;
      const ancora = cursorDir;
      cursorDir += h;
      return { ...no, ancoraTopo: ancora, ancoraBase: ancora + h };
    });

    return { fitasEsquerda, fitasDireita, altura, corpo, troncoTopo, tronco };
  }, [ledger, t]);

  // Sem repartição o bloco NÃO some — ele explica. Sumir era o pior estado: o
  // jogador não distinguia "esta equipe não tem economia de rodada" de "o gráfico
  // quebrou", e a frase vem pronta do backend, que é quem sabe a causa.
  if (!dados) {
    if (!ledger?.flowNote) return null;
    return (
      <div className="rounded-xl border border-white/10 bg-[#0c1626]/95 p-4" data-testid="team-history-money-flow">
        <BlockLabel>{t("myTeamTab.history.management.moneyFlow")}</BlockLabel>
        <p className="mt-2 text-[11px] leading-5 text-text-secondary">{ledger.flowNote}</p>
      </div>
    );
  }
  const { fitasEsquerda, fitasDireita, altura, corpo, troncoTopo, tronco } = dados;
  const janela =
    ledger.flowFirstSeason === ledger.flowLastSeason
      ? t("myTeamTab.history.management.flowWindowOne", { season: ledger.flowLastSeason })
      : t("myTeamTab.history.management.flowWindowRange", {
          first: ledger.flowFirstSeason,
          last: ledger.flowLastSeason,
        });

  return (
    <div className="rounded-xl border border-white/10 bg-[#0c1626]/95 p-4" data-testid="team-history-money-flow">
      <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
        <BlockLabel>{t("myTeamTab.history.management.moneyFlow")}</BlockLabel>
        {/* A legenda diz a JANELA, não "da fundação até aqui". O livro-caixa rodada
            a rodada existe só nas temporadas jogadas; as de backstory registram só
            o prêmio de construtores, e prometer a carreira inteira aqui seria
            vender uma soma que a tabela não tem. */}
        <span className="text-[11px] text-text-secondary">{janela}</span>
      </div>
      <svg
        viewBox={`0 0 ${FLOW_WIDTH} ${altura}`}
        className="mt-2 h-auto w-full"
        data-testid="team-history-money-flow-chart"
      >
        <defs>
          {/* Uma fita degrada da cor do NÓ para a cor da equipe no tronco. É o que
              faz o meio do desenho virar uma massa só — a receita da equipe — em
              vez de doze fios coloridos atravessando a tela. */}
          {fitasEsquerda.map((no) => (
            <linearGradient
              key={`ge-${no.key}`}
              id={`${uid}-e-${no.key}`}
              x1={FLOW_LEFT_X}
              x2={FLOW_TRUNK_X}
              y1="0"
              y2="0"
              gradientUnits="userSpaceOnUse"
            >
              <stop offset="0%" stopColor={no.hue} stopOpacity={0.6 * no.fade} />
              <stop offset="100%" stopColor="var(--team)" stopOpacity="0.42" />
            </linearGradient>
          ))}
          {fitasDireita.map((no) => (
            <linearGradient
              key={`gd-${no.key}`}
              id={`${uid}-d-${no.key}`}
              x1={FLOW_TRUNK_X + FLOW_TRUNK_W}
              x2={FLOW_RIGHT_X}
              y1="0"
              y2="0"
              gradientUnits="userSpaceOnUse"
            >
              <stop offset="0%" stopColor="var(--team)" stopOpacity="0.42" />
              <stop offset="100%" stopColor={no.hue} stopOpacity={0.6 * no.fade} />
            </linearGradient>
          ))}
        </defs>

        {fitasEsquerda.map((no) => (
          <g key={`fe-${no.key}`}>
            <path
              d={ribbonPath(FLOW_LEFT_X + FLOW_PILL_W, no.topo, no.base, FLOW_TRUNK_X, no.ancoraTopo, no.ancoraBase)}
              fill={`url(#${uid}-e-${no.key})`}
            />
            <rect
              x={FLOW_LEFT_X}
              y={no.topo}
              width={FLOW_PILL_W}
              height={no.base - no.topo}
              rx={FLOW_PILL_W / 2}
              fill={no.hue}
              fillOpacity={no.fade}
            />
            <FlowLabel x={FLOW_LEFT_X + FLOW_PILL_W + 8} y={no.topo - 7} anchor="start" node={no} />
          </g>
        ))}

        {fitasDireita.map((no) => (
          <g key={`fd-${no.key}`}>
            <path
              d={ribbonPath(
                FLOW_TRUNK_X + FLOW_TRUNK_W,
                no.ancoraTopo,
                no.ancoraBase,
                FLOW_RIGHT_X,
                no.topo,
                no.base,
              )}
              fill={`url(#${uid}-d-${no.key})`}
            />
            <rect
              x={FLOW_RIGHT_X}
              y={no.topo}
              width={FLOW_PILL_W}
              height={no.base - no.topo}
              rx={FLOW_PILL_W / 2}
              fill={no.hue}
              fillOpacity={no.fade}
            />
            <FlowLabel x={FLOW_RIGHT_X - 8} y={no.topo - 7} anchor="end" node={no} />
          </g>
        ))}

        {/* O tronco por cima das fitas: é ele que fecha a conta, e as pontas das
            fitas não devem vazar por dentro dele. */}
        <rect
          x={FLOW_TRUNK_X}
          y={troncoTopo}
          width={FLOW_TRUNK_W}
          height={corpo}
          rx={FLOW_TRUNK_W / 2}
          fill="var(--team)"
        />
        <text
          x={FLOW_TRUNK_X + FLOW_TRUNK_W / 2}
          y={troncoTopo - 9}
          textAnchor="middle"
          fontSize="11"
          fontWeight="600"
          fill="#e6edf3"
        >
          {t("myTeamTab.history.management.flowTrunk", { value: formatMoney(tronco) })}
        </text>
      </svg>
    </div>
  );
}

// Rótulo de um nó do fluxo: nome, valor e fatia numa linha só, como as etiquetas
// que flutuam ao lado das fitas. `textLength` fica de fora — deixar o SVG espremer
// o texto para caber quebraria a régua tipográfica do resto do dossiê.
function FlowLabel({ x, y, anchor, node }) {
  return (
    <text x={x} y={y} textAnchor={anchor} fontSize="11" fill="#8ea0b4">
      <tspan fill="#e6edf3">{node.label}</tspan>
      <tspan dx="7">{formatMoneyCompact(node.value)}</tspan>
      <tspan dx="7">{`${Math.round(node.share)}%`}</tspan>
    </text>
  );
}

// Fita do Sankey: duas cúbicas espelhadas com os controles no meio do vão, que é o
// que dá a curva em S sem depender de biblioteca.
function ribbonPath(x0, topo0, base0, x1, topo1, base1) {
  const meio = (x0 + x1) / 2;
  return [
    `M ${x0},${topo0}`,
    `C ${meio},${topo0} ${meio},${topo1} ${x1},${topo1}`,
    `L ${x1},${base1}`,
    `C ${meio},${base1} ${meio},${base0} ${x0},${base0}`,
    "Z",
  ].join(" ");
}

export function ManagementSection({ dossier }) {
  const { t } = useTranslation();
  // A saúde da operação é o único bloco que muda de cor por conteúdo — vermelho
  // para pressionada/crise, amarelo para estável, verde para saudável. A regra é
  // a mesma que monta a frase (../teamHistoryDossier.js), importada em vez de
  // recopiada.
  const tone = operationHealthTone(dossier.management.operationHealth);
  const ledger = dossier.management.ledger;
  return (
    <section className="grid gap-2.5">
      {/* O fluxo abre a aba: é o desenho que responde "de onde vem e para onde vai"
          antes de qualquer rótulo, e a largura das fitas carrega a leitura sozinha.
          Saúde e curva descem para depois dos extremos — a frase da saúde é um
          RESUMO, e resumo depois do dado lê melhor do que antes. */}
      {ledger ? <MoneyFlow ledger={ledger} /> : null}
      <div className="grid gap-2.5 md:grid-cols-2">
        <div className="rounded-xl border border-status-green/25 bg-[#0b1d19]/95 p-4">
          <BlockLabel>{t("myTeamTab.history.management.peakCash")}</BlockLabel>
          <strong className="mt-1.5 block font-mono text-sm text-status-green">{dossier.management.peakCash}</strong>
          <p className="mt-1.5 text-[11px] leading-5 text-text-secondary">{dossier.management.peakCashDetail}</p>
        </div>
        <div className="rounded-xl border border-status-red/25 bg-[#241014]/95 p-4">
          <BlockLabel>{t("myTeamTab.history.management.worstCrisis")}</BlockLabel>
          <strong className="mt-1.5 block font-mono text-sm text-status-red">{dossier.management.worstCrisis}</strong>
          <p className="mt-1.5 text-[11px] leading-5 text-text-secondary">{dossier.management.worstCrisisDetail}</p>
        </div>
      </div>
      {/* Saúde e curva seguem no MESMO painel: a frase é a leitura do momento e a
          curva é a prova dela. Separadas, o jogador lia "Monitorada" sem nada que
          dissesse se a equipe está subindo ou afundando. */}
      <div className={`rounded-xl border p-4 ${tone.card}`}>
        <BlockLabel>{t("myTeamTab.history.management.operationHealth")}</BlockLabel>
        <strong className={`mt-1.5 block text-xl font-semibold ${tone.text}`}>{dossier.management.operationHealth}</strong>
        <p className="mt-2 text-[11px] leading-5 text-text-secondary">{dossier.management.summary}</p>
        {ledger ? <CashCurve ledger={ledger} /> : null}
      </div>
      <div className="grid gap-2.5 md:grid-cols-2">
        <InfoCard
          label={t("myTeamTab.history.management.healthyYears")}
          value={dossier.management.healthyYears}
          detail={dossier.management.healthyYearsDetail}
        />
        <InfoCard
          label={t("myTeamTab.history.management.biggestInvestment")}
          value={dossier.management.biggestInvestment}
          detail={dossier.management.investmentDetail}
        />
      </div>
      {dossier.ownershipEvents?.length > 0 && (
        <div className="rounded-xl border border-status-yellow/25 bg-[#201a0b]/95 p-4">
          <BlockLabel>{t("myTeamTab.history.management.boardChanges")}</BlockLabel>
          <ul className="mt-2.5 grid gap-2.5 md:grid-cols-2">
            {dossier.ownershipEvents.map((event, index) => (
              <li key={index} className="flex items-start gap-3">
                <span className="mt-0.5 font-mono text-xs font-bold text-status-yellow">{event.year}</span>
                <div className="min-w-0">
                  <strong className="block text-xs font-semibold text-text-primary">{event.title}</strong>
                  <p className="text-[11px] leading-5 text-text-secondary">{event.financialNote}</p>
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
