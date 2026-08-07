import { useTranslation } from "react-i18next";
import Tooltip from "../../../ui/Tooltip";
import { CATEGORIES } from "../../preSeasonFormatters.js";

// Corredor que a gaveta de controles de janela (WindowControlsDrawer) reserva no
// canto: `right-5` + envoltório de 148px, com z-50 e fundo transparente. Tudo que
// cai ali para de receber mouse — foi assim que "Ver Quem Sai" morreu quando o
// layout foi para a largura cheia. Só o chevron de 40px é desenhado, então um
// painel SEM interação pode encostar nele; o que não pode é ser clicável.
const DRAWER_CHEVRON_CLEARANCE = "pr-[32px]";

// Descida dos DOIS blocos laterais — título e trilho descem juntos, senão um
// fica pendurado no topo enquanto o outro se acomoda. Resolve duas coisas:
//
// 1. O LayoutSwitch é `fixed right-3 top-2` e termina por volta dos 29px do topo
//    da janela. O conteúdo do header começa aos 22px (`pt-3` do container +
//    `py-2.5` do header), então a data ficava POR BAIXO da pastilha "Layout Novo".
// 2. O cartão do header não acaba na linha da ação — ele segue até a barra de
//    filtros (~42px a mais). Centrados só na linha, os blocos ficam altos demais
//    para o cartão inteiro; descer metade da barra os centra de verdade.
//
// `translate` e não `margin` de propósito: transform não entra no cálculo do
// grid, então descer os blocos não estica a linha nem move o botão do centro. Os
// 20px invadem a faixa da barra de filtros, mas as pastilhas são centradas e
// estreitas — as duas pontas dessa faixa estão vazias.
const SIDE_BLOCK_DROP = "translate-y-[20px]";

// Mesma regra do v1: as duas semanas de abertura têm nome próprio.
function openingStageKey(currentWeek, signingsStartWeek) {
  if (currentWeek >= signingsStartWeek) return null;
  return currentWeek <= 1 ? "snapshot" : "departures";
}

// Trilho de semanas: substitui a barra de progresso lisa. A diferença que ele
// carrega é o PORTÃO — as semanas de fotografia aparecem hachuradas, e a semana
// em que as contratações abrem deixa de ser uma regra invisível.
//
// Sem tooltip por semana, de propósito: o rótulo já imprime "S3" e a legenda
// abaixo já diz onde o portão está — o tooltip repetia os dois. Sem hover, o
// trilho pode encostar no corredor da gaveta sem que nada deixe de funcionar.
function WeekRail({ currentWeek, totalWeeks, signingsStartWeek }) {
  const { t } = useTranslation();
  const weeks = Array.from({ length: totalWeeks }, (_, i) => i + 1);
  return (
    <div className="flex items-end gap-[3px]" data-testid="preseason-week-rail">
      {weeks.map((week) => {
        const isGate = week < signingsStartWeek;
        const isNow = week === currentWeek;
        const isDone = week < currentWeek;
        const background = isNow
          ? "var(--accent-primary)"
          : isGate
            ? "repeating-linear-gradient(90deg, rgba(210,153,34,0.55) 0 4px, transparent 4px 8px)"
            : isDone
              ? "rgba(88,166,255,0.45)"
              : "rgba(255,255,255,0.10)";
        return (
          <span key={week} className="flex w-[22px] flex-col items-center gap-1">
            <span
              className="h-[5px] w-full rounded-sm"
              style={{ background, boxShadow: isNow ? "0 0 12px rgba(88,166,255,0.85)" : undefined }}
            />
            <span
              className="text-[8.5px] font-bold tracking-[0.06em]"
              style={{
                color: isNow
                  ? "var(--accent-primary)"
                  : isGate
                    ? "rgba(210,153,34,0.75)"
                    : isDone
                      ? "var(--text-secondary)"
                      : "rgba(255,255,255,0.28)",
              }}
            >
              {t("preSeason.v2.header.weekShort", { week })}
            </span>
          </span>
        );
      })}
    </div>
  );
}

