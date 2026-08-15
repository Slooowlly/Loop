import { useState } from "react";
import { useTranslation } from "react-i18next";

import RivalMarker from "../driver/RivalMarker";
import TeamLogoMark from "../team/TeamLogoMark";
import Tooltip from "../ui/Tooltip";
import WeekendModifiersTip, { LARGURA as LARGURA_MODIFICADORES } from "./WeekendModifiersTip";
import { getTeamGlow } from "../../utils/teamColors";
import { getReadableTeamColor } from "./raceGridContext";

// Coluna 3 da Sala de Estratégia: a tabela do campeonato, alternando entre pilotos e
// construtores. O hover destaca a dupla da equipe; `hoveredDriverId` vem de fora (é o
// mesmo realce dos nomes mencionados no texto do engenheiro).
function ChampionshipTablePanel({
  championshipTable,
  constructorsTable,
  playerTeamId,
  breakdownRiskTeams,
  weekendModifiers,
  hoveredDriverId,
}) {
  const { t } = useTranslation();
  const [standingsView, setStandingsView] = useState("pilotos");
  const [hoveredTeamId, setHoveredTeamId] = useState(null);

  return (
    <div className="xl:col-span-4 h-[500px] xl:h-[calc(100vh-19rem)] xl:min-h-[650px]">
      <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-6 h-full flex flex-col relative overflow-hidden">
        <div className="mb-4 flex flex-shrink-0 items-center justify-between gap-3">
          <p className="min-w-0 truncate text-[11px] font-bold uppercase tracking-[0.2em] text-[#58a6ff]">{t("nextRaceTab.labels.championshipTable")}</p>
          <div className="inline-flex shrink-0 items-center gap-1 rounded-full border border-white/10 bg-white/5 p-0.5">
            {[
              { id: "pilotos", label: t("nextRaceTab.labels.tabDrivers") },
              { id: "construtores", label: t("nextRaceTab.labels.tabConstructors") },
            ].map((tab) => (
              <button
                key={tab.id}
                type="button"
                onClick={() => setStandingsView(tab.id)}
                className={`px-3.5 py-1.5 rounded-full text-[11px] font-bold uppercase tracking-[0.06em] transition ${
                  standingsView === tab.id
                    ? "bg-[#58a6ff] text-[#06090e] shadow-[0_0_16px_rgba(88,166,255,0.35)]"
                    : "text-gray-400 hover:text-white"
                }`}
              >
                {tab.label}
              </button>
            ))}
          </div>
        </div>

        {standingsView === "pilotos" ? (
          championshipTable.length === 0 ? (
            <p className="text-sm text-gray-400">{t("nextRaceTab.labels.standingsUnavailable")}</p>
          ) : (
            <div className="flex-1 overflow-y-auto custom-scrollbar -mx-2 px-2 pb-2">
              <table className="w-full text-sm">
                <thead className="sticky top-0 bg-[#06090ebd] backdrop-blur z-20 text-[9px] text-gray-500 uppercase font-bold text-left border-b border-white/10">
                  <tr>
                    <th className="py-2 px-3 text-center w-8">#</th>
                    <th className="py-2 px-1">{t("nextRaceTab.labels.colDriver")}</th>
                    <th className="py-2 px-3 text-right">{t("nextRaceTab.labels.colPts")}</th>
                  </tr>
                </thead>
                <tbody>
                  {championshipTable.map((driver) => {
                    const isPlayer = driver.is_jogador;
                    const isHoveredTeam = hoveredTeamId != null && driver.equipe_id === hoveredTeamId;
                    const isMentionHovered = hoveredDriverId != null && driver.id === hoveredDriverId;
                    const teamColor = getReadableTeamColor(driver.equipe_cor);
                    // O balão explica o DIA deste piloto (forma, lesão, pressão…). Sem dado da
                    // esteira ele nem abre — melhor nenhum balão que um balão vazio.
                    const modifiers = weekendModifiers?.get?.(driver.id) ?? null;
                    return (
                      <Tooltip
                        key={driver.id}
                        // Ao lado, e não em cima: a linha ocupa a coluna inteira, então um
                        // balão centrado nela cobre a própria tabela que se está lendo.
                        lado="esquerda"
                        largura={LARGURA_MODIFICADORES}
                        desabilitado={!modifiers}
                        conteudo={
                          modifiers ? (
                            <WeekendModifiersTip
                              driverName={driver.nome_completo ?? driver.nome}
                              modifiers={modifiers}
                            />
                          ) : null
                        }
                      >
                        <tr
                          onMouseEnter={() => setHoveredTeamId(driver.equipe_id ?? null)}
                          onMouseLeave={() => setHoveredTeamId(null)}
                          className={`border-b transition-glass ${
                            isMentionHovered || isHoveredTeam
                              ? "border-transparent"
                              : isPlayer
                                ? "border-[#58a6ff]/40 bg-[#58a6ff]/10"
                                : "border-white/5 hover:bg-white/5"
                          }`}
                          style={
                            isMentionHovered
                              ? (() => {
                                  const tone = getTeamGlow(driver.equipe_cor);
                                  return { backgroundColor: tone.soft, boxShadow: `inset 0 0 0 1.5px ${tone.solid}` };
                                })()
                              : isHoveredTeam
                                ? (() => {
                                    const tone = getTeamGlow(driver.equipe_cor);
                                    return { backgroundColor: tone.soft, boxShadow: `inset 3px 0 0 0 ${tone.solid}` };
                                  })()
                                : undefined
                          }
                        >
                          <td className={`py-3 px-3 text-center ${isPlayer ? "font-extrabold text-[#58a6ff]" : "font-bold text-white"}`}>
                            {driver.posicao_campeonato}
                          </td>
                          <td className="py-3 px-1">
                            <div className="flex items-center gap-2">
                              <TeamLogoMark
                                teamName={driver.equipe_nome}
                                color={driver.equipe_cor}
                                size="xs"
                              />
                              <div className="min-w-0">
                                <p className={`flex items-center gap-1 leading-tight ${isPlayer ? "text-white font-bold" : "text-white font-medium"}`}>
                                  <span className="truncate">{driver.nome_completo ?? driver.nome}</span>
                                  <RivalMarker driverId={driver.id} />
                                  {breakdownRiskTeams.has(driver.equipe_id) ? (
                                    <Tooltip texto={t("nextRaceTab.labels.breakdownRiskDriverTip")}>
                                      <span
                                        className="shrink-0 text-[11px] leading-none"
                                        aria-label={t("nextRaceTab.labels.breakdownRiskDriverTip")}
                                      >
                                        🔧
                                      </span>
                                    </Tooltip>
                                  ) : null}
                                </p>
                                <p
                                  className="truncate text-[10px] font-semibold uppercase tracking-[0.04em] leading-tight"
                                  style={{ color: teamColor }}
                                >
                                  {driver.equipe_nome_curto ?? driver.equipe_nome ?? "—"}
                                </p>
                              </div>
                            </div>
                          </td>
                          <td className={`py-3 px-3 text-right align-top ${isPlayer ? "font-extrabold text-[#58a6ff]" : "font-bold text-white"}`}>
                            {driver.pontos}
                          </td>
                        </tr>
                      </Tooltip>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )
        ) : constructorsTable.length === 0 ? (
          <p className="text-sm text-gray-400">{t("nextRaceTab.labels.teamStandingsUnavailable")}</p>
        ) : (
          <div className="flex-1 overflow-y-auto custom-scrollbar -mx-2 px-2 pb-2">
            <table className="w-full text-sm">
              <thead className="sticky top-0 bg-[#06090ebd] backdrop-blur z-20 text-[9px] text-gray-500 uppercase font-bold text-left border-b border-white/10">
                <tr>
                  <th className="py-2 px-3 text-center w-8">#</th>
                  <th className="py-2 px-1">{t("nextRaceTab.labels.colTeam")}</th>
                  <th className="py-2 px-3 text-right">{t("nextRaceTab.labels.colPts")}</th>
                </tr>
              </thead>
              <tbody>
                {constructorsTable.map((team) => {
                  const isPlayerTeam = playerTeamId != null && team.id === playerTeamId;
                  const teamColor = getReadableTeamColor(team.cor_primaria);
                  return (
                    <tr
                      key={team.id}
                      className={`border-b ${isPlayerTeam ? "border-[#58a6ff]/40 bg-[#58a6ff]/10" : "border-white/5 hover:bg-white/5"}`}
                    >
                      <td className={`py-3 px-3 text-center ${isPlayerTeam ? "font-extrabold text-[#58a6ff]" : "font-bold text-white"}`}>
                        {team.posicao}
                      </td>
                      <td className="py-3 px-1">
                        <div className="flex items-center gap-2">
                          <TeamLogoMark teamName={team.nome} color={team.cor_primaria} size="xs" />
                          <div className="min-w-0">
                            <p
                              className="truncate font-semibold leading-tight"
                              style={{ color: teamColor }}
                            >
                              {team.nome}
                            </p>
                          </div>
                        </div>
                      </td>
                      <td className={`py-3 px-3 text-right align-top ${isPlayerTeam ? "font-extrabold text-[#58a6ff]" : "font-bold text-white"}`}>
                        {team.pontos}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}

export default ChampionshipTablePanel;
