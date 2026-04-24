import { Fragment, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import DriverDetailModal from "../../components/driver/DriverDetailModal";
import ResultBadge from "../../components/standings/ResultBadge";
import TrophyBadge from "../../components/standings/TrophyBadge";
import TeamLogoMarkShared from "../../components/team/TeamLogoMark";
import FlagIcon from "../../components/ui/FlagIcon";
import GlassCard from "../../components/ui/GlassCard";
import useCareerStore from "../../stores/useCareerStore";
import { categoryLabel } from "../../utils/formatters";
import { TeamHistoryDrawer } from "./MyTeamTab";

const ALL_CATEGORIES = [
  "mazda_rookie",
  "toyota_rookie",
  "mazda_amador",
  "toyota_amador",
  "bmw_m2",
  "production_challenger",
  "gt4",
  "gt3",
  "endurance",
];

const STANDARD_POINTS = {
  1: 25,
  2: 18,
  3: 15,
  4: 12,
  5: 10,
  6: 8,
  7: 6,
  8: 4,
  9: 2,
  10: 1,
};

const SPECIAL_STANDING_GROUPS = {
  production_challenger: [
    { id: "bmw", label: "BMW M2", color: "#bc8cff" },
    { id: "toyota", label: "Toyota GR86", color: "#f2cc60" },
    { id: "mazda", label: "Mazda MX-5", color: "#c8102e" },
  ],
  endurance: [
    { id: "lmp2", label: "LMP2", color: "#f2cc60" },
    { id: "gt3", label: "GT3", color: "#e73f47" },
    { id: "gt4", label: "GT4", color: "#58a6ff" },
  ],
};

const SPECIAL_TEAM_RELEGATION_COUNT = 3;
const PROMOTION_ONLY_TEAM_ZONE_CATEGORIES = new Set([
  "mazda_rookie",
  "toyota_rookie",
  "bmw_m2",
  "gt4",
  "gt3",
]);
const NO_MOVEMENT_TEAM_ZONE_CATEGORIES = new Set(["endurance"]);
const PRODUCTION_SPECIAL_FEEDERS = new Set([
  "mazda_rookie",
  "toyota_rookie",
  "mazda_amador",
  "toyota_amador",
  "bmw_m2",
]);
const ENDURANCE_SPECIAL_FEEDERS = new Set(["gt4", "gt3"]);

function getForcedSpecialStandingCategory(phase, playerTeamCategory, acceptedSpecialOffer) {
  if (phase !== "BlocoEspecial") {
    return null;
  }

  const offeredSpecialCategory =
    typeof acceptedSpecialOffer?.special_category === "string"
      ? acceptedSpecialOffer.special_category.trim().toLowerCase()
      : null;

  if (offeredSpecialCategory === "production_challenger" || offeredSpecialCategory === "endurance") {
    return offeredSpecialCategory;
  }

  if (playerTeamCategory === "production_challenger" || playerTeamCategory === "endurance") {
    return playerTeamCategory;
  }

  if (PRODUCTION_SPECIAL_FEEDERS.has(playerTeamCategory)) {
    return "production_challenger";
  }

  if (ENDURANCE_SPECIAL_FEEDERS.has(playerTeamCategory)) {
    return "endurance";
  }

  return null;
}

function StandingsTab() {
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const season = useCareerStore((state) => state.season);
  const acceptedSpecialOffer = useCareerStore((state) => state.acceptedSpecialOffer);
  const forcedSpecialCategory = getForcedSpecialStandingCategory(
    season?.fase,
    playerTeam?.categoria,
    acceptedSpecialOffer,
  );
  const [viewCategory, setViewCategory] = useState(
    () => forcedSpecialCategory ?? playerTeam?.categoria ?? ALL_CATEGORIES[0],
  );
  const [driverStandings, setDriverStandings] = useState([]);
  const [teamStandings, setTeamStandings] = useState([]);
  const [previousChampionId, setPreviousChampionId] = useState(null);
  const [selectedDriverId, setSelectedDriverId] = useState(null);
  const [hoveredDriverId, setHoveredDriverId] = useState(null);
  const [selectedHistoryTeam, setSelectedHistoryTeam] = useState(null);
  const [activeHistoryTab, setActiveHistoryTab] = useState("records");

  const categoryIndex = ALL_CATEGORIES.indexOf(viewCategory);
  function goUpCategory() {
    if (forcedSpecialCategory) {
      return;
    }
    if (categoryIndex < ALL_CATEGORIES.length - 1) {
      setViewCategory(ALL_CATEGORIES[categoryIndex + 1]);
    }
  }
  function goDownCategory() {
    if (forcedSpecialCategory) {
      return;
    }
    if (categoryIndex > 0) {
      setViewCategory(ALL_CATEGORIES[categoryIndex - 1]);
    }
  }
  const activeDriverId = hoveredDriverId ?? selectedDriverId;
  const activeDriver = driverStandings.find((d) => d.id === activeDriverId) ?? null;
  const selectedTeamId = activeDriver?.equipe_id ?? null;
  const selectedTeamColor = activeDriver?.equipe_cor ?? null;
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    if (forcedSpecialCategory && viewCategory !== forcedSpecialCategory) {
      setViewCategory(forcedSpecialCategory);
    }
  }, [forcedSpecialCategory, viewCategory]);

  useEffect(() => {
    let mounted = true;

    async function fetchStandings() {
      if (!careerId || !viewCategory) {
        setLoading(false);
        return;
      }

      setLoading(true);
      setError("");

      try {
        const [drivers, teams, previousChampions] = await Promise.all([
          invoke("get_drivers_by_category", {
            careerId,
            category: viewCategory,
          }),
          invoke("get_teams_standings", {
            careerId,
            category: viewCategory,
          }),
          invoke("get_previous_champions", {
            careerId,
            category: viewCategory,
          }),
        ]);

        if (!mounted) return;

        setDriverStandings(drivers);
        setTeamStandings(teams);
        setPreviousChampionId(previousChampions.driver_champion_id ?? null);
      } catch (invokeError) {
        if (!mounted) return;

        setError(
          typeof invokeError === "string"
            ? invokeError
            : "Não foi possível carregar a classificação.",
        );
      } finally {
        if (mounted) {
          setLoading(false);
        }
      }
    }

    fetchStandings();

    return () => {
      mounted = false;
    };
  }, [careerId, viewCategory, season?.ano, season?.rodada_atual, season?.fase]);

  const totalRodadas = driverStandings.length > 0
    ? Math.max(...driverStandings.map((d) => (d.results ?? []).length))
    : (season?.total_rodadas || 0);
  const completedRounds = viewCategory === playerTeam?.categoria
    ? Math.max(0, (season?.rodada_atual || 1) - 1)
    : totalRodadas;
  const positionDeltaMap = useMemo(
    () => buildPositionDeltaMap(driverStandings, completedRounds),
    [driverStandings, completedRounds],
  );
  const specialClassGroups = SPECIAL_STANDING_GROUPS[viewCategory] ?? null;
  const driverStandingSections = useMemo(
    () => buildSpecialStandingSections(driverStandings, specialClassGroups),
    [driverStandings, specialClassGroups],
  );
  const teamStandingSections = useMemo(
    () => buildSpecialStandingSections(teamStandings, specialClassGroups),
    [teamStandings, specialClassGroups],
  );
  const showSpecialPendingNotice =
    specialClassGroups != null && !hasSpecialStandingResults(driverStandings, teamStandings);

  useEffect(() => {
    if (!selectedHistoryTeam?.id) return;
    if (!teamStandings.some((team) => team.id === selectedHistoryTeam.id)) {
      setSelectedHistoryTeam(null);
    }
  }, [selectedHistoryTeam?.id, teamStandings]);


  if (loading) {
    return (
      <GlassCard hover={false} className="rounded-[28px] p-10">
        <p className="text-sm uppercase tracking-[0.22em] text-accent-primary">Dashboard</p>
        <h2 className="mt-3 text-3xl font-semibold text-text-primary">Carregando classificação</h2>
        <p className="mt-3 text-sm text-text-secondary">
          Buscando pilotos, construtores e resultados da categoria atual.
        </p>
      </GlassCard>
    );
  }

  if (error) {
    return (
      <GlassCard hover={false} className="rounded-[28px] border border-status-red/30 p-10">
        <p className="text-sm font-semibold text-status-red">{error}</p>
      </GlassCard>
    );
  }

  return (
    <>
      <div
        className="grid gap-5 xl:grid-cols-[1.6fr_0.95fr]"
      >
        <GlassCard hover={false} className="overflow-hidden rounded-[28px]">
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="flex items-center gap-2">
                <div className="flex flex-col">
                  <button
                    onClick={goUpCategory}
                    disabled={categoryIndex === ALL_CATEGORIES.length - 1}
                    className="text-[10px] leading-[1.1] transition-colors disabled:cursor-default disabled:opacity-20 text-text-muted hover:enabled:text-text-primary"
                    title="Categoria superior"
                  >
                    ▲
                  </button>
                  <button
                    onClick={goDownCategory}
                    disabled={categoryIndex === 0}
                    className="text-[10px] leading-[1.1] transition-colors disabled:cursor-default disabled:opacity-20 text-text-muted hover:enabled:text-text-primary"
                    title="Categoria inferior"
                  >
                    ▼
                  </button>
                </div>
                <p className="text-[11px] uppercase tracking-[0.22em] text-accent-primary">
                  {categoryLabel(viewCategory)}
                </p>
              </div>
              <h2 className="mt-2 text-2xl font-semibold text-text-primary">
                Classificação de pilotos
              </h2>
            </div>
            <p className="text-sm text-text-secondary">{driverStandings.length} pilotos</p>
          </div>

          {showSpecialPendingNotice ? (
            <SpecialPendingNotice category={viewCategory} phase={season?.fase} />
          ) : (
          <div className="mt-6 overflow-x-auto">
            <table className="w-full table-fixed" style={{ minWidth: 536 + totalRodadas * 68 }}>
              <colgroup>
                <col style={{ width: "62px" }} />
                <col style={{ width: "34px" }} />
                <col style={{ width: "42px" }} />
                <col style={{ width: "214px" }} />
                <col style={{ width: "118px" }} />
                {Array.from({ length: totalRodadas }, (_, index) => (
                  <col key={`rodada-col-${index + 1}`} style={{ width: "68px" }} />
                ))}
                <col style={{ width: "66px" }} />
              </colgroup>
              <thead>
                <tr className="border-b border-white/10 text-left text-[11px] uppercase tracking-[0.18em] text-text-muted">
                  <th className="py-3 pr-0.5">Pos</th>
                  <th className="py-3 pr-0.5 text-center" />
                  <th className="py-3 pr-1 text-center">Id</th>
                  <th className="py-3 pr-1">Piloto</th>
                  <th className="py-3 pr-1">Equipe</th>
                  {Array.from({ length: totalRodadas }, (_, index) => {
                    const rodada = index + 1;
                    return (
                      <th
                        key={rodada}
                        className={[
                          "px-1.5 py-3 text-center",
                          rodada > completedRounds ? "opacity-30" : "",
                          rodada === season?.rodada_atual ? "text-accent-primary" : "",
                        ].join(" ")}
                      >
                        R{rodada}
                      </th>
                    );
                  })}
                  <th className="py-3 pl-3 text-right">Pts</th>
                </tr>
              </thead>
              <tbody>
                {driverStandingSections.map((section) => (
                  <Fragment key={`drivers-${section.id}`}>
                    {specialClassGroups ? (
                      <tr>
                        <td colSpan={totalRodadas + 6} className="px-0 pt-4 pb-2">
                          <SpecialClassHeader section={section} sticky />
                        </td>
                      </tr>
                    ) : null}
                    {section.items.map((driver, index) => {
                  const isInSelectedTeam =
                    selectedTeamId != null && driver.equipe_id === selectedTeamId;
                  const teamColor = selectedTeamColor;
                  const displayPosition = specialClassGroups
                    ? index + 1
                    : driver.posicao_campeonato;

                  return (
                    <tr
                      key={driver.id}
                      role="button"
                      tabIndex={0}
                      onMouseEnter={() => setHoveredDriverId(driver.id)}
                      onMouseLeave={() => setHoveredDriverId(null)}
                      onClick={() =>
                        setSelectedDriverId((prev) => (prev === driver.id ? null : driver.id))
                      }
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          setSelectedDriverId((prev) => (prev === driver.id ? null : driver.id));
                        }
                      }}
                      className={[
                        "cursor-pointer border-b border-white/5 transition-glass",
                        !isInSelectedTeam && driver.is_jogador
                          ? "bg-accent-primary/8 hover:bg-accent-primary/15"
                          : !isInSelectedTeam
                            ? "hover:bg-white/5 focus-visible:bg-white/5"
                            : "",
                      ].join(" ")}
                      style={
                        isInSelectedTeam && teamColor
                          ? {
                              backgroundColor: `${teamColor}22`,
                              boxShadow: `inset 3px 0 0 0 ${teamColor}`,
                            }
                          : undefined
                      }
                    >
                      <td className="py-3 pr-0.5 text-sm font-semibold">
                        <div className="flex items-center gap-1">
                          <span className={podiumClass(index)}>{displayPosition}</span>
                          <PositionDelta delta={positionDeltaMap.get(driver.id)} />
                        </div>
                      </td>
                      <td className="py-3 pr-0.5 text-center">
                        <FlagIcon nacionalidade={driver.nacionalidade} className="mx-auto" />
                      </td>
                      <td className="py-3 pr-1 text-center text-sm font-medium text-text-secondary">
                        {driver.idade ?? "—"}
                      </td>
                      <td className="py-3 pr-1">
                        <div className="flex items-center gap-2">
                          <span
                            className={[
                              "block truncate text-sm whitespace-nowrap",
                              driver.is_jogador
                                ? "font-semibold text-accent-primary"
                                : "text-text-primary",
                            ].join(" ")}
                            title={driver.nome}
                          >
                            {driver.is_jogador ? "▸ " : ""}
                            {driver.nome}
                            {driver.is_jogador ? " ◂" : ""}
                          </span>
                          {driver.id === previousChampionId ? (
                            <span className="shrink-0 text-sm" title="Campeão da temporada anterior">
                              🏆
                            </span>
                          ) : null}
                        </div>
                      </td>
                      <td className="py-3 pr-1 pl-0 text-sm font-semibold uppercase tracking-[0.02em]">
                        <div className="flex items-center gap-1.5">
                          <TeamLogoMarkShared
                            teamName={driver.equipe_nome}
                            color={driver.equipe_cor}
                            size="sm"
                          />
                          <span
                            className="block truncate"
                            style={{ color: getReadableTeamColor(driver.equipe_cor) }}
                            title={driver.equipe_nome_curto ?? driver.equipe_nome ?? "—"}
                          >
                            {driver.equipe_nome_curto ?? driver.equipe_nome ?? "—"}
                          </span>
                        </div>
                      </td>
                      {(driver.results ?? []).map((result, rodadaIndex) => (
                        <td key={`${driver.id}-r${rodadaIndex + 1}`} className="px-1 py-3 text-center">
                          <ResultBadge result={result} />
                        </td>
                      ))}
                      <td className="py-3 pl-3 text-right font-mono text-sm font-semibold text-text-primary">
                        {driver.pontos}
                      </td>
                    </tr>
                  );
                    })}
                  </Fragment>
                ))}
              </tbody>
            </table>
          </div>
          )}
        </GlassCard>

        <GlassCard hover={false} className="rounded-[28px]">
          <div className="flex items-center justify-between gap-4">
            <div>
              <p className="text-[11px] uppercase tracking-[0.22em] text-accent-primary">
                Construtores
              </p>
              <h2 className="mt-2 text-2xl font-semibold text-text-primary">
                Classificação de equipes
              </h2>
            </div>
            <p className="text-sm text-text-secondary">{teamStandings.length} equipes</p>
          </div>

          <div className="mt-6 space-y-2">
            {showSpecialPendingNotice ? (
              <SpecialPendingTeamsNotice />
            ) : specialClassGroups ? (
              teamStandingSections.map((section) => (
                <div key={`teams-${section.id}`} className="space-y-2">
                  <SpecialClassHeader section={section} />
                  {section.items.map((team, index) => {
                    const isRelegationZone =
                      section.items.length > SPECIAL_TEAM_RELEGATION_COUNT
                      && index >= section.items.length - SPECIAL_TEAM_RELEGATION_COUNT;
                    return (
                      <TeamStandingCard
                        key={team.id}
                        team={team}
                        position={index + 1}
                        index={index}
                        isRelegationZone={isRelegationZone}
                        isHistoryActive={selectedHistoryTeam?.id === team.id}
                        onTeamHistoryOpen={(selectedTeam) => {
                          setSelectedHistoryTeam(selectedTeam);
                          setActiveHistoryTab("records");
                        }}
                      />
                    );
                  })}
                </div>
              ))
            ) : (() => {
              const { promotionCount, relegationCount } = getZoneCutoffs(viewCategory);
              const total = teamStandings.length;
              const items = [];
              teamStandings.forEach((team, index) => {
                if (index === promotionCount && promotionCount > 0) {
                  items.push(<ZoneDivider key="divider-promo" label="PROMOÇÃO ↑" variant="green" />);
                }
                if (relegationCount > 0 && index === total - relegationCount) {
                  items.push(<ZoneDivider key="divider-relego" label="REBAIXAMENTO ↓" variant="red" />);
                }
                items.push(
                  <div
                    key={team.id}
                    onDoubleClick={() => {
                      setSelectedHistoryTeam(team);
                      setActiveHistoryTab("records");
                    }}
                    className={[
                      "flex items-center justify-between rounded-2xl border px-4 py-3 transition-glass hover:bg-white/[0.05]",
                      selectedHistoryTeam?.id === team.id
                        ? "border-status-yellow/45 bg-status-yellow/10 shadow-[inset_4px_0_0_rgba(242,196,109,0.95)]"
                        : "border-white/6 bg-white/[0.03]",
                    ].join(" ")}
                  >
                    <div className="flex min-w-0 flex-1 items-center gap-3">
                      <span className={["w-7 text-center text-sm font-semibold", podiumClass(index)].join(" ")}>
                        {team.posicao}
                      </span>
                      <TeamLogoMarkShared teamName={team.nome} color={team.cor_primaria} />
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-1.5">
                          <p
                            className="truncate text-sm font-semibold"
                            style={{ color: getReadableTeamColor(team.cor_primaria) }}
                          >
                            {team.nome}
                          </p>
                          {(team.trofeus ?? []).map((trofeu, trophyIndex) => (
                            <TrophyBadge key={`${team.id}-t${trophyIndex}`} trofeu={trofeu} />
                          ))}
                        </div>
                        <TeamDriverLine team={team} />
                      </div>
                    </div>
                    <div className="shrink-0 pl-4 text-right">
                      <p className="font-mono text-base font-semibold text-text-primary">{team.pontos}</p>
                      <p className="text-xs text-text-secondary">{team.vitorias} vit.</p>
                    </div>
                  </div>
                );
              });
              return items;
            })()}
          </div>
        </GlassCard>
      </div>

      {selectedDriverId ? (
        <DriverDetailModal
          driverId={selectedDriverId}
          driverIds={driverStandings.map((driver) => driver.id)}
          onSelectDriver={setSelectedDriverId}
          onClose={() => setSelectedDriverId(null)}
        />
      ) : null}

      {selectedHistoryTeam ? (
        <TeamHistoryDrawer
          careerId={careerId}
          team={selectedHistoryTeam}
          teams={teamStandings}
          playerTeam={playerTeam}
          activeCategory={viewCategory}
          placement="left"
          activeTab={activeHistoryTab}
          onTabChange={setActiveHistoryTab}
          onSelectTeam={setSelectedHistoryTeam}
          onClose={() => setSelectedHistoryTeam(null)}
        />
      ) : null}
    </>
  );
}

