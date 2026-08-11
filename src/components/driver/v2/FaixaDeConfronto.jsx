import { useState } from "react";
import { useTranslation } from "react-i18next";

import {
  DUEL_LOSS_COLOR,
  DUEL_WIN_COLOR,
  agrupaPorTemporada,
  colunasDoConfronto,
  primeiroNome,
} from "./driverDetailV2Logic";

// ── O confronto desenhado ──
//
// As duas leituras do duelo com um rival: a FAIXA cheia, que abre o card do
// rival principal, e a miniatura sem legenda que cada linha da lista carrega.
//
// Saiu de `DriverDetailModalV2.jsx` em 11/08/2026, junto com a curva de mercado,
// pelo mesmo motivo: é desenho de gráfico, e desenho de gráfico da ficha mora em
// módulo irmão desde que a curva de campeonato abriu o precedente.

// A FAIXA DO CONFRONTO: uma marca por corrida dividida, na ordem em que
// aconteceram, agrupadas por temporada.
//
// O placar diz QUANTO ("36 a 29") e a faixa diz QUANDO — se a vantagem é antiga e
// virou, se é uma sequência recente, se um ano inteiro foi de mão única. Nenhum
// par de números conta isso, e é a coisa mais interessante que a aba tem para
// mostrar sobre duas pessoas que correm juntas há sete anos.
//
// A colocação de cada um saiu: "P6 contra P1" faz o olho comparar dois números
// para chegar na única coisa que importa, que é quem ganhou o dia. A cor da marca
// já é essa resposta, e o nome de quem venceu fica em cima.
export function DuelTimeline({ rival, dono }) {
  const { t } = useTranslation();
  const encontros = Array.isArray(rival.encontros) ? rival.encontros : [];
  // O último encontro é o estado de repouso da legenda: é o fato mais recente, e
  // quem não passar o mouse em nada leva daqui a última vez que se cruzaram.
  const [emFoco, setEmFoco] = useState(null);
  if (!encontros.length) return null;

  const destaque = encontros[emFoco ?? encontros.length - 1] ?? encontros[encontros.length - 1];
  const temporadas = agrupaPorTemporada(encontros);
  const vencedorLabel =
    destaque.vencedor === "piloto"
      ? dono
      : destaque.vencedor === "rival"
        ? primeiroNome(rival.nome)
        : t("driverDetail.rivals.noWinner");
  const vencedorCor =
    destaque.vencedor === "piloto"
      ? DUEL_WIN_COLOR
      : destaque.vencedor === "rival"
        ? DUEL_LOSS_COLOR
        : undefined;

  const colunas = colunasDoConfronto(encontros);
  const gapDestaque = Number.isFinite(destaque.gap) ? destaque.gap : null;

  return (
    <div className="mt-3.5" data-testid="driver-detail-duel-timeline">
      {/* A legenda vem ANTES do gráfico, e não depois: ela é o que o gráfico
          está dizendo, e legenda embaixo obriga o olho a descer para saber o
          que acabou de tocar. */}
      <div className="flex items-baseline justify-between gap-3 text-[11px] leading-4">
        <span className="min-w-0 truncate text-text-secondary">
          {t("driverDetail.rivals.meetingStamp", {
            track: destaque.pista,
            year: destaque.ano,
          })}
        </span>
        <strong
          className="shrink-0 font-semibold"
          style={vencedorCor ? { color: vencedorCor } : undefined}
          data-testid="driver-detail-duel-winner"
        >
          {vencedorLabel}
          {gapDestaque === null ? null : (
            <span className="ml-1.5 font-mono font-normal text-text-muted">
              {t("driverDetail.rivals.gapValue", { value: Math.abs(gapDestaque).toFixed(1) })}
            </span>
          )}
        </strong>
      </div>

      <svg
        viewBox={`0 0 ${colunas.length} ${DUEL_CHART.altura}`}
        preserveAspectRatio="none"
        className="mt-1.5 h-[46px] w-full"
        onMouseLeave={() => setEmFoco(null)}
        aria-hidden="true"
      >
        <DuelGapBand colunas={colunas} />
        {colunas.map((coluna) => (
          <rect
            key={coluna.indice}
            // A área de toque é a COLUNA INTEIRA, e não a barra: uma corrida de
            // três décimos desenha três pixels de barra, e caçar três pixels com
            // o ponteiro não é interação, é teste de pontaria.
            data-duel-mark={coluna.vencedor}
            data-em-foco={emFoco === coluna.indice || undefined}
            onMouseEnter={() => setEmFoco(coluna.indice)}
            x={coluna.indice}
            y={0}
            width={1}
            height={DUEL_CHART.altura}
            fill="transparent"
          />
        ))}
      </svg>

      {/* Os anos embaixo, com a largura de cada bloco proporcional às corridas
          daquele ano: um ano de 20 encontros ocupa o dobro de um de 10, então a
          própria largura já conta quanto os dois se cruzaram — sem eixo. */}
      <div className="flex">
        {temporadas.map((temporada) => (
          <span
            key={`${temporada.season_number}-${temporada.ano}`}
            data-duel-season={temporada.ano}
            className="min-w-0 truncate text-center text-[10px] leading-3 text-text-muted"
            style={{ flex: temporada.corridas.length }}
          >
            {temporada.ano || temporada.season_number}
          </span>
        ))}
      </div>
    </div>
  );
}

