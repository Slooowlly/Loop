import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import useCareerStore from "../../stores/useCareerStore";
import useExitToMenu from "../../hooks/useExitToMenu";
import LeaveToMenuModal from "./LeaveToMenuModal";
import {
  categoryLabel,
  formatCompactDate,
  formatNextRaceCountdown,
  formatSurfaceSeasonLabel,
} from "../../utils/formatters";
import { isLegacySeasonPhase } from "../../utils/seasonPhases";
import GlassButton from "../ui/GlassButton";
import FlagIcon from "../ui/FlagIcon";
import TeamLogoMark from "../team/TeamLogoMark";
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

  // Clicar no chip da equipe abre direto a pergunta de sair (salvando ou não).
  const { isSaving, exit, saveAndExit } = useExitToMenu();
  const [leaveConfirm, setLeaveConfirm] = useState(false);

  const visibleDate = calendarDisplayDate ?? temporalSummary?.current_display_date;
  const visibleCountdown = displayDaysUntilNextEvent ?? temporalSummary?.days_until_next_event;
  const hasNoPendingRace = !nextRace;
  const isFreeAgent = !playerTeam;
  const phase = season?.fase;
  const isLegacyPhase = isLegacySeasonPhase(phase);
  const hasPendingLegacyRegularRaces =
    isLegacyPhase && phase === "BlocoRegular" && (temporalSummary?.pending_in_phase ?? 0) > 0;
  const canAdvanceCalendar = Boolean(nextRace) || (
    !isFreeAgent &&
    hasPendingLegacyRegularRaces
  );
  // No Home (standings), com uma corrida marcada, o botão "Avançar calendário"
  // vive DENTRO do banner cinematográfico — então escondemos o duplicado da barra
  // superior (deixando só o cartão de data à direita). Nos demais casos/abas, o
  // botão da barra continua sendo o controle universal.
  const bannerOwnsAdvance =
    activeTab === "standings" && !showRaceBriefing && Boolean(nextRace);

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
    // Leva o jogador para o Calendário (com fade) para ele ver a animação dos
    // dias passando — MAS só quando há dias a passar. Se a corrida é HOJE, avançar
    // abre direto a sala de estratégia; piscar o calendário antes seria ruim.
    const daysUntilRace = Number(visibleCountdown);
    if (Number.isFinite(daysUntilRace) && daysUntilRace > 0) {
      onTabChange?.("calendar");
    }
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

      // LEGADO 9D: convocação e bloco especial só existem para saves pré-v33 em voo.
      if (isLegacyPhase && hasNoPendingRace && phase === "BlocoRegular") {
        await runConvocationWindow?.();
        return;
      }

      if (isLegacyPhase && hasNoPendingRace && phase === "BlocoEspecial") {
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

    // LEGADO 9D: estes labels só aparecem em saves pré-v33 em voo.
    if (isLegacyPhase && hasNoPendingRace && phase === "BlocoRegular") {
      return "Avançar para convocação";
    }

    if (isLegacyPhase && hasNoPendingRace && phase === "BlocoEspecial") {
      return "Pular bloco especial";
    }

    if (isLegacyPhase && hasNoPendingRace && phase === "PosEspecial") {
      return "Encerrar temporada";
    }

    if (phase === "Encerramento" || (hasNoPendingRace && phase === "Temporada")) {
      return "Avançar para pré-temporada";
    }

    if (phase === "PreTemporada") {
      return "Abrir mercado";
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
              <button
                type="button"
                onClick={() => setLeaveConfirm(true)}
                className="flex items-center gap-2 rounded-xl px-1.5 py-1 transition-colors hover:bg-white/8"
                title="Sair para o menu principal"
              >
                <TeamLogoMark
                  teamName={playerTeam?.nome}
                  color={playerTeam?.cor_primaria ?? "#58a6ff"}
                  size="sm"
                  testId="header-team-logo"
                />
                <span className="truncate text-xs font-bold uppercase tracking-[0.14em] text-text-primary">
                  {playerTeam?.nome ?? "-"}
                </span>
              </button>
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
              ) : bannerOwnsAdvance ? null : (
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
        nextRace ? (
          <NextRaceBanner
            nextRace={nextRace}
            season={season}
            playerTeam={playerTeam}
            countdownDays={visibleCountdown}
            onAdvance={handleNextRace}
            advanceLabel={getAdvanceButtonLabel()}
            advanceDisabled={isCalendarAdvancing || isAdvancing || isConvocating}
          />
        ) : (
          <div className="flex min-h-[110px] items-stretch h-[14vh]">
            <div className="mx-auto flex w-full max-w-[1680px] items-stretch px-3 sm:px-4 lg:px-5 xl:px-6">
              {hasNoPendingRace && playerTeam?.categoria ? (
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
        )
      )}

      <LeaveToMenuModal
        open={leaveConfirm}
        isSaving={isSaving}
        onSaveAndExit={saveAndExit}
        onExitWithoutSave={exit}
        onCancel={() => setLeaveConfirm(false)}
      />
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

// Banner cinematográfico da próxima corrida (Home). Protagonista da tela: imagem
// ampla do circuito ao fundo, gradientes escuros para legibilidade, dados 100%
// dinâmicos (mesmas variáveis do bloco antigo) e o botão "Avançar calendário"
// reaproveitando o handler existente (onAdvance = handleNextRace).
function NextRaceBanner({
  nextRace,
  season,
  playerTeam,
  countdownDays,
  onAdvance,
  advanceLabel,
  advanceDisabled,
}) {
  const trackName = nextRace.track_name;
  const totalRodadas = season?.total_rodadas ?? null;
  const championship = categoryLabel(playerTeam?.categoria);
  const country = trackCountry(trackName);
  const bannerDate = formatBannerDate(nextRace.display_date);
  const hasWeather = nextRace.clima != null || nextRace.temperatura != null;
  const countdown = compactCountdown(countdownDays);

  return (
    // Padding POR FORA do container de 1680px (igual à barra de nav e ao <main>),
    // para o card do banner ficar exatamente da mesma largura dos cards de baixo
    // ("GT3 Championship"/"Construtores").
    <div className="px-3 pb-1 pt-0.5 sm:px-4 lg:px-5 xl:px-6">
      <div className="mx-auto w-full max-w-[1680px]">
        <div className="group relative overflow-hidden rounded-[28px] border border-accent-primary/20 bg-[#03060f] shadow-[0_18px_45px_rgba(0,0,0,0.38)] min-h-[196px] md:h-[clamp(200px,21vh,230px)]">
        {/* Base decorativa (sempre atrás da imagem): quando a pista não tem foto,
            este fundo azul-marinho premium aparece no lugar de um preto vazio. */}
        <div
          className="pointer-events-none absolute inset-0"
          style={{
            background:
              "radial-gradient(120% 130% at 82% 0%, rgba(88,166,255,0.12), transparent 55%), linear-gradient(135deg, #0a1424 0%, #03060f 70%)",
          }}
        />
        {/* Imagem do circuito (fade-in ao carregar; cai na base acima se faltar). */}
        <BannerTrackImage trackName={trackName} />

        {/* Camada 1 — gradiente horizontal: escuro só à ESQUERDA (atrás do texto),
            transparente na metade direita para a foto aparecer nítida. */}
        <div
          className="pointer-events-none absolute inset-0"
          style={{
            background:
              "linear-gradient(90deg, rgba(3,8,18,0.95) 0%, rgba(3,8,18,0.78) 30%, rgba(3,8,18,0.32) 58%, rgba(3,8,18,0) 82%)",
          }}
        />
        {/* Camada 2 — gradiente vertical suave: só assenta a base (linha de infos). */}
        <div
          className="pointer-events-none absolute inset-0"
          style={{
            background:
              "linear-gradient(0deg, rgba(3,8,18,0.72) 0%, rgba(3,8,18,0) 56%)",
          }}
        />
        {/* Brilho decorativo discreto (canto superior direito) — sem mapa de pista
            para não arriscar um contorno incorreto. */}
        <div className="pointer-events-none absolute -right-16 -top-20 h-56 w-56 rounded-full bg-[radial-gradient(circle,rgba(88,166,255,0.14),transparent_68%)]" />

        {/* Conteúdo */}
        <div className="nrb-rise relative z-10 flex h-full flex-col justify-end gap-4 p-5 sm:p-6 md:flex-row md:items-end md:justify-between md:p-7">
          <div className="flex min-w-0 max-w-full flex-col md:max-w-[62%]">
            <p className="text-[11px] font-bold uppercase tracking-[0.28em] text-accent-primary">
              Corrida {nextRace.rodada}
              {totalRodadas ? ` de ${totalRodadas}` : ""}
            </p>

            <h2
              className="mt-2 break-words text-[clamp(1.9rem,4vw,2.9rem)] font-extrabold uppercase leading-[1.02] tracking-[-0.01em] text-white [text-shadow:0_2px_18px_rgba(0,0,0,0.55)]"
              title={trackName}
            >
              {trackName}
            </h2>

            <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-sm">
              {country && (
                <>
                  <span className="flex items-center gap-2">
                    <FlagIcon nacionalidade={country} />
                    <span className="font-semibold uppercase tracking-[0.06em] text-text-primary">
                      {country}
                    </span>
                  </span>
                  <span className="text-white/25">•</span>
                </>
              )}
              <span className="font-semibold uppercase tracking-[0.06em] text-accent-primary">
                {championship}
              </span>
            </div>

            <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-2 text-[13px] font-medium text-text-secondary">
              {bannerDate && (
                <span className="flex items-center gap-1.5">
                  <CalendarGlyph />
                  <span className="text-text-primary/90">{bannerDate}</span>
                </span>
              )}
              {nextRace.horario && (
                <span className="flex items-center gap-1.5">
                  <ClockGlyph />
                  <span className="text-text-primary/90">{nextRace.horario} LOCAL</span>
                </span>
              )}
              {hasWeather && (
                <span className="flex items-center gap-1.5">
                  <span className="sr-only">Clima</span>
                  <span aria-hidden="true" className="text-base leading-none">
                    {weatherEmoji(nextRace.clima)}
                  </span>
                  {nextRace.temperatura != null && (
                    <span className="text-text-primary/90">
                      {Math.round(nextRace.temperatura)}°C
                    </span>
                  )}
                  <span className="hidden text-text-secondary sm:inline">
                    {weatherLabel(nextRace.clima)}
                  </span>
                </span>
              )}
              {countdown && (
                <span className="inline-flex items-center gap-1.5 rounded-full border border-accent-primary/30 bg-accent-primary/10 px-2.5 py-1 text-[11px] font-bold uppercase tracking-[0.12em] text-accent-primary">
                  {countdown}
                </span>
              )}
            </div>
          </div>

          {/* Ações — botão principal reaproveita o handler de avanço do calendário. */}
          <div className="flex shrink-0 items-center justify-end md:pb-1">
            <button
              type="button"
              onClick={onAdvance}
              disabled={advanceDisabled}
              className="group/btn inline-flex h-12 items-center gap-2 rounded-xl bg-accent-primary px-6 text-sm font-black uppercase tracking-[0.04em] text-[#03060f] shadow-[0_0_22px_rgba(88,166,255,0.4)] transition hover:bg-accent-hover hover:shadow-[0_0_28px_rgba(88,166,255,0.55)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-primary active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-70 sm:h-[52px] sm:px-8 sm:text-base"
            >
              <span>{advanceLabel}</span>
              <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2.6"
                strokeLinecap="round"
                strokeLinejoin="round"
                className="h-4 w-4 transition-transform group-hover/btn:translate-x-0.5"
                aria-hidden="true"
              >
                <path d="M5 12h14" />
                <path d="m13 6 6 6-6 6" />
              </svg>
            </button>
          </div>
        </div>
        </div>
      </div>
    </div>
  );
}

// Imagem de fundo do banner: fundo escuro enquanto carrega, fade-in + zoom lento
// ao terminar, e some (mantendo o gradiente) caso o arquivo não exista. O alt e a
// origem seguem a mesma resolução usada antes (getTrackImageSrc), por isso o teste
// de mapeamento de miniaturas continua válido.
function BannerTrackImage({ trackName }) {
  const [loaded, setLoaded] = useState(false);
  const [failed, setFailed] = useState(false);

  return (
    <img
      src={getBannerImageSrc(trackName)}
      alt={trackName}
      loading="lazy"
      onLoad={() => setLoaded(true)}
      onError={() => setFailed(true)}
      className={[
        "absolute inset-0 h-full w-full object-cover object-[center_38%] transition-opacity duration-700",
        "[filter:saturate(0.96)_contrast(1.05)_brightness(1.02)]",
        loaded && !failed ? "opacity-100 nrb-zoom" : "opacity-0",
      ].join(" ")}
    />
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
    return `/utilities/tracks/${encodeURIComponent(entry.file)}`;
  }

  return `/utilities/tracks/${encodeURIComponent(trackName)}.png`;
}

// Imagens LARGAS/cinematográficas do banner do Home (pasta "Pistas Header").
// Mapa próprio (não a miniatura da tabela): resolve o nome da pista → arquivo
// panorâmico. Se a pista não tiver imagem larga, cai na miniatura de sempre
// (getTrackImageSrc), e o onError do banner segura o visual só com o gradiente.
// Os nomes de arquivo refletem exatamente o que está em disco (inclusive grafias
// como "chartlotte"/"outonpark"/"rudsoken").
const BANNER_IMAGE_DIR = "/utilities/tracks/Pistas%20Header";
const BANNER_IMAGE_FILES = [
  { match: ["algarve", "portimao"], file: "Algarve International Circuit.jpg" },
  { match: ["hermanos rodriguez", "rodriguez", "mexico city"], file: "Autódromo Hermanos Rodriguez.jpg" },
  { match: ["jose carlos pace", "interlagos"], file: "Autódromo José Carlos Pace.jpg" },
  { match: ["monza"], file: "Autódromo Nazionale Monza.jpg" },
  { match: ["brands hatch"], file: "Brands Hatch.jpg" },
  { match: ["cadwell"], file: "Cadwell Park Circuit.jpg" },
  { match: ["canadian tire", "mosport"], file: "Canadian Tire Motorsport Park.jpg" },
  { match: ["zandvoort"], file: "Circuit Park Zandvoort.jpg" },
  { match: ["barcelona", "catalunya"], file: "Circuit de Barcelona-Catalunya.jpg" },
  { match: ["magny-cours", "magny cours"], file: "Circuit de Nevers Magny-Cours.jpg" },
  { match: ["francorchamps", "spa-francorchamps"], file: "Circuit de Spa-Francorchamps.jpg" },
  { match: ["le mans", "24 heures", "sarthe", "24 hours"], file: "Circuit des 24 Heures du Mans.jpg" },
  { match: ["circuit of the americas", "cota", "of the americas"], file: "Circuit of the Americas.jpg" },
  { match: ["melbourne", "albert park"], file: "Circuito de Melbourne.jpg" },
  { match: ["detroit", "belle isle"], file: "Detroit Grand Prix at Belle Isle.jpg" },
  { match: ["donington"], file: "Donington Park Racing Circuit.jpg" },
  { match: ["fuji"], file: "Fuji International Speedway.jpg" },
  { match: ["hockenheim"], file: "HockenheimRing.jpg" },
  { match: ["hungaroring", "hungar"], file: "Hungaroring.jpg" },
  { match: ["long beach"], file: "Long Beach Street Circuit.jpg" },
  { match: ["road atlanta"], file: "Michelin Raceway Road Atlanta.jpg" },
  { match: ["mid-ohio", "mid ohio"], file: "Mid-Ohio Sports Car Course.jpg" },
  { match: ["misano"], file: "Misano World Circuit Marco Simoncelli.jpg" },
  { match: ["mount panorama", "bathurst"], file: "Mount Panorama Circuit.jpg" },
  { match: ["nurburgring", "nordschleife"], file: "Nürburgring Nordschleife.jpg" },
  { match: ["red bull ring", "spielberg"], file: "Red Bull Ring – Spielberg.jpg" },
  { match: ["sandown"], file: "Sandown International Motor Raceway.jpg" },
  { match: ["summit point", "jefferson"], file: "Summit Point — Raceway.jpg" },
  { match: ["thruxton"], file: "Thruxton Circuit.jpg" },
  { match: ["virginia", "vir full", "vir patriot"], file: "Virginia Int. Raceway.jpg" },
  { match: ["watkins"], file: "Watkins Glen International.jpg" },
  { match: ["mugello"], file: "autodromo Internazionale del Mugello.jpg" },
  { match: ["charlotte"], file: "chartlotte.jpg" },
  { match: ["daytona"], file: "daytona.jpg" },
  { match: ["laguna seca", "laguna"], file: "laguna seca.jpg" },
  { match: ["ledenon"], file: "ledenon.jpg" },
  { match: ["lime rock", "limerock"], file: "limerockpark.jpg" },
  { match: ["motorsport arena", "oschersleben"], file: "motorsportarena.jpg" },
  { match: ["navarra"], file: "navarra_panorama_1915x821.jpg" },
  { match: ["okayama"], file: "okayama.jpg" },
  { match: ["oran park"], file: "oran park.jpg" },
  { match: ["oulton park", "oulton"], file: "outonpark.jpg" },
  { match: ["road america"], file: "road america.jpg" },
  { match: ["rudskogen"], file: "rudsoken.jpg" },
  { match: ["sebring"], file: "sebring.jpg" },
  { match: ["snetterton"], file: "snetterton.jpg" },
  { match: ["sonoma", "sears point", "infineon"], file: "sonoma.jpg" },
  { match: ["tsukuba"], file: "tsukuba.jpg" },
  { match: ["winton"], file: "winton.jpg" },
  { match: ["suzuka"], file: "Suzuka International Racing Course.jpg" },
  { match: ["silverstone"], file: "Silverstone Circuit.jpg" },
  { match: ["philip island", "phillip island"], file: "Philip Island Grand Prix Circuit.jpg" },
  { match: ["zolder"], file: "Circuit Zolder.jpg" },
  // Arte adicionada recentemente (arquivos .png em disco).
  { match: ["adelaide"], file: "Adelaide Street Circuit.png" },
  { match: ["enzo e dino", "imola"], file: "Autódromo Internazionale Enzo e Dino Ferrari.png" },
  { match: ["barber"], file: "Barber Motorsports Park.png" },
  { match: ["chicago"], file: "Chicago Street Course.png" },
  { match: ["gilles villeneuve", "montreal"], file: "Circuit Gilles Villeneuve.png" },
  { match: ["jerez"], file: "Circuito de Jerez.png" },
  { match: ["indianapolis", "indy road"], file: "Indianapolis Motor Speedway.png" },
  { match: ["knockhill"], file: "Knockhill.png" },
  { match: ["miami"], file: "Miami.png" },
  // Pistas AINDA sem arte na pasta: o mapa já espera estes arquivos.
  // Basta soltar um arquivo com EXATAMENTE este nome em "Pistas Header/" que o banner
  // passa a usá-lo. Enquanto não existir, cai no fundo premium (fallback).
  { match: ["motegi", "mobility resort"], file: "Mobility Resort Motegi.png" },
  { match: ["aragon", "motorland"], file: "MotorLand Aragon.png" },
  { match: ["portland"], file: "Portland International Raceway.png" },
  { match: ["qualcomm", "coronado", "naval base"], file: "Qualcomm Circuit.png" },
  { match: ["sachsenring"], file: "Sachsenring.png" },
  { match: ["the bend", "shell v-power"], file: "The Bend Motorsport Park.png" },
  { match: ["petersburg", "st. pete", "st pete"], file: "St Petersburg Grand Prix.png" },
  { match: ["willow springs"], file: "Willow Springs International Raceway.png" },
];

function getBannerImageSrc(trackName) {
  const normalizedName = normalizeTrackName(trackName);
  const entry = BANNER_IMAGE_FILES.find(({ match }) =>
    match.some((candidate) => normalizedName.includes(candidate)),
  );

  if (entry) {
    return `${BANNER_IMAGE_DIR}/${encodeURIComponent(entry.file)}`;
  }

  // Sem imagem larga → usa a miniatura de sempre como fallback.
  return getTrackImageSrc(trackName);
}

function normalizeTrackName(trackName) {
  return (trackName ?? "")
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();
}

function weatherLabel(value) {
  if (value === "HeavyRain") return "Chuva Forte";
  if (value === "Wet") return "Chuva";
  if (value === "Damp") return "Úmido";
  return "Parcialmente Nublado";
}

function weatherEmoji(value) {
  if (value === "HeavyRain") return "\u{26C8}\u{FE0F}"; // ⛈️
  if (value === "Wet") return "\u{1F327}\u{FE0F}"; // 🌧️
  if (value === "Damp") return "\u{1F326}\u{FE0F}"; // 🌦️
  return "\u{26C5}"; // ⛅
}

// País de cada circuito, derivado do nome (metadado de referência, no espírito de
// TRACK_IMAGE_FILES). A bandeira/país só aparece quando o circuito é reconhecido —
// nunca uma bandeira errada. A label é passada crua ao FlagIcon, que já normaliza
// "Japão"→jp, "Estados Unidos"→us, etc.
const TRACK_COUNTRY = [
  {
    match: [
      "charlotte",
      "laguna seca",
      "lime rock",
      "limerock",
      "summit point",
      "jefferson",
      "virginia",
      "vir full",
      "vir patriot",
      "watkins",
      "road america",
      "road atlanta",
      "mid-ohio",
      "mid ohio",
      "sebring",
      "daytona",
      "sonoma",
      "sears point",
      "long beach",
      "detroit",
      "belle isle",
      "circuit of the americas",
      "cota",
      "indianapolis",
    ],
    country: "Estados Unidos",
  },
  { match: ["okayama", "tsukuba", "suzuka", "motegi", "fuji"], country: "Japão" },
  {
    match: [
      "oulton park",
      "oulton",
      "snetterton",
      "silverstone",
      "brands hatch",
      "cadwell",
      "donington",
      "thruxton",
    ],
    country: "Reino Unido",
  },
  {
    match: ["ledenon", "magny-cours", "magny cours", "le mans", "24 heures", "paul ricard"],
    country: "França",
  },
  {
    match: [
      "oschersleben",
      "motorsport arena",
      "nurburgring",
      "nordschleife",
      "hockenheim",
      "sachsenring",
    ],
    country: "Alemanha",
  },
  { match: ["navarra", "barcelona", "catalunya", "jerez", "valencia", "aragon"], country: "Espanha" },
  { match: ["algarve", "portimao"], country: "Portugal" },
  {
    match: ["oran park", "winton", "bathurst", "mount panorama", "phillip island", "sandown", "melbourne", "albert park"],
    country: "Austrália",
  },
  { match: ["rudskogen"], country: "Noruega" },
  { match: ["zolder", "spa-francorchamps", "francorchamps"], country: "Bélgica" },
  { match: ["monza", "imola", "mugello", "misano", "vallelunga"], country: "Itália" },
  { match: ["interlagos", "jose carlos pace", "velocitta", "goiania"], country: "Brasil" },
  { match: ["hermanos rodriguez", "rodriguez", "mexico city"], country: "México" },
  { match: ["canadian tire", "mosport"], country: "Canadá" },
  { match: ["hungaroring", "hungar"], country: "Hungria" },
  { match: ["red bull ring", "spielberg"], country: "Áustria" },
  { match: ["zandvoort"], country: "Holanda" },
];

function trackCountry(trackName) {
  const normalized = normalizeTrackName(trackName);
  if (!normalized) return null;
  const entry = TRACK_COUNTRY.find(({ match }) =>
    match.some((candidate) => normalized.includes(candidate)),
  );
  return entry?.country ?? null;
}

const BANNER_MONTHS_PT = [
  "JAN",
  "FEV",
  "MAR",
  "ABR",
  "MAI",
  "JUN",
  "JUL",
  "AGO",
  "SET",
  "OUT",
  "NOV",
  "DEZ",
];

// Data compacta em caixa alta ("06 FEV 2042"), lida direto da string YYYY-MM-DD
// para evitar deslocamento de fuso do `new Date`.
function formatBannerDate(displayDate) {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(displayDate ?? "");
  if (!match) return null;
  const [, year, month, day] = match;
  const monthLabel = BANNER_MONTHS_PT[Number(month) - 1] ?? month;
  return `${day} ${monthLabel} ${year}`;
}

// Contagem regressiva curtíssima ("EM 7 DIAS") a partir dos mesmos dados do header.
function compactCountdown(days) {
  if (days == null) return null;
  const value = Number(days);
  if (!Number.isFinite(value)) return null;
  if (value <= 0) return "HOJE";
  if (value === 1) return "AMANHÃ";
  if (value <= 27) return `EM ${value} DIAS`;
  if (value < 56) {
    const weeks = Math.max(1, Math.floor(value / 7));
    return weeks === 1 ? "EM 1 SEMANA" : `EM ${weeks} SEMANAS`;
  }
  const months = Math.max(2, Math.floor(value / 28));
  return `EM ${months} MESES`;
}

function CalendarGlyph() {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      className="h-4 w-4 text-accent-primary"
      aria-hidden="true"
    >
      <rect x="3" y="4.5" width="18" height="16" rx="2.5" />
      <path d="M3 9h18M8 2.5v4M16 2.5v4" />
    </svg>
  );
}

function ClockGlyph() {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      className="h-4 w-4 text-accent-primary"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7.5V12l3 2" />
    </svg>
  );
}

export default Header;
