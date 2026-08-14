import { useId, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { getCategoryColor } from "../../../utils/categoryColors";
import { RUN_MODE_POSITION } from "./evolutionPreferences.js";
import { MEDAL_COLORS, curvaTemDados, temporadasDisputadas } from "./teamHistoryV2Logic";
import { CHIP_GAP, CHIP_HEIGHT, CHIP_MIN_STEP, chipWidth } from "./teamHistoryV2Labels";
import { BlockLabel } from "./teamHistoryV2Primitives.jsx";

// Curva de campeonato: a posição FINAL por temporada.
//
// Separada da campanha acumulada em 11/08/2026 — veja
// [TeamHistoryChampionshipRun.jsx] para o porquê da divisão.

// Geometria da curva. O eixo é INVERTIDO — P1 no topo — porque no automobilismo
// "subir" é diminuir o número, e um gráfico em que a campanha campeã desce
// contraria a leitura antes de qualquer rótulo.
const CURVE_WIDTH = 640;
const CURVE_HEIGHT = 178;

// A calha da esquerda tem que caber o rótulo do eixo E o chip da primeira
// temporada, que é centrado no ponto e portanto invade meia largura de chip para
// fora do desenho. Com a calha estreita, "P4" ficava atrás do chip "P5".
const CURVE_LEFT = 54;
const CURVE_RIGHT = 622;

// O topo reserva a altura de um chip de posição: o ponto de P1 encosta em
// CURVE_TOP, e a etiqueta dele fica acima sem sair do quadro.
const CURVE_TOP = 30;
const CURVE_BOTTOM = 128;

// Onde o desenho de dados acaba e a régua do tempo começa. A linha do eixo separa
// as duas coisas: acima é posição, abaixo é quando aquilo aconteceu.
const CURVE_AXIS = 142;
const CURVE_STRIP_Y = 150;
const CURVE_YEAR_Y = 168;

// Mesma tinta do card que envolve o gráfico. Os pontos são vazados nessa cor para
// a linha não passar por dentro do marcador — sem isso, com quatro temporadas
// seguidas na mesma posição, o ponto some dentro do próprio traço.
const CURVE_SURFACE = "#0f1c2b";

// Curva de campeonato: a posição FINAL por temporada.
//
// Não repete a faixa de top 5 de Records: aquela mede corrida a corrida, esta
// mede o campeonato. Uma equipe regular pode ter poucos top 5 e ainda terminar
// em P3 — quando os dois gráficos discordam, a discordância É a informação.
export function ChampionshipCurve({ seasons, seletor = null, seletorModo = null, modo = RUN_MODE_POSITION }) {
  const { t } = useTranslation();
  const uid = useId().replace(/:/g, "");
  const dados = useMemo(() => {
    if (!curvaTemDados(seasons)) return null;
    const porPosicao = modo === RUN_MODE_POSITION;
    const rows = temporadasDisputadas(seasons);
    const pontos = rows.map((row, index) => {
      const digitos = String(row.position ?? "").match(/\d+/);
      const somados = Number(String(row.points ?? "").replace(/[^\d.-]/g, ""));
      return {
        index,
        year: String(row.year ?? ""),
        category: row.category || "",
        categoryId: row.categoryId || "",
        position: digitos ? Number(digitos[0]) : null,
        points: Number.isFinite(somados) ? somados : null,
      };
    });
    // O valor que o eixo desenha. Em colocação é a posição final; em pontos, o
    // total somado no ano. Pontos de temporadas diferentes NÃO são comparáveis
    // entre categorias — a régua de categorias embaixo do gráfico é o que diz
    // isso, e é por ela que a colocação continua sendo o padrão.
    const valor = (p) => (porPosicao ? p.position : p.points);
    const conhecidos = pontos.map(valor).filter((v) => v !== null && Number.isFinite(v));
    if (!conhecidos.length) return null;

    // `alto` é o topo do desenho e `baixo` o fundo — em colocação o eixo é
    // invertido (P1 no alto), em pontos não. O resto da geometria não precisa
    // saber qual dos dois está em jogo.
    //
    // Em colocação o fundo nunca sobe acima de P6: numa equipe que só terminou
    // em P1 e P2, esticar o eixo entre as duas transformaria um degrau em abismo.
    const pior = Math.max(6, ...conhecidos);
    const alto = porPosicao ? 1 : Math.max(1, ...conhecidos);
    const baixo = porPosicao ? pior : 0;
    const passo = pontos.length > 1 ? (CURVE_RIGHT - CURVE_LEFT) / (pontos.length - 1) : 0;
    const y = (v) => CURVE_TOP + ((v - alto) / (baixo - alto)) * (CURVE_BOTTOM - CURVE_TOP);
    const comXY = pontos.map((p) => ({
      ...p,
      valor: valor(p),
      x: CURVE_LEFT + p.index * passo,
      y: valor(p) === null || !Number.isFinite(valor(p)) ? null : y(valor(p)),
    }));
    // A linha quebra em cada temporada sem posição conhecida (campeonato em
    // andamento, arquivo incompleto): ligar por cima do buraco inventaria um
    // resultado que não existe.
    const trechos = [];
    let atual = [];
    for (const ponto of comXY) {
      if (ponto.y === null) {
        if (atual.length > 1) trechos.push(atual);
        atual = [];
      } else {
        atual.push(ponto);
      }
    }
    if (atual.length > 1) trechos.push(atual);
    return { pontos: comXY, trechos, porPosicao, alto, baixo, pior, passo, y };
  }, [seasons, modo]);

  if (!dados) return null;
  const { pontos, trechos, porPosicao, alto, baixo, passo, y } = dados;
  // As três marcas da régua, no mesmo lugar nos dois modos: topo, meio e fundo
  // da escala. `Set` porque numa escala curta o meio pode coincidir com a ponta.
  const marcas = [...new Set([alto, Math.round((alto + baixo) / 2), baixo])];
  const rotuloMarca = (marca) => (porPosicao ? `P${marca}` : String(Math.round(marca)));
  // O veredito do gráfico, na mesma pílula da campanha: a última temporada
  // fechada. Sem ela, trocar de vista fazia a pílula sumir junto com o resto do
  // cabeçalho, e as duas vistas pareciam blocos diferentes.
  const fechada = [...pontos].reverse().find((ponto) => ponto.position !== null);
  const rotulos = pontos.length > 8 ? 2 : 1;
  // Ids de gradiente precisam ser únicos no documento: o dossiê pode estar aberto
  // ao lado de outro gráfico com os mesmos nomes, e o `url(#...)` pega o primeiro.
  const areaId = `${uid}-area`;
  const glowId = `${uid}-glow`;
  // O chip por temporada só cabe quando as colunas são largas. Numa carreira
  // longa sobram os que carregam informação sozinhos: os títulos e a última
  // temporada já fechada.
  const ultimoFechado = [...pontos].reverse().find((ponto) => ponto.y !== null);
  const chipEmTodos = passo >= CHIP_MIN_STEP;
  return (
    <div>
      {/* O cabeçalho é o MESMO da campanha, slot a slot: rótulo, seletor de
          escala, recorte, e à direita o seletor de métrica com a pílula do
          veredito. Antes cada vista trazia o seu, então trocar de vista renomeava
          o bloco e fazia chrome aparecer do nada — duas telas em vez de duas
          vistas. */}
      <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1.5">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1.5">
          <BlockLabel>{t("myTeamTab.history.sport.championshipTitle")}</BlockLabel>
          {seletor}
          <span className="font-mono text-[10px] text-text-muted">
            {t("myTeamTab.history.sport.curveScope", {
              from: pontos[0]?.year ?? "",
              to: pontos[pontos.length - 1]?.year ?? "",
            })}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {seletorModo}
          {/* A pílula verde do pódio saiu junto com a faixa que ela legendava. Era
              uma terceira cor num gráfico que já tem duas — a da equipe e o ouro
              do título — para marcar uma zona que o eixo com P1 no topo já
              entrega. Esta é outra coisa: o mesmo veredito que a campanha mostra,
              na mesma pílula, aqui referido à última temporada fechada. */}
          {fechada ? (
            <span
              data-testid="team-history-curve-standing"
              className="flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.12em]"
              style={{
                borderColor: "color-mix(in srgb, var(--team) 45%, transparent)",
                backgroundColor: "color-mix(in srgb, var(--team) 10%, transparent)",
                color: "var(--team)",
              }}
            >
              {t("myTeamTab.history.sport.curveStanding", {
                position: fechada.position,
                year: fechada.year,
              })}
            </span>
          ) : null}
        </div>
      </div>
      <div className="mt-2.5 rounded-xl border border-white/[0.06] bg-[#0f1c2b] px-3 py-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.04)]">
        <svg
          viewBox={`0 0 ${CURVE_WIDTH} ${CURVE_HEIGHT}`}
          className="h-auto w-full"
          data-testid="team-history-curve"
        >
          <defs>
            {/* A área sob a linha é o que dá corpo ao gráfico: sem ela, quatro
                pontos ligados por um fio flutuavam num retângulo vazio. Ela desce
                até o fundo da escala, não até o eixo — a faixa de baixo é a régua
                do tempo, e a mancha da equipe não invade a régua. */}
            <linearGradient id={areaId} x1="0" y1={CURVE_TOP} x2="0" y2={CURVE_BOTTOM} gradientUnits="userSpaceOnUse">
              <stop offset="0%" stopColor="var(--team)" stopOpacity="0.42" />
              <stop offset="60%" stopColor="var(--team)" stopOpacity="0.12" />
              <stop offset="100%" stopColor="var(--team)" stopOpacity="0.01" />
            </linearGradient>
            {/* Brilho da linha. A cor da equipe é o assunto do gráfico, e um traço
                de 2px chapado num fundo escuro não sustenta esse papel. */}
            <filter id={glowId} x="-25%" y="-25%" width="150%" height="150%">
              <feGaussianBlur stdDeviation="2.4" result="borrao" />
              <feMerge>
                <feMergeNode in="borrao" />
                <feMergeNode in="SourceGraphic" />
              </feMerge>
            </filter>
          </defs>

          {/* Uma guia vertical por temporada. É o que transforma o vazio entre os
              pontos em colunas legíveis — sem elas o olho não sabe a qual ano
              pertence cada vértice quando a linha corre reta. */}
          {pontos.map((ponto) => (
            <line
              key={`guia-${ponto.year}`}
              x1={ponto.x}
              y1={CURVE_TOP}
              x2={ponto.x}
              y2={CURVE_AXIS}
              stroke="#ffffff"
              strokeOpacity="0.06"
              strokeDasharray="3 5"
            />
          ))}

          {/* Temporada sem posição fechada (campeonato em andamento, arquivo
              incompleto): a coluna fica marcada, mas nenhum ponto é inventado. */}
          {pontos.map((ponto) =>
            ponto.y === null ? (
              <g key={`aberta-${ponto.year}`}>
                <rect
                  x={ponto.x - Math.max(passo / 2, 6)}
                  y={CURVE_TOP}
                  width={Math.max(passo, 12)}
                  height={CURVE_AXIS - CURVE_TOP}
                  fill="#ffffff"
                  fillOpacity="0.022"
                />
                <line
                  x1={ponto.x}
                  y1={CURVE_TOP}
                  x2={ponto.x}
                  y2={CURVE_AXIS}
                  stroke="#7f93a8"
                  strokeOpacity="0.28"
                  strokeDasharray="2 4"
                />
              </g>
            ) : null,
          )}

          {marcas.map((tick) => (
            <g key={tick}>
              <line
                x1={CURVE_LEFT}
                y1={y(tick)}
                x2={CURVE_RIGHT}
                y2={y(tick)}
                stroke="#ffffff"
                strokeOpacity={tick === alto ? 0.1 : 0.05}
                strokeDasharray={tick === alto ? undefined : "2 5"}
              />
              {/* O rótulo recua o suficiente para o chip da primeira temporada
                  passar por fora dele. Todos com o mesmo peso: isto é a régua do
                  desenho, e régua não disputa atenção com a linha. O troféu que
                  ficava ao lado do P1 saiu — ele anunciava um marco onde só há
                  escala, e o título de verdade já é o ponto dourado na curva. */}
              <text
                x={CURVE_LEFT - 17}
                y={y(tick) + 3.4}
                textAnchor="end"
                fontSize="10"
                fontWeight="500"
                fill="#7c8ea3"
              >
                {rotuloMarca(tick)}
              </text>
            </g>
          ))}

          {/* Eixo vertical: fecha o desenho à esquerda e separa a escala da área
              de dados. */}
          <line x1={CURVE_LEFT} y1={CURVE_TOP - 6} x2={CURVE_LEFT} y2={CURVE_AXIS} stroke="#ffffff" strokeOpacity="0.12" />

          {/* Preenchimento e traço vêm do MESMO trecho: onde a linha quebra por
              falta de dado, a mancha quebra junto. */}
          {trechos.map((trecho) => (
            <path
              key={`area-${trecho[0].year}-${trecho.length}`}
              d={`M ${trecho[0].x},${CURVE_BOTTOM} ${trecho.map((p) => `L ${p.x},${p.y}`).join(" ")} L ${
                trecho[trecho.length - 1].x
              },${CURVE_BOTTOM} Z`}
              fill={`url(#${areaId})`}
            />
          ))}
          {trechos.map((trecho) => (
            <polyline
              key={`${trecho[0].year}-${trecho.length}`}
              points={trecho.map((p) => `${p.x},${p.y}`).join(" ")}
              fill="none"
              stroke="var(--team)"
              strokeWidth="2.5"
              strokeLinecap="round"
              strokeLinejoin="round"
              filter={`url(#${glowId})`}
            />
          ))}

          {pontos.map((ponto) => {
            if (ponto.y === null) return null;
            const campeao = ponto.position === 1;
            const cor = campeao ? MEDAL_COLORS.first : "var(--team)";
            return (
              <g key={ponto.year}>
                {/* Halo em todos, forte no título: o ponto tem que se destacar da
                    própria linha, mas só o campeão merece ser achado de longe. */}
                <circle cx={ponto.x} cy={ponto.y} r={campeao ? 9 : 7} fill={cor} fillOpacity={campeao ? 0.22 : 0.12} />
                {/* Recorte na cor do card — o marcador fica por cima da linha em
                    vez de se dissolver nela. */}
                <circle cx={ponto.x} cy={ponto.y} r={campeao ? 5.8 : 4.8} fill={CURVE_SURFACE} />
                <circle
                  data-season={ponto.year}
                  cx={ponto.x}
                  cy={ponto.y}
                  r={campeao ? 4.4 : 3.4}
                  fill={cor}
                  stroke={cor}
                  strokeOpacity="0.4"
                  strokeWidth="2"
                >
                  {/* O balão traz as DUAS leituras em qualquer modo: a métrica
                      escolhida manda no eixo, não no que se pode perguntar de um
                      ponto. */}
                  <title>
                    {t("myTeamTab.history.sport.curveTooltip", {
                      year: ponto.year,
                      category: ponto.category,
                      position: ponto.position ?? "—",
                      points: ponto.points ?? 0,
                    })}
                  </title>
                </circle>
              </g>
            );
          })}

          {/* Chip de posição sobre o ponto. Ler a colocação exata dependia de mirar
              o vértice contra a grade, e a grade só tem três marcas — entre P4 e P6
              não havia como saber se aquilo era P5. */}
          {pontos.map((ponto) => {
            if (ponto.y === null) return null;
            const campeao = ponto.position === 1;
            if (!chipEmTodos && !campeao && ponto !== ultimoFechado) return null;
            const texto = porPosicao ? `P${ponto.position}` : String(Math.round(ponto.points ?? 0));
            const largura = chipWidth(texto);
            // O chip fica acima do ponto; onde não há teto — P1 encosta no topo —
            // ele desce para baixo do marcador em vez de sair do quadro.
            const acima = ponto.y - CHIP_GAP - CHIP_HEIGHT / 2 >= CHIP_HEIGHT / 2;
            const cy = acima ? ponto.y - CHIP_GAP - CHIP_HEIGHT / 2 : ponto.y + CHIP_GAP + CHIP_HEIGHT / 2;
            const cx = Math.min(Math.max(ponto.x, largura / 2 + 2), CURVE_WIDTH - largura / 2 - 2);
            const cor = campeao ? MEDAL_COLORS.first : "var(--team)";
            return (
              <g key={`chip-${ponto.year}`} data-chip={ponto.year}>
                <rect
                  x={cx - largura / 2}
                  y={cy - CHIP_HEIGHT / 2}
                  width={largura}
                  height={CHIP_HEIGHT}
                  rx={4.5}
                  fill={CURVE_SURFACE}
                  fillOpacity="0.95"
                  stroke={cor}
                  strokeOpacity="0.55"
                />
                <text
                  x={cx}
                  y={cy + 3.6}
                  textAnchor="middle"
                  fontSize="10"
                  fontWeight="700"
                  letterSpacing="0.02em"
                  fill={cor}
                >
                  {texto}
                </text>
              </g>
            );
          })}

          {/* Eixo: o corte entre "onde terminou" e "quando foi". */}
          <line x1={CURVE_LEFT} y1={CURVE_AXIS} x2={CURVE_RIGHT} y2={CURVE_AXIS} stroke="#ffffff" strokeOpacity="0.09" />

          {/* Mesma tira de categoria da faixa de Records, aqui embaixo da curva:
              a queda de uma temporada quase sempre tem a promoção como causa, e
              as duas coisas precisam ser lidas juntas. O vão entre os blocos é
              largo de propósito — colada, a tira virava uma barra contínua
              atravessando o gráfico e competia com a própria curva. */}
          {pontos.map((ponto) => {
            // A tira vive DENTRO do eixo: nas pontas ela é meio bloco, senão
            // avançava por cima dos rótulos de posição à esquerda e sangrava para
            // fora do card à direita.
            const esquerda = Math.max(ponto.x - Math.max(passo / 2 - 5, 5), CURVE_LEFT);
            const direita = Math.min(ponto.x + Math.max(passo / 2 - 5, 5), CURVE_RIGHT);
            return (
              <rect
                key={`cat-${ponto.year}`}
                data-category={ponto.categoryId || undefined}
                x={esquerda}
                y={CURVE_STRIP_Y}
                width={Math.max(direita - esquerda, 8)}
                height={3}
                rx={1.5}
                fill={ponto.categoryId ? getCategoryColor(ponto.categoryId) : "transparent"}
                fillOpacity={ponto.y === null ? 0.3 : 0.85}
              />
            );
          })}
          {pontos.map((ponto) =>
            ponto.index % rotulos === 0 ? (
              <text
                key={`ano-${ponto.year}`}
                x={ponto.x}
                y={CURVE_YEAR_Y}
                textAnchor="middle"
                fontSize="10.5"
                fontWeight="600"
                fill={ponto.y === null ? "#4f6076" : "#93a7bb"}
              >
                {ponto.year}
              </text>
            ) : null,
          )}
        </svg>
      </div>
    </div>
  );
}