// A geometria do gráfico, em unidades do `viewBox`. O SVG estica na
// horizontal (`preserveAspectRatio="none"`) e mantém a altura, então uma unidade
// vertical é sempre a mesma coisa e uma horizontal é "uma corrida", qualquer que
// seja a largura do card.
const DUEL_CHART = {
  altura: 40,
  gapCentro: 20,
  gapAmplitude: 17,
  // Uma corrida decidida por um décimo tem que aparecer. Sem piso ela viraria
  // uma linha de zero pixel e o dia sumiria do gráfico.
  pisoDaBarra: 3,
};

// Uma barra por corrida, divergindo da linha central.
//
// O lado sai de quem venceu e a altura, do tempo entre os dois. Antes eram
// marcas de altura igual, e "ele me ganhou por três décimos vinte vezes" e "ele
// me tomou uma volta" desenhavam exatamente a mesma coisa.
function DuelGapBand({ colunas }) {
  const { gapCentro, gapAmplitude, pisoDaBarra } = DUEL_CHART;
  const maiorGap = Math.max(...colunas.map((coluna) => Math.abs(coluna.gap ?? 0)), 0);

  const altura = (gap) => {
    if (!maiorGap || !Number.isFinite(gap)) return pisoDaBarra;
    // RAIZ e não proporção direta: uma corrida em que alguém tomou uma volta
    // vale 40s e achataria as outras quarenta e quatro contra a linha central.
    // A raiz preserva a ordem e devolve as pequenas ao campo visível — o valor
    // exato quem dá é a legenda, aqui em cima.
    const escala = Math.sqrt(Math.abs(gap) / maiorGap);
    return Math.max(pisoDaBarra, escala * gapAmplitude);
  };

  return (
    <g data-duel-band="gap">
      <line
        x1={0}
        x2={colunas.length}
        y1={gapCentro}
        y2={gapCentro}
        stroke="rgba(255,255,255,0.1)"
        vectorEffect="non-scaling-stroke"
      />
      {colunas.map((coluna) => {
        if (coluna.vencedor !== "piloto" && coluna.vencedor !== "rival") {
          // Corrida que não decidiu nada vira um traço sobre a linha central:
          // ela aconteceu, e some do gráfico se não desenhar nada.
          return (
            <rect
              key={coluna.indice}
              x={coluna.indice + 0.15}
              y={gapCentro - 0.75}
              width={0.7}
              height={1.5}
              fill="rgba(255,255,255,0.16)"
            />
          );
        }
        const venceu = coluna.vencedor === "piloto";
        const h = altura(coluna.gap);
        return (
          <rect
            key={coluna.indice}
            x={coluna.indice + 0.15}
            y={venceu ? gapCentro - h : gapCentro}
            width={0.7}
            height={h}
            fill={venceu ? DUEL_WIN_COLOR : DUEL_LOSS_COLOR}
            opacity={0.85}
          />
        );
      })}
    </g>
  );
}

// A faixa da linha: sem legenda, sem ano e sem hover — nesse tamanho ela é uma
// FORMA, não um gráfico de consulta.
//
// DOZE corridas, e não vinte: com quarenta e cinco marcas em 96px cada uma some
// abaixo de dois pixels e as três linhas viram a mesma textura vermelha. O
// recorte curto é o que ainda descreve a relação, e é ele que faz um rival
// parecer diferente do outro na lista.
const MINI_TIMELINE_RACES = 12;

export function MiniTimeline({ encontros }) {
  const lista = Array.isArray(encontros) ? encontros.slice(-MINI_TIMELINE_RACES) : [];
  if (!lista.length) return null;

  return (
    <span className="hidden w-24 shrink-0 gap-px sm:flex" aria-hidden="true">
      {lista.map((corrida, indice) => (
        <span
          key={`${corrida.season_number}-${corrida.rodada}-${indice}`}
          data-duel-mark={corrida.vencedor}
          className="h-2 min-w-[2px] flex-1 rounded-[1px]"
          style={{
            backgroundColor:
              corrida.vencedor === "piloto"
                ? DUEL_WIN_COLOR
                : corrida.vencedor === "rival"
                  ? DUEL_LOSS_COLOR
                  : "rgba(255,255,255,0.14)",
            opacity: 0.82,
          }}
        />
      ))}
    </span>
  );
}
