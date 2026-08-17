import { useTranslation } from "react-i18next";
import { ChevronDown, ChevronUp } from "lucide-react";
import TeamLogoMark from "../../team/TeamLogoMark";
import { DossieDoPiloto } from "../../driver/DriverMiniCard";
import {
  WEEKLY_MARKET_MOVEMENT_BADGES,
  RELATION_EMPHASIS,
  subcatColor,
  subcatLabel,
} from "../preSeasonFormatters.js";

// Seta de passo entre movimentos — o mesmo gesto da ficha do piloto, em escala
// menor porque o modal também é. Chevron vertical de propósito: o trilho segue
// a ordem de leitura da lista do fechamento, que é vertical.
function StepButton({ label, direction, target, onSelect }) {
  const Chevron = direction === "up" ? ChevronUp : ChevronDown;
  return (
    <button
      type="button"
      aria-label={label}
      disabled={!target}
      onClick={() => target && onSelect(target)}
      data-testid={`transfer-detail-step-${direction}`}
      className={`grid h-12 w-12 place-items-center rounded-2xl border backdrop-blur-sm transition-glass ${
        target
          ? "border-white/15 bg-[#0d1727]/90 text-text-secondary hover:border-white/30 hover:bg-[#14233a] hover:text-text-primary"
          : "cursor-not-allowed border-white/[0.06] bg-[#0b111a]/70 text-[#4a525d]"
      }`}
    >
      <Chevron size={20} strokeWidth={1.6} aria-hidden="true" />
    </button>
  );
}

