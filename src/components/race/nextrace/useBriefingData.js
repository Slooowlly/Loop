import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import { buildBriefingContext } from "../../../pages/tabs/nextRaceContext";
import { buscarDadosDaPreCorrida } from "../../../stores/career/preRaceFetch";
import { readCachedPreRaceStandings } from "./nextRaceHelpers";

// O retrato da etapa chega em arrays (é o que o store guarda); a tela quer busca por
// chave. A conversão é aqui, e não no fetch, porque Set/Map não sobrevivem ao cache.
const paraConjuntoDeEquipes = (ids) => new Set(Array.isArray(ids) ? ids : []);
const paraMapaDeModificadores = (linhas) =>
  new Map((Array.isArray(linhas) ? linhas : []).map((linha) => [linha.driver_id, linha]));

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
  const [breakdownForecast, setBreakdownForecast] = useState(
    () => readCachedPreRaceStandings()?.breakdownForecast ?? null,
  );
  // IDs das EQUIPES com risco real de quebra na próxima corrida → 🔧 na tabela do campeonato.
  const [breakdownRiskTeams, setBreakdownRiskTeams] = useState(() =>
    paraConjuntoDeEquipes(readCachedPreRaceStandings()?.breakdownRiskTeamIds),
  );
  // Modificadores da esteira por piloto (forma, lesão, pressão…) → tooltip da tabela do
  // campeonato. Buscado de uma vez para o grid inteiro: o hover não pode disparar invoke.
  const [weekendModifiers, setWeekendModifiers] = useState(() =>
    paraMapaDeModificadores(readCachedPreRaceStandings()?.weekendModifierRows),
  );
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

      // Aplica o retrato da etapa venha ele do cache ou da busca — os dois caminhos
      // devolvem exatamente a mesma forma, porque saem do mesmo módulo (`preRaceFetch`).
      function aplicarRetrato(retrato) {
        setDriverStandings(retrato.driverStandings);
        setTeamStandings(retrato.teamStandings);
        setBriefingPhraseHistory(retrato.phraseHistory);
        setBreakdownForecast(retrato.breakdownForecast ?? null);
        setBreakdownRiskTeams(paraConjuntoDeEquipes(retrato.breakdownRiskTeamIds));
        setWeekendModifiers(paraMapaDeModificadores(retrato.weekendModifierRows));
        setBriefingError("");
      }

      // O prefetch (animação de avanço) já buscou o retrato desta etapa e guardou na
      // store. A pré-corrida é estática até a corrida rodar, então usamos o cache direto
      // e evitamos re-disparar os comandos pesados — a Sala abre os Favoritos na hora.
      const cached = readCachedPreRaceStandings();
      if (cached) {
        if (active) {
          aplicarRetrato(cached);
          setIsLoadingBriefing(false);
        }
        return;
      }

      setIsLoadingBriefing(true);
      setBriefingError("");

      try {
        // MESMA função que o pré-carregamento usa. Espelhar a lista de comandos à mão
        // aqui era o que deixava o cache incompleto em silêncio quando a Sala crescia.
        const retrato = await buscarDadosDaPreCorrida({
          careerId,
          categoria: playerTeam.categoria,
        });

        if (!active) return;

        aplicarRetrato(retrato);
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

  // A previsão de quebra, o marcador 🔧 do grid e os modificadores da esteira NÃO têm
  // effect próprio: os três vêm no mesmo retrato do `buscarDadosDaPreCorrida` acima.
  // Enquanto tinham, eles ficavam de fora do pré-carregamento e chegavam depois da tela.

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
    weekendModifiers,
  };
}