function podiumClass(index) {
  if (index === 0) return "text-[#ffd700]";
  if (index === 1) return "text-[#c0c0c0]";
  if (index === 2) return "text-[#cd7f32]";
  return "text-text-secondary";
}

function getReadableTeamColor(color) {
  if (!color || !/^#([0-9a-f]{6})$/i.test(color)) {
    return "#7d8590";
  }

  const hex = color.slice(1);
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  const luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;

  if (luminance < 0.32) {
    const mixWithWhite = 0.58;
    const boost = (channel) => Math.round(channel + (255 - channel) * mixWithWhite);
    return `rgb(${boost(r)}, ${boost(g)}, ${boost(b)})`;
  }

  return color;
}

function formatTeamDriverName(name) {
  return typeof name === "string" && name.trim().length > 0 ? name.trim() : "-";
}

function formatTeamDriverPair(team) {
  return `${formatTeamDriverName(team.piloto_1_nome)} / ${formatTeamDriverName(team.piloto_2_nome)}`;
}

function TeamDriverLine({ team }) {
  const driverNames = formatTeamDriverPair(team);

  return (
    <p
      className="block truncate whitespace-nowrap text-xs text-text-secondary"
      title={driverNames}
    >
      {driverNames}
    </p>
  );
}

function hasSpecialStandingResults(driverStandings, teamStandings) {
  return (
    driverStandings.some((driver) => {
      const hasRoundResult = (driver.results ?? []).some(Boolean);
      return hasRoundResult || driver.pontos > 0 || driver.vitorias > 0 || driver.podios > 0;
    })
    || teamStandings.some((team) => team.pontos > 0 || team.vitorias > 0)
  );
}

