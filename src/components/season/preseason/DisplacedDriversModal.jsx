import { useTranslation } from "react-i18next";
import TeamLogoMark from "../../team/TeamLogoMark";
import {
  LICENSE_COLORS,
  licenseTooltip,
  formatLastChampionshipResult,
} from "../preSeasonFormatters.js";

// Modal: Pilotos sem vaga (fim da pré-temporada).
export default function DisplacedDriversModal({ groups, totalCount, onClose, onConfirm }) {
  const { t } = useTranslation();
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div className="glass-strong animate-fade-in mx-4 w-full max-w-4xl rounded-2xl p-6 md:p-7">
        <div className="mb-1 text-body-sm font-bold uppercase tracking-[0.22em] text-[#f85149]">
          {t("preSeason.displaced.eyebrow")}
        </div>
        <h2 className="mb-1 text-[18px] font-bold leading-tight text-[color:var(--text-primary)]">
          {t("preSeason.displaced.title")}
        </h2>
        <p className="mb-5 text-body text-[color:var(--text-secondary)]">
          {t("preSeason.displaced.subtitle", { count: totalCount })}
        </p>

        <div className="mb-6 max-h-[70vh] space-y-4 overflow-y-auto pr-1">
          {groups.map((group) => (
            <section key={group.category} className="space-y-2.5">
              <div
                className="flex items-center gap-3 rounded-xl px-3 py-2"
                style={{
                  background: `linear-gradient(135deg, ${group.color}22 0%, rgba(255,255,255,0.03) 100%)`,
                  borderLeft: `3px solid ${group.color}`,
                }}
              >
                <span
                  className="text-[13px] font-bold uppercase tracking-[0.16em]"
                  style={{ color: group.color }}
                >
                  {group.label}
                </span>
                <div
                  className="h-px flex-1"
                  style={{ background: `linear-gradient(to right, ${group.color}55, transparent)` }}
                />
                <span className="text-body-sm text-[color:var(--text-muted)]">
                  {group.drivers.length}
                </span>
              </div>

              <div className="grid grid-cols-1 gap-2 xl:grid-cols-2">
                {group.drivers.map((d) => {
                  const lic = LICENSE_COLORS[d.license_sigla] ?? LICENSE_COLORS.R;
                  const licenseTitle = licenseTooltip(d.license_sigla);
                  const lastChampionshipResult = formatLastChampionshipResult(d);

                  return (
                    <div
                      key={d.driver_id}
                      className="flex items-center gap-3 rounded-xl px-3.5 py-3 shadow-[0_10px_24px_rgba(0,0,0,0.18)]"
                      style={{
                        background: "rgba(8, 13, 24, 0.76)",
                        border: "1px solid rgba(255, 255, 255, 0.12)",
                        boxShadow:
                          "inset 0 1px 0 rgba(255,255,255,0.05), 0 10px 24px rgba(0,0,0,0.18)",
                      }}
                    >
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-2">
                          <p className="text-[17px] font-bold leading-tight text-[color:var(--text-primary)]">
                            {d.driver_name}
                          </p>
                        </div>
                        <div className="mt-1.5 space-y-0.5 text-body-sm text-[color:var(--text-muted)]">
                          {d.previous_team_name && d.seasons_at_last_team > 0 && (
                            <div className="min-w-0">
                              <div className="text-[10px] font-bold uppercase tracking-[0.14em] text-[color:var(--text-muted)]">
                                {t("preSeason.displaced.formerTeam")}
                              </div>
                              <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-1">
                                <TeamLogoMark
                                  teamName={d.previous_team_name}
                                  color={d.previous_team_color}
                                  size="xs"
                                  testId="displaced-driver-previous-team-logo"
                                />
                                <span
                                  className="block truncate text-[14px] font-semibold"
                                  style={{ color: d.previous_team_color ?? "var(--text-secondary)" }}
                                >
                                  {d.previous_team_name}
                                </span>
                                {lastChampionshipResult && (
                                  <span className="text-[13px] font-bold text-[color:var(--text-secondary)]">
                                    {`• ${lastChampionshipResult}`}
                                  </span>
                                )}
                              </div>
                              <span className="text-[12px]">{t("preSeason.displaced.seasons", { count: d.seasons_at_last_team })}</span>
                            </div>
                          )}
                        </div>
                      </div>
                      <span
                        aria-label={licenseTitle}
                        className="shrink-0 rounded-lg px-2 py-1.5 text-[11px] text-[10px] font-black uppercase tracking-[0.12em] min-w-[3.25rem] min-w-[2.4rem] text-center shadow-[inset_0_1px_0_rgba(255,255,255,0.18)]"
                        style={{ background: lic.bg, color: lic.text }}
                        title={licenseTitle}
                      >
                        {d.license_sigla}
                      </span>
                    </div>
                  );
                })}
              </div>
            </section>
          ))}
        </div>

        <div className="flex gap-3">
          <button
            onClick={onClose}
            className="transition-glass flex-1 rounded-xl border border-white/15 bg-white/5 px-4 py-2.5 text-body font-semibold text-[color:var(--text-secondary)] hover:bg-white/10"
          >
            {t("preSeason.actions.back")}
          </button>
          <button
            onClick={onConfirm}
            className="transition-glass glow-blue flex-1 rounded-xl border border-[#3fb95099] bg-[#3fb950] px-4 py-2.5 text-body font-bold text-[#06101f] hover:bg-[#52d16a]"
          >
            {t("preSeason.actions.startSeason")}
          </button>
        </div>
      </div>
    </div>
  );
}
