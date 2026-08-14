import { useTranslation } from "react-i18next";

import { formatAudience } from "./raceEventContext";

// F-07 — a exibição rica do interesse ESPERADO do evento.
//
// O `event_interest/` modula economia, narrativa e motivação, e até aqui o jogador
// via dele um número solto no canto do card de clima: "Público 62.000", sem tier,
// sem escala e sem dizer o que aquilo tem a ver com ele. Sentia-se o efeito (o
// patrocínio rendeu mais, a notícia veio mais forte) sem nunca ver a causa.
//
// O card responde três perguntas, na ordem em que elas importam:
//
//   1. Que tamanho tem esta etapa? — `tier_label`, já traduzido pelo backend, com o
//      público como número grande.
//   2. Onde ela cai no calendário? — o rótulo de porte da ocasião.
//   3. O que isso tem a ver comigo? — a fração do público que a equipe do jogador
//      puxa (`public_fame_share`), que é o fio entre a fama dele e a bilheteria.
//
// TODO número aqui vem do backend. A única exceção é herdada e está anotada como
// dívida no próprio `raceEventContext.js`: `estimateAudience` inventa o público
// quando o save é antigo e não tem `display_value`. Não amplie o padrão.
//
// A barra do público existe porque um número absoluto não se julga sozinho: 41.000
// é muito ou pouco? Ela mede contra o topo da escala de tiers (`EventoPrincipal`),
// que é o mesmo teto que o backend usa para classificar — não é uma régua nova.
const TETO_DE_PUBLICO = 90000;

function EventInterestCard({ interestLabel, audienceEstimate, audienceRankLabel, fameSharePct }) {
  const { t } = useTranslation();
  // Sem público não há evento a descrever: save antigo sem `event_interest` cai aqui
  // e o card simplesmente não existe, em vez de desenhar uma barra em zero que se
  // leria como "ninguém vai".
  if (!audienceEstimate) return null;

  const fracao = Math.max(0, Math.min(100, (audienceEstimate / TETO_DE_PUBLICO) * 100));

  return (
    <div
      data-testid="event-interest-card"
      className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-5 bg-gradient-to-r from-black/40 to-transparent"
    >
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <p className="text-[10px] uppercase tracking-widest text-[#f5c76d] font-bold">
            {t("nextRaceTab.eventInterest.label")}
          </p>
          <p className="mt-1 text-xl font-bold text-white truncate">{interestLabel}</p>
          {audienceRankLabel ? (
            <p className="mt-0.5 text-[10px] text-gray-500">{audienceRankLabel}</p>
          ) : null}
        </div>
        <div className="text-right shrink-0">
          <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">
            {t("nextRaceTab.labels.audience")}
          </p>
          <p
            className="text-xl font-bold text-white"
            style={{ fontVariantNumeric: "tabular-nums" }}
          >
            {formatAudience(audienceEstimate)}
          </p>
        </div>
      </div>

      <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-white/[0.08]">
        <div
          className="h-full rounded-full bg-[#f5c76d]"
          style={{ width: `${fracao}%` }}
          data-testid="event-interest-bar"
        />
      </div>

      {/* A cota de público só aparece quando existe: ela depende de o jogador ter
          equipe, e um "0% do público" no card diria que a estrela dele não puxa
          ninguém quando o dado é que não há equipe para puxar. */}
      {fameSharePct ? (
        <p className="mt-3 text-[11px] leading-relaxed text-gray-400" data-testid="event-interest-share">
          {t("nextRaceTab.eventInterest.fameShare", { percent: fameSharePct })}
        </p>
      ) : null}
    </div>
  );
}

export default EventInterestCard;