function specialPendingMessage(phase) {
  if (phase === "JanelaConvocacao") {
    return "A competição começa quando a janela de convocação for finalizada. Os resultados aparecem aqui quando a primeira corrida for simulada.";
  }
  if (phase === "BlocoEspecial") {
    return "O bloco especial já foi aberto, mas ainda não existe resultado registrado. A classificação aparece quando a primeira corrida for simulada.";
  }
  return "Esta competição acontece depois da temporada regular e da janela de convocação. Os resultados aparecem aqui quando o bloco especial for simulado.";
}

function SpecialPendingNotice({ category, phase }) {
  return (
    <div className="mt-6 rounded-3xl border border-white/10 bg-white/[0.035] p-6 text-center shadow-[inset_0_1px_0_rgba(255,255,255,0.05)]">
      <p className="text-[11px] font-semibold uppercase tracking-[0.22em] text-accent-primary">
        {categoryLabel(category)}
      </p>
      <h3 className="mt-3 text-xl font-semibold text-text-primary">
        Competição especial ainda não aconteceu
      </h3>
      <p className="mx-auto mt-3 max-w-xl text-sm leading-6 text-text-secondary">
        {specialPendingMessage(phase)}
      </p>
    </div>
  );
}

function SpecialPendingTeamsNotice() {
  return (
    <div className="rounded-2xl border border-white/8 bg-white/[0.025] px-4 py-5 text-sm leading-6 text-text-secondary">
      A classificação de equipes será liberada junto com os resultados do bloco especial.
    </div>
  );
}

