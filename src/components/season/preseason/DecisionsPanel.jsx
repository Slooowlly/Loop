import { useTranslation } from "react-i18next";
import OfferCategoryRow from "./OfferCategoryRow";
import WeeklyClosingMovement from "./WeeklyClosingMovement";
import { CLASS_LABELS } from "../preSeasonFormatters.js";

// O que a coluna mostra enquanto o mercado não contrata: SÓ O NÚMERO.
//
// Nenhuma ficha, nenhuma categoria, nada clicável. Ver quais equipes têm vaga já na
// semana 1 entrega a janela inteira de graça — o jogador decidiria antes de o mercado
// existir. O que ele leva das semanas de abertura é o tamanho da oportunidade, não o
// mapa dela. As fichas voltam na semana em que dá pra assinar.
function OpeningWeekCount({ totalOffers, forecast, playerSignedThisWindow, t }) {
  // Contrato em dia: não vai ao mercado, e aí não há número nenhum a dar.
  if (playerSignedThisWindow && !forecast) {
    return (
      <div className="glass-light rounded-xl border-dashed p-6 text-center text-body text-[color:var(--text-secondary)]">
        {t("preSeason.forecast.notInMarket")}
      </div>
    );
  }
  const forecastLabel = forecast
    ? forecast.min === forecast.max
      ? t("preSeason.forecast.exact", { count: forecast.max })
      : t("preSeason.forecast.range", { min: forecast.min, max: forecast.max })
    : null;
  return (
    <div className="glass-light rounded-xl border-dashed p-6 text-center">
      <p className="text-title-lg font-black tabular-nums text-[color:var(--text-primary)]">
        {totalOffers}
      </p>
      <p className="mt-1 text-body-sm text-[color:var(--text-secondary)]">
        {t("preSeason.forecast.seatCount", { count: totalOffers })}
      </p>
      {forecastLabel && (
        <p className="mt-3 border-t border-white/[0.08] pt-3 text-body-sm text-[color:var(--text-secondary)]">
          {forecastLabel}
        </p>
      )}
    </div>
  );
}

