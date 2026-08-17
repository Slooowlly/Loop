import { useTranslation } from "react-i18next";
import OfferCardRich from "./OfferCardRich";
import ProposalCard from "./ProposalCard";
import { subcatColor } from "../preSeasonFormatters.js";

// Modal "Suas ofertas": fichas completas das equipes, agrupadas por categoria
// (e por função N1/N2 dentro de cada uma).
//
// As propostas formais entram AQUI, no topo, e não na coluna da janela: toda
// escolha do jogador (aceitar, recusar, assinar) acontece nesta tela e em
// nenhuma outra. A separação continua valendo — proposta é a equipe indo atrás
// dele, com prazo; oferta é assento aberto que ele pode buscar — mas as duas
// decisões moram no mesmo lugar.
export default function OffersModal({
  offersByCategory,
  offersModalCat,
  totalOffers,
  playerTier,
  playerProposals = [],
  isAdvancingWeek,
  onClose,
  onClearCat,
  onRespondProposal,
  onViewContract,
}) {
  const { t } = useTranslation();
  const modalGroups = offersModalCat
    ? offersByCategory.filter((g) => g.cat === offersModalCat)
    : offersByCategory;
  // Com filtro de categoria ligado, as propostas seguem o mesmo recorte.
  const modalProposals = offersModalCat
    ? playerProposals.filter((p) => p.categoria === offersModalCat)
    : playerProposals;
  const modalCount =
    modalGroups.reduce((sum, g) => sum + g.n1.length + g.n2.length, 0) + modalProposals.length;
  const modalCatLabel = offersModalCat ? modalGroups[0]?.label : null;
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div className="glass-strong animate-fade-in flex max-h-[90vh] w-full max-w-5xl flex-col rounded-2xl">
        <div className="flex items-start justify-between gap-4 border-b border-white/[0.08] px-6 py-5">
          <div>
            <div className="text-body-sm font-bold uppercase tracking-[0.22em] text-[color:var(--accent-primary)]">
              {modalCatLabel ? t("preSeason.offersModal.eyebrowCat", { label: modalCatLabel }) : t("preSeason.offersModal.eyebrow")}
            </div>
            <h2 className="mt-1 text-[18px] font-bold leading-tight text-[color:var(--text-primary)]">
              {t("preSeason.offersModal.countTitle", { count: modalCount })}
            </h2>
            <p className="mt-1 text-body-sm text-[color:var(--text-secondary)]">
              {t("preSeason.offersModal.subtitle")}
            </p>
            {offersModalCat && offersByCategory.length > 1 && (
              <button
                type="button"
                onClick={onClearCat}
                className="mt-2 text-body-sm font-semibold text-[color:var(--accent-primary)] hover:underline"
              >
                {t("preSeason.offersModal.viewAll", { count: totalOffers })}
              </button>
            )}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="transition-glass glass-light shrink-0 rounded-lg px-3 py-2 text-body font-bold text-[color:var(--text-secondary)] hover:text-[color:var(--text-primary)]"
            aria-label={t("preSeason.actions.close")}
          >
            ✕
          </button>
        </div>

        <div className="scroll-area space-y-5 overflow-y-auto px-6 py-5">
          {modalProposals.length > 0 && (
            <section className="space-y-3">
              <div className="flex items-center gap-3">
                <span className="relative inline-flex h-2.5 w-2.5">
                  <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-amber-400/80" />
                  <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-amber-400" />
                </span>
                <span className="text-body font-black uppercase tracking-[0.16em] text-amber-300">
                  {t("preSeason.proposals.title")}
                </span>
                <div className="h-px flex-1 bg-gradient-to-r from-amber-400/35 to-transparent" />
                <span className="text-body-sm text-[color:var(--text-muted)]">
                  {t("preSeason.offers.vacancies", { count: modalProposals.length })}
                </span>
              </div>
              <p className="text-body-sm text-[color:var(--text-secondary)]">
                {t("preSeason.proposals.subtitle")}
              </p>
              <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
                {modalProposals.map((p) => (
                  <ProposalCard
                    key={p.proposal_id}
                    proposal={p}
                    isAdvancingWeek={isAdvancingWeek}
                    onRespond={onRespondProposal}
                  />
                ))}
              </div>
            </section>
          )}

          {modalGroups.map((group) => {
            const n = group.n1.length + group.n2.length;
            const isPromotion = playerTier != null && group.tier > playerTier;
            const accent = subcatColor(group.cat);
            return (
              <section key={group.cat} className="space-y-3">
                <div className="flex items-center gap-3">
                  <span
                    className="text-body font-black uppercase tracking-[0.16em]"
                    style={{ color: accent }}
                  >
                    {group.label}
                  </span>
                  {isPromotion && (
                    <span className="rounded-md bg-[rgba(63,185,80,0.14)] px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.14em] text-[color:var(--status-green)]">
                      ↑ {t("preSeason.offersModal.promotion")}
                    </span>
                  )}
                  <div className="h-px flex-1" style={{ background: `linear-gradient(to right, ${accent}55, transparent)` }} />
                  <span className="text-body-sm text-[color:var(--text-muted)]">
                    {t("preSeason.offers.vacancies", { count: n })}
                  </span>
                </div>
                {[["n1", group.n1], ["n2", group.n2]].map(([roleKey, list]) =>
                  list.length === 0 ? null : (
                    <div key={roleKey} className="space-y-3">
                      <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
                        {t(`preSeason.offersModal.role.${roleKey}`)}
                      </p>
                      <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
                        {list.map((o) => (
                          <OfferCardRich
                            key={o.seat_id}
                            offer={o}
                            isAdvancingWeek={isAdvancingWeek}
                            onViewContract={onViewContract}
                          />
                        ))}
                      </div>
                    </div>
                  ),
                )}
              </section>
            );
          })}
        </div>
      </div>
    </div>
  );
}
