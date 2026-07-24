import { useTranslation } from "react-i18next";
import { ordinal } from "../../../i18n/format.js";
import { formatSalaryAnnual, extractNationalityLabel } from "../../../utils/formatters";
import TeamLogoMark from "../../team/TeamLogoMark";
import FlagIcon from "../../ui/FlagIcon";
import {
  PIP_COUNT,
  tierBucket,
  pipsFilled,
  championshipColor,
  tierColor,
  isRookieCategory,
  formatTeammateTenure,
  famaTierLabel,
  formatCashCompact,
} from "../preSeasonFormatters.js";

// Card RICO (ficha da equipe) usado dentro do modal de ofertas.
// Mostra os dados de scouting; o foco e a relação (vínculo) ficam na tela de contrato.
export default function OfferCardRich({ offer, isAdvancingWeek, onViewContract }) {
  const { t } = useTranslation();
  const accent = offer.team_color || "#58a6ff";
  const rookie = isRookieCategory(offer.category);
  const pedigree =
    (offer.team_titles_drivers ?? 0) +
    (offer.team_titles_constructors ?? 0) +
    (offer.team_historic_wins ?? 0);
  const countryLabel = extractNationalityLabel(offer.team_country) || offer.team_country || "";
  const dur = offer.offer_duration ?? 1;
  // No rookie o carro não afeta o resultado → não mostrar (seria enganoso).
  const stats = [
    !rookie && { key: "car", value: offer.car_performance_rating },
    { key: "reliability", value: offer.team_reliability },
    { key: "prestige", value: offer.team_reputation },
  ].filter(Boolean);
  return (
    <article
      className={[
        "glass animate-scale-in overflow-hidden rounded-2xl",
        offer.active_interest
          ? "ring-2 ring-[#f2c46d]/70 shadow-[0_0_28px_rgba(242,196,109,0.22)]"
          : "",
      ].join(" ")}
      style={{ borderLeft: `3px solid ${accent}` }}
    >
      {offer.active_interest && (
        <div className="flex items-center gap-1.5 bg-[#f2c46d1a] px-4 py-1.5 text-[10px] font-bold uppercase tracking-[0.16em] text-[#f2c46d]">
          <span>◆</span>
          <span>{t("preSeason.offers.card.activeInterest")}</span>
        </div>
      )}
      {/* Cabeçalho: identidade da equipe */}
      <div
        className="flex items-center gap-3 px-4 py-3.5"
        style={{ background: `linear-gradient(135deg, ${accent}22 0%, rgba(255,255,255,0.02) 100%)` }}
      >
        <TeamLogoMark teamName={offer.team_name} color={accent} size="lg" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span
              className="rounded-md px-1.5 py-0.5 text-[10px] font-black uppercase tracking-[0.14em]"
              style={{ color: accent, background: `${accent}22` }}
            >
              {offer.role === "N1" ? t("preSeason.offers.card.driver1") : t("preSeason.offers.card.driver2")}
            </span>
            <span
              className="text-[10px] font-bold uppercase tracking-[0.16em]"
              style={{ color: accent }}
            >
              {offer.category_label || offer.category}
            </span>
          </div>
          <p className="mt-1 truncate text-title-md font-bold">{offer.team_name}</p>
          <div className="mt-0.5 flex items-center gap-1.5 text-[11px] text-[color:var(--text-muted)]">
            <FlagIcon nacionalidade={offer.team_country} className="h-3.5 w-5" />
            {countryLabel && <span>{countryLabel}</span>}
            {offer.team_founded_year ? <span>{t("preSeason.foundedSince", { year: offer.team_founded_year })}</span> : null}
          </div>
        </div>
        <div className="text-right">
          <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">{t("preSeason.offers.card.salary")}</p>
          <p className="num-medium font-bold text-[color:var(--status-green)]">{formatSalaryAnnual(offer.salary)}</p>
        </div>
      </div>

      <div className="space-y-3 px-4 py-3.5">
        {/* Atributos em FAIXA de texto (sem números) */}
        <div className={`grid grid-cols-1 gap-2 ${stats.length === 3 ? "sm:grid-cols-3" : "sm:grid-cols-2"}`}>
          {stats.map(({ key, value }) => (
            <div key={key} className="glass-light rounded-lg p-2.5">
              <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">{t(`preSeason.offers.card.stats.${key}`)}</p>
              <p className="mt-0.5 text-body font-bold" style={{ color: tierColor(value) }}>
                {t(`preSeason.offers.card.tierScales.${key}.${tierBucket(value)}`)}
              </p>
              <div className="mt-1.5 flex gap-0.5">
                {Array.from({ length: PIP_COUNT }).map((_, i) => (
                  <span
                    key={i}
                    className="h-1 flex-1 rounded-full"
                    style={{ background: i < pipsFilled(value) ? tierColor(value) : "#21262d" }}
                  />
                ))}
              </div>
            </div>
          ))}
        </div>

        {/* Última temporada — em destaque */}
        {(() => {
          const pos = offer.team_last_position;
          const posColor = pos != null ? championshipColor(pos) : "var(--text-muted)";
          return (
            <div
              className="flex items-center justify-between rounded-xl px-3.5 py-2.5"
              style={{ background: `${accent}12`, border: `1px solid ${accent}2e` }}
            >
              <div>
                <p className="text-[9px] font-bold uppercase tracking-[0.22em] text-[color:var(--text-muted)]">
                  {t("preSeason.offers.card.lastSeason")}
                </p>
                <p className="text-[10px] text-[color:var(--text-secondary)]">
                  {pos != null ? t("preSeason.offers.card.championshipPosition") : t("preSeason.offers.card.noHistory")}
                </p>
              </div>
              <p className="text-[34px] font-black leading-none" style={{ color: posColor }}>
                {pos != null ? ordinal(pos) : t("preSeason.offers.card.debutant")}
              </p>
            </div>
          );
        })()}

        {/* Ficha textual */}
        <div className={`grid grid-cols-2 gap-2 ${rookie ? "sm:grid-cols-2" : "sm:grid-cols-3"}`}>
          <div className="glass-light rounded-lg p-2.5">
            <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">{t("preSeason.offers.card.cash")}</p>
            <p
              className="mt-0.5 num-medium font-bold"
              style={{ color: offer.team_cash < 0 ? "var(--status-red)" : "var(--status-green)" }}
            >
              {formatCashCompact(offer.team_cash)}
            </p>
          </div>
          {!rookie && (
            <div className="glass-light rounded-lg p-2.5">
              <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">{t("preSeason.offers.card.titles")}</p>
              <p className="mt-0.5 num-medium font-bold text-[color:var(--text-primary)]">
                {(offer.team_titles_drivers ?? 0)}
                <span className="text-[color:var(--text-muted)]"> · </span>
                {(offer.team_titles_constructors ?? 0)}
              </p>
              <p className="text-[9px] text-[color:var(--text-muted)]">{t("preSeason.offers.card.driversConstructors")}</p>
            </div>
          )}
          <div className="glass-light rounded-lg p-2.5">
            <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">{t("preSeason.offers.card.winsPodiums")}</p>
            <p className="mt-0.5 num-medium font-bold text-[color:var(--text-primary)]">
              {offer.team_historic_wins ?? 0}
              <span className="text-[color:var(--text-muted)]"> · </span>
              {offer.team_historic_podiums ?? 0}
            </p>
          </div>
        </div>

        {/* Companheiro de equipe (hover → estatísticas) */}
        <div className="glass-light rounded-lg p-2.5">
          <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">{t("preSeason.offers.card.teammate")}</p>
          {offer.teammate_name ? (
            <div className="group relative mt-0.5 cursor-help">
              <div className="flex items-center justify-between gap-2">
                <div className="flex min-w-0 items-center gap-1.5">
                  <p className="text-body min-w-0 truncate font-semibold text-[color:var(--text-primary)]">
                    {offer.teammate_name}
                  </p>
                  <span className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full border border-white/25 text-[9px] font-bold text-[color:var(--text-muted)] transition-colors group-hover:border-[color:var(--accent-primary)] group-hover:text-[color:var(--accent-primary)]">
                    ?
                  </span>
                </div>
                <span className="shrink-0 text-[10px] text-[color:var(--text-muted)]">
                  {formatTeammateTenure(offer.teammate_tenure)}
                </span>
              </div>
              {/* Tooltip de estatísticas */}
              <div className="pointer-events-none absolute bottom-full left-0 z-30 mb-2 w-64 rounded-xl border border-white/10 bg-[#0d1117] p-3 opacity-0 shadow-[0_8px_24px_rgba(0,0,0,0.5)] transition-opacity duration-150 group-hover:opacity-100">
                <p className="mb-2 min-w-0 truncate text-[10px] font-bold uppercase tracking-[0.16em] text-[color:var(--text-secondary)]">
                  {offer.teammate_name}
                  {offer.teammate_age != null ? t("preSeason.offers.card.teammateAge", { count: offer.teammate_age }) : ""}
                </p>
                <div className="grid grid-cols-2 gap-x-3 gap-y-1.5">
                  {[
                    ["races", offer.teammate_races ?? 0],
                    ["wins", offer.teammate_wins ?? 0],
                    ["podiums", offer.teammate_podiums ?? 0],
                    ["poles", offer.teammate_poles ?? 0],
                    ["titles", offer.teammate_titles ?? 0],
                    ["points", Math.round(offer.teammate_career_points ?? 0)],
                  ].map(([statKey, val]) => (
                    <div key={statKey} className="flex items-center justify-between">
                      <span className="text-[11px] text-[color:var(--text-muted)]">{t(`preSeason.offers.card.teammateStats.${statKey}`)}</span>
                      <span className="num-medium text-[12px] font-bold text-[color:var(--text-primary)]">{val}</span>
                    </div>
                  ))}
                </div>
                {(offer.teammate_strengths?.length > 0 || offer.teammate_weaknesses?.length > 0) && (
                  <div className="mt-2 space-y-1 border-t border-white/8 pt-2">
                    {offer.teammate_strengths?.length > 0 && (
                      <div className="flex items-start gap-1.5">
                        <span className="mt-px shrink-0 text-[11px] font-bold text-[color:var(--status-green)]">▲</span>
                        <span className="text-[11px] text-[color:var(--text-secondary)]">
                          {offer.teammate_strengths.join(" · ")}
                        </span>
                      </div>
                    )}
                    {offer.teammate_weaknesses?.length > 0 && (
                      <div className="flex items-start gap-1.5">
                        <span className="mt-px shrink-0 text-[11px] font-bold text-[#f85149]">▼</span>
                        <span className="text-[11px] text-[color:var(--text-secondary)]">
                          {offer.teammate_weaknesses.join(" · ")}
                        </span>
                      </div>
                    )}
                  </div>
                )}
                {offer.teammate_fama != null && (
                  <div className="mt-2 flex items-center justify-between border-t border-white/8 pt-2">
                    <span className="text-[11px] text-[color:var(--text-muted)]">{t("preSeason.offers.card.fame")}</span>
                    <span
                      title={
                        offer.teammate_carisma != null
                          ? t("preSeason.offers.card.fameCharismaTooltip", { fama: offer.teammate_fama, carisma: offer.teammate_carisma })
                          : t("preSeason.offers.card.fameTooltip", { fama: offer.teammate_fama })
                      }
                      className="num-medium text-[12px] font-bold text-[color:var(--accent-secondary)]"
                    >
                      {famaTierLabel(offer.teammate_fama)}
                    </span>
                  </div>
                )}
                <div className="mt-2 flex items-center justify-between border-t border-white/8 pt-2">
                  <span className="text-[11px] text-[color:var(--text-muted)]">{t("preSeason.offers.card.salary")}</span>
                  <span className="num-medium text-[12px] font-bold text-[color:var(--status-green)]">
                    {offer.teammate_salary != null ? formatSalaryAnnual(offer.teammate_salary) : "—"}
                  </span>
                </div>
              </div>
            </div>
          ) : (
            <p className="text-body mt-0.5 text-[color:var(--text-muted)]">{t("preSeason.offers.card.freeSlot")}</p>
          )}
        </div>

        {/* Duração do contrato ofertado */}
        <div className="flex items-center justify-between rounded-lg bg-black/18 px-3 py-2">
          <span className="text-[9px] font-bold uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
            {t("preSeason.offers.card.offeredContract")}
          </span>
          <span
            className="num-medium text-body font-bold"
            style={{ color: dur >= 2 ? "var(--status-green)" : "var(--text-primary)" }}
          >
            {t("preSeason.offers.card.contractDuration", { count: dur })}
            {dur >= 2 && <span className="ml-1.5 text-[9px] font-semibold">{t("preSeason.offers.card.project")}</span>}
          </span>
        </div>

        <button
          onClick={() => onViewContract?.(offer)}
          disabled={isAdvancingWeek}
          className="transition-glass glow-blue w-full rounded-lg border border-[#58a6ff66] bg-[#58a6ff33] px-3 py-2.5 text-body font-bold text-[color:var(--accent-primary)] hover:bg-[#58a6ff55] disabled:cursor-not-allowed disabled:opacity-50"
        >
          {t("preSeason.offers.card.viewContract")}
        </button>
        {pedigree === 0 && (
          <p className="text-center text-[10px] text-[color:var(--text-muted)]">{t("preSeason.offers.card.noPedigree")}</p>
        )}
      </div>
    </article>
  );
}