function SpecialClassHeader({ section, sticky = false }) {
  return (
    <div
      className={[
        "flex items-center justify-center gap-3 py-2.5",
        sticky ? "sticky left-0 z-10 w-[min(760px,calc(100vw-3rem))]" : "w-full",
      ].join(" ")}
    >
      <span
        className="h-px flex-1"
        style={{
          background: `linear-gradient(90deg, transparent 0%, ${section.color}4d 58%, ${section.color}c2 100%)`,
        }}
      />
      <span
        className="shrink-0 px-3 text-center text-[17px] font-black uppercase leading-none tracking-[0.22em]"
        style={{
          color: section.color,
          textShadow: `0 0 18px ${section.color}55`,
        }}
      >
        {section.label}
      </span>
      <span
        className="h-px flex-1"
        style={{
          background: `linear-gradient(90deg, ${section.color}c2 0%, ${section.color}4d 42%, transparent 100%)`,
        }}
      />
    </div>
  );
}

function TeamStandingCard({ team, position, index, isRelegationZone = false, isHistoryActive = false, onTeamHistoryOpen }) {
  const cardClassName = [
    "flex items-center justify-between rounded-2xl border px-4 py-3 transition-glass",
    isHistoryActive
      ? "border-status-yellow/45 bg-status-yellow/10 shadow-[inset_4px_0_0_rgba(242,196,109,0.95)] hover:bg-status-yellow/15"
      : "",
    isRelegationZone
      ? "border-status-red/35 bg-status-red/[0.12] shadow-[inset_3px_0_0_0_rgba(248,81,73,0.75)] hover:bg-status-red/[0.18]"
      : !isHistoryActive
        ? "border-white/6 bg-white/[0.03] hover:bg-white/[0.05]"
        : "",
  ].join(" ");

  return (
    <div
      onDoubleClick={() => onTeamHistoryOpen?.(team)}
      className={cardClassName}
      data-relegation-zone={isRelegationZone ? "true" : undefined}
    >
      <div className="flex min-w-0 flex-1 items-center gap-3">
        <span
          className={[
            "w-7 text-center text-sm font-semibold",
            isRelegationZone ? "text-status-red" : podiumClass(index),
          ].join(" ")}
        >
          {position}
        </span>
        <TeamLogoMarkShared teamName={team.nome} color={team.cor_primaria} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <p
              className="block truncate text-sm font-semibold"
              style={{ color: getReadableTeamColor(team.cor_primaria) }}
            >
              {team.nome}
            </p>
            {(team.trofeus ?? []).map((trofeu, trophyIndex) => (
              <TrophyBadge key={`${team.id}-t${trophyIndex}`} trofeu={trofeu} />
            ))}
          </div>
          <TeamDriverLine team={team} />
        </div>
      </div>
      <div className="shrink-0 pl-4 text-right">
        <p className="font-mono text-base font-semibold text-text-primary">{team.pontos}</p>
        <p className="text-xs text-text-secondary">{team.vitorias} vit.</p>
      </div>
    </div>
  );
}

