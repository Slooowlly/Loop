import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import GlassButton from "../../components/ui/GlassButton";
import GlassCard from "../../components/ui/GlassCard";
import LoadingOverlay from "../../components/ui/LoadingOverlay";
import TeamLogoMark from "../../components/team/TeamLogoMark";
import RivalMarker from "../../components/driver/RivalMarker";
import IracingTutorialModal from "../../components/iracing/IracingTutorialModal";
import WeatherButton from "../../components/race/WeatherButton";
import BreakdownRiskButton from "../../components/race/BreakdownRiskButton";
import useCareerStore from "../../stores/useCareerStore";
import { useAttentionStore } from "../../stores/useAttentionStore";
import { exportSuccess } from "../../utils/sfx";
import { renderTextWithDriverMentions } from "../../utils/driverMentions";
import { getTeamGlow } from "../../utils/teamColors";
import { isLegacySeasonPhase } from "../../utils/seasonPhases";
import {
  buildBriefingContext,
  getFavoriteMedalTone,
  formatAudience,
  getReadableTeamColor,
} from "./nextRaceContext";
import { isPortuguese, localizedAiError } from "../../utils/aiFallback";

// Chave de "já vi o tutorial do iRacing" (mostrado só na 1ª ida ao iRacing).
const IRACING_TUTORIAL_KEY = "loop.iracingTutorialSeen";

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

// Tempo (ms) na Sala de Estratégia a partir do qual consideramos que o jogador LEU a
// prévia. Abaixo disso (simular/sair antes), conta como "não leu" para o gate de IA.
const PRE_RACE_READ_MS = 10000;

// Teto de espera (ms) do skeleton da prévia de IA. Enquanto a IA é buscada, mostramos
// um placeholder (não o template) para evitar o flash template→IA. Se estourar esse
// tempo, caímos no template em vez de segurar o skeleton indefinidamente.
const AI_PREVIEW_MAX_WAIT_MS = 8000;

// Lê o cache de standings pré-buscado na store (preenchido pelo prefetch durante a
// animação de avanço), mas SÓ se for da corrida atual. A pré-corrida é estática até a
// corrida rodar, então quando o cache bate a Sala abre os Favoritos na hora — sem
// re-disparar get_drivers_by_category/get_teams_standings (os comandos que faziam o
// "Montando análise" demorar toda vez que se volta à Sala). Cache miss → busca normal.
function readCachedPreRaceStandings() {
  const state = useCareerStore.getState();
  const cache = state.preRaceStandings;
  return cache && cache.raceId && cache.raceId === state.nextRace?.id ? cache : null;
}