// Modal: Detalhe da transferência (clique num movimento do fechamento semanal).
// `events` é o trilho das setas: a lista achatada do fechamento, na ordem da
// tela. Sem ela (ou com um movimento só) o modal abre como sempre, sem setas.
export default function TransferDetailModal({ event: ev, events = [], onSelect, onClose }) {
  const { t } = useTranslation();
  // Por referência mesmo: o `ev` veio de um item desta lista, e os grupos são
  // memoizados — enquanto o modal está aberto ninguém recria os objetos.
  const stepIndex = events.indexOf(ev);
  const previousEvent = stepIndex > 0 ? events[stepIndex - 1] : null;
  const nextEvent =
    stepIndex >= 0 && stepIndex < events.length - 1 ? events[stepIndex + 1] : null;
  const showRail = Boolean(onSelect) && events.length > 1 && stepIndex >= 0;
  const badge = WEEKLY_MARKET_MOVEMENT_BADGES[ev.movement_kind];
  const emphasis = RELATION_EMPHASIS[ev.relation];
  const isDebut = !ev.from_team;
  const fromTeam = ev.from_team;
  const toTeam = ev.to_team || ev.team_name;
  const isRenewal = ev.movement_kind === "renewal" || (fromTeam && fromTeam === toTeam);
  const accent = badge?.color ?? subcatColor(ev.categoria);
  const tenure = ev.seasons_at_previous;
  const fromCatLabel = ev.from_categoria ? subcatLabel(ev.from_categoria) : null;
  const toCatLabel = ev.categoria ? subcatLabel(ev.categoria) : null;
  const sameCat = fromCatLabel && fromCatLabel === toCatLabel;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      {/* O wrapper dá às setas uma âncora com a largura do card: a calha fica
          sempre no mesmo lugar à direita, independente da altura do conteúdo. */}
      <div className="relative mx-4 w-full max-w-lg">
        {showRail && (
          <div
            data-testid="transfer-detail-step-rail"
            className="absolute left-full top-1/2 ml-3 flex -translate-y-1/2 flex-col gap-2"
          >
            <StepButton
              label={t("driverDetail.navigator.previous")}
              direction="up"
              target={previousEvent}
              onSelect={onSelect}
            />
            <StepButton
              label={t("driverDetail.navigator.next")}
              direction="down"
              target={nextEvent}
              onSelect={onSelect}
            />
          </div>
        )}

      <div className="glass-strong animate-fade-in relative max-h-[85vh] w-full overflow-y-auto rounded-2xl p-6 md:p-7">
        <button
          onClick={onClose}
          aria-label={t("preSeason.actions.close")}
          className="absolute right-4 top-4 flex h-8 w-8 items-center justify-center rounded-lg border border-white/[0.12] bg-white/5 text-[color:var(--text-muted)] transition-glass hover:bg-white/10 hover:text-[color:var(--text-primary)]"
        >
          ✕
        </button>

        <div className="mb-1 flex flex-wrap items-center gap-1.5">
          {badge && (
            <div
              className="inline-flex items-center gap-1.5 rounded-md border px-2 py-0.5 text-[11px] font-black uppercase tracking-[0.16em]"
              style={{ color: badge.color, background: badge.bg, borderColor: badge.border }}
            >
              <span className="text-[13px] leading-none">{badge.symbol}</span>
              {t(`preSeason.movementBadge.${ev.movement_kind}`)}
            </div>
          )}
          {emphasis && (
            <div
              className="inline-flex items-center gap-1.5 rounded-md border px-2 py-0.5 text-[11px] font-black uppercase tracking-[0.16em]"
              style={{ color: emphasis.color, background: emphasis.bg, borderColor: emphasis.border }}
            >
              <span className="text-[13px] leading-none">{emphasis.symbol}</span>
              {t(`preSeason.relation.${ev.relation}`)}
            </div>
          )}
        </div>
        <h2 className="mb-5 text-[20px] font-bold leading-tight text-[color:var(--text-primary)]">
          {ev.driver_name}
        </h2>

        {/* Renovação: permaneceu na mesma equipe (sem De → Para) */}
        {isRenewal ? (
          <div className="mb-5 flex flex-col items-center gap-2 rounded-xl border border-white/10 bg-black/25 px-4 py-5 text-center">
            <div className="text-[10px] font-bold uppercase tracking-[0.16em] text-[color:var(--text-muted)]">
              {t("preSeason.transferDetail.stayed")}
            </div>
            <TeamLogoMark teamName={toTeam} color={accent} size="md" />
            <span className="block truncate text-[15px] font-bold text-[color:var(--text-primary)]">
              {toTeam}
            </span>
          </div>
        ) : (
        /* De → Para (equipes) */
        <div className="mb-5 flex items-center justify-between gap-3 rounded-xl border border-white/10 bg-black/25 px-4 py-4">
          {/* Sem rótulo "De"/"Para": a seta e a ordem de leitura já dizem o
              sentido, e as duas palavras só roubavam altura do bloco. */}
          <div className="flex min-w-0 flex-1 flex-col items-center gap-2 text-center">
            {isDebut ? (
              <span className="text-[14px] font-semibold text-[color:var(--text-secondary)]">
                {t("preSeason.transferDetail.careerDebut")}
              </span>
            ) : (
              <>
                <TeamLogoMark teamName={fromTeam} color={accent} size="md" />
                <span className="block truncate text-[14px] font-bold text-[color:var(--text-primary)]">
                  {fromTeam}
                </span>
              </>
            )}
          </div>

          <span className="shrink-0 text-[22px] font-black" style={{ color: accent }}>
            →
          </span>

          <div className="flex min-w-0 flex-1 flex-col items-center gap-2 text-center">
            {toTeam ? (
              <>
                <TeamLogoMark teamName={toTeam} color={accent} size="md" />
                <span className="block truncate text-[14px] font-bold text-[color:var(--text-primary)]">
                  {toTeam}
                </span>
              </>
            ) : (
              <span className="text-[14px] font-semibold text-[color:var(--text-secondary)]">
                {t("preSeason.transferDetail.noTeam")}
              </span>
            )}
          </div>
        </div>
        )}

        {/* Categoria */}
        {toCatLabel && (
          <div className="mb-3 flex items-center justify-center gap-2 text-[13px]">
            {fromCatLabel && !sameCat ? (
              <>
                <span className="font-semibold text-[color:var(--text-secondary)]">{fromCatLabel}</span>
                <span className="font-black" style={{ color: accent }}>→</span>
                <span className="font-bold text-[color:var(--text-primary)]">{toCatLabel}</span>
              </>
            ) : (
              <span className="font-bold text-[color:var(--text-primary)]">{toCatLabel}</span>
            )}
          </div>
        )}

        {/* Tempo de casa */}
        <p className="text-center text-body-sm text-[color:var(--text-muted)]">
          {isDebut
            ? t("preSeason.transferDetail.careerDebut")
            : tenure != null && tenure > 0
              ? isRenewal
                ? t("preSeason.transferDetail.tenureCurrent", { count: tenure })
                : t("preSeason.transferDetail.tenurePrevious", { count: tenure })
              : isRenewal
                ? t("preSeason.transferDetail.renewed")
                : t("preSeason.transferDetail.previousTeam")}
        </p>

        {/* O dossiê do piloto — o mesmo corpo da ficha rápida do mercado, sem o
            nome (já está no título do modal). Só quando o evento sabe o id:
            movimento antigo serializado sem driver_id continua abrindo o modal
            como antes. */}
        {ev.driver_id && (
          <div className="mt-5 rounded-xl border border-white/10 bg-black/25 pb-1">
            <DossieDoPiloto driverId={ev.driver_id} ocultarNome />
          </div>
        )}
      </div>
      </div>
    </div>
  );
}