function buildSpecialStandingSections(items, classGroups) {
  if (!classGroups) {
    return [{ id: "all", label: null, color: "#7d8590", items }];
  }

  const knownIds = new Set(classGroups.map((group) => group.id));
  const sections = classGroups
    .map((group) => ({
      ...group,
      items: items.filter((item) => normalizeClassId(item.classe) === group.id),
    }))
    .filter((section) => section.items.length > 0);

  const unknownItems = items.filter((item) => !knownIds.has(normalizeClassId(item.classe)));
  if (unknownItems.length > 0) {
    sections.push({
      id: "outros",
      label: "Outros",
      color: "#7d8590",
      items: unknownItems,
    });
  }

  return sections;
}

function normalizeClassId(value) {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

function buildPositionDeltaMap(drivers, completedRounds) {
  const deltaMap = new Map();

  if (!Array.isArray(drivers) || completedRounds <= 1) {
    return deltaMap;
  }

  const previousRoundCount = completedRounds - 1;
  const previousStandings = [...drivers]
    .map((driver) => ({
      id: driver.id,
      nome: driver.nome,
      currentPosition: driver.posicao_campeonato ?? Number.MAX_SAFE_INTEGER,
      previousPoints: calculatePointsThroughRound(driver.results, previousRoundCount),
      previousBestFinish: calculateBestFinish(driver.results, previousRoundCount),
    }))
    .sort((left, right) => {
      if (right.previousPoints !== left.previousPoints) {
        return right.previousPoints - left.previousPoints;
      }
      if (left.previousBestFinish !== right.previousBestFinish) {
        return left.previousBestFinish - right.previousBestFinish;
      }
      if (left.currentPosition !== right.currentPosition) {
        return left.currentPosition - right.currentPosition;
      }
      return left.nome.localeCompare(right.nome, "pt-BR");
    });

  previousStandings.forEach((driver, index) => {
    deltaMap.set(driver.id, index + 1);
  });

  return new Map(
    drivers.map((driver) => {
      const previousPosition = deltaMap.get(driver.id);
      const currentPosition = driver.posicao_campeonato ?? 0;
      return [driver.id, previousPosition ? previousPosition - currentPosition : 0];
    }),
  );
}

function calculatePointsThroughRound(results, roundCount) {
  if (!Array.isArray(results) || roundCount <= 0) {
    return 0;
  }

  return results.slice(0, roundCount).reduce((total, result) => total + pointsForResult(result), 0);
}

function calculateBestFinish(results, roundCount) {
  if (!Array.isArray(results) || roundCount <= 0) {
    return Number.MAX_SAFE_INTEGER;
  }

  return results.slice(0, roundCount).reduce((best, result) => {
    if (!result || result.is_dnf) {
      return best;
    }
    return Math.min(best, result.position ?? Number.MAX_SAFE_INTEGER);
  }, Number.MAX_SAFE_INTEGER);
}

function pointsForResult(result) {
  if (!result || result.is_dnf) {
    return 0;
  }

  return STANDARD_POINTS[result.position] ?? 0;
}

function getZoneCutoffs(categoria) {
  if (PROMOTION_ONLY_TEAM_ZONE_CATEGORIES.has(categoria)) {
    return { promotionCount: 1, relegationCount: 0 };
  }
  if (NO_MOVEMENT_TEAM_ZONE_CATEGORIES.has(categoria)) {
    return { promotionCount: 0, relegationCount: 0 };
  }
  return { promotionCount: 1, relegationCount: 1 };
}

function ZoneDivider({ label, variant }) {
  const colorClass = variant === "green" ? "text-status-green border-status-green/30" : "text-status-red border-status-red/30";
  const lineClass = variant === "green" ? "border-status-green/20" : "border-status-red/20";
  return (
    <div className="flex items-center gap-3 py-1">
      <div className={["flex-1 border-t border-dashed", lineClass].join(" ")} />
      <span className={["text-[10px] font-semibold uppercase tracking-[0.18em] px-2 py-0.5 rounded border", colorClass].join(" ")}>
        {label}
      </span>
      <div className={["flex-1 border-t border-dashed", lineClass].join(" ")} />
    </div>
  );
}

function PositionDelta({ delta }) {
  if (!delta) {
    return <span className="text-[10px] font-semibold text-text-muted">•</span>;
  }

  if (delta > 0) {
    return (
      <span className="inline-flex items-center justify-center gap-0.5 text-[10px] font-semibold leading-none text-status-green">
        ▲{delta}
      </span>
    );
  }

  return (
    <span className="inline-flex items-center justify-center gap-0.5 text-[10px] font-semibold leading-none text-status-red">
      ▼{Math.abs(delta)}
    </span>
  );
}

export default StandingsTab;
