import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import GlassButton from "../../components/ui/GlassButton";
import GlassCard from "../../components/ui/GlassCard";
import LoadingOverlay from "../../components/ui/LoadingOverlay";
import TeamLogoMark from "../../components/team/TeamLogoMark";
import useCareerStore from "../../stores/useCareerStore";
import { buildFavoriteExpectationSelection, recentResults } from "./nextRaceBriefing";
import {
  buildEditorialCopy,
  classifyChampionshipState,
  classifyWeekendState,
} from "./nextRaceEditorial";

function getDisplayError(error, fallback) {
  if (typeof error === "string") {
    return error;
  }

  if (typeof error?.message === "string" && error.message.trim()) {
    return error.message;
  }

  const rendered = error?.toString?.();
  if (typeof rendered === "string" && rendered.trim() && rendered !== "[object Object]") {
    return rendered;
  }

  return fallback;
}

function NextRaceTab() {
  const [error, setError] = useState("");
  const [exportNotice, setExportNotice] = useState("");
  const [hasExistingPreseason, setHasExistingPreseason] = useState(false);
  const [driverStandings, setDriverStandings] = useState([]);
  const [teamStandings, setTeamStandings] = useState([]);
  const [briefingPhraseHistory, setBriefingPhraseHistory] = useState({ season_number: 0, entries: [] });
  const [isLoadingBriefing, setIsLoadingBriefing] = useState(true);
  const [briefingError, setBriefingError] = useState("");

  const player = useCareerStore((state) => state.player);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const nextRace = useCareerStore((state) => state.nextRace);
  const nextRaceBriefing = useCareerStore((state) => state.nextRaceBriefing);
  const temporalSummary = useCareerStore((state) => state.temporalSummary);
  const season = useCareerStore((state) => state.season);
  const isSimulating = useCareerStore((state) => state.isSimulating);
  const isAdvancing = useCareerStore((state) => state.isAdvancing);
  const careerId = useCareerStore((state) => state.careerId);
  const simulateRace = useCareerStore((state) => state.simulateRace);
  const advanceSeason = useCareerStore((state) => state.advanceSeason);
  const skipAllPendingRaces = useCareerStore((state) => state.skipAllPendingRaces);
  const enterPreseason = useCareerStore((state) => state.enterPreseason);
  const runConvocationWindow = useCareerStore((state) => state.runConvocationWindow);
  const finishSpecialBlock = useCareerStore((state) => state.finishSpecialBlock);
  const startCalendarAdvance = useCareerStore((state) => state.startCalendarAdvance);
  const isConvocating = useCareerStore((state) => state.isConvocating);
  const isEnteringPreseason = useCareerStore((state) => state.isEnteringPreseason);
  const hasPendingRegularRaces =
    season?.fase === "BlocoRegular" && (temporalSummary?.pending_in_phase ?? 0) > 0;

  useEffect(() => {
    let active = true;

    async function detectPreseason() {
      if (!careerId || nextRace) return;

      try {
        await invoke("get_preseason_state", { careerId });
        if (active) {
          setHasExistingPreseason(true);
          await enterPreseason();
        }
      } catch (_error) {
        if (active) {
          setHasExistingPreseason(false);
        }
      }
    }

    detectPreseason();

    return () => {
      active = false;
    };
  }, [careerId, enterPreseason, nextRace]);

  useEffect(() => {
    let active = true;

    async function loadBriefingContext() {
      if (!careerId || !nextRace || !playerTeam?.categoria) {
        if (active) {
          setDriverStandings([]);
          setTeamStandings([]);
          setBriefingPhraseHistory({ season_number: 0, entries: [] });
          setIsLoadingBriefing(false);
        }
        return;
      }

      setIsLoadingBriefing(true);
      setBriefingError("");

      try {
        const [drivers, teams, phraseHistory] = await Promise.all([
          invoke("get_drivers_by_category", {
            careerId,
            category: playerTeam.categoria,
          }),
          invoke("get_teams_standings", {
            careerId,
            category: playerTeam.categoria,
          }),
          invoke("get_briefing_phrase_history", {
            careerId,
          }).catch(() => ({ season_number: 0, entries: [] })),
        ]);

        if (!active) return;

        setDriverStandings(Array.isArray(drivers) ? drivers : []);
        setTeamStandings(Array.isArray(teams) ? teams : []);
        setBriefingPhraseHistory(
          phraseHistory && Array.isArray(phraseHistory.entries)
            ? phraseHistory
            : { season_number: 0, entries: [] },
        );
      } catch (invokeError) {
        if (!active) return;

        setBriefingError(
          typeof invokeError === "string"
            ? invokeError
            : invokeError?.toString?.() ?? "Não foi possível montar o briefing.",
        );
      } finally {
        if (active) {
          setIsLoadingBriefing(false);
        }
      }
    }

    loadBriefingContext();

    return () => {
      active = false;
    };
  }, [careerId, nextRace, playerTeam?.categoria]);

  const briefing = useMemo(
    () =>
      buildBriefingContext({
        player,
        playerTeam,
        season,
        nextRace,
        nextRaceBriefing,
        driverStandings,
        teamStandings,
        briefingPhraseHistory,
      }),
    [
      player,
      playerTeam,
      season,
      nextRace,
      nextRaceBriefing,
      driverStandings,
      teamStandings,
      briefingPhraseHistory,
    ],
  );

  useEffect(() => {
    let active = true;

    async function persistBriefingPhrases() {
      if (!careerId || !season?.numero || !nextRace?.rodada || briefing.favorites.length === 0) {
        return;
      }

      const entries = briefing.favorites
        .map((driver) => ({
          round_number: nextRace.rodada,
          driver_id: driver.id,
          bucket_key: driver.expectationBucketKey,
          phrase_id: driver.expectationPhraseId,
        }))
        .filter((entry) => entry.bucket_key && entry.phrase_id);

      if (entries.length === 0) {
        return;
      }

      const allPersisted = entries.every((entry) =>
        briefingPhraseHistory.entries.some(
          (saved) =>
            saved.season_number === season.numero &&
            saved.round_number === entry.round_number &&
            saved.driver_id === entry.driver_id &&
            saved.bucket_key === entry.bucket_key &&
            saved.phrase_id === entry.phrase_id,
        ),
      );

      if (allPersisted) {
        return;
      }

      try {
        const updatedHistory = await invoke("save_briefing_phrase_history", {
          careerId,
          seasonNumber: season.numero,
          entries,
        });

        if (!active) return;
        if (updatedHistory && Array.isArray(updatedHistory.entries)) {
          setBriefingPhraseHistory(updatedHistory);
        }
      } catch (_error) {
        // Silencioso: a variação recente melhora a imersão, mas não deve quebrar o briefing.
      }
    }

    persistBriefingPhrases();

    return () => {
      active = false;
    };
  }, [
    briefing.favorites,
    briefingPhraseHistory.entries,
    careerId,
    nextRace?.rodada,
    season?.numero,
  ]);

  async function handleSimulate() {
    setError("");
    setExportNotice("");

    try {
      await simulateRace();
    } catch (invokeError) {
      setError(getDisplayError(invokeError, "Não foi possível simular a corrida."));
    }
  }

  async function handleSeasonAdvance() {
    setError("");
    setExportNotice("");

    try {
      if (season?.fase === "BlocoEspecial") {
        await finishSpecialBlock();
        return;
      }

      if (!nextRace && hasPendingRegularRaces) {
        await startCalendarAdvance();
        return;
      }

      if (!nextRace && season?.fase === "BlocoRegular") {
        await runConvocationWindow();
        return;
      }

      if (!nextRace && season?.fase === "PosEspecial") {
        await advanceSeason();
        return;
      }

      if (hasExistingPreseason) {
        await enterPreseason();
        return;
      }

      await advanceSeason();
    } catch (invokeError) {
      setError(getDisplayError(invokeError, "Não foi possível avançar para a pré-temporada."));
    }
  }

  function handleExport() {
    setExportNotice("Exportação para o iRacing chega em breve.");
  }

  if (!nextRace) {
    const isFreeAgent = !playerTeam;
    return (
      <div className="relative">
        <LoadingOverlay
          open={isAdvancing || isConvocating || isEnteringPreseason}
          title={
            isEnteringPreseason
              ? "Abrindo mercado de transferências"
              : isFreeAgent
              ? "Pulando temporada"
              : season?.fase === "BlocoEspecial"
              ? "Simulando bloco especial"
              : season?.fase === "BlocoRegular"
              ? "Abrindo convocação"
              : "Virando a temporada"
          }
          message={
            isEnteringPreseason
              ? "Carregando equipes, propostas e pilotos disponíveis."
              : isFreeAgent
              ? "Simulando todas as corridas da temporada sem sua participação."
              : season?.fase === "BlocoEspecial"
              ? "As corridas especiais restantes estão sendo resolvidas em lote para avançar o calendário."
              : season?.fase === "BlocoRegular"
              ? "A janela especial está sendo aberta sem passar pelo mercado normal."
              : "Evolução, aposentadorias, promoções e preparação da pré-temporada em andamento."
          }
        />

        <GlassCard hover={false} className="rounded-[28px] p-10">
          <div className="py-6 text-center">
            <div className="text-6xl">{isFreeAgent ? "🏳️" : "PQ"}</div>
            <p className="mt-4 text-sm uppercase tracking-[0.22em] text-accent-primary">
              {isFreeAgent ? "Agente livre" : "Próxima corrida"}
            </p>
            <h2 className="mt-3 text-3xl font-semibold text-text-primary">
              {isFreeAgent
                ? "Sem equipe nesta temporada"
                : season?.fase === "BlocoEspecial"
                ? "Bloco especial em andamento"
                : season?.fase === "PosEspecial"
                ? "Especial finalizado"
                : "Temporada finalizada"}
            </h2>
            <p className="mt-3 text-sm text-text-secondary">
              {isFreeAgent
                ? "Você não tem equipe nesta temporada. Pule para a próxima pré-temporada e tente o mercado novamente."
                : season?.fase === "BlocoEspecial"
                ? "Você ficou fora das categorias especiais. Use este atalho para simular o restante do bloco e avançar o calendário."
                : season?.fase === "BlocoRegular"
                ? hasPendingRegularRaces
                  ? "Sua categoria já fechou o campeonato, mas ainda há corridas regulares acontecendo no calendário."
                  : "Sua temporada regular terminou. Agora você pode analisar notícias e resultados com calma, e só abrir a janela de convocação quando quiser."
                : season?.fase === "PosEspecial"
                ? "A temporada especial terminou. Você pode conferir notícias e standings finais antes de abrir o fechamento da temporada."
                : hasExistingPreseason
                ? "A pré-temporada já foi iniciada. Você pode voltar direto para o mercado semanal."
                : "Todas as corridas da temporada atual já foram disputadas."}
            </p>
            <div className="mt-6">
              <GlassButton
                variant="primary"
                disabled={isAdvancing || isConvocating || isEnteringPreseason}
                onClick={() => {
                if (isFreeAgent) {
                  setError("");
                  skipAllPendingRaces().catch((e) => {
                    setError(getDisplayError(e, "Erro ao pular temporada."));
                  });
                } else {
                  void handleSeasonAdvance();
                }
              }}
              >
                {isAdvancing || isConvocating || isEnteringPreseason
                  ? "Processando..."
                  : isFreeAgent
                  ? "Pular temporada"
                  : season?.fase === "BlocoEspecial"
                  ? "Pular bloco especial"
                  : hasPendingRegularRaces
                  ? "Avançar calendário"
                  : season?.fase === "BlocoRegular"
                  ? "Avançar para convocação"
                  : season?.fase === "PosEspecial"
                  ? "Encerrar temporada"
                  : hasExistingPreseason
                  ? "Continuar pré-temporada"
                  : "Avançar para pré-temporada"}
              </GlassButton>
            </div>
            {error ? <p className="mt-4 text-sm text-status-red">{error}</p> : null}
          </div>
        </GlassCard>
      </div>
    );
  }

  return (
    <div className="relative min-h-[calc(100vh-100px)]">
      {/* Background glass effect specific to this dashboard */}
      <div className="fixed inset-0 z-0 overflow-hidden pointer-events-none opacity-60">
        <div className="absolute inset-x-0 -top-40 h-[600px] bg-[url('https://images.unsplash.com/photo-1541443131876-44b03de101c5?auto=format&fit=crop&q=80')] bg-cover opacity-15 filter blur-[30px] mix-blend-screen transform scale-110"></div>
        <div className="absolute inset-0 bg-gradient-to-b from-transparent via-[#06090e]/80 to-[#06090e]"></div>
      </div>

      <div className="relative z-10 space-y-6">
        <LoadingOverlay
          open={isSimulating}
          title="Simulando corrida"
          message="Classificação, corrida e atualização do campeonato em andamento."
        />

        {/* HEADER COM BOTÕES */}
        <header className="flex flex-col md:flex-row justify-between items-start md:items-end mb-4">
          <div>
            <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-[#58a6ff] mb-2">
              <span className="mr-2">🏁</span>Sala de Estratégia
            </p>
            <h1 className="text-[2.5rem] font-extrabold text-white leading-none">{nextRace.track_name}</h1>
            <div className="flex flex-wrap items-center gap-3 mt-3">
              <span className="border border-white/10 bg-white/5 px-3 py-1.5 rounded-lg text-xs font-bold text-white">
                Etapa {nextRace.rodada} de {season?.total_rodadas ?? "?"}
              </span>
              <span className="text-sm font-medium text-gray-400 capitalize">
                {briefing.eventDateShort} • {briefing.timePeriodHighlight}
              </span>
            </div>
          </div>

          <div className="flex flex-col sm:flex-row items-center gap-4 mt-6 md:mt-0 w-full sm:w-auto">
            <button
              onClick={handleSimulate}
              disabled={isSimulating || !nextRace}
              className="w-full sm:w-auto px-5 py-2 border border-white/10 bg-white/5 hover:bg-white/10 text-gray-300 font-semibold rounded-lg transition text-xs flex justify-center items-center gap-1.5 opacity-80 hover:opacity-100 disabled:opacity-50"
            >
              {isSimulating ? "Simulando..." : "Simular Corrida"}
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" className="w-4 h-4 text-[#58a6ff]">
                <path fillRule="evenodd" d="M14.615 1.595a.75.75 0 01.359.852L12.982 9.75h7.268a.75.75 0 01.548 1.262l-10.5 11.25a.75.75 0 01-1.272-.71l1.992-7.302H3.75a.75.75 0 01-.548-1.262l10.5-11.25a.75.75 0 01.913-.143z" clipRule="evenodd" />
              </svg>
            </button>
            <button
              onClick={handleExport}
              className="w-full sm:w-auto px-10 py-3.5 bg-[#58a6ff] hover:bg-blue-400 text-[#06090e] font-black uppercase rounded-xl transition text-base shadow-[0_0_20px_rgba(88,166,255,0.4)] flex justify-center items-center gap-2"
            >
              Exportar Dados
            </button>
          </div>
        </header>

        {exportNotice && <p className="text-right text-sm text-[#58a6ff]">{exportNotice}</p>}
        {error && <p className="text-right text-sm text-red-500">{error}</p>}

        {/* GRID PRINCIPAL (4-4-4) */}
        <div className="grid grid-cols-1 xl:grid-cols-12 gap-6 items-stretch pb-10">
          
          {/* 1) NARRATIVA DA ETAPA */}
          <div className="xl:col-span-4 flex flex-col gap-5 xl:h-[650px]">
            {/* Condições Compactas */}
            <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-5 flex justify-between items-center bg-gradient-to-r from-black/40 to-transparent">
              <div className="flex items-center gap-4">
                <div className="text-4xl">{briefing.weatherIcon}</div>
                <div>
                  <p className="text-[10px] uppercase tracking-widest text-[#58a6ff] font-bold">Condição de Pista</p>
                  <p className="text-xl font-bold text-white">
                    {briefing.weatherSummary} <span className="text-xs text-gray-400">{briefing.trackTemperatureLabel}</span>
                  </p>
                </div>
              </div>
              <div className="text-right">
                <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">Público</p>
                <p className="text-xl font-bold text-white">{formatAudience(briefing.audienceEstimate)}</p>
              </div>
            </div>

            {/* Narrativa Expandida */}
            <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-6 flex-1 flex flex-col relative overflow-hidden">
              <div className="absolute -right-10 -top-10 h-40 w-40 rounded-full bg-[radial-gradient(circle,rgba(240,195,107,0.1),transparent_65%)] pointer-events-none"></div>
              <p className="text-[11px] uppercase tracking-[0.2em] text-[#f5c76d] mb-4 font-bold relative z-10 flex items-center">
                <span className="mr-2 text-sm">📰</span>Narrativa da Etapa
              </p>

              <div className="flex-1 overflow-y-auto custom-scrollbar pr-2 relative z-10 flex flex-col">
                <h3 className="text-2xl font-bold text-white leading-snug mb-4">{briefing.headline}</h3>
                <p className="text-[15px] text-gray-300 leading-relaxed mb-4">
                  {briefing.paragraphs[0] ?? briefing.attendanceNarrative}
                </p>
                <p className="text-[15px] text-gray-300 leading-relaxed mb-6">
                  {briefing.paragraphs[1] || briefing.actionHint}
                </p>

                {/* Leitura de Box Expandida */}
                <div className="bg-black/30 border border-white/5 p-4 rounded-2xl relative mt-auto">
                  <div className="absolute top-2 right-4 text-[#58a6ff] opacity-20 pointer-events-none">
                    <span className="text-6xl font-serif leading-none h-[40px] block overflow-hidden">"</span>
                  </div>
                  <p className="text-[10px] uppercase tracking-[0.15em] text-[#58a6ff] mb-2 font-bold">Voz da Equipe</p>
                  <p className="text-sm italic text-gray-200 leading-relaxed">"{briefing.quote}"</p>
                  <p className="text-xs font-semibold text-gray-400 mt-3 text-right">
                    -{" "}
                    <span style={briefing.teamColor ? { color: getReadableTeamColor(briefing.teamColor) } : undefined}>
                      {briefing.teamVoiceLabel}
                    </span>
                  </p>
                </div>
              </div>
            </div>
          </div>

          {/* 2) METAS HORIZONTAIS E FAVORITOS */}
          <div className="xl:col-span-4 flex flex-col gap-5 xl:h-[650px]">
            {/* Aviso de contrato expirando */}
            {nextRaceBriefing?.contract_warning != null &&
              Math.max(0, (season?.total_rodadas ?? 0) - (nextRace?.rodada ?? 0)) <= 1 && (
              <div className="bg-amber-900/30 border border-amber-500/40 rounded-2xl px-4 py-3 flex items-start gap-3">
                <span className="text-amber-400 text-base leading-none mt-0.5">⚠</span>
                <div>
                  <p className="text-[10px] uppercase tracking-[0.15em] text-amber-400 font-bold mb-0.5">Contrato expirando</p>
                  <p className="text-xs text-amber-100 leading-relaxed">
                    Seu contrato com <span className="font-semibold">{nextRaceBriefing.contract_warning.equipe_nome}</span> encerra ao fim desta temporada.
                  </p>
                </div>
              </div>
            )}

            {/* Metas */}
            <div className="grid grid-cols-3 gap-3">
              <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-2xl p-4 text-center flex flex-col justify-start items-center">
                <span className="text-2xl mb-1.5 block leading-none">👥</span>
                <p className="text-[9px] uppercase font-bold text-gray-500 tracking-wider">Meta Equipe</p>
                <p className="text-[10px] text-white font-semibold mt-1 leading-tight">
                  {briefing.goals[0]?.value}
                </p>
              </div>
              <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-2xl p-4 text-center flex flex-col justify-start items-center">
                <span className="text-xl mb-1.5 block leading-none">👤</span>
                <p className="text-[9px] uppercase font-bold text-gray-500 tracking-wider">Meta Pessoal</p>
                <p className="text-[10px] text-white font-semibold mt-1 leading-tight">
                  {briefing.goals[1]?.value}
                </p>
              </div>
              <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-2xl p-4 text-center flex flex-col justify-start items-center">
                <span className="text-xl mb-1.5 block leading-none">🏆</span>
                <p className="text-[9px] uppercase font-bold text-gray-500 tracking-wider">Meta Título</p>
                <p className="text-[10px] text-white font-semibold mt-1 leading-tight">
                  {briefing.goals[2]?.value}
                </p>
              </div>
            </div>

            {/* Favoritos ao Pódio */}
            <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-6 flex-1 flex flex-col min-h-0">
              <p className="text-[12px] font-bold uppercase tracking-[0.2em] text-[#58a6ff] mb-5">Os 5 Favoritos ao Pódio</p>

              <div className="space-y-4 flex-1 overflow-y-auto custom-scrollbar pr-1">
                {isLoadingBriefing ? (
                  <p className="text-sm text-gray-400">Montando analise...</p>
                ) : (
                  briefing.favorites.map((driver, index) => {
                    let medalTone = getFavoriteMedalTone(index);
                    const isJogador = driver.is_jogador;

                    return (
                      <div
                        key={driver.id}
                        className={`border rounded-2xl p-4 flex flex-col xl:flex-row gap-3 xl:gap-0 justify-between xl:items-center transition hover:bg-white/5 ${
                          isJogador ? "bg-[#58a6ff]/10 border-[#58a6ff]/30" : "bg-black/20 border-white/5"
                        }`}
                      >
                        <div className="flex items-center gap-4">
                          <span className={`font-black w-8 text-center text-[30px] ${isJogador ? "text-[#58a6ff]" : medalTone}`}>
                            {index + 1}
                          </span>
                          <TeamLogoMark
                            teamName={driver.equipe_nome}
                            color={driver.equipe_cor}
                            size="sm"
                            testId="strategy-favorite-team-logo"
                          />
                          <div>
                            <p className="text-base font-bold text-white leading-none mb-1.5">{driver.nome}</p>
                            <p
                              className="text-[11px] font-bold uppercase"
                              style={{ color: getReadableTeamColor(driver.equipe_cor) }}
                            >
                              {driver.equipe_nome}
                            </p>
                          </div>
                        </div>
                        <div className="flex gap-1.5 justify-end xl:ml-0 overflow-x-auto custom-scrollbar pb-1 xl:pb-0">
                          {driver.formChips.map((chip, chipIdx) => {
                            let customStyle = "bg-gray-500/10 text-gray-400 border-gray-500/30";
                            if (chip.label === "P1") customStyle = "bg-[#f5c76d]/10 text-[#f5c76d] border-[#f5c76d]/30";
                            else if (chip.label === "P2") customStyle = "bg-[#d8dfef]/10 text-[#d8dfef] border-[#d8dfef]/30";
                            else if (chip.label === "P3") customStyle = "bg-[#cf8d63]/10 text-[#cf8d63] border-[#cf8d63]/30";
                            else if (chip.label.includes("DNF")) customStyle = "bg-red-500/10 text-red-500 border-red-500/30";
                            else if (chip.label.startsWith("P") && parseInt(chip.label.substring(1)) <= 6)
                              customStyle = "bg-[#58a6ff]/10 text-[#58a6ff] border-[#58a6ff]/30";

                            return (
                              <span
                                key={chipIdx}
                                className={`border px-2 py-1 rounded text-[10px] whitespace-nowrap font-bold ${customStyle}`}
                              >
                                {chip.label}
                              </span>
                            );
                          })}
                        </div>
                      </div>
                    );
                  })
                )}
              </div>
            </div>
          </div>

          {/* 3) TABELA CAMPEONATO */}
          <div className="xl:col-span-4 h-[500px] xl:h-[650px]">
            <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-6 h-full flex flex-col relative overflow-hidden">
              <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-[#58a6ff] mb-4">Tabela Geral do Campeonato</p>
              
              {briefing.championshipTable.length === 0 ? (
                <p className="text-sm text-gray-400">Classificação indisponível no momento.</p>
              ) : (
                <div className="flex-1 overflow-y-auto custom-scrollbar -mx-2 px-2 pb-2">
                  <table className="w-full text-sm">
                    <thead className="sticky top-0 bg-[#06090ebd] backdrop-blur z-20 text-[9px] text-gray-500 uppercase font-bold text-left border-b border-white/10">
                      <tr>
                        <th className="py-2 px-3 text-center w-8">#</th>
                        <th className="py-2 px-1">Piloto</th>
                        <th className="py-2 px-3 text-right">Pts</th>
                      </tr>
                    </thead>
                    <tbody>
                      {briefing.championshipTable.map((driver) => {
                        const isPlayer = driver.is_jogador;
                        return (
                          <tr
                            key={driver.id}
                            className={`border-b ${isPlayer ? "border-[#58a6ff]/40 bg-[#58a6ff]/10" : "border-white/5 hover:bg-white/5"}`}
                          >
                            <td className={`py-3 px-3 text-center ${isPlayer ? "font-extrabold text-[#58a6ff]" : "font-bold text-white"}`}>
                              {driver.posicao_campeonato}
                            </td>
                            <td className={`py-3 px-1 ${isPlayer ? "text-white font-bold" : "text-white font-medium"}`}>
                              {driver.nome_completo ?? driver.nome}
                            </td>
                            <td className={`py-3 px-3 text-right ${isPlayer ? "font-extrabold text-[#58a6ff]" : "font-bold text-white"}`}>
                              {driver.pontos}
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
          
        </div>
      </div>
    </div>
  );
}

function getFavoriteMedalTone(index) {
  if (index === 0) return "text-[#f5c76d]";
  if (index === 1) return "text-[#d8dfef]";
  if (index === 2) return "text-[#cf8d63]";
  return "text-gray-500";
}


function buildBriefingContext({
  player,
  playerTeam,
  season,
  nextRace,
  nextRaceBriefing,
  driverStandings,
  teamStandings,
  briefingPhraseHistory,
}) {
  const orderedDrivers = [...driverStandings].sort(
    (left, right) => (left.posicao_campeonato ?? 999) - (right.posicao_campeonato ?? 999),
  );
  const orderedTeams = [...teamStandings].sort(
    (left, right) => (left.posicao ?? 999) - (right.posicao ?? 999),
  );
  const playerStanding =
    orderedDrivers.find((driver) => driver.is_jogador) ??
    orderedDrivers.find((driver) => driver.id === player?.id) ??
    null;
  const standingsTopFive = orderedDrivers.slice(0, 5);
  const leader = standingsTopFive[0] ?? null;
  const trackHistory = nextRaceBriefing?.track_history ?? null;
  const briefingRival = nextRaceBriefing?.primary_rival ?? null;
  const weekendStories = normalizeWeekendStories(nextRaceBriefing?.weekend_stories);
  const rival = resolvePrimaryRival(orderedDrivers, playerStanding, briefingRival);
  const teammate =
    playerStanding && playerStanding.equipe_id
      ? orderedDrivers.find(
          (driver) => driver.equipe_id === playerStanding.equipe_id && driver.id !== playerStanding.id,
        ) ?? null
      : null;
  const teamStanding =
    orderedTeams.find((team) => team.id === playerTeam?.id) ?? orderedTeams[0] ?? null;
  const gapToLeader = Math.max(0, (leader?.pontos ?? 0) - (playerStanding?.pontos ?? 0));
  const behindDriver =
    playerStanding && playerStanding.posicao_campeonato > 0
      ? orderedDrivers[playerStanding.posicao_campeonato] ?? null
      : null;
  const gapBehind =
    playerStanding && behindDriver
      ? Math.max(0, (playerStanding.pontos ?? 0) - (behindDriver.pontos ?? 0))
      : null;
  const remainingRounds = Math.max(0, (season?.total_rodadas ?? 0) - (nextRace?.rodada ?? 0));
  const ratedDrivers = orderedDrivers
    .map((driver) => ({
      ...driver,
      rating: buildFavoriteRating(driver),
      formLabel: buildFormLabel(driver),
      formChips: buildFormChips(driver),
    }))
    .sort((left, right) => right.rating - left.rating || left.posicao_campeonato - right.posicao_campeonato);
  const favorites = ratedDrivers
    .slice()
    .sort((left, right) => right.rating - left.rating || left.posicao_campeonato - right.posicao_campeonato)
    .slice(0, 5)
    .map((driver, index) => {
      const selection = buildFavoriteExpectationSelection(driver, index, {
        seasonNumber: season?.numero,
        roundNumber: nextRace?.rodada,
        historyEntries: briefingPhraseHistory?.entries ?? [],
      });

      return {
        ...driver,
        expectation: selection.text,
        expectationPhraseId: selection.phraseId,
        expectationBucketKey: selection.bucketKey,
      };
    });
  const audienceEstimate = nextRace?.event_interest?.display_value ?? estimateAudience(nextRace?.event_interest?.tier_label);
  const totalRounds = Math.max(1, season?.total_rodadas ?? 1);
  const currentRound = Math.max(1, nextRace?.rodada ?? 1);
  const playerCompetitive = ratedDrivers.find((driver) => driver.id === playerStanding?.id) ?? null;
  const leaderCompetitive = ratedDrivers.find((driver) => driver.id === leader?.id) ?? null;
  const outlook = buildCompetitiveOutlook({
    playerStanding,
    leader,
    remainingRounds,
    playerRating: playerCompetitive?.rating ?? 0,
    leaderRating: leaderCompetitive?.rating ?? 0,
  });
  const attendanceNarrative =
    audienceEstimate > 0
      ? `A expectativa do paddock aponta para ${formatAudience(audienceEstimate)} de público estimado ao longo do fim de semana.`
      : "O paddock espera bom movimento de público nesta etapa.";
  const championshipState = classifyChampionshipState({
    playerStanding,
    leader,
    remainingRounds,
    outlook,
    gapBehind,
  });
  const weekendState = classifyWeekendState({
    trackHistory,
    briefingRival,
    nextRace,
    weekendStories,
  });
  const editorialCopy = buildEditorialCopy({
    championshipState,
    weekendState,
    playerStanding,
    leader,
    rival,
    briefingRival,
    playerTeam,
    nextRace,
    trackHistory,
    weekendStories,
    gapToLeader,
    gapBehind,
    remainingRounds,
    audienceEstimate,
  });

  return {
    audienceEstimate,
    audienceRankLabel: buildAudienceRankLabel(nextRace, season),
    eventDateShort: formatEventSummaryDate(nextRace?.display_date),
    interestLabel: nextRace?.event_interest?.tier_label ?? "Padrão da temporada",
    broadcastLabel: isLiveCoverageEvent(nextRace, season) ? "Cobertura" : "Expectativa",
    broadcastValue: isLiveCoverageEvent(nextRace, season)
      ? "Ao vivo"
      : buildTeamExpectationValue({ playerStanding, teamStanding, gapToLeader, outlook }),
    headline: editorialCopy.headline,
    historyValue: editorialCopy.historyValue,
    historyMeta: editorialCopy.historyMeta,
    paragraphs: editorialCopy.paragraphs,
    goals: buildGoals({
      playerStanding,
      teammate,
      teamStanding,
      gapToLeader,
      remainingRounds,
      outlook,
      driverAbove: playerStanding?.posicao_campeonato > 1
        ? orderedDrivers[playerStanding.posicao_campeonato - 2] ?? null
        : null,
    }),
    favorites,
    championshipTable: orderedDrivers,
    standingsTopFive,
    gapToLeaderLabel: gapToLeader === 0 ? "Liderança" : `${gapToLeader} pts`,
    gapBehindLabel: gapBehind == null ? "Sem perseguidor direto" : `${gapBehind} pts`,
    scenario: editorialCopy.scenario,
    progressPercent: Math.max(5, Math.min(100, Math.round((currentRound / totalRounds) * 100))),
    progressLabel: `${currentRound}/${totalRounds}`,
    quote: editorialCopy.quote,
    teamVoiceLabel: playerTeam?.nome ?? "Equipe do jogador",
    teamColor: playerTeam?.cor_primaria ?? null,
    paddockSupport: editorialCopy.paddockSupport ?? attendanceNarrative,
    attendanceNarrative,
    weatherIcon: buildWeatherIcon(nextRace?.clima),
    weatherSummary: buildWeatherSummary(nextRace?.clima),
    weatherNarrative: buildWeatherNarrative(nextRace?.clima),
    trackTemperatureLabel:
      nextRace?.temperatura == null ? "-" : `${Math.round(nextRace.temperatura)}°C`,
    temperatureNarrative: buildTemperatureNarrative(nextRace?.temperatura),
    trackConditionLabel: buildTrackConditionLabel(nextRace?.clima),
    boxNarrative: buildBoxNarrative(nextRace?.clima),
    timePeriodPrefix: buildTimePeriodPrefix(nextRace?.horario),
    timePeriodHighlight: buildTimePeriodHighlight(nextRace?.horario),
    actionHint: editorialCopy.actionHint,
    rivalSummary: editorialCopy.rivalSummary,
    rivalSupport: editorialCopy.rivalSupport,
    weekendStories,
    weekendStoriesMeta: editorialCopy.weekendStoriesMeta,
    weekendStoriesEmpty: editorialCopy.weekendStoriesEmpty,
  };
}

function normalizeWeekendStories(stories) {
  if (!Array.isArray(stories)) {
    return [];
  }

  return stories.map((story) => ({
    id: story.id,
    icon: story.icon,
    title: story.title,
    summary: story.summary,
    importanceLabel: story.importance ?? "Contexto",
  }));
}

function resolvePrimaryRival(orderedDrivers, playerStanding, briefingRival) {
  if (briefingRival?.driver_id) {
    const matchingDriver = orderedDrivers.find((driver) => driver.id === briefingRival.driver_id);
    if (matchingDriver) {
      return matchingDriver;
    }

    return {
      id: briefingRival.driver_id,
      nome: briefingRival.driver_name,
      posicao_campeonato: briefingRival.championship_position,
      pontos:
        briefingRival.is_ahead || !playerStanding
          ? (playerStanding?.pontos ?? 0) + (briefingRival.gap_points ?? 0)
          : Math.max(0, (playerStanding?.pontos ?? 0) - (briefingRival.gap_points ?? 0)),
    };
  }

  return resolveDirectRival(orderedDrivers, playerStanding);
}

function buildCompetitiveOutlook({ playerStanding, leader, remainingRounds, playerRating, leaderRating }) {
  if (!playerStanding || !leader) {
    return {
      titleFight: "neutral",
      targetResult: "clean",
    };
  }

  const recentKnown = recentResults(playerStanding).filter(Boolean);
  const averageFinish = recentKnown.length
    ? recentKnown.reduce((total, result) => total + (result.position ?? 12), 0) / recentKnown.length
    : null;
  const topFiveCount = recentKnown.filter((result) => !result.is_dnf && (result.position ?? 99) <= 5).length;
  const podiumCount = recentKnown.filter((result) => !result.is_dnf && (result.position ?? 99) <= 3).length;
  const winCount = recentKnown.filter((result) => !result.is_dnf && result.position === 1).length;
  const racesLeftIncludingCurrent = Math.max(1, remainingRounds + 1);
  const gapToLeader = Math.max(0, (leader.pontos ?? 0) - (playerStanding.pontos ?? 0));
  const ratingGap = Math.max(0, leaderRating - playerRating);
  const weakRecentForm = averageFinish != null && averageFinish >= 7;
  const strongRecentForm = averageFinish != null && averageFinish <= 4.5;
  const titleLongshot =
    playerStanding.posicao_campeonato >= 6 ||
    gapToLeader > racesLeftIncludingCurrent * 12 ||
    (racesLeftIncludingCurrent <= 2 && (weakRecentForm || topFiveCount === 0 || ratingGap >= 10));
  const titleContender =
    gapToLeader <= racesLeftIncludingCurrent * 6 &&
    (strongRecentForm || topFiveCount >= 2 || podiumCount >= 1 || ratingGap <= 4);

  let titleFight = "outsider";
  if (playerStanding.posicao_campeonato === 1) {
    titleFight = "leader";
  } else if (titleContender) {
    titleFight = "contender";
  } else if (titleLongshot) {
    titleFight = "longshot";
  }

  let targetResult = "top8";
  if (winCount >= 1 || podiumCount >= 2 || playerRating >= 80) {
    targetResult = "podium";
  } else if (topFiveCount >= 1 || (averageFinish != null && averageFinish <= 6)) {
    targetResult = "top5";
  }

  return {
    titleFight,
    targetResult,
    averageFinish,
    topFiveCount,
    podiumCount,
    winCount,
    racesLeftIncludingCurrent,
    gapToLeader,
  };
}

function resolveDirectRival(driverStandings, playerStanding) {
  if (!playerStanding || playerStanding.posicao_campeonato <= 0) {
    return null;
  }

  if (playerStanding.posicao_campeonato === 1) {
    return driverStandings[1] ?? null;
  }

  return driverStandings[playerStanding.posicao_campeonato - 2] ?? null;
}

function buildFavoriteRating(driver) {
  const recentScore = recentResults(driver).reduce((total, result) => {
    if (!result) return total;
    if (result.is_dnf) return total - 10;
    return total + Math.max(0, 14 - (result.position ?? 12));
  }, 0);

  const rawScore =
    (driver.skill ?? 70) * 0.74 +
    (driver.pontos ?? 0) * 0.24 +
    (driver.vitorias ?? 0) * 6 +
    (driver.podios ?? 0) * 1.4 +
    recentScore;

  return Math.max(52, Math.min(98, Math.round(rawScore / 2.1)));
}

function buildFormLabel(driver) {
  const snapshot = recentResults(driver)
    .map((result) => {
      if (!result) return "P--";
      if (result.is_dnf) return "DNF";
      return `P${result.position ?? "--"}`;
    })
    .join(" - ");

  return snapshot ? `Forma recente: ${snapshot}` : "Sem histórico recente.";
}

function buildFormChips(driver) {
  const chips = recentResults(driver).map((result) => {
    if (!result) {
      return {
        label: "Sem dado",
        tone: "border-white/10 bg-white/[0.04] text-text-secondary",
      };
    }

    if (result.is_dnf) {
      return {
        label: "DNF",
        tone: "border-status-red/30 bg-status-red/12 text-status-red",
      };
    }

    const position = result.position ?? 99;
    if (position === 1) {
      return {
        label: "P1",
        tone: "border-podium-gold/30 bg-podium-gold/10 text-podium-gold",
      };
    }
    if (position === 2) {
      return {
        label: "P2",
        tone: "border-podium-silver/30 bg-podium-silver/10 text-podium-silver",
      };
    }
    if (position === 3) {
      return {
        label: "P3",
        tone: "border-podium-bronze/30 bg-podium-bronze/10 text-podium-bronze",
      };
    }

    if (position <= 6) {
      return {
        label: `P${position}`,
        tone: "border-accent-primary/25 bg-accent-primary/10 text-accent-primary",
      };
    }

    return {
      label: `P${position}`,
      tone: "border-white/10 bg-white/[0.04] text-text-secondary",
    };
  });

  return chips.length > 0
    ? chips
    : [{ label: "Sem histórico", tone: "border-white/10 bg-white/[0.04] text-text-secondary" }];
}

function getFavoritePositionTone(index) {
  if (index === 0) return "text-[#f5c76d]";
  if (index === 1) return "text-[#d8dfef]";
  if (index === 2) return "text-[#cf8d63]";
  return "text-text-primary";
}

function buildGoals({ playerStanding, teammate, teamStanding, gapToLeader, remainingRounds, outlook, driverAbove }) {
  const teamGoal =
    teamStanding?.posicao === 1
      ? "Manter a liderança do campeonato de equipes."
      : teamStanding
        ? `Levar a equipe ao top ${Math.min(3, teamStanding.posicao)} entre os construtores.`
        : "Sair da etapa com pontos fortes para a equipe.";

  const playerPos = playerStanding?.posicao_campeonato ?? 0;
  const teammatePos = teammate?.posicao_campeonato ?? 0;
  const teammateIsClose = teammate && Math.abs(playerPos - teammatePos) <= 2;

  const personalGoal = teammateIsClose
    ? `Terminar a frente de ${teammate.nome} na leitura interna do box.`
    : driverAbove
      ? `Superar ${driverAbove.nome} e subir para o ${playerPos - 1}º no campeonato.`
      : "Executar um fim de semana limpo, sem perdas na largada.";

  let championshipGoal = "Pontuar forte para manter o campeonato vivo.";
  if (playerStanding?.posicao_campeonato === 1) {
    championshipGoal = "Controlar os danos e sair da etapa ainda no topo.";
  } else if (outlook?.titleFight === "longshot") {
    championshipGoal = "Somar o máximo de pontos possível e manter o campeonato respeitável até o fim.";
  } else if (gapToLeader <= 7) {
    championshipGoal = "Atacar a liderança agora que a distância é curta.";
  } else if (remainingRounds <= 3) {
    championshipGoal = "Maximizar pontos agora para não deixar a temporada escapar.";
  }

  return [
    { label: "Meta da equipe", value: teamGoal },
    { label: "Meta pessoal", value: personalGoal },
    { label: "Meta do campeonato", value: championshipGoal },
  ];
}

function buildWeatherSummary(clima) {
  if (clima === "HeavyRain") return "Chuva forte";
  if (clima === "Wet") return "Chuva";
  if (clima === "Damp") return "Úmido";
  return "Seco";
}

function buildWeatherIcon(clima) {
  if (clima === "HeavyRain") return "⛈";
  if (clima === "Wet") return "🌧";
  if (clima === "Damp") return "🌦";
  return "☀";
}

function buildWeatherNarrative(clima) {
  if (clima === "HeavyRain") return "Corrida reativa, spray alto e erro caro.";
  if (clima === "Wet") return "Pista pedindo paciência na entrada e tração limpa.";
  if (clima === "Damp") return "Linha mudando rápido volta a volta.";
  return "Janela previsível para empurrar mais cedo.";
}

function buildTemperatureNarrative(temperatura) {
  if (temperatura == null) return "Leitura térmica ainda indefinida para o fim de semana.";
  if (temperatura <= 16) return "Ar frio ajudando a segurar desgaste.";
  if (temperatura <= 28) return "Temperatura equilibrada para stints consistentes.";
  return "Calor cobrando mais do conjunto de pneus.";
}

function buildTrackConditionLabel(clima) {
  if (clima === "HeavyRain") return "Visibilidade apertada";
  if (clima === "Wet") return "Trajetória molhada";
  if (clima === "Damp") return "Janela instável";
  return "Alta aderência";
}

function buildBoxNarrative(clima) {
  if (clima === "HeavyRain") return "Linha ideal curta e comunicação constante.";
  if (clima === "Wet") return "Trajetória molhada e janela sensível.";
  if (clima === "Damp") return "Aderencia oscilando fora do trilho seco.";
  return "Alta aderência para atacar mais cedo.";
}

function formatEventSummaryDate(displayDate) {
  if (!displayDate) return "--/--";

  const [year, month, day] = displayDate.split("-");
  if (!year || !month || !day) return displayDate;
  return `${day}/${month}`;
}

function buildTimePeriodPrefix(horario) {
  const hour = parseHour(horario);
  if (hour == null) return "Horário ";
  if (hour < 6) return "Madrugada de ";
  if (hour < 12) return "Início da ";
  if (hour < 18) return "Início da ";
  return "Início da ";
}

function buildTimePeriodHighlight(horario) {
  const hour = parseHour(horario);
  if (hour == null) return "pista";
  if (hour < 6) return "madrugada";
  if (hour < 12) return "manhã";
  if (hour < 18) return "tarde";
  return "noite";
}

function parseHour(horario) {
  if (typeof horario !== "string") return null;
  const [rawHour] = horario.split(":");
  const parsed = Number.parseInt(rawHour, 10);
  return Number.isNaN(parsed) ? null : parsed;
}

function buildAudienceRankLabel(nextRace, season) {
  const totalRounds = Math.max(1, season?.total_rodadas ?? 1);
  const round = nextRace?.rodada ?? 1;
  const interestTier = nextRace?.event_interest?.tier_label?.toLowerCase() ?? "";

  if (round === 1 || round === totalRounds) {
    return "Maior público da temporada";
  }

  if (interestTier.includes("principal")) {
    return "3º Maior público da temporada";
  }

  if (interestTier.includes("alto")) {
    return "Entre os maiores públicos da temporada";
  }

  return "Movimento forte dentro da temporada";
}

function isLiveCoverageEvent(nextRace, season) {
  const totalRounds = Math.max(1, season?.total_rodadas ?? 1);
  const round = nextRace?.rodada ?? 1;
  const interestTier = nextRace?.event_interest?.tier_label?.toLowerCase() ?? "";

  return round === 1 || round === totalRounds || interestTier.includes("principal");
}

function buildTeamExpectationValue({ playerStanding, teamStanding, gapToLeader, outlook }) {
  if (playerStanding?.posicao_campeonato === 1) {
    return "Controlar a ponta";
  }

  if (outlook?.titleFight === "longshot") {
    return "Pontuar forte";
  }

  if (gapToLeader <= 10) {
    return "Pressionar a frente";
  }

  if ((teamStanding?.posicao ?? 99) <= 3) {
    return "Top 5 no radar";
  }

  return "Fim de semana limpo";
}

function estimateAudience(tierLabel) {
  if (tierLabel?.toLowerCase().includes("principal")) return 84000;
  if (tierLabel?.toLowerCase().includes("alto")) return 62000;
  if (tierLabel?.toLowerCase().includes("moderado")) return 41000;
  return 28000;
}

function formatAudience(value) {
  return value ? value.toLocaleString("pt-BR") : "-";
}

export default NextRaceTab;



function getReadableTeamColor(color) {
  if (!color || !/^#([0-9a-f]{6})$/i.test(color)) {
    return "#58a6ff";
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
