import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import useCareerStore from "../../stores/useCareerStore";
import {
  categoryLabel,
  formatCompactDate,
  formatDate,
  formatNextRaceCountdown,
  formatSurfaceSeasonLabel,
} from "../../utils/formatters";
import GlassButton from "../ui/GlassButton";
import TabNavigation from "./TabNavigation";

function Header({ activeTab, onTabChange }) {
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const season = useCareerStore((state) => state.season);
  const nextRace = useCareerStore((state) => state.nextRace);
  const temporalSummary = useCareerStore((state) => state.temporalSummary);
  const calendarDisplayDate = useCareerStore((state) => state.calendarDisplayDate);
  const displayDaysUntilNextEvent = useCareerStore((state) => state.displayDaysUntilNextEvent);
  const isCalendarAdvancing = useCareerStore((state) => state.isCalendarAdvancing);
  const isAdvancing = useCareerStore((state) => state.isAdvancing);
  const isConvocating = useCareerStore((state) => state.isConvocating);
  const showRaceBriefing = useCareerStore((state) => state.showRaceBriefing);
  const startCalendarAdvance = useCareerStore((state) => state.startCalendarAdvance);
  const advanceSeason = useCareerStore((state) => state.advanceSeason);
  const skipAllPendingRaces = useCareerStore((state) => state.skipAllPendingRaces);
  const runConvocationWindow = useCareerStore((state) => state.runConvocationWindow);
  const finishSpecialBlock = useCareerStore((state) => state.finishSpecialBlock);
  const closeRaceBriefing = useCareerStore((state) => state.closeRaceBriefing);
  const [seasonChampion, setSeasonChampion] = useState(null);

  const visibleDate = calendarDisplayDate ?? temporalSummary?.current_display_date;
  const visibleCountdown = displayDaysUntilNextEvent ?? temporalSummary?.days_until_next_event;
  const hasNoPendingRace = !nextRace;
  const isFreeAgent = !playerTeam;
  const phase = season?.fase;
  const canAdvanceCalendar = Boolean(nextRace) || (
    !isFreeAgent &&
    phase === "BlocoRegular" &&
    (temporalSummary?.pending_in_phase ?? 0) > 0
  );

  useEffect(() => {
    let mounted = true;

    async function loadSeasonChampion() {
      if (!careerId || !playerTeam?.categoria || !hasNoPendingRace) {
        if (mounted) {
          setSeasonChampion(null);
        }
        return;
      }

      try {
        const standings = await invoke("get_drivers_by_category", {
          careerId,
          category: playerTeam.categoria,
        });

        if (!mounted) return;

        const champion = Array.isArray(standings)
          ? standings.find((driver) => driver?.posicao_campeonato === 1) ?? standings[0] ?? null
          : null;

        setSeasonChampion(champion);
      } catch (error) {
        console.error("Erro ao carregar campeão da temporada para o header:", error);
        if (mounted) {
          setSeasonChampion(null);
        }
      }
    }

    loadSeasonChampion();

    return () => {
      mounted = false;
    };
  }, [careerId, playerTeam?.categoria, hasNoPendingRace, season?.ano]);

  function handleNextRace() {
    void Promise.resolve(startCalendarAdvance?.()).catch((error) => {
      console.error("Erro ao avançar calendário pelo header:", error);
    });
  }

  async function handleAdvanceSeason() {
    try {
      if (isFreeAgent && hasNoPendingRace) {
        await skipAllPendingRaces?.();
        return;
      }

      if (hasNoPendingRace && phase === "BlocoRegular") {
        await runConvocationWindow?.();
        return;
      }

      if (hasNoPendingRace && phase === "BlocoEspecial") {
        await finishSpecialBlock?.();
        return;
      }

      await advanceSeason?.();
    } catch (error) {
      console.error("Erro ao avançar temporada pelo header:", error);
    }
  }

  function getAdvanceButtonLabel() {
    if (isCalendarAdvancing || isAdvancing || isConvocating) {
      return "Avançando...";
    }

    if (canAdvanceCalendar) {
      return "Avançar calendário";
    }

    if (isFreeAgent && hasNoPendingRace) {
      return "Pular temporada";
    }

    if (hasNoPendingRace && phase === "BlocoRegular") {
      return "Avançar para convocação";
    }

    if (hasNoPendingRace && phase === "BlocoEspecial") {
      return "Pular bloco especial";
    }

    if (hasNoPendingRace && phase === "PosEspecial") {
      return "Encerrar temporada";
    }

    return "Avançar temporada";
  }

  function handleBackToBriefingOrigin() {
    closeRaceBriefing?.();
  }

  return (
    <header className="relative z-20 flex flex-col">
      <div className="shrink-0 px-3 py-2 sm:px-4 lg:px-5 xl:px-6">
        <div className="mx-auto flex w-full max-w-[1680px] items-center">
          <div className="flex min-w-0 flex-1 items-center gap-2">
            {!showRaceBriefing && (
              <>
                <span
                  className="h-3 w-3 shrink-0 rounded-full"
                  style={{ backgroundColor: playerTeam?.cor_primaria ?? "#58a6ff" }}
                />
                <span className="truncate text-xs font-bold uppercase tracking-[0.14em] text-text-primary">
                  {playerTeam?.nome ?? "-"}
                </span>
              </>
            )}
          </div>

          {showRaceBriefing ? (
            <div className="flex items-center gap-3">
              <span
                className="h-4 w-4 shrink-0 rounded-full"
                style={{ backgroundColor: playerTeam?.cor_primaria ?? "#58a6ff" }}
              />
              <span className="text-3xl font-bold tracking-[-0.035em] text-text-primary">
                {playerTeam?.nome ?? "-"}
              </span>
            </div>
          ) : (
            <TabNavigation activeTab={activeTab} onTabChange={onTabChange} />
          )}

          <div className="flex flex-1 justify-end">
            <div className="flex items-center gap-3 rounded-2xl border border-white/10 bg-white/[0.05] px-4 py-2 backdrop-blur-md">
              <div className="text-right">
                <p className="text-[10px] uppercase tracking-[0.18em] text-text-secondary">
                  Data {formatCompactDate(visibleDate)}
                </p>
                <p className="mt-1 text-xs font-semibold text-text-primary">
                  {formatNextRaceCountdown(visibleCountdown)}
                </p>
              </div>
              {showRaceBriefing ? (
                <GlassButton
                  variant="primary"
                  className="rounded-full px-5 py-2.5"
                  onClick={handleBackToBriefingOrigin}
                >
                  Voltar
                </GlassButton>
              ) : (
                <GlassButton
                  variant="primary"
                  disabled={isCalendarAdvancing || isAdvancing || isConvocating}
                  className="rounded-full px-5 py-2.5"
                  onClick={canAdvanceCalendar ? handleNextRace : handleAdvanceSeason}
                >
                  {getAdvanceButtonLabel()}
                </GlassButton>
              )}
            </div>
          </div>
        </div>
      </div>

      {activeTab === "standings" && !showRaceBriefing && (
        <div className="flex min-h-[110px] items-stretch h-[14vh]">
          <div className="mx-auto flex w-full max-w-[1680px] items-stretch px-3 sm:px-4 lg:px-5 xl:px-6">
            {nextRace ? (
              <div className="flex w-full items-center gap-6">
                <TrackImage
                  trackName={nextRace.track_name}
                  rodada={nextRace.rodada}
                  totalRodadas={season?.total_rodadas ?? "?"}
                />

                <div className="min-w-0 flex-1">
                  <p className="text-[11px] font-semibold uppercase tracking-[0.22em] text-accent-primary">
                    Próximo Evento
                  </p>
                  <h2 className="mt-1 truncate text-3xl font-bold tracking-[-0.02em] text-text-primary sm:text-4xl">
                    {nextRace.track_name}
                  </h2>
                  {nextRace.display_date && (
                    <div className="mt-1.5 text-xs text-text-muted">
                      {formatDate(nextRace.display_date)}
                    </div>
                  )}
                  <div className="mt-1 flex items-center gap-3 text-sm text-text-secondary">
                    <span className="flex items-center gap-1.5">
                      <span className="opacity-60">Horário</span>
                      {nextRace.horario} Local
                    </span>
                    <span className="opacity-30">·</span>
                    <span>{categoryLabel(playerTeam?.categoria)}</span>
                  </div>
                </div>

                <div className="shrink-0">
                  <StatBlock
                    label="Clima"
                    value={weatherWithTemp(nextRace.clima, nextRace.temperatura)}
                    icon={weatherIcon(nextRace.clima)}
                  />
                </div>
              </div>
            ) : hasNoPendingRace && playerTeam?.categoria ? (
              <SeasonFinishedBanner
                season={season}
                category={playerTeam.categoria}
                champion={seasonChampion}
              />
            ) : (
              <p className="text-sm text-text-muted">
                {season
                  ? isFreeAgent
                    ? `${formatSurfaceSeasonLabel(season)} - Sem equipe nesta temporada`
                    : `${formatSurfaceSeasonLabel(season)} - Sem corrida pendente`
                  : "Carregando..."}
              </p>
            )}
          </div>
        </div>
      )}
    </header>
  );
}