// Coluna DIREITA: Decisões Pendentes.
// min-h-0 + overflow-y-auto (igual às outras 2 colunas): fica preso à altura
// da linha do grid e rola até o fim. NÃO usar self-start + max-h fixo, senão
// o painel cresce com o conteúdo e o fundo some sob o overflow-hidden do shell.
export default function DecisionsPanel({
  playerProposals,
  playerOffers,
  playerSignedThisWindow,
  playerBrand,
  isComplete,
  isOpeningWeek,
  interestForecast,
  totalOffers,
  promoOfferGroups,
  brandOfferGroups,
  otherOfferGroups,
  weeklyClosingGroups,
  openOffersFor,
  setTransferDetail,
}) {
  const { t } = useTranslation();
  return (
    <aside className="glass scroll-area animate-drawer-in min-h-0 overflow-y-auto rounded-2xl px-4 py-4 lg:px-5 lg:py-5">
      {/* Propostas formais: as fichas moram no modal de ofertas, que é a única tela
          onde o jogador aceita ou recusa. Aqui fica só a porta para lá — sem ela o
          v1 não teria como abrir o modal numa semana só de proposta. */}
      {playerProposals.length > 0 && (
        <button
          type="button"
          onClick={() => openOffersFor(null)}
          data-testid="proposals-open-modal"
          className="transition-glass mb-5 flex w-full items-center gap-2 rounded-xl border border-amber-400/40 bg-amber-400/10 px-3.5 py-3 text-left hover:bg-amber-400/[0.16]"
        >
          <span className="relative inline-flex h-2.5 w-2.5 shrink-0">
            <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-amber-400/80" />
            <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-amber-400" />
          </span>
          <span className="min-w-0 flex-1 truncate text-body-sm font-bold uppercase tracking-[0.16em] text-amber-300">
            {t("preSeason.offers.summaryProposals", { count: playerProposals.length })}
          </span>
          <span className="shrink-0 text-amber-300">›</span>
        </button>
      )}

      <div className="mb-4 flex h-6 items-center gap-2">
        <span className="relative inline-flex h-2.5 w-2.5">
          {playerOffers.length > 0 && !isOpeningWeek && (
            <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-[#58a6ff]/80" />
          )}
          <span
            className={`relative inline-flex h-2.5 w-2.5 rounded-full ${
              isOpeningWeek ? "bg-[color:var(--text-muted)]" : "bg-[color:var(--accent-primary)]"
            }`}
          />
        </span>
        <p
          className={`text-body-sm font-bold uppercase tracking-[0.22em] ${
            isOpeningWeek ? "text-[color:var(--text-secondary)]" : "text-[color:var(--accent-primary)]"
          }`}
        >
          {isOpeningWeek ? t("preSeason.forecast.title") : t("preSeason.offers.title")}
        </p>
      </div>

      {/* Semanas de abertura: só o número, antes de qualquer ramo de listagem — as vagas
          podem existir no banco, mas as fichas delas não são da conta do jogador ainda. */}
      {isOpeningWeek ? (
        <OpeningWeekCount
          totalOffers={totalOffers}
          forecast={interestForecast}
          playerSignedThisWindow={playerSignedThisWindow}
          t={t}
        />
      ) : playerOffers.length === 0 ? (
        <div className="glass-light rounded-xl border-dashed p-6 text-center text-body text-[color:var(--text-secondary)]">
          {playerSignedThisWindow
            ? t("preSeason.offers.emptySigned")
            : isComplete
              ? t("preSeason.offers.emptyClosed")
              : t("preSeason.offers.emptyNone")}
        </div>
      ) : (
        <div className="space-y-4">
          <p className="text-body-sm text-[color:var(--text-secondary)]">
            {t("preSeason.offers.summary", { count: totalOffers })}
          </p>

          {/* ANDAR 1 — Promoção: destaque. Card grande, verde, no topo. */}
          {promoOfferGroups.length > 0 && (
            <div className="space-y-2">
              {promoOfferGroups.map((group) => {
                const n = group.n1.length + group.n2.length;
                return (
                  <button
                    key={group.cat}
                    type="button"
                    onClick={() => openOffersFor(group.cat)}
                    data-testid={`offer-category-row-${group.cat}`}
                    className="transition-glass glow-green group flex w-full items-center gap-3 rounded-xl border border-[color:var(--status-green)]/45 bg-[color:var(--status-green)]/10 px-4 py-3.5 text-left hover:bg-[color:var(--status-green)]/16"
                  >
                    <span className="text-[20px] leading-none">⭐</span>
                    <span className="min-w-0 flex-1">
                      <span className="block text-[9px] font-black uppercase tracking-[0.22em] text-[color:var(--status-green)]">
                        {t("preSeason.offers.promotion")}
                      </span>
                      <span className="mt-0.5 block truncate text-title-md font-black">
                        {group.label}
                      </span>
                      <span className="block text-[10px] uppercase tracking-[0.16em] text-[color:var(--text-muted)]">
                        {t("preSeason.offers.promotionVacancies", { count: n })}
                      </span>
                    </span>
                    <span className="text-title-md text-[color:var(--status-green)] transition-transform group-hover:translate-x-0.5">›</span>
                  </button>
                );
              })}
            </div>
          )}

          {/* ANDAR 2 — Sua marca: agrupada, fileira densa. */}
          {brandOfferGroups.length > 0 && (
            <div className="space-y-1.5">
              <p className="px-0.5 text-[9px] font-black uppercase tracking-[0.22em] text-[color:var(--text-muted)]">
                {t("preSeason.offers.continueIn", { brand: CLASS_LABELS[playerBrand] ?? playerBrand })}
              </p>
              {brandOfferGroups.map((g) => (
                <OfferCategoryRow key={g.cat} group={g} onSelect={openOffersFor} />
              ))}
            </div>
          )}

          {/* ANDAR 3 — Outras oportunidades: fileira densa, sem chip. */}
          {otherOfferGroups.length > 0 && (
            <div className="space-y-1.5">
              <p className="px-0.5 text-[9px] font-black uppercase tracking-[0.22em] text-[color:var(--text-muted)]">
                {t("preSeason.offers.otherOpportunities")}
              </p>
              {otherOfferGroups.map((g) => (
                <OfferCategoryRow key={g.cat} group={g} onSelect={openOffersFor} />
              ))}
            </div>
          )}

          <button
            type="button"
            onClick={() => openOffersFor(null)}
            className="transition-glass glow-blue mt-1 w-full rounded-xl border border-[#58a6ff66] bg-[#58a6ff22] px-3 py-2.5 text-body font-bold text-[color:var(--accent-primary)] hover:bg-[#58a6ff44]"
          >
            {t("preSeason.offers.viewAll", { count: totalOffers })}
          </button>
        </div>
      )}

      <div
        data-testid="weekly-closing-market"
        className="mt-4 rounded-xl border border-white/[0.08] bg-black/[0.18] px-4 py-4"
      >
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[color:var(--text-muted)]">
          {t("preSeason.weeklyClosing.title")}
        </p>
        {weeklyClosingGroups.length ? (
          <div className="mt-3 space-y-3">
            {weeklyClosingGroups.map((group) => (
              <section key={group.category} className="space-y-2">
                <div
                  className="flex items-center justify-center rounded-lg border px-3 py-2"
                  style={{
                    borderColor: `${group.color}30`,
                    background: `linear-gradient(135deg, ${group.color}16 0%, rgba(255,255,255,0.025) 100%)`,
                  }}
                >
                  <p
                    className="text-center text-[11px] font-black uppercase tracking-[0.16em]"
                    style={{ color: group.color }}
                  >
                    {group.label}
                  </p>
                </div>
                <div className="space-y-2">
                  {group.events.map((event, index) => (
                    <WeeklyClosingMovement
                      key={`${event.event_type}-${event.driver_id ?? event.driver_name}-${index}`}
                      event={event}
                      color={group.color}
                      onSelect={setTransferDetail}
                    />
                  ))}
                </div>
              </section>
            ))}
          </div>
        ) : (
          <p className="mt-2 text-body text-[color:var(--text-secondary)]">
            {t("preSeason.weeklyClosing.empty")}
          </p>
        )}
      </div>
    </aside>
  );
}
