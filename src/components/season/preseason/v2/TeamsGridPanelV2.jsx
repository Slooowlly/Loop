import { useTranslation } from "react-i18next";
import CategoryStripV2 from "./CategoryStripV2";
import TeamGridCardV2 from "./TeamGridCardV2";
import {
  CLASS_LABELS,
  MULTICLASS_ORDER,
  MULTICLASS_SUBCLASS_TONES,
  subcatColor,
  getTeamMovementOrder,
  getTeamMappingSortValue,
  isRealCareerDebutCategory,
} from "../../preSeasonFormatters.js";
import { countOpenSeats, countSeatsAtRisk } from "./seatSelectors.js";

// Ordem única: classificação da temporada que acabou (com promovidas/rebaixadas
// agrupadas no fim, como no v1). O grid é uma FOTO — reordená-lo por outro
// critério tira do jogador a única referência estável que ele tem dele.
function TeamCardGrid({ teams, accent, hoveredFreeAgentCat, setHistoryTeam }) {
  const ordered = [...teams].sort((a, b) => {
    const movementDiff = getTeamMovementOrder(a) - getTeamMovementOrder(b);
    if (movementDiff !== 0) return movementDiff;
    const positionDiff = getTeamMappingSortValue(a) - getTeamMappingSortValue(b);
    if (positionDiff !== 0) return positionDiff;
    return a.nome.localeCompare(b.nome);
  });
  return (
    <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
      {ordered.map((team) => (
        <TeamGridCardV2
          key={team.id}
          team={team}
          accent={accent}
          hoveredFreeAgentCat={hoveredFreeAgentCat}
          onOpenHistory={setHistoryTeam}
        />
      ))}
    </div>
  );
}

export default function TeamsGridPanelV2({
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
    <main className="glass animate-fade-in flex min-h-0 flex-col rounded-2xl">
      {/* A legenda de estados saiu daqui: com o contrato vencendo virando apenas a
          cor do contador de anos, sobraram dois estados — assento livre (escrito na
          própria linha) e aposentadoria (ícone com tooltip). Nenhum dos dois precisa
          de legenda; ter uma fileira de chips explicando chips era o retrato do
          problema. */}
      <div className="flex items-baseline gap-3 border-b border-white/[0.07] px-4 py-2.5 lg:px-5">
        <p className="text-[9px] font-black uppercase tracking-[0.22em] text-[color:var(--text-secondary)]">
          {t("preSeason.grid.title")}
        </p>
        <p className="text-[10px] text-[color:var(--text-muted)]">
          {t("preSeason.teamHistoryDblClick")}
        </p>
      </div>

      <div ref={mainGridRef} className="scroll-area min-h-0 flex-1 overflow-y-auto px-4 py-4 lg:px-5">
        {loadingGrid ? (
          <div className="py-20 text-center text-body text-[color:var(--text-muted)]">
            {t("preSeason.grid.loading")}
          </div>
        ) : gridData.length === 0 ? (
          <div className="py-20 text-center text-body text-[color:var(--text-muted)]">
            {t("preSeason.grid.empty")}
          </div>
        ) : (
          <div>
            {sortedClasses.map((teamClass, classIndex) => {
              const teams = groupedTeams[teamClass];
              const accent = subcatColor(teamClass);
              const seatTotal = teams.length * 2;
              const seatsOpen = countOpenSeats(teams);
              const seatsAtRisk = countSeatsAtRisk(teams);
              const startsRookieBlock =
                classIndex > 0
                && isRealCareerDebutCategory(teamClass)
                && !isRealCareerDebutCategory(sortedClasses[classIndex - 1]);
              const sectionSpacing = classIndex === 0 ? "" : startsRookieBlock ? "mt-12" : "mt-9";

              return (
                <section key={teamClass} className={sectionSpacing}>
                  <CategoryStripV2
                    categoryKey={teamClass}
                    teamCount={teams.length}
                    seatTotal={seatTotal}
                    seatsOpen={seatsOpen}
                    seatsAtRisk={seatsAtRisk}
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
                          const clsTeams = byClass.get(cls);
                          const dividerColor = MULTICLASS_SUBCLASS_TONES[teamClass]?.[cls] ?? subcatColor(cls);
                          const clsSeatsOpen = countOpenSeats(clsTeams);
                          const clsSeatsAtRisk = countSeatsAtRisk(clsTeams);
                          return (
                            <div key={cls}>
                              {/* Sub-classe (carro) mantém o divisor forte do v1 — é o que
                                  separa LMP2 de GT3 dentro de Endurance —, mas agora ele
                                  carrega os mesmos contadores da faixa da categoria. */}
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
                                    {clsSeatsOpen > 0 && (
                                      <span style={{ color: "var(--status-green)" }}>
                                        {" · "}
                                        {t("preSeason.grid.vacancies", { count: clsSeatsOpen })}
                                      </span>
                                    )}
                                    {clsSeatsOpen === 0 && clsSeatsAtRisk > 0 && (
                                      <span style={{ color: "var(--status-yellow)" }}>
                                        {" · "}
                                        {t("preSeason.v2.grid.atRiskCount", { count: clsSeatsAtRisk })}
                                      </span>
                                    )}
                                  </span>
                                </div>
                                <div
                                  className="h-px flex-1"
                                  style={{ background: `linear-gradient(to left, transparent, ${dividerColor}88)` }}
                                />
                              </div>
                              <TeamCardGrid
                                teams={clsTeams}
                                accent={dividerColor}
                                hoveredFreeAgentCat={hoveredFreeAgentCat}
                                setHistoryTeam={setHistoryTeam}
                              />
                            </div>
                          );
                        });
                      })()}
                    </div>
                  ) : (
                    <TeamCardGrid
                      teams={teams}
                      accent={accent}
                      hoveredFreeAgentCat={hoveredFreeAgentCat}
                      setHistoryTeam={setHistoryTeam}
                    />
                  )}
                </section>
              );
            })}
          </div>
        )}
      </div>
    </main>
  );
}