export default function PreSeasonHeaderV2({
  isComplete,
  isMarketOpen,
  playerOffers,
  playerProposals,
  selectedCat,
  setSelectedCat,
  currentWeek,
  totalWeeks,
  signingsStartWeek,
  currentDateLabel,
  isAdvancingWeek,
  handleAdvanceWeek,
  startError,
  categoryCounters,
}) {
  const { t } = useTranslation();
  const stageKey = openingStageKey(currentWeek, signingsStartWeek);

  return (
    <header className="glass-strong animate-fade-in mb-2 rounded-2xl px-4 py-2.5 lg:px-5">
      {/* Três colunas com o meio fixo (`1fr auto 1fr`): é o que centra o botão na
          TELA, e não no espaço que sobra entre os vizinhos. As duas laterais
          precisam medir IGUAL — qualquer recuo na linha inteira desloca o centro
          junto. Por isso o corredor da gaveta é reservado dentro da coluna da
          direita (padding não muda a largura da trilha), e não na linha. */}
      <div
        data-testid="preseason-header-row"
        className="grid grid-cols-[1fr_auto_1fr] items-center gap-5"
      >

        {/* Título + estado da janela */}
        <div className={`min-w-0 ${SIDE_BLOCK_DROP}`}>
          <div className="flex items-center gap-2">
            <span className="rounded-full border border-[#58a6ff4d] bg-[#58a6ff24] px-2.5 py-0.5 text-[9px] font-black uppercase tracking-[0.22em] text-[color:var(--accent-primary)]">
              {t("preSeason.header.eyebrow")}
            </span>
            <span
              className={`rounded-full border px-2.5 py-0.5 text-[9px] font-black uppercase tracking-[0.18em] ${
                isMarketOpen
                  ? "border-[#3fb95055] bg-[#3fb9501a] text-[color:var(--status-green)]"
                  : "border-[#d2992255] bg-[#d2992218] text-[color:var(--status-yellow)]"
              }`}
            >
              {isMarketOpen ? t("preSeason.header.marketOpen") : t("preSeason.header.marketClosed")}
            </span>
            {playerOffers.length > 0 && !stageKey && (
              <span className="glass-light rounded-full px-2.5 py-0.5 text-[10px] font-bold tracking-[0.1em] text-[color:var(--accent-primary)]">
                {t("preSeason.header.offerCount", { count: playerOffers.length })}
              </span>
            )}
          </div>
          <h1 className="mt-1 truncate text-[22px] font-bold leading-[1.08] tracking-[-0.02em]">
            {isComplete
              ? t("preSeason.header.titleClosed")
              : stageKey
                ? t(`preSeason.stage.${stageKey}.title`)
                : t("preSeason.header.titleOpen")}
          </h1>
          {stageKey && !isComplete && (
            <p className="mt-0.5 max-w-[520px] text-[11px] leading-[1.35] text-[color:var(--text-secondary)]">
              {t(`preSeason.stage.${stageKey}.subtitle`)}
            </p>
          )}
        </div>

        {/* A ÚNICA ação da tela, no centro e no tamanho que ela merece. Estava
            espremida no canto disputando espaço com um painel que ninguém clica. */}
        <button
          onClick={handleAdvanceWeek}
          disabled={isAdvancingWeek || (isComplete && playerProposals.length > 0)}
          data-testid="preseason-advance-week"
          className={`transition-glass justify-self-center rounded-2xl border px-12 py-4 text-[16px] font-black uppercase tracking-[0.18em] disabled:cursor-not-allowed disabled:opacity-50 ${
            isComplete
              ? "border-[#3fb95099] bg-[#3fb950] text-[#06101f] hover:bg-[#52d16a]"
              : "glow-blue border-[#58a6ff99] bg-[#58a6ff] text-[#06101f] hover:bg-[#79b8ff]"
          }`}
        >
          {isAdvancingWeek
            ? t("preSeason.actions.processing")
            : isComplete
              ? t("preSeason.actions.startSeason")
              : stageKey
                ? t(`preSeason.stage.${stageKey}.action`)
                : t("preSeason.actions.advanceWeek")}
        </button>

        {/* Trilho de semanas: painel de leitura, sem nada para clicar. Encosta no
            canto — informa onde a janela está sem pedir a atenção que a ação
            precisa. Só o chevron da gaveta fica reservado. */}
        <div
          data-testid="preseason-week-panel"
          className={`flex flex-col items-end gap-1 ${DRAWER_CHEVRON_CLEARANCE} ${SIDE_BLOCK_DROP}`}
        >
          <div className="flex items-baseline gap-3">
            <span className="text-[8.5px] font-black uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
              {t("preSeason.v2.header.windowLabel")}
            </span>
            <span className="text-[10.5px] font-bold tabular-nums text-[color:var(--text-secondary)]">
              {currentDateLabel}
            </span>
          </div>
          <WeekRail
            currentWeek={currentWeek}
            totalWeeks={totalWeeks}
            signingsStartWeek={signingsStartWeek}
          />
          <div className="flex items-center gap-2">
            {signingsStartWeek > 1 && (
              <span className="text-[8.5px] font-bold uppercase tracking-[0.1em] text-[rgba(210,153,34,0.85)]">
                {t("preSeason.v2.header.gateLegend", { until: signingsStartWeek - 1 })}
              </span>
            )}
            <span className="text-[8.5px] font-bold uppercase tracking-[0.1em] text-[color:var(--text-muted)]">
              {currentWeek >= signingsStartWeek
                ? t("preSeason.v2.header.signingsOpenNow")
                : t("preSeason.v2.header.signingsOpenAt", { week: signingsStartWeek })}
            </span>
          </div>
        </div>
      </div>

      {/* Filtros de categoria.
          O contador só ganha cor quando há assento VAZIO — esse é o número que muda
          uma decisão. O "pode abrir" fica cinza: na semana da fotografia ele existe
          em todas as categorias ao mesmo tempo, e nove números âmbar lado a lado não
          apontam para lugar nenhum. Largura natural, não esticada: um chip do tamanho
          de um botão de ação passa a competir com o botão de ação. */}
      <div
        data-testid="preseason-filter-bar"
        className="mt-2 flex max-w-full justify-center overflow-x-auto"
      >
        <div className="glass inline-flex w-fit items-center gap-0.5 whitespace-nowrap rounded-full p-1">
          {CATEGORIES.map((cat, i) => {
            if (cat.isSeparator) return <span key={i} className="mx-1 h-4 w-px bg-white/10" />;
            const active = selectedCat === cat.id;
            const counter = categoryCounters?.[cat.id];
            const isOpenKind = counter?.kind === "open";
            // Só vaga REAL vira número na barra. O "pode abrir" existe em todas as
            // categorias ao mesmo tempo na semana da fotografia: impresso, virava
            // uma fileira de "24? 12? 20? 16?" que não aponta para lugar nenhum.
            // Ele continua acessível — no tooltip, quando o jogador perguntar.
            const hasCounter = counter && counter.count > 0;
            const showsNumber = hasCounter && isOpenKind;
            const chip = (
              <button
                onClick={() => setSelectedCat(cat.id)}
                data-testid={`preseason-filter-${cat.id}`}
                className={`transition-glass flex cursor-pointer items-center gap-2 rounded-full border px-3 py-1 text-[12px] font-semibold ${
                  active
                    ? "border-white/30 bg-white/14 text-[color:var(--accent-primary)]"
                    : "border-transparent bg-white/3 text-[color:var(--text-secondary)] hover:bg-white/8 hover:text-[color:var(--text-primary)]"
                }`}
              >
                <span
                  className="inline-block h-1.5 w-1.5 shrink-0 rounded-full"
                  style={{ backgroundColor: cat.color }}
                />
                {cat.id === "all" ? t("preSeason.filters.all") : cat.label}
                {showsNumber && (
                  <span className="font-black tabular-nums" style={{ color: "var(--status-green)" }}>
                    {counter.count}
                  </span>
                )}
              </button>
            );
            return hasCounter ? (
              <Tooltip
                key={cat.id}
                texto={
                  isOpenKind
                    ? t("preSeason.v2.filters.openTooltip", { count: counter.count })
                    : t("preSeason.v2.filters.riskTooltip", { count: counter.count })
                }
              >
                {chip}
              </Tooltip>
            ) : (
              <span key={cat.id}>{chip}</span>
            );
          })}
        </div>
      </div>

      {startError && (
        <p className="mt-2 text-center text-body-sm text-[color:var(--status-red)]">{startError}</p>
      )}
    </header>
  );
}
