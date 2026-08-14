import { useTranslation } from "react-i18next";

import SeasonSectionHeader from "../SeasonSectionHeader";
import TeamLogoMark from "../../team/TeamLogoMark";
import { CATEGORY_LOGOS } from "./constantes.js";
import { buildClassGroups, classAccentColor, countTeamVacancies } from "./agrupamentos.js";

function TeamPilotRow({ name, fallback, isNew = false, accentColor = "rgba(88,166,255,0.9)" }) {
  const empty = !name;

  return (
    <div className="flex items-center gap-3 py-2 first:pt-0 last:pb-0">
      <div
        className="h-2 w-2 shrink-0 rounded-full"
        style={{ background: empty ? "rgba(255,255,255,0.16)" : accentColor }}
      />
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <p
          className={`min-w-0 flex-1 truncate text-[11px] leading-[1.2] ${empty ? "italic text-[color:var(--text-muted)]" : "font-semibold text-[color:var(--text-primary)]"}`}
        >
          {name || fallback}
        </p>
        {!empty && isNew && (
          <span className="rounded-full border border-[#58a6ff55] bg-[#58a6ff1a] px-2 py-0.5 text-[9px] font-black uppercase tracking-[0.18em] text-[#8cc8ff]">
            NEW
          </span>
        )}
      </div>
    </div>
  );
}

export default function MapaEquipes({ filteredSections, totalVisibleTeams, currentDay }) {
  const { t } = useTranslation();

  return (
    <main className="glass scroll-area animate-fade-in min-h-0 overflow-y-auto rounded-2xl px-5 py-4 lg:px-6 lg:py-5">
      <div className="mb-5 flex h-6 items-center justify-between">
        <p className="text-body-sm font-bold uppercase tracking-[0.2em] text-[color:var(--text-secondary)]">
          {t("convocation.teamMap.title")}
        </p>
        <p className="text-body text-[color:var(--text-muted)]">
          {t("convocation.teamMap.teamsCount", { count: totalVisibleTeams })}
        </p>
      </div>

      {filteredSections.length === 0 ? (
        <div className="py-20 text-center text-body text-[color:var(--text-muted)]">
          {t("convocation.teamMap.empty")}
        </div>
      ) : (
        <div className="space-y-3">
          {filteredSections.map((section, index) => (
            <section key={section.category} className={index > 0 ? "mt-10" : ""}>
              <div
                data-testid={`convocation-category-header-${section.category}`}
                className="mb-5 flex flex-col items-center justify-center gap-3 rounded-xl px-4 py-6 text-center"
                style={{
                  background: `linear-gradient(135deg, ${section.color}22 0%, ${section.color}0a 100%)`,
                  borderLeft: `3px solid ${section.color}`,
                  boxShadow: `0 0 18px ${section.color}18`,
                }}
              >
                {CATEGORY_LOGOS[section.category] ? (
                  <img
                    src={CATEGORY_LOGOS[section.category]}
                    alt={section.label}
                    className="h-48 w-auto max-w-full object-contain drop-shadow-[0_18px_36px_rgba(0,0,0,0.38)] lg:h-52"
                    draggable={false}
                  />
                ) : (
                  <span
                    className="text-[17px] font-bold uppercase tracking-[0.18em]"
                    style={{ color: section.color }}
                  >
                    {section.label}
                  </span>
                )}
                <span
                  data-testid="convocation-category-count"
                  className="shrink-0 rounded-full border px-3 py-1 text-[11px] font-bold uppercase tracking-[0.12em]"
                  style={{
                    color: section.color,
                    borderColor: `${section.color}55`,
                    backgroundColor: `${section.color}14`,
                  }}
                >
                  {t("convocation.teamMap.teamsCount", { count: section.teams.length })}
                </span>
              </div>

              <div className="space-y-8">
                {buildClassGroups(section.teams, section.category).map((classGroup) => (
                  <div key={`${section.category}:${classGroup.className}`} className="space-y-3">
                    <SeasonSectionHeader
                      title={classGroup.className.toUpperCase()}
                      color={classAccentColor(classGroup.className)}
                      detail={t("convocation.teamMap.teamsCount", { count: classGroup.teams.length })}
                      titleTestId="convocation-class-title"
                    />

                    <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
                      {classGroup.teams.map((team) => {
                        const vacancies = countTeamVacancies(team);
                        const accentColor = classAccentColor(classGroup.className);
                        const isOddCount = classGroup.teams.length % 2 === 1;
                        const isLastOddCard =
                          isOddCount && classGroup.teams[classGroup.teams.length - 1]?.id === team.id;

                        return (
                          <article
                            key={team.id}
                            className={`glass transition-glass relative overflow-hidden rounded-xl border p-3 hover:-translate-y-0.5 hover:scale-[1.01] ${
                              isLastOddCard ? "lg:col-span-2 lg:mx-auto lg:w-[calc(50%-0.375rem)]" : ""
                            }`}
                            style={{ borderColor: "rgba(255,255,255,0.11)" }}
                          >
                            <div className="relative mb-3 flex items-start gap-3">
                              <div className="flex min-w-0 flex-1 items-center gap-3">
                                <TeamLogoMark
                                  teamName={team.nome}
                                  color={accentColor}
                                  size="md"
                                  testId="convocation-team-logo"
                                />
                                <div className="min-w-0 flex-1">
                                  <p className="truncate text-[19px] font-bold leading-[1.05]">
                                    {team.nome}
                                  </p>
                                </div>
                                {vacancies > 0 && (
                                  <span className="ml-auto shrink-0 rounded-md border border-white/[0.12] bg-white/5 px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.08em] text-[color:var(--text-secondary)]">
                                    {t("convocation.teamMap.vacancies", { count: vacancies })}
                                  </span>
                                )}
                              </div>
                            </div>

                            <div className="relative divide-y divide-white/[0.08]">
                              <TeamPilotRow
                                name={team.piloto_1_nome}
                                fallback={t("convocation.teamMap.pilotOpen", { slot: 1 })}
                                isNew={team.piloto_1_new_badge_day === currentDay}
                                accentColor={accentColor}
                              />
                              <TeamPilotRow
                                name={team.piloto_2_nome}
                                fallback={t("convocation.teamMap.pilotOpen", { slot: 2 })}
                                isNew={team.piloto_2_new_badge_day === currentDay}
                                accentColor={accentColor}
                              />
                            </div>
                          </article>
                        );
                      })}
                    </div>
                  </div>
                ))}
              </div>
            </section>
          ))}
        </div>
      )}
    </main>
  );
}
