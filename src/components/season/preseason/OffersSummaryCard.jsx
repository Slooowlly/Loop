import { useTranslation } from "react-i18next";
import { subcatColor } from "../preSeasonFormatters.js";

// Bloco ÚNICO das ofertas: o número grande à esquerda e um trilho colorido por
// categoria à direita. Substitui os três andares de fichas (promoção / marca do
// jogador / demais marcas) mais o botão "Ver ofertas" — quatro caminhos para o
// mesmo modal, empilhados numa coluna estreita.
//
// O número soma ofertas E propostas formais, porque é isso que o jogador tem
// para decidir. Ele NÃO decide nada aqui: este bloco só conta e leva ao modal,
// que é a única tela onde aceitar, recusar e assinar existem.
//
// A cor de cada trilho vem de subcatColor, a mesma fonte que pinta a categoria
// no resto da interface. O clique abre o modal SEM filtro: a separação por
// categoria já é o corpo dele, e é lá que ela cabe.

// Quantos trilhos o resumo desenha antes de o excedente virar uma linha só. O
// bloco é um resumo — passando disso ele vira a própria lista que saiu daqui.
const MAX_TRILHOS = 5;

export default function OffersSummaryCard({ groups, totalOffers, proposals = [], onSelect }) {
  const { t } = useTranslation();

  const visiveis = groups.slice(0, MAX_TRILHOS);
  const excedente = groups.length - visiveis.length;
  const total = totalOffers + proposals.length;
  // Proposta na última semana some se o jogador não abrir a tela: a urgência
  // precisa aparecer no bloco, mesmo com o card morando dentro do modal.
  const propostasNoPrazoFinal = proposals.filter(
    (p) => p.semanas_restantes != null && p.semanas_restantes <= 0,
  ).length;
  // A categoria da proposta conta junto: os trilhos só desenham as das ofertas,
  // e o rodapé diria "1 categoria" com um time de outra te chamando.
  const categoriasDistintas = new Set([
    ...groups.map((g) => g.cat),
    ...proposals.map((p) => p.categoria).filter(Boolean),
  ]).size;
  // Vagas de promoção somadas (bucket 0 = categoria acima da do jogador). Conta
  // VAGAS, não categorias: "1 delas" não diria se é uma vaga ou cinco.
  const vagasDePromocao = groups
    .filter((g) => g.bucket === 0)
    .reduce((soma, g) => soma + g.n1.length + g.n2.length, 0);

  return (
    <button
      type="button"
      onClick={() => onSelect?.(null)}
      data-testid="offers-summary-card"
      className="transition-glass glow-blue mb-2.5 flex w-full items-center gap-3.5 rounded-xl border border-white/[0.08] bg-black/15 p-3 text-left hover:bg-black/30"
    >
      <span className="shrink-0">
        <span className="mb-1 flex items-center gap-1.5">
          <span className="h-1.5 w-1.5 rounded-full bg-[color:var(--accent-primary)]" />
          <span className="text-[9px] font-black uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
            {t("preSeason.offers.title")}
          </span>
        </span>
        {/* Proposta na última semana some sem aviso se o jogador avançar a semana:
            aí o número pulsa e fica âmbar, a mesma cor do trilho que explica. */}
        <span
          className={`block text-[38px] font-black leading-none tabular-nums ${
            propostasNoPrazoFinal > 0 ? "animate-pulse text-amber-300" : ""
          }`}
        >
          {total}
        </span>
        <span className="mt-1 block text-[9px] font-bold uppercase tracking-[0.14em] text-[color:var(--text-muted)]">
          {t("preSeason.offers.summaryCategories", { count: categoriasDistintas })}
        </span>
      </span>

      <span className="flex min-w-0 flex-1 flex-col gap-1.5">
        {proposals.length > 0 && (
          <span className="flex items-center gap-2">
            <span className="h-4 w-[3px] shrink-0 rounded-sm bg-amber-400" />
            <span className="min-w-0 flex-1 truncate text-[10px] font-bold uppercase tracking-[0.1em] text-amber-300">
              {propostasNoPrazoFinal > 0
                ? t("preSeason.offers.summaryProposalsLastWeek", {
                    count: proposals.length,
                    urgentes: propostasNoPrazoFinal,
                  })
                : t("preSeason.offers.summaryProposals", { count: proposals.length })}
            </span>
          </span>
        )}

        {visiveis.map((g) => {
          const n = g.n1.length + g.n2.length;
          return (
            <span key={g.cat} className="flex items-center gap-2">
              <span
                className="h-4 w-[3px] shrink-0 rounded-sm"
                style={{ background: subcatColor(g.cat) }}
              />
              <span className="min-w-0 flex-1 truncate text-[11px] font-bold uppercase tracking-[0.1em]">
                {g.label}
              </span>
              <span className="shrink-0 text-[10px] font-semibold tabular-nums text-[color:var(--text-muted)]">
                {n}
              </span>
            </span>
          );
        })}

        {excedente > 0 && (
          <span className="flex items-center gap-2">
            <span className="h-4 w-[3px] shrink-0 rounded-sm bg-white/20" />
            <span className="truncate text-[10px] font-bold uppercase tracking-[0.1em] text-[color:var(--text-muted)]">
              {t("preSeason.offers.summaryMore", { count: excedente })}
            </span>
          </span>
        )}

        {vagasDePromocao > 0 && (
          <span className="flex items-center gap-2">
            <span className="h-4 w-[3px] shrink-0 rounded-sm bg-[color:var(--status-green)]" />
            <span className="truncate text-[10px] font-bold uppercase tracking-[0.1em] text-[color:var(--status-green)]">
              {t("preSeason.offers.promotionSeats", { count: vagasDePromocao })}
            </span>
          </span>
        )}
      </span>

      <span className="shrink-0 text-[18px] leading-none text-[color:var(--text-muted)]">›</span>
    </button>
  );
}
