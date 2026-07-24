import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import { buildBriefingContext } from "../../../pages/tabs/nextRaceContext";
import { readCachedPreRaceStandings } from "./nextRaceHelpers";

// Contexto do briefing da próxima etapa: standings da categoria, histórico de frases,
// previsão de quebra e as equipes em risco. Devolve o briefing já montado.
export function useBriefingData({
  careerId,
  player,
  playerTeam,
  season,
  nextRace,
  nextRaceBriefing,
  playerInterests,
}) {
  const { t } = useTranslation();
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

  return {
    briefing,
    isLoadingBriefing,
    briefingError,
    breakdownForecast,
    breakdownRiskTeams,
  };
}