function SeasonFinishedBanner({ season, category, champion }) {
  const championName = champion?.nome ?? "Campeão definido";
  const seasonLabel = season ? formatSurfaceSeasonLabel(season) : "Fim de temporada";

  return (
    <div className="relative flex w-full items-center overflow-hidden rounded-[28px] border border-yellow-500/20 bg-[linear-gradient(135deg,rgba(24,17,5,0.96),rgba(9,15,26,0.95))] px-6 py-5 shadow-[0_18px_45px_rgba(0,0,0,0.28)]">
      <div className="absolute inset-y-0 left-0 w-44 bg-[radial-gradient(circle_at_left,rgba(250,204,21,0.20),transparent_72%)]" />
      <div className="absolute -right-8 top-1/2 h-28 w-28 -translate-y-1/2 rounded-full bg-yellow-300/10 blur-2xl" />

      <div className="relative flex min-w-0 flex-1 items-center gap-5">
        <div className="flex h-16 w-16 shrink-0 items-center justify-center rounded-2xl border border-yellow-400/30 bg-yellow-400/10 text-2xl font-black text-yellow-200 shadow-[0_0_24px_rgba(250,204,21,0.16)]">
          1
        </div>

        <div className="min-w-0">
          <p className="text-[11px] font-semibold uppercase tracking-[0.28em] text-yellow-300">
            Temporada Encerrada
          </p>
          <h2 className="mt-2 truncate text-3xl font-bold tracking-[-0.03em] text-text-primary sm:text-4xl">
            {championName}
          </h2>
          <p className="mt-2 text-sm text-yellow-50/80 sm:text-base">
            {seasonLabel} terminou com {championName} no topo da {categoryLabel(category)}.
          </p>
        </div>
      </div>
    </div>
  );
}

