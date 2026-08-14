import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";

import Tooltip from "../../ui/Tooltip";
import { BlockLabel, MedalKey } from "./primitivosDaFicha.jsx";
import { MEDAL_COLORS, finishColor } from "./driverDetailV2Logic";

const FORM_HEIGHT = 104;
// Largura máxima de uma corrida na faixa de forma e o vão entre elas — os mesmos
// números das classes `max-w-[64px]`/`basis-16` e `gap-1` das colunas. Ficam aqui
// porque o teto de largura de cada temporada é calculado a partir deles.
const FORM_CELL = 64;
const FORM_CELL_GAP = 4;
// A divisa de uma temporada para a outra: `border-l` + `pl-4`.
const FORM_GROUP_DIVIDER = 17;
// Trilhos das rodadas que ainda vêm. Fixo e generoso: o que passar da faixa é
// cortado pelo `overflow-hidden`, e o fade esconde o corte.
const FORM_GHOST_SLOTS = 24;
const FORM_GHOST_MASK =
  "linear-gradient(90deg, rgba(0,0,0,0.9) 0%, rgba(0,0,0,0.35) 55%, rgba(0,0,0,0) 100%)";

// Forma recente: uma coluna por corrida, altura = quão à frente o piloto chegou,
// cor = a colocação.
//
// O v1 desenhava isso como uma linha de tendência num SVG de 760px — bonito e
// caro de ler: para saber onde foi a vitória você precisava seguir a curva até o
// ponto mais alto e depois procurar o rótulo. Aqui a vitória é dourada e alta, o
// DNF é vermelho e não tem coluna, e a sequência se lê de relance.
//
// A escala é INVERTIDA de propósito: P1 no topo. Posição é a única métrica do
// jogo em que menor é melhor, e desenhar a barra crescendo para baixo seria
// tecnicamente correto e visualmente mentiroso.
//
// A janela é o CALENDÁRIO, não um número redondo de corridas. Cinco quadradinhos
// não diziam de que ano eram nem quanto do ano cobriam — podiam ser o fim de uma
// temporada, o começo da seguinte ou metade de cada. Agora entra a temporada
// anterior inteira mais a atual até aqui, separadas: à esquerda o ano fechado, à
// direita o que está acontecendo.
export function RecentFormStrip({ seasons, entries, context }) {
  const { t } = useTranslation();
  const grupos = useMemo(() => {
    // `temporadas` é o caminho normal. `entries` é o payload antigo (save aberto
    // por build anterior), que não sabe de que temporada é cada corrida: vira um
    // grupo só, sem rótulo de ano — melhor que sumir com a faixa.
    const porTemporada = Array.isArray(seasons) && seasons.length
      ? seasons
          .filter((season) => Array.isArray(season?.resultados) && season.resultados.length)
          .map((season) => ({
            key: `s${season.season_number}`,
            year: season.ano || null,
            current: Boolean(season.atual),
            rows: season.resultados,
          }))
      : [{ key: "janela", year: null, current: false, rows: Array.isArray(entries) ? entries : [] }];

    const comCorridas = porTemporada.filter((grupo) => grupo.rows.length);
    if (!comCorridas.length) return [];

    // A escala do eixo é COMUM às temporadas: com uma régua por grupo, um P8 de
    // 2025 desenharia mais alto que um P8 de 2026 só porque o pior resultado do
    // ano foi outro, e a faixa deixaria de comparar as duas.
    const finishes = comCorridas
      .flatMap((grupo) => grupo.rows)
      .filter((row) => !row?.dnf && Number.isFinite(row?.chegada))
      .map((row) => row.chegada);
    const worst = Math.max(20, ...finishes, 1);

    return comCorridas.map((grupo) => {
      const bars = grupo.rows.map((row, index) => {
        const dnf = Boolean(row?.dnf);
        const finish = Number.isFinite(row?.chegada) ? row.chegada : null;
        return {
          key: `${grupo.key}-${row?.rodada ?? "r"}-${index}`,
          round: row?.rodada ?? null,
          dnf,
          finish,
          height: dnf || finish === null ? 0 : ((worst - finish) / Math.max(1, worst - 1)) * 100,
          color: finishColor(dnf, finish),
          title: dnf
            ? t("driverDetail.v2.form.tooltipDnf", { round: row?.rodada ?? "-" })
            : t("driverDetail.v2.form.tooltip", {
                round: row?.rodada ?? "-",
                position: finish ?? "-",
              }),
        };
      });
      const pontuadas = bars.filter((bar) => !bar.dnf && bar.finish);
      return {
        ...grupo,
        bars,
        average: pontuadas.length
          ? pontuadas.reduce((soma, bar) => soma + bar.finish, 0) / pontuadas.length
          : null,
        best: pontuadas.length ? Math.min(...pontuadas.map((bar) => bar.finish)) : null,
        dnfs: bars.filter((bar) => bar.dnf).length,
      };
    });
  }, [seasons, entries, t]);

  if (!grupos.length) {
    return (
      <div className="mt-5">
        <BlockLabel>{t("driverDetail.summary.recentFormTitle")}</BlockLabel>
        <div className="mt-2 rounded-xl bg-[#0f1c2b] px-4 py-3.5 text-xs text-text-secondary">
          {context === "sem_time_temporada_passada"
            ? t("driverDetail.summary.noTeamLastSeasonBody")
            : context
              ? t("driverDetail.summary.noRacesLastSeasonBody")
              : t("driverDetail.summary.insufficientBody")}
        </div>
      </div>
    );
  }

  return (
    <div className="mt-5">
      {/* Sem contador ao lado do rótulo: as barras JÁ são a contagem, uma por
          corrida, e o número repetia em cinza o que o desenho mostra inteiro. */}
      <BlockLabel>{t("driverDetail.summary.recentFormTitle")}</BlockLabel>

      <div
        className="mt-2 flex gap-2 rounded-xl bg-[#0f1c2b] px-4 py-3.5"
        data-testid="driver-detail-form-strip"
      >
        {/* A calha do eixo fica FORA da área dos grupos: dentro, ela entraria na
            divisão proporcional e o "P1" deslizaria junto com as barras. */}
        <div className="w-7 shrink-0 pt-4">
          <div className="relative" style={{ height: FORM_HEIGHT }}>
            <span className="absolute right-0 top-0 -translate-y-1/2 font-mono text-[10px] text-text-muted">
              P1
            </span>
            <span className="absolute bottom-0 right-0 translate-y-1/2 font-mono text-[10px] text-text-muted">
              {t("driverDetail.summary.worst")}
            </span>
          </div>
        </div>

        {/* Cada grupo cresce na PROPORÇÃO do número de corridas — é o que faz uma
            coluna de 2025 ter a mesma largura de uma de 2026 mesmo com os anos
            tendo calendários de tamanhos diferentes. */}
        <div className="flex min-w-0 flex-1 gap-4">
          {grupos.map((grupo, index) => (
            <div
              key={grupo.key}
              data-season={grupo.year ?? undefined}
              data-current={grupo.current ? "true" : undefined}
              className={`min-w-0 ${index > 0 ? "border-l border-white/[0.12] pl-4" : ""}`}
              style={{
                flexGrow: grupo.bars.length,
                flexBasis: 0,
                // Temporada FECHADA não cresce além do que as corridas dela
                // ocupam. Sem esse teto, um ano de 6 corridas ficava com 6/7 da
                // faixa enquanto as colunas paravam em FORM_CELL — e sobrava um
                // buraco entre a última corrida e a divisa do ano seguinte.
                // A sobra vai toda para a temporada em curso, que é onde ela
                // significa alguma coisa: o resto do calendário.
                ...(grupo.current
                  ? { minWidth: 168 }
                  : {
                      maxWidth:
                        grupo.bars.length * FORM_CELL +
                        (grupo.bars.length - 1) * FORM_CELL_GAP +
                        (index > 0 ? FORM_GROUP_DIVIDER : 0),
                    }),
              }}
            >
              {/* Cabeçalho do grupo: o ano à esquerda e a leitura DAQUELE ano à
                  direita. Média cruzando duas temporadas não é média de nada — é
                  a mistura de dois campeonatos num número só. */}
              <div className="mb-1 flex items-baseline justify-between gap-2">
                <span
                  className={`truncate font-mono text-[10px] ${
                    grupo.current ? "text-[color:var(--team)]" : "text-text-muted"
                  }`}
                >
                  {grupo.year
                    ? grupo.current
                      ? t("driverDetail.v2.form.seasonCurrent", { year: grupo.year })
                      : grupo.year
                    : t("driverDetail.summary.recentFormTitle")}
                </span>
                <span className="shrink-0 truncate text-[10px] text-text-muted">
                  {grupo.average
                    ? t("driverDetail.v2.form.seasonAverage", { value: grupo.average.toFixed(1) })
                    : ""}
                  {grupo.dnfs > 0 ? ` · ${t("driverDetail.v2.form.seasonDnfs", { count: grupo.dnfs })}` : ""}
                </span>
              </div>

              <div className="relative flex items-end gap-1" style={{ height: FORM_HEIGHT }}>
                {grupo.bars.map((bar) => (
                  <Tooltip key={bar.key} texto={bar.title}>
                  <div
                    data-round={bar.round ?? undefined}
                    data-dnf={bar.dnf ? "true" : undefined}
                    className="relative h-full min-w-[14px] max-w-[64px] grow basis-16"
                  >
                    {/* Trilho: a coluna VAZIA, sempre desenhada. Sem ele, uma
                        corrida de último lugar não tinha pixel algum e ficava
                        idêntica a uma corrida que não existiu. */}
                    <div className="absolute inset-0 rounded-md bg-white/[0.045]" />
                    {bar.dnf ? (
                      <div className="absolute inset-x-0 bottom-0 grid h-full place-items-center rounded-md border border-dashed border-status-red/40">
                        <X
                          size={12}
                          strokeWidth={1.8}
                          aria-hidden="true"
                          className="text-status-red/70"
                        />
                      </div>
                    ) : (
                      <div
                        className="absolute inset-x-0 bottom-0 rounded-md"
                        style={{
                          height: `${bar.height}%`,
                          minHeight: "4px",
                          backgroundImage: `linear-gradient(180deg, ${bar.color}, color-mix(in srgb, ${bar.color} 72%, #0b1524))`,
                        }}
                      />
                    )}
                    {/* A colocação vai DENTRO da coluna, no topo. Some quando o
                        calendário é longo: com vinte corridas o rótulo de 10px
                        não cabe na barra, e P1 sobre P12 sobre P9 vira borrão. */}
                    {grupo.bars.length <= 12 ? (
                      <span className="absolute inset-x-0 -top-0.5 text-center font-mono text-[10px] font-bold text-text-primary">
                        {bar.dnf ? "" : `P${bar.finish ?? "-"}`}
                      </span>
                    ) : null}
                  </div>
                  </Tooltip>
                ))}

                {/* A sobra da temporada em curso não é vazio: são as rodadas que
                    ainda vêm. Desenhar o trilho delas ocupa a largura que antes
                    ficava morta e diz que o ano continua. O fade existe porque a
                    ficha NÃO sabe quantas faltam — a faixa some antes de virar
                    uma contagem que ninguém prometeu. */}
                {grupo.current ? (
                  <div
                    aria-hidden="true"
                    className="pointer-events-none flex h-full min-w-0 flex-1 gap-1 overflow-hidden"
                    style={{ maskImage: FORM_GHOST_MASK, WebkitMaskImage: FORM_GHOST_MASK }}
                  >
                    {Array.from({ length: FORM_GHOST_SLOTS }, (_, slot) => (
                      <div
                        key={`vazio-${slot}`}
                        className="h-full shrink-0 rounded-md bg-white/[0.022]"
                        style={{ width: FORM_CELL }}
                      />
                    ))}
                  </div>
                ) : null}
              </div>

              <div className="flex gap-1">
                {grupo.bars.map((bar, barIndex) => (
                  <span
                    key={`round-${bar.key}`}
                    className="mt-1 min-w-[14px] max-w-[64px] grow basis-16 text-center font-mono text-[10px] text-text-muted"
                  >
                    {/* Um rótulo a cada N rodadas em calendário longo, pelo mesmo
                        motivo do rótulo de posição. */}
                    {bar.round && barIndex % Math.max(1, Math.ceil(grupo.bars.length / 8)) === 0
                      ? `R${bar.round}`
                      : ""}
                  </span>
                ))}
                {/* Espelha o bloco dos trilhos futuros para os rótulos ficarem
                    embaixo da coluna certa. */}
                {grupo.current ? <span aria-hidden="true" className="min-w-0 flex-1" /> : null}
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="mt-1.5 flex flex-wrap items-center gap-3 text-[10px] text-text-muted">
        <MedalKey color={MEDAL_COLORS.first} label={t("driverDetail.v2.medals.first")} />
        <MedalKey color={MEDAL_COLORS.second} label={t("driverDetail.v2.medals.second")} />
        <MedalKey color={MEDAL_COLORS.third} label={t("driverDetail.v2.medals.third")} />
        <MedalKey color={MEDAL_COLORS.nearMiss} label={t("driverDetail.v2.medals.rest")} />
        <MedalKey color={MEDAL_COLORS.dnf} label="DNF" />
      </div>
    </div>
  );
}
