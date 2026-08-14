import { useTranslation } from "react-i18next";
import TeamLogoMark from "../../team/TeamLogoMark";
import {
  WEEKLY_MARKET_MOVEMENT_BADGES,
  RELATION_EMPHASIS,
  subcatColor,
  subcatLabel,
} from "../preSeasonFormatters.js";

// Modal: Detalhe da transferência (clique num movimento do fechamento semanal).
export default function TransferDetailModal({ event: ev, onClose }) {
  const { t } = useTranslation();
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
      <div className="glass-strong animate-fade-in relative mx-4 w-full max-w-lg rounded-2xl p-6 md:p-7">
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
          <div className="flex min-w-0 flex-1 flex-col items-center gap-2 text-center">
            <div className="text-[10px] font-bold uppercase tracking-[0.16em] text-[color:var(--text-muted)]">
              {t("preSeason.transferDetail.from")}
            </div>
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
            <div className="text-[10px] font-bold uppercase tracking-[0.16em] text-[color:var(--text-muted)]">
              {t("preSeason.transferDetail.to")}
            </div>
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
      </div>
    </div>
  );
}