function NextRaceTab() {
  const { t } = useTranslation();
  const [error, setError] = useState("");
  const [exportNotice, setExportNotice] = useState("");
  const [isExporting, setIsExporting] = useState(false);
  const [exported, setExported] = useState(false);
  const [showGoToast, setShowGoToast] = useState(false);
  const [iracingFocusMsg, setIracingFocusMsg] = useState("");
  const [showTutorial, setShowTutorial] = useState(false);
  const [tutorialMsg, setTutorialMsg] = useState("");
  const toastTimers = useRef([]);
  const [confirmSim, setConfirmSim] = useState(false);
  // "Pegar a cor do carro": botão ao lado de Simular Corrida que abre o modal.
  // Só aparece quando o jogador já conectou ao iRacing (temos o custid) e ainda
  // não vinculou a cor a este save.
  const [canPickPaint, setCanPickPaint] = useState(false);
  const [showPaintPrompt, setShowPaintPrompt] = useState(false);
  const [paintBusy, setPaintBusy] = useState(false);
  const [paintError, setPaintError] = useState("");
  const [paintToast, setPaintToast] = useState("");
  const [hasExistingPreseason, setHasExistingPreseason] = useState(false);
  const [driverStandings, setDriverStandings] = useState(
    () => readCachedPreRaceStandings()?.driverStandings ?? [],
  );
  const [teamStandings, setTeamStandings] = useState(
    () => readCachedPreRaceStandings()?.teamStandings ?? [],
  );
  const [briefingPhraseHistory, setBriefingPhraseHistory] = useState(
    () => readCachedPreRaceStandings()?.phraseHistory ?? { season_number: 0, entries: [] },
  );
  // Já temos standings em cache desta etapa → abre sem o "Montando análise".
  const [isLoadingBriefing, setIsLoadingBriefing] = useState(() => !readCachedPreRaceStandings());
  // Previsão de risco de quebra do carro (aviso pré-corrida — Peça 3 / Feature 1).
  const [breakdownForecast, setBreakdownForecast] = useState(null);
  // IDs das EQUIPES com risco real de quebra na próxima corrida → 🔧 na tabela do campeonato.
  const [breakdownRiskTeams, setBreakdownRiskTeams] = useState(() => new Set());
  const [briefingError, setBriefingError] = useState("");
  // Tabela do campeonato: alterna entre pilotos e construtores; hover destaca a dupla da equipe.
  const [standingsView, setStandingsView] = useState("pilotos");
  const [hoveredStandingsTeamId, setHoveredStandingsTeamId] = useState(null);
  // Piloto realçado ao passar o mouse num nome mencionado no texto do engenheiro: acende
  // o mesmo piloto nos Favoritos e na Tabela do Campeonato.
  const [hoveredDriverId, setHoveredDriverId] = useState(null);
  // Prévia pré-corrida por IA (narrativa + voz da equipe, curtas). null → template.
  const [aiBriefing, setAiBriefing] = useState(null);
  // A prévia de IA está sendo buscada agora? Enquanto true, exibimos um skeleton no
  // lugar do template (evita o flash template→IA quando a IA chega logo em seguida).
  const [aiPending, setAiPending] = useState(false);
  // Reroll de debug da prévia por IA (força regenerar, ignora cache + cooldown).
  const [aiReroll, setAiReroll] = useState({ busy: false, status: null });
  // Debug: ver o template original mesmo quando há prévia de IA em cache. A IA fica
  // guardada em `aiBriefing`, então alternar de volta não regenera nada.
  const [showTemplate, setShowTemplate] = useState(false);

  const player = useCareerStore((state) => state.player);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const nextRace = useCareerStore((state) => state.nextRace);
  const nextRaceBriefing = useCareerStore((state) => state.nextRaceBriefing);
  const preRaceAi = useCareerStore((state) => state.preRaceAi);
  const temporalSummary = useCareerStore((state) => state.temporalSummary);
  const season = useCareerStore((state) => state.season);
  const playerInterests = useCareerStore((state) => state.playerInterests);
  const language = useCareerStore((state) => state.language);
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
  // Glow de atenção dos cards (Clima, Risco de Quebra): pulsa até o jogador abrir
  // o card NESTA corrida; depois cala. Reaparece sozinho na corrida seguinte.
  // Assino a fatia específica de `seen` pra o card recalcular o glow assim que abre.
  const markAttnSeen = useAttentionStore((state) => state.markSeen);
  const raceId = nextRace?.id;
  const weatherSeen = useAttentionStore(
    (state) => !!raceId && !!state.seen[`${raceId}:weather`]
  );
  const breakdownSeen = useAttentionStore(
    (state) => !!raceId && !!state.seen[`${raceId}:breakdown`]
  );
  const weatherGlow = weatherSeen ? "" : "attn-glow-delayed";
  const breakdownGlow = breakdownSeen ? "" : "attn-glow-delayed";
  const phase = season?.fase;
  const isLegacyPhase = isLegacySeasonPhase(phase);
  const hasPendingRegularRaces =
    isLegacyPhase && phase === "BlocoRegular" && (temporalSummary?.pending_in_phase ?? 0) > 0;

  useEffect(() => {
    let active = true;

    async function detectPreseason() {
      if (!careerId || nextRace || phase !== "PreTemporada") return;

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
  }, [careerId, enterPreseason, nextRace, phase]);

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

      // O prefetch (animação de avanço) já buscou os standings desta etapa e guardou na
      // store. A pré-corrida é estática até a corrida rodar, então usamos o cache direto
      // e evitamos re-disparar os comandos pesados — a Sala abre os Favoritos na hora.
      const cached = readCachedPreRaceStandings();
      if (cached) {
        if (active) {
          setDriverStandings(cached.driverStandings);
          setTeamStandings(cached.teamStandings);
          setBriefingPhraseHistory(cached.phraseHistory);
          setBriefingError("");
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
            : invokeError?.toString?.() ?? t("nextRaceTab.errors.buildBriefing"),
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
    // Dep em `nextRace?.id` (não no objeto): o store recria o objeto `nextRace` em
    // atualizações não relacionadas, e depender do objeto refazia este fetch + resetava
    // `isLoadingBriefing(true)` em loop → "Montando análise" preso. Alinha com os
    // effects irmãos abaixo, que já usam `nextRace?.id`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [careerId, nextRace?.id, playerTeam?.categoria]);

  // Previsão de risco de quebra da próxima corrida (Monte Carlo sobre o desgaste real do carro).
  useEffect(() => {
    let active = true;
    if (!careerId) return undefined;
    invoke("get_breakdown_forecast", { careerId })
      .then((f) => {
        if (active) setBreakdownForecast(f && f.available ? f : null);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [careerId, nextRace?.id]);

  // Equipes com risco real de quebra na próxima corrida (marcador 🔧 na tabela do campeonato).
  useEffect(() => {
    let active = true;
    if (!careerId) return undefined;
    invoke("get_grid_breakdown_risk", { careerId })
      .then((ids) => {
        if (active) setBreakdownRiskTeams(new Set(Array.isArray(ids) ? ids : []));
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [careerId, nextRace?.id]);

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
        playerInterests,
        breakdownForecast,
      }),
    [
      player,
      playerTeam,
      season,
      nextRace,
      nextRaceBriefing,
      playerInterests,
      driverStandings,
      teamStandings,
      briefingPhraseHistory,
      breakdownForecast,
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

  // Decide se a opção "pegar a cor do carro" aparece na Sala de Estratégia: só
  // quando o jogador JÁ conectou ao iRacing (temos o custid — ele correu de verdade,
  // não simulou) e ainda NÃO vinculou a cor a este save. Checa o vínculo uma vez e
  // então faz um poll leve da conexão (o custid pode surgir enquanto ele está aqui).
  useEffect(() => {
    let active = true;
    if (!careerId) {
      setCanPickPaint(false);
      return undefined;
    }
    let handle;
    invoke("iracing_linked_custid", { careerId })
      .then((linked) => {
        if (!active) return;
        if (linked !== null && linked !== undefined) {
          setCanPickPaint(false); // já vinculado → não aparece mais aqui (vai pro mercado)
          return;
        }
        const check = () => {
          invoke("iracing_has_player_id")
            .then((hasId) => {
              if (active) setCanPickPaint(Boolean(hasId));
            })
            .catch(() => {});
        };
        check();
        handle = setInterval(check, 5000);
      })
      .catch(() => {});
    return () => {
      active = false;
      if (handle) clearInterval(handle);
    };
  }, [careerId]);

  async function handleGrabPaint() {
    if (!careerId) return;
    const categoria = (playerTeam?.categoria ?? "").toLowerCase();
    const carKey =
      categoria.includes("gr86") || categoria.includes("toyota")
        ? "gr86"
        : categoria.includes("bmw") || categoria.includes("m2")
        ? "bmwm2"
        : "mx5";
    setPaintBusy(true);
    setPaintError("");
    try {
      const res = await invoke("iracing_link_player_paint", { careerId, carKey });
      setShowPaintPrompt(false);
      setCanPickPaint(false); // vinculado → não aparece mais na Sala de Estratégia
      setPaintToast(
        t("nextRaceTab.paint.toastPainted", {
          color: res?.color ?? t("nextRaceTab.paint.teamColorFallback"),
        }),
      );
      const timer = setTimeout(() => setPaintToast(""), 6000);
      toastTimers.current.push(timer);
    } catch (e) {
      setPaintError(getDisplayError(e, t("nextRaceTab.errors.grabPaint")));
    } finally {
      setPaintBusy(false);
    }
  }

  // Prévia pré-corrida por IA: ao abrir a Sala de Estratégia, manda os fatos do
  // briefing ao servidor e troca a narrativa + voz da equipe pela versão da IA.
  // Cacheada por etapa no backend; em cooldown/erro mantém o template (aiBriefing
  // fica null). Mostra o template imediatamente e troca quando a IA chega.
  useEffect(() => {
    let active = true;
    const raceId = nextRace?.id;
    const facts = briefing.aiFacts;
    setAiBriefing(null);
    setAiPending(false);
    // Prefetch durante a animação de avanço já gerou esta etapa → usa direto (sem
    // novo fetch e sem flash; o render lê de `preRaceAi`).
    if (preRaceAi?.raceId && preRaceAi.raceId === raceId) {
      return undefined;
    }
    // Só dispara com o contexto do briefing já carregado (standings/forma), senão
    // poderíamos cachear uma prévia com fatos incompletos.
    if (!careerId || !raceId || !facts || isLoadingBriefing) {
      return undefined;
    }
    // Busca em voo → skeleton no lugar do template até a IA chegar (ou o teto estourar).
    setAiPending(true);
    const maxWait = window.setTimeout(() => {
      if (active) setAiPending(false);
    }, AI_PREVIEW_MAX_WAIT_MS);
    invoke("pre_race_briefing_ai", { careerId, raceId, facts })
      .then((res) => {
        if (active && res?.narrative && res?.team_voice) {
          setAiBriefing({
            headline: res.headline ?? null,
            narrative: res.narrative,
            teamVoice: res.team_voice,
          });
        }
      })
      .catch(() => {})
      .finally(() => {
        if (active) setAiPending(false);
      });
    return () => {
      active = false;
      window.clearTimeout(maxWait);
    };
  }, [careerId, nextRace?.id, briefing.aiFacts, isLoadingBriefing, preRaceAi?.raceId]);

  // --- Detecção de leitura da prévia (alimenta o gate de engajamento da IA) ---
  // Cronometra o tempo na Sala de Estratégia por etapa. "Leu" = ficou ≥ PRE_RACE_READ_MS
  // (exportar não para o cronômetro; simular/sair antes conta como não-leu). Reporta no
  // simular ou ao trocar de corrida / sair da tela (cleanup). Guard evita report duplo.
  const viewStartRef = useRef(0);
  const readReportedRef = useRef(false);

  const reportPreRaceEngagement = useCallback(() => {
    if (readReportedRef.current) return;
    readReportedRef.current = true;
    if (!careerId) return;
    const read = Date.now() - viewStartRef.current >= PRE_RACE_READ_MS;
    invoke("report_pre_race_engagement", { careerId, read }).catch(() => {});
  }, [careerId]);

  useEffect(() => {
    if (!nextRace?.id) return undefined;
    viewStartRef.current = Date.now();
    readReportedRef.current = false;
    return () => {
      reportPreRaceEngagement();
    };
  }, [nextRace?.id, reportPreRaceEngagement]);

  // Reroll de debug: força o servidor a regenerar a prévia (ignora cache e cooldown)
  // e troca a narrativa + voz da equipe na hora. Útil para afinar fatos/prompt.
  async function handleRerollAi() {
    const raceId = nextRace?.id;
    const facts = briefing.aiFacts;
    if (!careerId || !raceId || !facts || aiReroll.busy) {
      return;
    }
    setAiReroll({ busy: true, status: null });
    try {
      const res = await invoke("pre_race_briefing_ai", { careerId, raceId, facts, force: true });
      if (res?.narrative && res?.team_voice) {
        setAiBriefing({
          headline: res.headline ?? null,
          narrative: res.narrative,
          teamVoice: res.team_voice,
        });
        setShowTemplate(false); // mostra o resultado novo da IA
        setAiReroll({ busy: false, status: res.status ?? "ok" });
      } else {
        setAiReroll({ busy: false, status: res?.status ?? "error" });
      }
    } catch (e) {
      setAiReroll({ busy: false, status: "error" });
    }
  }

  async function handleSimulate() {
    setConfirmSim(false);
    setError("");
    setExportNotice("");
    // Registra a leitura desta etapa antes de sair da tela (simular = ação de saída).
    reportPreRaceEngagement();

    try {
      await simulateRace();
    } catch (invokeError) {
      setError(getDisplayError(invokeError, t("nextRaceTab.errors.simulate")));
    }
  }

  async function handleSeasonAdvance() {
    setError("");
    setExportNotice("");

    try {
      // LEGADO 9D: o bloco especial só segue vivo para saves pré-v33 em voo.
      if (isLegacyPhase && phase === "BlocoEspecial") {
        await finishSpecialBlock();
        return;
      }

      if (!nextRace && hasPendingRegularRaces) {
        await startCalendarAdvance();
        return;
      }

      if (isLegacyPhase && !nextRace && phase === "BlocoRegular") {
        await runConvocationWindow();
        return;
      }

      if (isLegacyPhase && !nextRace && phase === "PosEspecial") {
        await advanceSeason();
        return;
      }

      if (phase === "PreTemporada" || hasExistingPreseason) {
        await enterPreseason();
        return;
      }

      await advanceSeason();
    } catch (invokeError) {
      setError(getDisplayError(invokeError, t("nextRaceTab.errors.advancePreseason")));
    }
  }

  async function handleExport() {
    setError("");
    const categoria = playerTeam?.categoria;
    if (!careerId || !categoria) {
      setError(t("nextRaceTab.errors.noCareerCategory"));
      return;
    }
    const rosterName = `Carreira ${player?.nome ?? "Loop"}`.trim();
    const cat = categoria.toLowerCase();
    const carKey = cat.includes("gr86") || cat.includes("toyota")
      ? "gr86"
      : cat.includes("bmw") || cat.includes("m2")
      ? "bmwm2"
      : "mx5"; // mazda e padrão
    setExportNotice("");
    setIracingFocusMsg("");
    setIsExporting(true);
    try {
      await invoke("iracing_generate_roster", { careerId, categoria, rosterName, carKey });
      await invoke("iracing_generate_season", { careerId, categoria, rosterName, carKey });
      // Garante a macro de bandeira instalada agora — o iRacing está fechado neste
      // momento, então a escrita no app.ini "cola" (o sim só reescreve ao fechar).
      // Não-fatal: se falhar (ex.: app.ini não encontrado), não bloqueia a exportação.
      await invoke("iracing_install_yellow_macro").catch(() => {});
      dismissToasts();
      setExported(true);
      exportSuccess();
      // Stack de toasts: "Dados exportados" agora; "Entrar no iRacing" surge logo
      // abaixo (empurrando o primeiro pra cima). Ambos somem em 15s.
      toastTimers.current.push(setTimeout(() => setShowGoToast(true), 550));
      toastTimers.current.push(setTimeout(() => dismissToasts(), 15000));
    } catch (invokeError) {
      setError(getDisplayError(invokeError, t("nextRaceTab.errors.export")));
    } finally {
      setIsExporting(false);
    }
  }

  // Limpa os toasts pós-exportação e seus timers.
  function dismissToasts() {
    toastTimers.current.forEach(clearTimeout);
    toastTimers.current = [];
    setExported(false);
    setShowGoToast(false);
    setIracingFocusMsg("");
  }

  // Vai pro iRacing de fato: foca a janela se aberta; senão tenta abrir o
  // iRacingUI. Devolve { ok, message } para quem chamou decidir onde mostrar.
  async function goToIracingNow() {
    try {
      const focused = await invoke("iracing_focus_window");
      if (focused) return { ok: true };
      const launched = await invoke("iracing_launch_ui");
      if (launched) return { ok: true };
      return {
        ok: false,
        message: t("nextRaceTab.iracing.notOpen"),
      };
    } catch {
      return { ok: false, message: t("nextRaceTab.iracing.cantOpen") };
    }
  }

  // Clique no toast de ação: na 1ª vez abre o tutorial; depois vai direto.
  async function handleGoToIracing() {
    setIracingFocusMsg("");
    const seen = (() => {
      try {
        return localStorage.getItem(IRACING_TUTORIAL_KEY) === "1";
      } catch {
        return false;
      }
    })();
    if (!seen) {
      setTutorialMsg("");
      setShowTutorial(true);
      return;
    }
    const res = await goToIracingNow();
    if (res.ok) dismissToasts();
    else setIracingFocusMsg(res.message);
  }

  function markTutorialSeen() {
    try {
      localStorage.setItem(IRACING_TUTORIAL_KEY, "1");
    } catch {
      /* ignore */
    }
  }

  // "Done" no último passo: marca como visto e leva pro iRacing.
  async function handleTutorialDone() {
    markTutorialSeen();
    const res = await goToIracingNow();
    if (res.ok) {
      setShowTutorial(false);
      dismissToasts();
    } else {
      setTutorialMsg(res.message);
    }
  }

  // Fechar (X/fora): também marca como visto — o tutorial é só na 1ª vez.
  function handleTutorialClose() {
    markTutorialSeen();
    setShowTutorial(false);
  }

  if (!nextRace) {
    const isFreeAgent = !playerTeam;
    const emptyHeading = isFreeAgent
      ? t("nextRaceTab.status.noTeamHeading")
      : phase === "PreTemporada"
      ? t("nextRaceTab.status.preseasonOpenHeading")
      : phase === "Encerramento"
      ? t("nextRaceTab.status.seasonEndHeading")
      : isLegacyPhase && phase === "BlocoEspecial"
      ? t("nextRaceTab.status.specialBlockHeading")
      : isLegacyPhase && phase === "PosEspecial"
      ? t("nextRaceTab.status.specialDoneHeading")
      : t("nextRaceTab.status.seasonFinishedHeading");
    const emptyDescription = isFreeAgent
      ? t("nextRaceTab.status.noTeamDesc")
      : phase === "PreTemporada"
      ? t("nextRaceTab.status.preseasonOpenDesc")
      : phase === "Encerramento"
      ? t("nextRaceTab.status.seasonEndDesc")
      : isLegacyPhase && phase === "BlocoEspecial"
      ? t("nextRaceTab.status.specialBlockDesc")
      : isLegacyPhase && phase === "BlocoRegular"
      ? hasPendingRegularRaces
        ? t("nextRaceTab.status.regularPendingDesc")
        : t("nextRaceTab.status.regularDoneDesc")
      : isLegacyPhase && phase === "PosEspecial"
      ? t("nextRaceTab.status.posEspecialDesc")
      : hasExistingPreseason
      ? t("nextRaceTab.status.preseasonStartedDesc")
      : t("nextRaceTab.status.allRacesDoneDesc");
    const emptyButtonLabel = isFreeAgent
      ? t("nextRaceTab.actions.skipSeason")
      : isLegacyPhase && phase === "BlocoEspecial"
      ? t("nextRaceTab.actions.skipSpecialBlock")
      : hasPendingRegularRaces
      ? t("nextRaceTab.actions.advanceCalendar")
      : isLegacyPhase && phase === "BlocoRegular"
      ? t("nextRaceTab.actions.advanceToCallup")
      : isLegacyPhase && phase === "PosEspecial"
      ? t("nextRaceTab.actions.endSeason")
      : phase === "PreTemporada" || hasExistingPreseason
      ? t("nextRaceTab.actions.continuePreseason")
      : t("nextRaceTab.actions.advanceToPreseason");
    return (
      <div className="relative">
        <LoadingOverlay
          open={isAdvancing || isConvocating || isEnteringPreseason}
          title={
            isEnteringPreseason
              ? t("nextRaceTab.loading.openingMarketTitle")
              : isFreeAgent
              ? t("nextRaceTab.loading.skippingSeasonTitle")
              : isLegacyPhase && phase === "BlocoEspecial"
              ? t("nextRaceTab.loading.simulatingSpecialTitle")
              : isLegacyPhase && phase === "BlocoRegular"
              ? t("nextRaceTab.loading.openingCallupTitle")
              : t("nextRaceTab.loading.turningSeasonTitle")
          }
          message={
            isEnteringPreseason
              ? t("nextRaceTab.loading.openingMarketMsg")
              : isFreeAgent
              ? t("nextRaceTab.loading.skippingSeasonMsg")
              : isLegacyPhase && phase === "BlocoEspecial"
              ? t("nextRaceTab.loading.simulatingSpecialMsg")
              : isLegacyPhase && phase === "BlocoRegular"
              ? t("nextRaceTab.loading.openingCallupMsg")
              : t("nextRaceTab.loading.turningSeasonMsg")
          }
        />

        <GlassCard hover={false} className="rounded-[28px] p-10">
          <div className="py-6 text-center">
            <div className="text-6xl">{isFreeAgent ? "🏳️" : "PQ"}</div>
            <p className="mt-4 text-sm uppercase tracking-[0.22em] text-accent-primary">
              {isFreeAgent ? t("nextRaceTab.labels.freeAgent") : t("nextRaceTab.labels.nextRace")}
            </p>
            <h2 className="mt-3 text-3xl font-semibold text-text-primary">
              {emptyHeading}
            </h2>
            <p className="mt-3 text-sm text-text-secondary">
              {emptyDescription}
            </p>
            <div className="mt-6">
              <GlassButton
                variant="primary"
                disabled={isAdvancing || isConvocating || isEnteringPreseason}
                onClick={() => {
                if (isFreeAgent) {
                  setError("");
                  skipAllPendingRaces().catch((e) => {
                    setError(getDisplayError(e, t("nextRaceTab.errors.skipSeason")));
                  });
                } else {
                  void handleSeasonAdvance();
                }
              }}
              >
                {isAdvancing || isConvocating || isEnteringPreseason
                  ? t("nextRaceTab.actions.processing")
                  : emptyButtonLabel}
              </GlassButton>
            </div>
            {error ? <p className="mt-4 text-sm text-status-red">{error}</p> : null}
          </div>
        </GlassCard>
      </div>
    );
  }

  // Prévia da IA a exibir: o reroll/fetch local (`aiBriefing`) tem prioridade; senão,
  // a versão pré-buscada na animação de avanço (`preRaceAi`), se for desta etapa.
  const prefetchedAi =
    preRaceAi?.raceId && preRaceAi.raceId === nextRace?.id
      ? {
          headline: preRaceAi.headline ?? null,
          narrative: preRaceAi.narrative,
          teamVoice: preRaceAi.teamVoice,
        }
      : null;
  const effectiveAi = aiBriefing ?? prefetchedAi;
  // Exibindo a versão da IA? (há prévia e o debug não forçou o template.)
  const usingAi = Boolean(effectiveAi?.narrative) && !showTemplate;
  // Buscando a IA e ainda sem nada pra mostrar → skeleton (não o template), pra não
  // piscar o template por 1s antes da IA. `showTemplate` (debug) sempre vence.
  const showAiSkeleton = aiPending && !usingAi && !showTemplate;
  // Sem IA e fora do skeleton: em português mostramos o template determinístico; em
  // outro idioma, "erro na geração de texto" (não despejamos PT para quem não é PT).
  const aiFallbackError =
    !usingAi && !showAiSkeleton && !isPortuguese(language) ? localizedAiError(language) : null;
  // Controles de debug da IA (Rerolar / Ver template / badge): só em dev, ou se
  // VITE_AI_DEBUG=true. No build de produção ficam ocultos para o jogador.
  const showAiDebug = import.meta.env.DEV || import.meta.env.VITE_AI_DEBUG === "true";
  // Pilotos cujos nomes o texto do engenheiro pode mencionar (elenco da categoria).
  const mentionDrivers = briefing.championshipTable ?? [];
  const renderNarrative = (text) =>
    renderTextWithDriverMentions(text, mentionDrivers, hoveredDriverId, setHoveredDriverId);

  return (
    <div className="relative min-h-[calc(100vh-100px)]">
      {/* Background glass effect specific to this dashboard */}
      <div className="fixed inset-0 z-0 overflow-hidden pointer-events-none opacity-60">
        <div className="absolute inset-x-0 -top-40 h-[600px] bg-[url('https://images.unsplash.com/photo-1541443131876-44b03de101c5?auto=format&fit=crop&q=80')] bg-cover opacity-15 filter blur-[30px] mix-blend-screen transform scale-110"></div>
        <div className="absolute inset-0 bg-gradient-to-b from-transparent via-[#06090e]/80 to-[#06090e]"></div>
      </div>

      {/* Popup: pegar a cor do carro (1ª corrida da 1ª temporada) */}
      {showPaintPrompt && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm">
          <div className="w-full max-w-md rounded-2xl border border-white/10 bg-[#0d1117] p-6 shadow-2xl">
            <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-[#58a6ff]">
              <span className="mr-2">🎨</span>{t("nextRaceTab.paint.eyebrow")}
            </p>
            <h2 className="mt-2 text-xl font-extrabold text-white">
              {t("nextRaceTab.paint.promptTitle")}
            </h2>
            <p className="mt-2 text-sm leading-relaxed text-gray-400">
              {t("nextRaceTab.paint.promptBody")}
            </p>

            {paintError && (
              <p className="mt-3 rounded-lg border border-status-red/30 bg-status-red/10 px-3 py-2 text-xs text-status-red">
                {paintError}
              </p>
            )}

            <div className="mt-5 flex flex-col gap-2">
              <button
                onClick={handleGrabPaint}
                disabled={paintBusy}
                className="w-full rounded-lg border border-[#58a6ff66] bg-[#58a6ff33] px-4 py-3 text-sm font-bold text-[#58a6ff] transition hover:bg-[#58a6ff55] disabled:cursor-not-allowed disabled:opacity-60"
              >
                {paintBusy ? t("nextRaceTab.paint.painting") : t("nextRaceTab.paint.grab")}
              </button>
              <button
                onClick={() => {
                  setShowPaintPrompt(false);
                  setPaintError("");
                }}
                disabled={paintBusy}
                className="w-full rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-xs font-semibold text-gray-400 transition hover:bg-white/10 disabled:opacity-60"
              >
                {t("nextRaceTab.paint.notNow")}
              </button>
            </div>
            <p className="mt-3 text-center text-[10px] text-gray-500">
              {t("nextRaceTab.paint.requiresTradingPaints")}
            </p>
          </div>
        </div>
      )}

      {/* Toast de confirmação da pintura */}
      {paintToast && (
        <div className="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-xl border border-[#58a6ff44] bg-[#0d1117] px-4 py-2.5 text-sm font-semibold text-white shadow-2xl">
          {paintToast}
        </div>
      )}

      <div className="relative z-10 space-y-6">
        <LoadingOverlay
          open={isSimulating}
          title={t("nextRaceTab.loading.simulatingRaceTitle")}
          message={t("nextRaceTab.loading.simulatingRaceMsg")}
        />

        {/* HEADER COM BOTÕES */}
        <header className="flex flex-col md:flex-row justify-between items-start md:items-end mb-4">
          <div>
            <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-[#58a6ff] mb-2">
              <span className="mr-2">🏁</span>{t("nextRaceTab.labels.strategyRoom")}
            </p>
            <h1 className="text-[2.5rem] font-extrabold text-white leading-none">{nextRace.track_name}</h1>
            <div className="flex flex-wrap items-center gap-3 mt-3">
              <span className="border border-white/10 bg-white/5 px-3 py-1.5 rounded-lg text-xs font-bold text-white">
                {t("nextRaceTab.labels.stageOf", {
                  round: nextRace.rodada,
                  total: season?.total_rodadas ?? "?",
                })}
              </span>
              <span className="text-sm font-medium text-gray-400 capitalize">
                {briefing.eventDateShort} • {briefing.timePeriodHighlight}
              </span>
            </div>
          </div>

          <div className="flex flex-col sm:flex-row items-center gap-4 mt-6 md:mt-0 w-full sm:w-auto">
            <div className="flex flex-col items-center gap-1 w-full sm:w-auto">
              <button
                onClick={() => setConfirmSim(true)}
                disabled={isSimulating || !nextRace}
                className="w-full sm:w-auto px-5 py-2 border border-white/10 bg-white/5 hover:bg-white/10 text-gray-300 font-semibold rounded-lg transition text-xs flex justify-center items-center gap-1.5 opacity-80 hover:opacity-100 disabled:opacity-50"
              >
                {isSimulating ? t("nextRaceTab.actions.simulating") : t("nextRaceTab.actions.simulateRace")}
                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" className="w-4 h-4 text-[#58a6ff]">
                  <path fillRule="evenodd" d="M14.615 1.595a.75.75 0 01.359.852L12.982 9.75h7.268a.75.75 0 01.548 1.262l-10.5 11.25a.75.75 0 01-1.272-.71l1.992-7.302H3.75a.75.75 0 01-.548-1.262l10.5-11.25a.75.75 0 01.913-.143z" clipRule="evenodd" />
                </svg>
              </button>
              {confirmSim && !isSimulating && (
                <span className="text-[11px] text-gray-400 whitespace-nowrap">
                  {t("nextRaceTab.actions.simulateConfirm")}{" "}
                  <button onClick={handleSimulate} className="text-[#58a6ff] font-semibold hover:underline">
                    {t("nextRaceTab.actions.yes")}
                  </button>
                  {" · "}
                  <button onClick={() => setConfirmSim(false)} className="text-gray-500 hover:underline">
                    {t("nextRaceTab.actions.cancel")}
                  </button>
                </span>
              )}
            </div>
            {canPickPaint && (
              <button
                onClick={() => {
                  setPaintError("");
                  setShowPaintPrompt(true);
                }}
                className="w-full sm:w-auto px-5 py-2 border border-[#58a6ff66] bg-[#58a6ff22] hover:bg-[#58a6ff33] text-[#58a6ff] font-semibold rounded-lg transition text-xs flex justify-center items-center gap-1.5"
              >
                {t("nextRaceTab.paint.grab")}
              </button>
            )}
            <button
              onClick={handleExport}
              disabled={isExporting}
              className={`w-full sm:w-auto px-10 py-3.5 font-black uppercase rounded-xl transition text-base flex justify-center items-center gap-2 disabled:opacity-70 ${
                exported
                  ? "bg-green-500 hover:bg-green-400 text-[#06090e] shadow-[0_0_22px_rgba(34,197,94,0.55)]"
                  : "bg-[#58a6ff] hover:bg-blue-400 text-[#06090e] shadow-[0_0_20px_rgba(88,166,255,0.4)]"
              }`}
            >
              {exported ? (
                <>
                  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" className="w-5 h-5">
                    <path fillRule="evenodd" d="M19.916 4.626a.75.75 0 01.208 1.04l-9 13.5a.75.75 0 01-1.154.114l-6-6a.75.75 0 011.06-1.06l5.353 5.353 8.493-12.74a.75.75 0 011.04-.207z" clipRule="evenodd" />
                  </svg>
                  {t("nextRaceTab.actions.exported")}
                </>
              ) : isExporting ? (
                t("nextRaceTab.actions.exporting")
              ) : (
                t("nextRaceTab.actions.run")
              )}
            </button>
          </div>
        </header>

        {exportNotice && <p className="text-right text-sm text-[#58a6ff]">{exportNotice}</p>}
        {error && <p className="text-right text-sm text-red-500">{error}</p>}

        {exported && (
          <div className="fixed bottom-6 right-6 z-50 flex w-[300px] flex-col items-stretch gap-3">
            {/* Toast 1 — confirmação (empurrado pra cima quando o 2 surge) */}
            <div className="animate-toast-up flex items-center gap-3 rounded-2xl border border-green-300/40 bg-green-500/95 px-4 py-3.5 text-[#06090e] shadow-[0_10px_30px_rgba(34,197,94,0.45)]">
              <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[#06090e]/15">
                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" className="h-4 w-4">
                  <path fillRule="evenodd" d="M19.916 4.626a.75.75 0 01.208 1.04l-9 13.5a.75.75 0 01-1.154.114l-6-6a.75.75 0 011.06-1.06l5.353 5.353 8.493-12.74a.75.75 0 011.04-.207z" clipRule="evenodd" />
                </svg>
              </span>
              <div className="min-w-0">
                <p className="text-sm font-bold leading-tight">{t("nextRaceTab.toast.exportedTitle")}</p>
                <p className="text-[11px] font-medium leading-tight text-[#06090e]/70">{t("nextRaceTab.toast.exportedMsg")}</p>
              </div>
            </div>

            {/* Toast 2 — ação (surge embaixo, empurrando o de cima) */}
            {showGoToast && (
              <div className="animate-toast-up">
                <button
                  onClick={handleGoToIracing}
                  className="group flex w-full items-center gap-3 rounded-2xl border border-[#58a6ff]/40 bg-[#101826]/95 px-4 py-3.5 text-left shadow-[0_10px_30px_rgba(0,0,0,0.45)] backdrop-blur-md transition hover:border-[#58a6ff]/70 hover:bg-[#16223a]/95"
                >
                  <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[#58a6ff]/15 text-base">🏁</span>
                  <div className="min-w-0 flex-1">
                    <p className="text-sm font-bold leading-tight text-text-primary">{t("nextRaceTab.toast.enterIracingTitle")}</p>
                    <p className={`text-[11px] font-medium leading-tight ${iracingFocusMsg ? "text-red-400" : "text-text-muted"}`}>
                      {iracingFocusMsg || t("nextRaceTab.toast.bringSimForward")}
                    </p>
                  </div>
                  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" className="h-4 w-4 shrink-0 text-[#58a6ff] transition-transform group-hover:translate-x-0.5">
                    <path d="m9 18 6-6-6-6" />
                  </svg>
                </button>
              </div>
            )}
          </div>
        )}

        {showTutorial && (
          <IracingTutorialModal
            onFinish={handleTutorialDone}
            onClose={handleTutorialClose}
            message={tutorialMsg}
          />
        )}

        {/* GRID PRINCIPAL (4-4-4) */}
        <div className="grid grid-cols-1 xl:grid-cols-12 gap-6 items-stretch pb-10">
          
          {/* 1) NARRATIVA DA ETAPA */}
          <div className="xl:col-span-4 flex flex-col gap-5 xl:h-[calc(100vh-17rem)] xl:min-h-[650px]">
            {/* Condições Compactas — o CARD INTEIRO abre a previsão do tempo. */}
            <WeatherButton
              careerId={careerId}
              raceId={nextRace?.id}
              forecast
              onOpen={() => markAttnSeen(raceId, "weather")}
              className={`group ${weatherGlow} w-full text-left bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-5 flex justify-between items-center bg-gradient-to-r from-black/40 to-transparent transition hover:border-[#58a6ff]/40`}
            >
              <div className="flex items-center gap-4">
                <div className="text-4xl">{briefing.weatherIcon}</div>
                <div>
                  <p className="text-[10px] uppercase tracking-widest text-[#58a6ff] font-bold">
                    {t("nextRaceTab.labels.trackCondition")} <span className="opacity-60 group-hover:opacity-100">›</span>
                  </p>
                  <p className="text-xl font-bold text-white">
                    {briefing.weatherSummary} <span className="text-xs text-gray-400">{briefing.trackTemperatureLabel}</span>
                  </p>
                  <p className="text-[10px] text-gray-500 group-hover:text-[#58a6ff] transition">
                    {t("nextRaceTab.labels.seeForecast")}
                  </p>
                </div>
              </div>
              <div className="text-right">
                <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">{t("nextRaceTab.labels.audience")}</p>
                <p className="text-xl font-bold text-white">{formatAudience(briefing.audienceEstimate)}</p>
              </div>
            </WeatherButton>

            {/* Risco de quebra (aviso pré-corrida) — card compacto pulsante que abre o
                detalhamento; o glow cala depois de aberto nesta corrida. */}
            <BreakdownRiskButton
              forecast={breakdownForecast}
              onOpen={() => markAttnSeen(raceId, "breakdown")}
              className={`group ${breakdownGlow} w-full text-left bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-5 transition hover:border-[#58a6ff]/40`}
            />

            {/* Narrativa Expandida */}
            <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-6 flex-1 flex flex-col relative overflow-hidden">
              <div className="absolute -right-10 -top-10 h-40 w-40 rounded-full bg-[radial-gradient(circle,rgba(240,195,107,0.1),transparent_65%)] pointer-events-none"></div>
              <div className="flex items-center justify-between mb-4 relative z-10">
                <p className="text-[11px] uppercase tracking-[0.2em] text-[#f5c76d] font-bold flex items-center">
                  <span className="mr-2 text-sm">🎧</span>{t("nextRaceTab.labels.trackEngineer")}
                  {showAiDebug && effectiveAi ? (
                    usingAi ? (
                      <span className="ml-2 px-1.5 py-0.5 rounded text-[9px] tracking-normal bg-[#58a6ff]/15 text-[#58a6ff] border border-[#58a6ff]/30">
                        {t("nextRaceTab.debug.aiBadge")}
                      </span>
                    ) : (
                      <span className="ml-2 px-1.5 py-0.5 rounded text-[9px] tracking-normal bg-white/[0.06] text-gray-400 border border-white/10">
                        {t("nextRaceTab.debug.templateBadge")}
                      </span>
                    )
                  ) : null}
                </p>
              </div>

              <div className="flex-1 overflow-y-auto custom-scrollbar pr-2 relative z-10 flex flex-col">
                {showAiSkeleton ? (
                  <div className="space-y-5" aria-hidden="true">
                    <div className="h-7 w-3/4 rounded-lg bg-white/[0.07] animate-pulse" />
                    <div className="space-y-2.5">
                      <div className="h-4 w-full rounded bg-white/[0.05] animate-pulse" />
                      <div className="h-4 w-[94%] rounded bg-white/[0.05] animate-pulse" />
                      <div className="h-4 w-[86%] rounded bg-white/[0.05] animate-pulse" />
                    </div>
                    <div className="space-y-2.5">
                      <div className="h-4 w-[96%] rounded bg-white/[0.05] animate-pulse" />
                      <div className="h-4 w-[68%] rounded bg-white/[0.05] animate-pulse" />
                    </div>
                  </div>
                ) : usingAi ? (
                  <>
                    {effectiveAi.headline ? (
                      <h3 className="text-2xl font-bold text-white leading-snug mb-4">{effectiveAi.headline}</h3>
                    ) : null}
                    {effectiveAi.narrative
                      .split(/\n{2,}/)
                      .map((para) => para.trim())
                      .filter(Boolean)
                      .map((para, index) => (
                        <p key={index} className="text-[15px] text-gray-300 leading-relaxed mb-4">
                          {renderNarrative(para)}
                        </p>
                      ))}
                  </>
                ) : aiFallbackError ? (
                  <p className="text-[15px] text-gray-500 italic leading-relaxed">{aiFallbackError}</p>
                ) : (
                  <>
                    <h3 className="text-2xl font-bold text-white leading-snug mb-4">{briefing.headline}</h3>
                    <p className="text-[15px] text-gray-300 leading-relaxed mb-4">
                      {renderNarrative(briefing.paragraphs[0] ?? briefing.attendanceNarrative)}
                    </p>
                    <p className="text-[15px] text-gray-300 leading-relaxed mb-6">
                      {renderNarrative(briefing.paragraphs[1] || briefing.actionHint)}
                    </p>
                  </>
                )}

                {/* Leitura de Box Expandida */}
                <div className="bg-black/30 border border-white/5 p-4 rounded-2xl relative mt-auto">
                  <div className="absolute top-2 right-4 text-[#58a6ff] opacity-20 pointer-events-none">
                    <span className="text-6xl font-serif leading-none h-[40px] block overflow-hidden">"</span>
                  </div>
                  <p className="text-[10px] uppercase tracking-[0.15em] text-[#58a6ff] mb-2 font-bold">
                    {t("nextRaceTab.labels.teamVoice")} <span className="text-gray-500 font-semibold normal-case tracking-normal">{t("nextRaceTab.labels.toPress")}</span>
                  </p>
                  {showAiSkeleton ? (
                    <div className="space-y-2" aria-hidden="true">
                      <div className="h-3.5 w-full rounded bg-white/[0.05] animate-pulse" />
                      <div className="h-3.5 w-[72%] rounded bg-white/[0.05] animate-pulse" />
                    </div>
                  ) : aiFallbackError ? (
                    <p className="text-sm italic text-gray-500 leading-relaxed">{aiFallbackError}</p>
                  ) : (
                    <p className="text-sm italic text-gray-200 leading-relaxed">"{renderNarrative(usingAi ? effectiveAi.teamVoice : briefing.quote)}"</p>
                  )}
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
          <div className="xl:col-span-4 flex flex-col gap-5 xl:h-[calc(100vh-17rem)] xl:min-h-[650px]">
            {/* Aviso de contrato expirando */}
            {nextRaceBriefing?.contract_warning != null &&
              Math.max(0, (season?.total_rodadas ?? 0) - (nextRace?.rodada ?? 0)) <= 1 && (
              <div className="bg-amber-900/30 border border-amber-500/40 rounded-2xl px-4 py-3 flex items-start gap-3">
                <span className="text-amber-400 text-base leading-none mt-0.5">⚠</span>
                <div>
                  <p className="text-[10px] uppercase tracking-[0.15em] text-amber-400 font-bold mb-0.5">{t("nextRaceTab.labels.contractExpiring")}</p>
                  <p className="text-xs text-amber-100 leading-relaxed">
                    {t("nextRaceTab.labels.contractWarningPrefix")} <span className="font-semibold">{nextRaceBriefing.contract_warning.equipe_nome}</span> {t("nextRaceTab.labels.contractWarningSuffix")}
                  </p>
                </div>
              </div>
            )}

            {/* Metas */}
            <div className="grid grid-cols-3 gap-3">
              <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-2xl p-4 text-center flex flex-col justify-start items-center">
                <span className="text-2xl mb-1.5 block leading-none">👥</span>
                <p className="text-[9px] uppercase font-bold text-gray-500 tracking-wider">{t("nextRaceTab.labels.goalTeam")}</p>
                <p className="text-[10px] text-white font-semibold mt-1 leading-tight">
                  {briefing.goals[0]?.value}
                </p>
              </div>
              <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-2xl p-4 text-center flex flex-col justify-start items-center">
                <span className="text-xl mb-1.5 block leading-none">👤</span>
                <p className="text-[9px] uppercase font-bold text-gray-500 tracking-wider">{t("nextRaceTab.labels.goalPersonal")}</p>
                <p className="text-[10px] text-white font-semibold mt-1 leading-tight">
                  {briefing.goals[1]?.value}
                </p>
              </div>
              <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-2xl p-4 text-center flex flex-col justify-start items-center">
                <span className="text-xl mb-1.5 block leading-none">🏆</span>
                <p className="text-[9px] uppercase font-bold text-gray-500 tracking-wider">{t("nextRaceTab.labels.goalTitle")}</p>
                <p className="text-[10px] text-white font-semibold mt-1 leading-tight">
                  {briefing.goals[2]?.value}
                </p>
              </div>
            </div>

            {/* Favoritos ao Pódio */}
            <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-6 flex-1 flex flex-col min-h-0">
              <p className="text-[12px] font-bold uppercase tracking-[0.2em] text-[#58a6ff] mb-5">{t("nextRaceTab.labels.podiumFavorites")}</p>

              <div className="space-y-4 flex-1 overflow-y-auto custom-scrollbar pr-1">
                {isLoadingBriefing ? (
                  <p className="text-sm text-gray-400">{t("nextRaceTab.labels.buildingAnalysis")}</p>
                ) : (
                  briefing.favorites.map((driver, index) => {
                    let medalTone = getFavoriteMedalTone(index);
                    const isJogador = driver.is_jogador;
                    const isMentionHovered = hoveredDriverId != null && driver.id === hoveredDriverId;

                    return (
                      <div
                        key={driver.id}
                        className={`border rounded-2xl p-4 flex flex-col xl:flex-row gap-3 xl:gap-0 justify-between xl:items-center transition hover:bg-white/5 ${
                          isJogador ? "bg-[#58a6ff]/10 border-[#58a6ff]/30" : "bg-black/20 border-white/5"
                        }`}
                        style={
                          isMentionHovered
                            ? (() => {
                                const tone = getTeamGlow(driver.equipe_cor);
                                return { borderColor: tone.solid, boxShadow: `0 0 18px ${tone.glow}` };
                              })()
                            : undefined
                        }
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
                            <p className="text-base font-bold text-white leading-none mb-1.5 flex items-center gap-1.5">
                              {driver.nome}
                              <RivalMarker driverId={driver.id} />
                            </p>
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
          <div className="xl:col-span-4 h-[500px] xl:h-[calc(100vh-17rem)] xl:min-h-[650px]">
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
                briefing.championshipTable.length === 0 ? (
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
                        {briefing.championshipTable.map((driver) => {
                          const isPlayer = driver.is_jogador;
                          const isHoveredTeam =
                            hoveredStandingsTeamId != null && driver.equipe_id === hoveredStandingsTeamId;
                          const isMentionHovered = hoveredDriverId != null && driver.id === hoveredDriverId;
                          const teamColor = getReadableTeamColor(driver.equipe_cor);
                          return (
                            <tr
                              key={driver.id}
                              onMouseEnter={() => setHoveredStandingsTeamId(driver.equipe_id ?? null)}
                              onMouseLeave={() => setHoveredStandingsTeamId(null)}
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
                                        <span
                                          className="shrink-0 text-[11px] leading-none"
                                          title={t("nextRaceTab.labels.breakdownRiskDriverTip")}
                                          aria-label={t("nextRaceTab.labels.breakdownRiskDriverTip")}
                                        >
                                          🔧
                                        </span>
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
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                )
              ) : briefing.constructorsTable.length === 0 ? (
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
                      {briefing.constructorsTable.map((team) => {
                        const isPlayerTeam = briefing.playerTeamId != null && team.id === briefing.playerTeamId;
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
          
        </div>
      </div>
    </div>
  );
}

export default NextRaceTab;
