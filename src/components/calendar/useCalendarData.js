import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import useDeferredLoading from "../../hooks/useLoading";
import useCareerStore from "../../stores/useCareerStore";
import { isLegacySeasonPhase } from "../../utils/seasonPhases";
import i18n from "../../i18n/index.js";
import {
  ALL_CALENDAR_CATEGORIES,
  formatIsoDateKey,
  parseDisplayDate,
} from "../../utils/calendarShared.js";
import { TRACK_COUNTRIES } from "../../utils/trackCountries.js";

function withFetchedCategory(entries = [], category) {
  return entries.map((entry) => ({
    ...entry,
    categoria: entry.categoria ?? category,
  }));
}

// Busca os calendários (categoria do jogador, bloco especial e demais categorias) e
// deriva tudo que a aba desenha: mapas por data, estatísticas e a lista de próximos.
function useCalendarData(activeTab) {
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const nextRace = useCareerStore((state) => state.nextRace);
  const season = useCareerStore((state) => state.season);
  const acceptedSpecialOffer = useCareerStore((state) => state.acceptedSpecialOffer);
  const calendarDisplayDate = useCareerStore((state) => state.calendarDisplayDate);
  const temporalSummary = useCareerStore((state) => state.temporalSummary);

  const [calendar, setCalendar] = useState([]);
  const [specialCalendar, setSpecialCalendar] = useState([]);
  const [otherCalendars, setOtherCalendars] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  const showLoadingUI = useDeferredLoading(loading);
  const isLegacyCalendar = isLegacySeasonPhase(season?.fase);

  useEffect(() => {
    let mounted = true;

    async function fetchCalendar() {
      if (!careerId || !playerTeam?.categoria) {
        setCalendar([]);
        setSpecialCalendar([]);
        setOtherCalendars([]);
        setLoading(false);
        return;
      }

      setLoading(true);
      setError("");
      setOtherCalendars([]);

      try {
        const specialCategory = isLegacyCalendar ? acceptedSpecialOffer?.special_category ?? null : null;
        const visibleCategories = new Set([playerTeam.categoria, specialCategory].filter(Boolean));
        const otherCategories = ALL_CALENDAR_CATEGORIES.filter((category) => !visibleCategories.has(category));
        const [regularEntries, specialEntries] = await Promise.all([
          invoke("get_calendar_for_category", { careerId, category: playerTeam.categoria })
            .then((entries) => withFetchedCategory(entries, playerTeam.categoria)),
          specialCategory
            ? invoke("get_calendar_for_category", { careerId, category: specialCategory })
              .then((entries) => withFetchedCategory(entries, specialCategory))
            : Promise.resolve([]),
        ]);

        if (!mounted) return;
        setCalendar(regularEntries);
        setSpecialCalendar(specialEntries);
        setLoading(false);

        Promise.all(
          otherCategories.map((category) => (
            invoke("get_calendar_for_category", { careerId, category })
              .then((entries) => withFetchedCategory(entries, category))
          )),
        )
          .then((otherEntries) => {
            if (mounted) setOtherCalendars(otherEntries.flat());
          })
          .catch(() => {
            if (mounted) setOtherCalendars([]);
          });
      } catch (err) {
        if (mounted) setError(typeof err === "string" ? err : i18n.t("calendar.loadError"));
      } finally {
        if (mounted) setLoading(false);
      }
    }

    fetchCalendar();
    return () => {
      mounted = false;
    };
  }, [
    acceptedSpecialOffer?.special_category,
    careerId,
    isLegacyCalendar,
    playerTeam?.categoria,
    season?.rodada_atual,
  ]);

  const displayedCalendar = useMemo(
    () => [...calendar, ...specialCalendar],
    [calendar, specialCalendar],
  );

  const seasonYear = useMemo(() => {
    if (season?.ano) return season.ano;
    for (const race of displayedCalendar) {
      const parsed = parseDisplayDate(race.display_date);
      if (parsed) return parsed.year;
    }
    return new Date().getFullYear();
  }, [displayedCalendar, season]);

  const racesByDate = useMemo(() => {
    const map = {};
    for (const race of displayedCalendar) {
      const parsed = parseDisplayDate(race.display_date);
      if (!parsed) continue;
      map[formatIsoDateKey(parsed.year, parsed.month, parsed.day)] = {
        ...race,
        _isSpecialRace: race.season_phase === "BlocoEspecial",
      };
    }
    return map;
  }, [displayedCalendar]);

  const otherCategoryRacesByDate = useMemo(() => {
    const map = {};
    for (const race of otherCalendars) {
      const parsed = parseDisplayDate(race.display_date);
      if (!parsed) continue;
      const key = formatIsoDateKey(parsed.year, parsed.month, parsed.day);
      (map[key] ??= []).push(race);
    }
    return map;
  }, [otherCalendars]);

  const currentDateParts = useMemo(() => {
    if (activeTab !== "calendar") return null;
    return parseDisplayDate(calendarDisplayDate ?? temporalSummary?.current_display_date ?? null);
  }, [activeTab, calendarDisplayDate, temporalSummary]);

  const nextRaceEntry = useMemo(
    () => displayedCalendar.find((race) => race.id === nextRace?.id) ?? null,
    [displayedCalendar, nextRace?.id],
  );

  const upcoming = useMemo(() => {
    return [...displayedCalendar]
      .filter((race) => race.display_date && race.status !== "Concluida")
      .sort((a, b) => a.display_date.localeCompare(b.display_date));
  }, [displayedCalendar]);

  const stats = useMemo(() => {
    const total = displayedCalendar.length;
    const done = displayedCalendar.filter((race) => race.status === "Concluida").length;
    const countries = new Set(
      displayedCalendar.map((race) => TRACK_COUNTRIES[race.track_name]).filter(Boolean),
    ).size;
    const durationMin = displayedCalendar.reduce((sum, race) => sum + (race.duracao_corrida_min || 0), 0);
    const wet = displayedCalendar.filter((race) => race.clima === "Wet" || race.clima === "HeavyRain").length;
    const specials = displayedCalendar.filter((race) => race.season_phase === "BlocoEspecial").length;
    return { total, done, countries, durationMin, wet, specials };
  }, [displayedCalendar]);

  return {
    careerId,
    playerTeam,
    nextRace,
    season,
    temporalSummary,
    loading,
    showLoadingUI,
    error,
    isLegacyCalendar,
    displayedCalendar,
    seasonYear,
    racesByDate,
    otherCategoryRacesByDate,
    currentDateParts,
    nextRaceEntry,
    upcoming,
    stats,
  };
}

export default useCalendarData;
