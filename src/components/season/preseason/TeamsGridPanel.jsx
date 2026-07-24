import { useTranslation } from "react-i18next";
import MarketCategoryHeader from "./MarketCategoryHeader";
import TeamGridCard from "./TeamGridCard";
import {
  CLASS_LABELS,
  MULTICLASS_ORDER,
  MULTICLASS_SUBCLASS_TONES,
  subcatColor,
  getTeamMovementOrder,
  getTeamMappingSortValue,
  count_team_vacancies,
  isRealCareerDebutCategory,
} from "../preSeasonFormatters.js";

export default function TeamsGridPanel({
  mainGridRef,
  loadingGrid,
  gridData,
  sortedClasses,
  groupedTeams,
  hoveredFreeAgentCat,
  setHistoryTeam,
}) {
  const { t } = useTranslation();
  return (
    <main ref={mainGridRef} className="glass scroll-area animate-fade-in min-h-0 overflow-y-auto rounded-2xl px-5 py-4 lg:px-6 lg:py-5">
      <div className="mb-5 flex h-6 items-center justify-between">
        <p className="text-body-sm font-bold uppercase tracking-[0.2em] text-[color:var(--text-secondary)]">
          {t("preSeason.grid.title")}
        </p>
        <p className="text-body text-[color:var(--text-muted)]">{t("preSeason.grid.subtitle")}</p>
      </div>

      {loadingGrid ? (
        <div className="py-20 text-center text-body text-[color:var(--text-muted)]">
          {t("preSeason.grid.loading")}
        </div>
      ) : gridData.length === 0 ? (
        <div className="py-20 text-center text-body text-[color:var(--text-muted)]">
          {t("preSeason.grid.empty")}
        </div>
      ) : (
        <div className="space-y-3">
          {sortedClasses.map((teamClass, classIndex) => {
            const teams = [...groupedTeams[teamClass]].sort((a, b) => {
              const movementOrderDiff = getTeamMovementOrder(a) - getTeamMovementOrder(b);
              if (movementOrderDiff !== 0) return movementOrderDiff;

              const previousPositionDiff = getTeamMappingSortValue(a) - getTeamMappingSortValue(b);
              if (previousPositionDiff !== 0) return previousPositionDiff;

              return a.nome.localeCompare(b.nome);
            });
            const accent = subcatColor(teamClass);
            const totalVacancies = teams.reduce((sum, team) => sum + count_team_vacancies(team), 0);
            const startsRookieBlock =
              classIndex > 0
              && isRealCareerDebutCategory(teamClass)
              && !isRealCareerDebutCategory(sortedClasses[classIndex - 1]);
            const sectionSpacing = classIndex === 0 ? "" : startsRookieBlock ? "mt-14" : "mt-10";

            return (
              <section key={teamClass} className={sectionSpacing}>
                <MarketCategoryHeader
                  categoryKey={teamClass}
                  detail={t("preSeason.grid.vacancies", { count: totalVacancies })}
                />

                {MULTICLASS_ORDER[teamClass] ? (
                  <div className="space-y-5">
                    {(() => {
                      const order = MULTICLASS_ORDER[teamClass];
                      const byClass = new Map();
                      for (const team of teams) {
                        const cls = team.classe || "outras";
                        if (!byClass.has(cls)) byClass.set(cls, []);
                        byClass.get(cls).push(team);
                      }
                      const orderedClasses = [...byClass.keys()].sort((a, b) => {
                        const ia = order.indexOf(a);
                        const ib = order.indexOf(b);
                        if (ia !== -1 && ib !== -1) return ia - ib;
                        if (ia !== -1) return -1;
                        if (ib !== -1) return 1;
                        return a.localeCompare(b);
                      });
                      return orderedClasses.map((cls) => {
                        const clsColor = subcatColor(cls);
                        // Cor do MENU (divisor): puxa o tom da categoria-pai (roxo
                        // no Production, verde no Endurance); cai na cor da classe
                        // fora das multiclasses. As equipes seguem com a cor delas.
                        const dividerColor = MULTICLASS_SUBCLASS_TONES[teamClass]?.[cls] ?? clsColor;
                        const clsTeams = byClass.get(cls);
                        return (
                          <div key={cls}>
                            {/* Divisor GRANDE centralizado por classe (carro). */}
                            <div className="mb-4 mt-1 flex items-center gap-4">
                              <div
                                className="h-px flex-1"
                                style={{ background: `linear-gradient(to right, transparent, ${dividerColor}88)` }}
                              />
                              <div className="flex flex-col items-center">
                                <span
                                  className="text-[24px] font-black uppercase leading-none tracking-[0.22em]"
                                  style={{ color: dividerColor, textShadow: `0 0 22px ${dividerColor}55` }}
                                >
                                  {CLASS_LABELS[cls] ?? cls.toUpperCase()}
                                </span>
                                <span className="mt-1.5 text-[9px] font-semibold uppercase tracking-[0.16em] text-[color:var(--text-muted)]">
                                  {t("preSeason.grid.teamCount", { count: clsTeams.length })}
                                </span>
                              </div>
                              <div
                                className="h-px flex-1"
                                style={{ background: `linear-gradient(to left, transparent, ${dividerColor}88)` }}
                              />
                            </div>
                            {/* accent do card = tom da categoria (roxo/verde): pinta as
                                pills "Vaga aberta" e o glow de hover; o logo do time
                                segue com a cor_primaria própria. */}
                            <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
                              {clsTeams.map((team) => (
                                <TeamGridCard
                                  key={team.id}
                                  team={team}
                                  accent={dividerColor}
                                  hoveredFreeAgentCat={hoveredFreeAgentCat}
                                  onOpenHistory={setHistoryTeam}
                                />
                              ))}
                            </div>
                          </div>
                        );
                      });
                    })()}
                  </div>
                ) : (
                  <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
                    {teams.map((team) => (
                      <TeamGridCard
                        key={team.id}
                        team={team}
                        accent={accent}
                        hoveredFreeAgentCat={hoveredFreeAgentCat}
                        onOpenHistory={setHistoryTeam}
                      />
                    ))}
                  </div>
                )}
              </section>
            );
          })}
        </div>
      )}
    </main>
  );
}