function TrackImage({ trackName, rodada, totalRodadas }) {
  const src = getTrackImageSrc(trackName);

  return (
    <div className="relative my-3 w-64 shrink-0 self-stretch overflow-hidden rounded-2xl border border-white/10 bg-white/5">
      <img
        src={src}
        alt={trackName}
        className="h-full w-full object-cover"
        onError={(event) => {
          event.currentTarget.style.display = "none";
        }}
      />
      <div className="absolute left-2 top-2 rounded border border-accent-primary/40 bg-[rgba(10,15,28,0.55)] px-2 py-0.5 backdrop-blur-[8px]">
        <span className="text-[10px] font-bold uppercase tracking-[0.14em] text-accent-primary">
          Corrida {rodada}/{totalRodadas}
        </span>
      </div>
    </div>
  );
}

const TRACK_IMAGE_FILES = [
  { match: ["charlotte"], file: "charlotte.png" },
  { match: ["laguna seca"], file: "lagunaseca.png" },
  { match: ["lime rock"], file: "limerock.jpeg" },
  { match: ["okayama"], file: "okayama.png" },
  { match: ["oulton park"], file: "oultonpark.jpeg" },
  { match: ["snetterton"], file: "snetterton.jpeg" },
  { match: ["summit point", "jefferson"], file: "summitpoint.png" },
  { match: ["tsukuba"], file: "Tsukuba.png" },
  { match: ["virginia international raceway", "vir full", "vir patriot"], file: "virginia.jpeg" },
  { match: ["ledenon"], file: "ledenon.png" },
  { match: ["oschersleben", "motorsport arena"], file: "motorsport arena.png" },
  { match: ["navarra"], file: "Navarra.png" },
  { match: ["oran park"], file: "oranpark.png" },
  { match: ["rudskogen"], file: "rudskogen.jpeg" },
  { match: ["winton"], file: "winton.jpeg" },
];

function getTrackImageSrc(trackName) {
  const normalizedName = normalizeTrackName(trackName);
  const entry = TRACK_IMAGE_FILES.find(({ match }) =>
    match.some((candidate) => normalizedName.includes(candidate)),
  );

  if (entry) {
    return `/tracks/${encodeURIComponent(entry.file)}`;
  }

  return `/tracks/${encodeURIComponent(trackName)}.png`;
}

function normalizeTrackName(trackName) {
  return (trackName ?? "")
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();
}

function StatBlock({ label, value, icon }) {
  return (
    <div className="text-right">
      <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">{label}</p>
      <p className="mt-0.5 flex items-center justify-end gap-1 text-sm font-semibold text-text-secondary">
        {icon && <span>{icon}</span>}
        {value}
      </p>
    </div>
  );
}

function weatherWithTemp(clima, temperatura) {
  const label = weatherLabel(clima);
  if (temperatura == null) return label;
  return `${Math.round(temperatura)}° ${label}`;
}

function weatherLabel(value) {
  if (value === "HeavyRain") return "Chuva Forte";
  if (value === "Wet") return "Chuva";
  if (value === "Damp") return "Úmido";
  return "Parcialmente Nublado";
}

function weatherIcon(value) {
  if (value === "HeavyRain") return "Chuva";
  if (value === "Wet") return "Garoa";
  if (value === "Damp") return "Úmido";
  return "Sol";
}

export default Header;
